use crate::jellyfin::{Catalog, Item, encode, genre_label, search_key};
use maud::{DOCTYPE, Markup, html};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Clone, Default, Deserialize)]
#[serde(default)]
pub struct Filters {
    pub q: String,
    pub genre: String,
    pub sort: String,
    pub kind: String,
    pub page: usize,
}

fn icon(name: &str) -> Markup {
    html! { svg class="icon" aria-hidden="true" { use href=(format!("/assets/icons.svg?v=4#{name}")); } }
}

fn avatar(catalog: Option<&Catalog>) -> Markup {
    let id = catalog
        .and_then(|c| c.user.avatar_id.as_deref())
        .unwrap_or("default");
    html! { span class=(format!("avatar avatar-{id}")) {
        img src=(format!("/profile/avatar?v={}", encode(id))) width="96" height="96" alt="" decoding="async";
    } }
}

fn layout(
    title: &str,
    active: &str,
    catalog: Option<&Catalog>,
    show_search: bool,
    content: Markup,
) -> Markup {
    html! {
        (DOCTYPE)
        html lang="es" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover";
                meta name="theme-color" content="#08090b";
                meta name="description" content="Tu biblioteca de películas y series en Megalodon.";
                title { (title) " · Megalodon" }
                link rel="icon" type="image/png" href="/assets/logo.png";
                link rel="stylesheet" href="/assets/app.css?v=5";
                link rel="stylesheet" href="/assets/avatars.css?v=1";
                link rel="modulepreload" href="/assets/megalodon_ui.js";
                link rel="preload" href="/assets/megalodon_ui_bg.wasm" r#as="fetch" r#type="application/wasm" crossorigin="anonymous" fetchpriority="high";
                script type="module" src="/assets/boot.js?v=1" {}
            }
            body class=(format!("page-{active}")) {
                div class="navigation-progress" role="status" { span class="sr-only" { "Cargando página" } }
                a class="skip" href="#main" { "Saltar al contenido" }
                header class="site-header" {
                    div class="header-inner" {
                        a class="brand" href="/" aria-label="Megalodon, inicio" translate="no" {
                            span class="brand-mark" { img src="/assets/logo.png" width="42" height="42" alt=""; }
                            span { "Megalodon" }
                        }
                        nav class="main-nav" aria-label="Navegación principal" {
                            @for (path, key, label, glyph) in [("/", "home", "Inicio", "home"), ("/movies", "movies", "Películas", "film"), ("/series", "series", "Series", "tv"), ("/library", "library", "Biblioteca", "grid")] {
                                a href=(path) aria-current=[(active == key).then_some("page")] aria-label=(label) {
                                    (icon(glyph)) span { (label) }
                                }
                            }
                        }
                        div class="header-actions" {
                            @if show_search { button class="icon-button" type="button" popovertarget="search-panel" aria-label="Abrir búsqueda" { (icon("search")) } }
                            button class="profile-trigger" type="button" popovertarget="profile-panel" aria-label="Abrir perfil" { (avatar(catalog)) }
                        }
                    }
                }
                @if show_search {
                    div id="search-panel" class="search-panel floating-panel" popover {
                        form class="global-search" action="/library" method="get" role="search" aria-label="Buscar en toda la biblioteca" {
                            label class="sr-only" for="global-search" { "Buscar películas y series" }
                            input id="global-search" name="q" type="search" placeholder="Películas, series, géneros…" maxlength="160" autocomplete="off" autofocus;
                            button type="submit" aria-label="Buscar en la biblioteca" { (icon("arrow")) }
                        }
                        p { "Busca por título, género o año de estreno." }
                    }
                }
                div id="profile-panel" class="profile-panel floating-panel" popover {
                    div class="profile-identity" { (avatar(catalog)) div { strong { (catalog.map_or("Megalodon", |c| c.user.name.as_str())) } span { "Tu perfil de Jellyfin" } } }
                    a href="/connection" { (icon("library")) "Mi biblioteca" (icon("arrow")) }
                    a href="/library" { (icon("grid")) "Explorar colección" (icon("arrow")) }
                }
                main id="main" { (content) }
                footer class="site-footer wrap" {
                    span { "Megalodon" }
                    a href="/connection" { @if let Some(data) = catalog { (data.movies()) " películas · " (data.series()) " series" } @else { "Mi biblioteca" } (icon("arrow")) }
                }
            }
        }
    }
}

fn notice(warning: Option<&str>) -> Markup {
    html! { @if let Some(warning) = warning { aside class="notice" role="status" { (icon("info")) p { (warning) } a href="/connection" { "Ver conexión" } } } }
}

fn metadata(item: &Item) -> Markup {
    html! {
        div class="metadata" {
            @if let Some(rating) = item.community_rating { span class="rating" { (icon("star")) (format!("{rating:.1}")) } }
            @if let Some(year) = item.production_year { span { (year) } }
            span { (item.label()) }
            @if !item.runtime().is_empty() { span { (item.runtime()) } }
            @if let Some(rating) = &item.official_rating { span class="age-rating" { (rating) } }
        }
    }
}

fn poster(item: &Item) -> Markup {
    html! {
        article class="poster-card" {
            a href=(format!("/item/{}", item.id)) aria-label=(format!("Ver ficha de {}", item.name)) {
                div class="poster-art" {
                    (media_image(&item.image("Primary", 480), 320, 480, "poster-media", false))
                    @if item.user_data.as_ref().is_some_and(|u| u.played) { span class="watched" aria-label="Visto" { (icon("check")) } }
                    span class="poster-open" { (icon("arrow")) }
                }
                h3 { (&item.name) }
                p { (item.year()) @if item.production_year.is_some() { " · " } (item.label()) @if let Some(rating) = item.community_rating { span class="card-score" { (icon("star")) (format!("{rating:.1}")) } } }
            }
        }
    }
}

fn media_image(src: &str, width: u32, height: u32, class: &str, eager: bool) -> Markup {
    let srcset = (width == 1920).then(|| {
        format!(
            "{} 800w, {} 1280w, {} 1920w",
            src.replace("width=1920", "width=800"),
            src.replace("width=1920", "width=1280"),
            src
        )
    });
    html! {
        div class=(format!("media-frame {class}")) data-media {
            img src=(src) srcset=[srcset] sizes=[(width == 1920).then_some("100vw")] width=(width) height=(height) alt="" loading=(if eager { "eager" } else { "lazy" }) fetchpriority=(if eager { "high" } else { "auto" }) decoding="async";
            span class="media-fallback" { (icon("film")) span { "Imagen no disponible" } }
        }
    }
}

fn rail_controls(id: &str, title: &str) -> Markup {
    html! {
        div class="rail-buttons" {
            button class="rail-button" type="button" data-scroll="previous" aria-controls=(id) aria-label=(format!("Anterior en {title}")) disabled { (icon("back")) }
            button class="rail-button" type="button" data-scroll="next" aria-controls=(id) aria-label=(format!("Siguiente en {title}")) { (icon("arrow")) }
        }
    }
}

fn rail(id: &str, title: &str, variant: &str, content: Markup) -> Markup {
    html! {
        div class="rail-window" {
            div id=(id) class=(format!("media-rail {variant}")) data-rail tabindex="0" role="region" aria-label=(format!("Desplazar {title}")) { (content) }
        }
    }
}

fn shelf(id: &str, title: &str, items: &[&Item], href: &str) -> Markup {
    html! { @if !items.is_empty() {
        section class="shelf" aria-label=(title) {
            div class="section-heading" { h2 { (title) } div class="shelf-controls" { a class="text-link" href=(href) { "Ver todo" (icon("arrow")) } (rail_controls(id, title)) } }
            (rail(id, title, "poster-row", html! { @for item in items { (poster(item)) } }))
        }
    } }
}

fn landscape_shelf(id: &str, title: &str, items: &[&Item], href: &str) -> Markup {
    html! { @if !items.is_empty() {
        section class="shelf" aria-label=(title) {
            div class="section-heading" { h2 { (title) } div class="shelf-controls" { a class="text-link" href=(href) { "Ver todo" (icon("arrow")) } (rail_controls(id, title)) } }
            (rail(id, title, "landscape-row", html! { @for item in items {
                article class="landscape-card" {
                    a href=(format!("/item/{}", item.id)) aria-label=(format!("Ver ficha de {}", item.name)) {
                        div class="landscape-art" {
                            (media_image(&item.image(if item.image_tags.contains_key("Thumb") { "Thumb" } else { item.art_kind() }, 800), 800, 450, "landscape-media", false))
                            span class="landscape-open" { (icon("arrow")) }
                        }
                        div class="landscape-caption" { h3 { (&item.name) } p { (item.year()) " · " (item.label()) } }
                    }
                }
            } }))
        }
    } }
}

fn film_title(item: &Item, id: &str) -> Markup {
    html! {
        @if item.image_tags.contains_key("Logo") {
            h1 id=(id) class="sr-only" { (&item.name) }
            img class="film-logo" src=(item.image("Logo", 800)) width="800" height="300" alt="" fetchpriority="high";
        } @else { h1 id=(id) { (&item.name) } }
    }
}

fn feature_slide(item: &Item, active: bool) -> Markup {
    html! {
        div class="hero-slide" id=(format!("feature-{}", item.id)) data-active=(if active { "true" } else { "false" }) aria-hidden=(if active { "false" } else { "true" }) hidden[!active] inert[!active] {
            div class="hero-visual" {
                (media_image(&item.image(item.art_kind(), 1920), 1920, 1080, "hero-media", active))
                div class="hero-shade" {}
            }
            div class="hero-content" {
                div class="hero-copy" {
                    (film_title(item, &format!("hero-title-{}", item.id)))
                    (metadata(item))
                    @if let Some(overview) = &item.overview { p class="hero-overview" { (overview) } }
                }
                div class="hero-actions" {
                    a class="button primary" href=(format!("/item/{}", item.id)) { "Explorar " (if item.kind == "Series" { "serie" } else { "película" }) (icon("arrow")) }
                    a class="hero-collection" href=(if item.kind == "Series" { "/series" } else { "/movies" }) { "Ver colección" }
                }
            }
        }
    }
}

pub fn home(catalog: &Catalog, selected: Option<&str>, warning: Option<&str>) -> Markup {
    let mut latest: Vec<_> = catalog.items.iter().collect();
    latest.sort_by(|a, b| {
        b.date_created
            .cmp(&a.date_created)
            .then_with(|| a.name.cmp(&b.name))
    });
    let featured: Vec<_> = latest
        .iter()
        .copied()
        .filter(|i| !i.backdrop_image_tags.is_empty())
        .take(4)
        .collect();
    let hero = featured
        .iter()
        .copied()
        .find(|i| Some(i.id.as_str()) == selected)
        .or(featured.first().copied())
        .or(latest.first().copied());
    let recent: Vec<_> = latest
        .iter()
        .copied()
        .filter(|i| hero.is_none_or(|h| i.id != h.id))
        .take(10)
        .collect();
    let series: Vec<_> = latest
        .iter()
        .copied()
        .filter(|i| i.kind == "Series")
        .take(10)
        .collect();
    let movies: Vec<_> = latest
        .iter()
        .copied()
        .filter(|i| i.kind == "Movie")
        .take(14)
        .collect();
    layout(
        "Inicio",
        "home",
        Some(catalog),
        true,
        html! {
            @if let Some(item) = hero {
                section class="hero home-hero" aria-labelledby=(format!("hero-title-{}", item.id)) {
                    @if featured.is_empty() { (feature_slide(item, true)) }
                    @else { @for feature in &featured { (feature_slide(feature, feature.id == item.id)) } }
                    @if featured.len() > 1 {
                        nav class="feature-picker" aria-label="Títulos destacados" {
                            @for feature in &featured {
                                a href=(format!("/?feature={}", feature.id)) data-feature-target=(format!("feature-{}", feature.id)) aria-label=(format!("Destacar {}", feature.name)) aria-current=[(feature.id == item.id).then_some("true")] {
                                    img src=(feature.image(feature.art_kind(), 320)) width="160" height="90" alt="" loading="lazy";
                                    span class="sr-only" { (&feature.name) }
                                    span class="feature-wait" aria-hidden="true" {}
                                }
                            }
                        }
                    }
                    p class="hero-status" role="status" {}
                }
                div class="wrap home-content" {
                    (notice(warning))
                    (landscape_shelf("recent-rail", "Recién añadidos", &recent, "/library?sort=recent"))
                    (shelf("movies-rail", "Tus películas", &movies, "/movies"))
                    (landscape_shelf("series-rail", "Tus series", &series, "/series"))
                }
            } @else {
                div class="wrap page-content" { (notice(warning)) (empty("Tu biblioteca está lista para llenarse", "Añade películas y series a Jellyfin y actualiza la biblioteca.", "/connection", "Ver biblioteca")) }
            }
        },
    )
}

fn filter_menu(
    id: &str,
    label: &str,
    value: &str,
    options: &[(String, String)],
    path: &str,
    filters: &Filters,
) -> Markup {
    let caption = options
        .iter()
        .find(|(v, _)| v == value)
        .map(|(_, label)| label.as_str())
        .unwrap_or(label);
    let links: Vec<_> = options
        .iter()
        .map(|(option, name)| {
            let mut next = filters.clone();
            match id {
                "genre" => next.genre = option.clone(),
                "sort" => next.sort = option.clone(),
                _ => next.kind = option.clone(),
            }
            (option, name, filter_url(path, &next, 1))
        })
        .collect();
    html! {
        div class="filter-control" {
            button class="filter-trigger" type="button" popovertarget=(format!("filter-{id}")) aria-label=(format!("{label}: {caption}")) { (caption) (icon("chevron")) }
            div id=(format!("filter-{id}")) class=(format!("filter-popover filter-{id}")) popover {
                p class="menu-label" { (label) }
                nav aria-label=(label) { @for (option, name, href) in &links {
                    a href=(href) aria-current=[(value == *option).then_some("true")] { span { (name) } @if value == *option { (icon("check")) } }
                } }
            }
        }
    }
}

fn filter_url(path: &str, filters: &Filters, page: usize) -> String {
    format!(
        "{path}?q={}&genre={}&sort={}&kind={}&page={page}",
        encode(&filters.q),
        encode(&filters.genre),
        encode(&filters.sort),
        encode(&filters.kind)
    )
}

fn filtered_items<'a>(catalog: &'a Catalog, section: &str, filters: &Filters) -> Vec<&'a Item> {
    let query = search_key(filters.q.trim());
    let mut items: Vec<_> = catalog
        .items
        .iter()
        .filter(|item| {
            (section != "movies" || item.kind == "Movie")
                && (section != "series" || item.kind == "Series")
                && (filters.kind.is_empty() || item.kind == filters.kind)
                && (filters.genre.is_empty()
                    || item.display_genres().contains(&genre_label(&filters.genre)))
                && (query.is_empty()
                    || search_key(&format!(
                        "{} {} {}",
                        item.name,
                        item.year(),
                        item.display_genres().join(" ")
                    ))
                    .contains(&query))
        })
        .collect();
    items.sort_by(|a, b| {
        let order = match filters.sort.as_str() {
            "recent" => b.date_created.cmp(&a.date_created),
            "year" => b.production_year.cmp(&a.production_year),
            "rating" => b
                .community_rating
                .unwrap_or(0.0)
                .total_cmp(&a.community_rating.unwrap_or(0.0)),
            _ => std::cmp::Ordering::Equal,
        };
        order.then_with(|| search_key(&a.name).cmp(&search_key(&b.name)))
    });
    items
}

pub fn browse(
    catalog: &Catalog,
    section: &str,
    filters: &Filters,
    warning: Option<&str>,
) -> Markup {
    let (title, path) = match section {
        "movies" => ("Películas", "/movies"),
        "series" => ("Series", "/series"),
        _ => ("Biblioteca", "/library"),
    };
    let items = filtered_items(catalog, section, filters);
    let pages = items.len().div_ceil(48).max(1);
    let current = filters.page.clamp(1, pages);
    let mut genres = vec![(String::new(), "Todos los géneros".into())];
    genres.extend(catalog.genres().keys().map(|g| (g.clone(), g.clone())));
    let kinds = vec![
        (String::new(), "Todo el contenido".into()),
        ("Movie".into(), "Películas".into()),
        ("Series".into(), "Series".into()),
    ];
    let sorts = vec![
        (String::new(), "Título: A–Z".into()),
        ("recent".into(), "Últimas incorporaciones".into()),
        ("year".into(), "Año más reciente".into()),
        ("rating".into(), "Mejor valoradas".into()),
    ];
    layout(
        title,
        section,
        Some(catalog),
        false,
        html! {
            div class="wrap page-content browse-page" {
                (notice(warning))
                div class="page-heading" { h1 { (title) } span class="collection-count" { (items.len()) (if items.len() == 1 { " título" } else { " títulos" }) } }
                div class="browse-toolbar" {
                    form class="catalog-search" action=(path) method="get" role="search" aria-label="Buscar en el catálogo" {
                        (icon("search")) label class="sr-only" for="search" { "Buscar títulos" }
                        input id="search" name="q" type="search" placeholder="Busca en tu colección" value=(&filters.q) maxlength="160" autocomplete="off";
                        input type="hidden" name="genre" value=(&filters.genre);
                        input type="hidden" name="kind" value=(&filters.kind);
                        input type="hidden" name="sort" value=(&filters.sort);
                        button type="submit" aria-label="Buscar títulos" { (icon("arrow")) }
                    }
                    div class="filter-controls" {
                        (filter_menu("genre", "Género", &filters.genre, &genres, path, filters))
                        @if section == "library" { (filter_menu("kind", "Contenido", &filters.kind, &kinds, path, filters)) }
                        (filter_menu("sort", "Ordenar por", &filters.sort, &sorts, path, filters))
                    }
                }
                @if !filters.q.is_empty() || !filters.genre.is_empty() || !filters.kind.is_empty() {
                    div class="results-summary" {
                        p { @if !filters.q.trim().is_empty() { "Resultados para «" (&filters.q) "»" } @else { (items.len()) (if items.len() == 1 { " título en esta selección" } else { " títulos en esta selección" }) } }
                        a class="text-link" href=(path) { "Limpiar filtros" (icon("close")) }
                    }
                }
                @if items.is_empty() { (empty("No encontramos ese título", "Prueba con otro nombre o cambia los filtros de búsqueda.", path, "Ver todos los títulos")) }
                @else {
                    div class="poster-grid" { @for item in items.iter().skip((current - 1) * 48).take(48) { (poster(item)) } }
                    @if pages > 1 {
                        nav class="pagination" aria-label="Páginas del catálogo" {
                            @if current > 1 { a class="button secondary" href=(filter_url(path, filters, current - 1)) { (icon("back")) "Anterior" } }
                            span { "Página " (current) " de " (pages) }
                            @if current < pages { a class="button secondary" href=(filter_url(path, filters, current + 1)) { "Siguiente" (icon("arrow")) } }
                        }
                    }
                }
            }
        },
    )
}

pub fn detail(
    catalog: &Catalog,
    item: &Item,
    seasons: &BTreeMap<u32, Vec<Item>>,
    selected: Option<u32>,
    requested_page: usize,
    warning: Option<&str>,
) -> Markup {
    let is_series = item.kind == "Series";
    let active = if is_series { "series" } else { "movies" };
    let season = selected
        .filter(|s| seasons.contains_key(s))
        .or_else(|| seasons.keys().copied().find(|s| *s > 0))
        .or(seasons.keys().next().copied());
    let episode_count: usize = seasons.values().map(Vec::len).sum();
    let selected_episodes = season.and_then(|number| seasons.get(&number));
    let episode_pages = selected_episodes.map_or(1, |episodes| episodes.len().div_ceil(40).max(1));
    let episode_page = requested_page.clamp(1, episode_pages);
    let cast: Vec<_> = item
        .people
        .iter()
        .filter(|p| p.kind == "Actor")
        .take(12)
        .collect();
    let directors: Vec<_> = item
        .people
        .iter()
        .filter(|p| p.kind == "Director")
        .map(|p| p.name.as_str())
        .collect();
    layout(
        &item.name,
        active,
        Some(catalog),
        true,
        html! {
            div class="hero-wrap" { section class="hero detail-hero" aria-labelledby="detail-title" {
                (media_image(&item.image(item.art_kind(), 1920), 1920, 1080, "hero-media", true))
                div class="hero-shade" {}
                div class="hero-content" {
                    a class="back-link" href=(format!("/{active}")) { (icon("back")) (if is_series { "Todas las series" } else { "Todas las películas" }) }
                    (film_title(item, "detail-title"))
                    (metadata(item))
                    @if let Some(overview) = &item.overview { p class="hero-overview" { (overview) } }
                    @if is_series && episode_count > 0 { a class="button primary" href="#episodes" { "Ver episodios" (icon("arrow")) } }
                    @else { a class="button primary" href="#about" { "Sinopsis y reparto" (icon("arrow")) } }
                }
            } }
            div class="wrap detail-content" {
                (notice(warning))
                nav class="detail-tabs" aria-label="Contenido de la ficha" {
                    a href="#about" { "Información" }
                    @if is_series { a href="#episodes" { "Episodios" } }
                    @if !cast.is_empty() { a href="#cast" { "Reparto" } }
                }
                section class="about" id="about" aria-labelledby="about-title" {
                    (media_image(&item.image("Primary", 480), 320, 480, "detail-poster", false))
                    div class="synopsis" {
                        h2 id="about-title" { "Sinopsis" }
                        p { (item.overview.as_deref().filter(|s| !s.is_empty()).unwrap_or("Jellyfin todavía no tiene una sinopsis para este título.")) }
                        div class="detail-genres" { @for genre in item.display_genres() { a href=(format!("/library?genre={}", encode(genre))) { (genre) } } }
                    }
                    aside class="facts" aria-label="Información del título" {
                        dl {
                            @if !directors.is_empty() { div { dt { "Dirección" } dd { (directors.join(", ")) } } }
                            @if is_series && episode_count > 0 { div { dt { "En tu biblioteca" } dd { (seasons.len()) (if seasons.len() == 1 { " temporada · " } else { " temporadas · " }) (episode_count) (if episode_count == 1 { " episodio" } else { " episodios" }) } } }
                            @if !item.studios.is_empty() { div { dt { "Producción" } dd { (item.studios.iter().map(|s| s.name.as_str()).collect::<Vec<_>>().join(", ")) } } }
                            @if let Some(date) = &item.premiere_date { div { dt { "Estreno" } dd { (date.get(..10).unwrap_or(date)) } } }
                            @if let Some(data) = &item.user_data { @if data.played { div { dt { "Tu actividad" } dd class="accent-text" { (icon("check")) "Visto" } } } }
                        }
                        p class="playback-note" { (icon("info")) "La reproducción llegará en la siguiente versión." }
                    }
                }
                @if is_series {
                    section class="episodes-section" id="episodes" aria-labelledby="episodes-title" {
                        div class="section-heading" { h2 id="episodes-title" { "Episodios" } span class="muted" { (episode_count) " en tu biblioteca" } }
                        @if seasons.is_empty() { p class="empty-inline" { "Todavía no hay episodios disponibles para esta serie." } }
                        @else {

                            div class="season-toolbar" {
                                button class="season-trigger" type="button" popovertarget="season-picker" {
                                    @if let Some(number) = season { @if number == 0 { "Especiales" } @else { "Temporada " (number) } }
                                    (icon("chevron"))
                                }
                                span { (selected_episodes.map_or(0, Vec::len)) (if selected_episodes.is_some_and(|episodes| episodes.len() == 1) { " episodio" } else { " episodios" }) }
                                div id="season-picker" class="season-picker filter-popover" popover {
                                    nav class="season-tabs" aria-label="Temporadas" { @for number in seasons.keys() {
                                        a href=(format!("/item/{}?season={number}#episodes", item.id)) aria-current=[(Some(*number) == season).then_some("page")] { @if *number == 0 { "Especiales" } @else { "Temporada " (number) } @if Some(*number) == season { (icon("check")) } }
                                    } }
                                }
                            }
                            div class="episode-list" { @if let Some(episodes) = selected_episodes { @for (index, episode) in episodes.iter().enumerate().skip((episode_page - 1) * 40).take(40) {
                                details class="episode" {
                                    summary aria-label=(format!("Episodio {}: {}. Ver sinopsis", episode.index_number.unwrap_or(index as u32 + 1), episode.name)) {
                                        span class="episode-number" { (format!("{:02}", episode.index_number.unwrap_or(index as u32 + 1))) }
                                        (media_image(&episode.image("Primary", 480), 320, 180, "episode-media", false))
                                        div class="episode-info" { h3 { (&episode.name) } p { (episode.overview.as_deref().unwrap_or("Consulta la información de este episodio.")) } }
                                        div class="episode-tail" { span { (episode.runtime()) } @if episode.user_data.as_ref().is_some_and(|u| u.played) { span class="accent-text" aria-label="Visto" { (icon("check")) } } (icon("chevron")) }
                                    }
                                    div class="episode-expanded" { p { (episode.overview.as_deref().unwrap_or("Sin sinopsis disponible en Jellyfin.")) } (metadata(episode)) }
                                }
                            } } }
                            @if episode_pages > 1 {
                                nav class="pagination" aria-label="Páginas de episodios" {
                                    @if episode_page > 1 { a class="button secondary" href=(format!("/item/{}?season={}&episode_page={}#episodes", item.id, season.unwrap_or(0), episode_page - 1)) { (icon("back")) "Anterior" } }
                                    span { "Página " (episode_page) " de " (episode_pages) }
                                    @if episode_page < episode_pages { a class="button secondary" href=(format!("/item/{}?season={}&episode_page={}#episodes", item.id, season.unwrap_or(0), episode_page + 1)) { "Siguiente" (icon("arrow")) } }
                                }
                            }
                        }
                    }
                }
                @if !cast.is_empty() {
                    section class="cast-section" id="cast" aria-labelledby="cast-title" {
                        div class="section-heading" { h2 id="cast-title" { "Reparto" } (rail_controls("cast-rail", "Reparto")) }
                        (rail("cast-rail", "Reparto", "cast-row", html! { @for person in cast {
                            article class="cast-person" {
                                @if let Some(tag) = &person.primary_image_tag { (media_image(&format!("/image/{}/Primary?width=240&tag={}", person.id, encode(tag)), 160, 160, "cast-avatar", false)) }
                                @else { div class="person-placeholder" aria-hidden="true" { (icon("person")) } }
                                h3 { (&person.name) } @if let Some(role) = &person.role { p { (role) } }
                            }
                        } }))
                    }
                }
            }
        },
    )
}

pub fn connection(catalog: &Catalog, warning: Option<&str>) -> Markup {
    layout(
        "Mi biblioteca",
        "connection",
        Some(catalog),
        true,
        html! {
            div class="wrap page-content connection-page" {
                a class="back-link" href="/" { (icon("back")) "Volver al inicio" }
                h1 { "Mi biblioteca" }
                (notice(warning))
                section class="connection-panel" aria-labelledby="server-title" {
                    div class="connection-status" { (avatar(Some(catalog))) div { h2 id="server-title" { (&catalog.server_name) } p { @if warning.is_some() { "Mostrando la última biblioteca disponible" } @else { span class="status-dot" {} "Conectado a Jellyfin" } } } }
                    dl class="library-stats" { div { dt { "Películas" } dd { (catalog.movies()) } } div { dt { "Series" } dd { (catalog.series()) } } div { dt { "Perfil" } dd class="profile-stat" { (&catalog.user.name) } } }
                    p class="connection-description" { "La biblioteca se actualiza al navegar si han pasado cinco minutos. También puedes buscar nuevos títulos ahora." }
                    form action="/refresh" method="post" { button class="button primary" type="submit" { (icon("refresh")) "Actualizar biblioteca" } }
                }
                a class="text-link" href="/library" { "Explorar los " (catalog.items.len()) " títulos" (icon("arrow")) }
            }
        },
    )
}

fn empty(title: &str, message: &str, href: &str, action: &str) -> Markup {
    html! { div class="empty-state" { div class="empty-icon" { (icon("library")) } h2 { (title) } p { (message) } a class="button secondary" href=(href) { (action) (icon("arrow")) } } }
}

pub fn error(message: &str) -> Markup {
    layout(
        "Conectar biblioteca",
        "connection",
        None,
        false,
        html! {
            div class="wrap page-content error-page" { h1 { "Vamos a conectar." } p class="error-description" { (message) }
                form action="/refresh" method="post" { button class="button primary" type="submit" { (icon("refresh")) "Volver a intentar" } }
                p class="muted" { "Cuando Jellyfin esté disponible, tus películas y series aparecerán aquí." }
            }
        },
    )
}

pub fn not_found() -> Markup {
    layout(
        "Título no encontrado",
        "",
        None,
        true,
        html! { div class="wrap page-content" { (empty("Esta historia no está aquí", "Puede que se haya eliminado de tu biblioteca o que el enlace haya cambiado.", "/library", "Volver a la biblioteca")) } },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jellyfin::User;
    use std::time::Instant;
    #[test]
    fn filters_and_rendering_preserve_real_data_safely() {
        let catalog = Catalog {
            items: vec![
                Item {
                    id: "a".into(),
                    name: "Cómo entrenar a tu dragón".into(),
                    kind: "Movie".into(),
                    genres: vec!["Fantasy".into(), "Fantasía".into()],
                    ..Default::default()
                },
                Item {
                    id: "b".into(),
                    name: "<script>alert(1)</script>".into(),
                    kind: "Series".into(),
                    ..Default::default()
                },
            ],
            user: User::default(),
            server_name: "Servidor".into(),
            loaded: Instant::now(),
        };
        assert_eq!(
            filtered_items(
                &catalog,
                "movies",
                &Filters {
                    q: "COMO entrenar".into(),
                    ..Default::default()
                }
            )
            .len(),
            1
        );
        assert!(
            filtered_items(
                &catalog,
                "series",
                &Filters {
                    genre: "Fantasía".into(),
                    ..Default::default()
                }
            )
            .is_empty()
        );
        let page = browse(
            &catalog,
            "library",
            &Filters {
                page: usize::MAX,
                ..Default::default()
            },
            None,
        )
        .into_string();
        assert!(!page.contains("<script>"));
        assert!(page.contains("&lt;script&gt;"));
        assert!(page.contains("/item/b"));
        assert_eq!(catalog.genres().get("Fantasía"), Some(&1));
    }

    #[test]
    fn long_seasons_are_bounded_and_every_episode_remains_reachable() {
        let show = Item {
            id: "show".into(),
            name: "Una serie".into(),
            kind: "Series".into(),
            ..Default::default()
        };
        let episodes: Vec<_> = (1..=81)
            .map(|i| Item {
                id: format!("e{i}"),
                name: format!("Capítulo {i}"),
                kind: "Episode".into(),
                index_number: Some(i),
                ..Default::default()
            })
            .collect();
        let seasons = BTreeMap::from([(1, episodes)]);
        let catalog = Catalog {
            items: vec![show.clone()],
            user: User::default(),
            server_name: String::new(),
            loaded: Instant::now(),
        };
        let first = detail(&catalog, &show, &seasons, None, 0, None).into_string();
        assert_eq!(first.matches("class=\"episode\"").count(), 40);
        assert!(first.contains("episode_page=2#episodes"));
        let last = detail(&catalog, &show, &seasons, Some(1), usize::MAX, None).into_string();
        assert_eq!(last.matches("class=\"episode\"").count(), 1);
        assert!(last.contains("Capítulo 81"));
        assert!(last.contains("Página 3 de 3"));
    }
}
