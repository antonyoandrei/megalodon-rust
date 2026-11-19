mod avatars;
mod jellyfin;
mod views;

use axum::{
    Json, Router,
    extract::{OriginalUri, Path, Query, Request, State},
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use jellyfin::{Config, Library, valid_id};
use serde::Deserialize;
use std::{collections::BTreeMap, sync::Arc};

type App = Arc<Library>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load().map_err(std::io::Error::other)?;
    let port = config.port;
    let library = Arc::new(Library::new(config).map_err(std::io::Error::other)?);
    let routes = Router::new()
        .route("/", get(home))
        .route("/library", get(browse))
        .route("/movies", get(browse))
        .route("/series", get(browse))
        .route("/item/{id}", get(detail))
        .route("/connection", get(connection))
        .route("/refresh", post(refresh))
        .route("/image/{id}/{kind}", get(image))
        .route("/profile/avatar", get(profile_avatar))
        .route("/assets/{name}", get(asset))
        .route("/api/catalog", get(api_catalog))
        .route("/health", get(health))
        .fallback(not_found)
        .layer(middleware::from_fn_with_state(library.clone(), local_only))
        .with_state(library);
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await?;
    println!("Megalodon · http://127.0.0.1:{port}");
    println!("Catálogo local en Rust. Ctrl+C para cerrar.");
    axum::serve(listener, routes)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}

async fn local_only(State(app): State<App>, request: Request, next: Next) -> Response {
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let valid_host = [
        format!("127.0.0.1:{}", app.config.port),
        format!("localhost:{}", app.config.port),
    ]
    .contains(&host.to_ascii_lowercase());
    let cross_site = request
        .headers()
        .get("sec-fetch-site")
        .is_some_and(|v| v == "cross-site");
    if !valid_host || cross_site {
        return (
            StatusCode::FORBIDDEN,
            "Este cliente está disponible solo en este equipo.",
        )
            .into_response();
    }
    if request.method() != axum::http::Method::GET
        && let Some(origin) = request.headers().get(header::ORIGIN)
    {
        let expected = format!("http://{host}");
        if origin.as_bytes() != expected.as_bytes() {
            return StatusCode::FORBIDDEN.into_response();
        }
    }
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert("x-content-type-options", "nosniff".parse().unwrap());
    headers.insert("x-frame-options", "DENY".parse().unwrap());
    headers.insert("referrer-policy", "same-origin".parse().unwrap());
    headers.insert("content-security-policy", "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src 'self'; object-src 'none'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'".parse().unwrap());
    if !headers.contains_key(header::CACHE_CONTROL) {
        headers.insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    }
    response
}

fn page(markup: maud::Markup) -> Response {
    Html(markup.into_string()).into_response()
}
fn failure(message: &str) -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Html(views::error(message).into_string()),
    )
        .into_response()
}

#[derive(Default, Deserialize)]
struct HomeQuery {
    feature: Option<String>,
}

async fn home(State(app): State<App>, Query(query): Query<HomeQuery>) -> Response {
    match app.catalog(false).await {
        Ok((data, warning)) => page(views::home(
            &data,
            query.feature.as_deref(),
            warning.as_deref(),
        )),
        Err(error) => failure(&error),
    }
}

async fn browse(
    State(app): State<App>,
    OriginalUri(uri): OriginalUri,
    Query(mut filters): Query<views::Filters>,
) -> Response {
    filters.q = filters.q.chars().take(160).collect();
    let section = match uri.path() {
        "/movies" => "movies",
        "/series" => "series",
        _ => "library",
    };
    match app.catalog(false).await {
        Ok((data, warning)) => page(views::browse(&data, section, &filters, warning.as_deref())),
        Err(error) => failure(&error),
    }
}

#[derive(Default, Deserialize)]
struct DetailQuery {
    season: Option<u32>,
    episode_page: Option<usize>,
}

async fn detail(
    State(app): State<App>,
    Path(id): Path<String>,
    Query(query): Query<DetailQuery>,
) -> Response {
    if !valid_id(&id) {
        return not_found().await;
    }
    let (catalog, warning) = match app.catalog(false).await {
        Ok(value) => value,
        Err(error) => return failure(&error),
    };
    if !catalog.items.iter().any(|item| item.id == id) {
        return not_found().await;
    }
    let item = match app.detail(&catalog.user.id, &id).await {
        Ok(item) => item,
        Err(error) => return failure(&error),
    };
    let mut seasons = BTreeMap::new();
    let mut episodes_error = None;
    if item.kind == "Series" {
        match app
            .all_items(
                &catalog.user.id,
                "Episode",
                Some(&id),
                "ParentIndexNumber,IndexNumber,SortName",
            )
            .await
        {
            Ok(episodes) => {
                for episode in episodes {
                    seasons
                        .entry(episode.parent_index_number.unwrap_or(0))
                        .or_insert_with(Vec::new)
                        .push(episode);
                }
            }
            Err(error) => episodes_error = Some(error),
        }
    }
    page(views::detail(
        &catalog,
        &item,
        &seasons,
        query.season,
        query.episode_page.unwrap_or(1),
        episodes_error.as_deref().or(warning.as_deref()),
    ))
}

async fn connection(State(app): State<App>) -> Response {
    match app.catalog(false).await {
        Ok((data, warning)) => page(views::connection(&data, warning.as_deref())),
        Err(error) => failure(&error),
    }
}

async fn refresh(State(app): State<App>) -> Response {
    match app.catalog(true).await {
        Ok((_, None)) => Redirect::to("/connection").into_response(),
        Ok((data, Some(warning))) => page(views::connection(&data, Some(&warning))),
        Err(error) => failure(&error),
    }
}

async fn api_catalog(State(app): State<App>) -> Response {
    match app.catalog(false).await {
        Ok((data, warning)) => Json(serde_json::json!({"total":data.items.len(),"movies":data.movies(),"series":data.series(),"items":data.items,"warning":warning})).into_response(),
        Err(error) => (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":error}))).into_response(),
    }
}

async fn health(State(app): State<App>) -> Response {
    match app.catalog(false).await {
        Ok((data, warning)) => Json(serde_json::json!({"ok":warning.is_none(),"runtime":"rust","total":data.items.len(),"movies":data.movies(),"series":data.series()})).into_response(),
        Err(_) => (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"ok":false,"runtime":"rust"}))).into_response(),
    }
}

#[derive(Deserialize)]
struct ImageQuery {
    width: Option<u32>,
    tag: Option<String>,
}

async fn image(
    State(app): State<App>,
    Path((id, kind)): Path<(String, String)>,
    Query(query): Query<ImageQuery>,
) -> Response {
    if !valid_id(&id) || !["Primary", "Backdrop", "Thumb", "Logo"].contains(&kind.as_str()) {
        return StatusCode::BAD_REQUEST.into_response();
    }
    proxy_image(&app, &format!("/Items/{id}/Images/{kind}"), query).await
}

async fn profile_avatar(State(app): State<App>) -> Response {
    let Ok((catalog, _)) = app.catalog(false).await else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    if let Some(bytes) = catalog.user.avatar_id.as_deref().and_then(avatars::bytes) {
        return (
            [
                (header::CONTENT_TYPE, "image/png"),
                (header::CACHE_CONTROL, "private, max-age=300"),
            ],
            bytes,
        )
            .into_response();
    }
    if catalog.user.primary_image_tag.is_some() && valid_id(&catalog.user.id) {
        return proxy_image(
            &app,
            &format!("/Users/{}/Images/Primary", catalog.user.id),
            ImageQuery {
                width: Some(160),
                tag: catalog.user.primary_image_tag.clone(),
            },
        )
        .await;
    }
    (
        [
            (header::CONTENT_TYPE, "image/svg+xml"),
            (header::CACHE_CONTROL, "private, max-age=60"),
        ],
        include_str!("../static/avatar-fallback.svg"),
    )
        .into_response()
}

async fn proxy_image(app: &Library, path: &str, query: ImageQuery) -> Response {
    let mut params = vec![
        (
            "MaxWidth",
            query.width.unwrap_or(480).clamp(100, 1920).to_string(),
        ),
        ("Quality", "85".to_owned()),
        ("Format", "webp".to_owned()),
    ];
    if let Some(tag) = query.tag.filter(|v| valid_id(v)) {
        params.push(("Tag", tag));
    }
    let result = app
        .client
        .get(format!("{}{path}", app.config.base_url))
        .query(&params)
        .send()
        .await;
    if let Ok(mut response) = result
        && response.status().is_success()
    {
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned();
        if ["image/jpeg", "image/png", "image/webp", "image/avif"].contains(&content_type.as_str())
            && response.content_length().unwrap_or(0) < 15_000_000
        {
            let mut bytes = Vec::new();
            loop {
                match response.chunk().await {
                    Ok(Some(chunk)) if bytes.len() + chunk.len() <= 15_000_000 => {
                        bytes.extend_from_slice(&chunk)
                    }
                    Ok(None) if !bytes.is_empty() => {
                        return (
                            [
                                (header::CONTENT_TYPE, content_type),
                                (header::CACHE_CONTROL, "private, max-age=86400".to_owned()),
                            ],
                            bytes,
                        )
                            .into_response();
                    }
                    _ => break,
                }
            }
        }
    }
    (
        [
            (header::CONTENT_TYPE, "image/svg+xml"),
            (header::CACHE_CONTROL, "private, max-age=60"),
        ],
        include_str!("../static/placeholder.svg"),
    )
        .into_response()
}

async fn asset(Path(name): Path<String>) -> Response {
    let (kind, bytes): (&str, &[u8]) = match name.as_str() {
        "boot.js" => (
            "text/javascript; charset=utf-8",
            include_bytes!("../static/boot.js"),
        ),
        "megalodon_ui.js" => (
            "text/javascript; charset=utf-8",
            include_bytes!("../static/client/megalodon_ui.js"),
        ),
        "megalodon_ui_bg.wasm" => (
            "application/wasm",
            include_bytes!("../static/client/megalodon_ui_bg.wasm"),
        ),
        "app.css" => (
            "text/css; charset=utf-8",
            include_bytes!("../static/app.css"),
        ),
        "avatars.css" => (
            "text/css; charset=utf-8",
            include_bytes!("../static/avatars.css"),
        ),
        "sora.woff2" => ("font/woff2", include_bytes!("../static/sora.woff2")),
        "space-grotesk.woff2" => (
            "font/woff2",
            include_bytes!("../static/space-grotesk.woff2"),
        ),
        "logo.png" => ("image/png", include_bytes!("../static/logo.png")),
        "icons.svg" => ("image/svg+xml", include_bytes!("../static/icons.svg")),
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    (
        [
            (header::CONTENT_TYPE, kind),
            (
                header::CACHE_CONTROL,
                if name.ends_with(".css")
                    || name.ends_with(".js")
                    || name.ends_with(".wasm")
                    || name == "icons.svg"
                {
                    "no-cache"
                } else {
                    "public, max-age=86400"
                },
            ),
        ],
        bytes,
    )
        .into_response()
}

async fn not_found() -> Response {
    (
        StatusCode::NOT_FOUND,
        Html(views::not_found().into_string()),
    )
        .into_response()
}
