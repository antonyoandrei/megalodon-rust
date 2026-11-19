use super::*;
use crate::jellyfin::{Config, Item};

async fn fixture(direct: bool) -> (Arc<Library>, data::Playback, tokio::task::JoinHandle<()>) {
    async fn video(headers: HeaderMap, uri: axum::http::Uri) -> Response {
        assert!(
            headers
                .get("authorization")
                .unwrap()
                .to_str()
                .unwrap()
                .contains("test-session")
        );
        assert!(!uri.to_string().contains("api_key"));
        if uri.path().ends_with(".m3u8") {
            return ([("content-type", "application/vnd.apple.mpegurl")],
                "#EXTM3U\n#EXT-X-MAP:URI=\"init.mp4?api_key=test-session\"\n#EXTINF:4,\nsegment.ts?api_key=test-session\n#EXT-X-ENDLIST\n").into_response();
        }
        if headers.get("range").is_some_and(|v| v == "bytes=2-5") {
            return (
                StatusCode::PARTIAL_CONTENT,
                [
                    ("content-range", "bytes 2-5/10"),
                    ("accept-ranges", "bytes"),
                ],
                "CDEF",
            )
                .into_response();
        }
        ([("content-type", "video/mp4")], "ABCDEFGHIJ").into_response()
    }
    let router = Router::new()
        .route(
            "/Items/movie/PlaybackInfo",
            post(move |headers: HeaderMap| async move {
                assert!(
                    headers
                        .get("authorization")
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .contains("test-session")
                );
                axum::Json(json!({"PlaySessionId":"remote-session","MediaSources":[{
                    "Id":"source", "SupportsDirectPlay":direct,
                    "TranscodingUrl":"/Videos/movie/master.m3u8?api_key=test-session"
                }]}))
            }),
        )
        .route("/Videos/movie/{*path}", get(video))
        .route(
            "/Videos/ActiveEncodings",
            axum::routing::delete(|| async { StatusCode::NO_CONTENT }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let library = Arc::new(
        Library::new(Config {
            base_url: format!("http://{address}"),
            token: "test-session".into(),
            user_id: Some("profile".into()),
            port: 3210,
        })
        .unwrap(),
    );
    let playback = data::Playback {
        item: serde_json::from_value::<Item>(
            json!({"Id":"movie", "Name":"A & B", "Type":"Movie", "RunTimeTicks":1200000000_u64}),
        )
        .unwrap(),
        url: String::new(),
        headers: String::new(),
        source_id: "source".into(),
        session_id: "local-session".into(),
        start: 0.0,
        subtitles: vec![],
    };
    (library, playback, task)
}

#[tokio::test]
async fn relay_forwards_ranges_and_revokes_its_ticket_on_close() {
    let (library, playback, server) = fixture(true).await;
    let mut relay = Relay::start(
        library,
        &playback,
        "profile",
        Ipv4Addr::LOCALHOST,
        Some(Ipv4Addr::LOCALHOST),
        0.0,
    )
    .await
    .unwrap();
    let url = relay.media_url();
    assert!(!url.contains("test-session"));
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("Range", "bytes=2-5")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(response.headers()["content-range"], "bytes 2-5/10");
    assert_eq!(response.text().await.unwrap(), "CDEF");
    let wrong = url.replace(&relay.state.ticket, "wrong-ticket");
    assert_eq!(
        client.get(wrong).send().await.unwrap().status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        client.get(relay.watch_url()).send().await.unwrap().status(),
        StatusCode::NOT_FOUND
    );
    relay.close();
    if let Ok(response) = client.get(url).send().await {
        assert!(!response.status().is_success());
    }
    server.abort();
}

#[tokio::test]
async fn hls_rewrites_manifest_and_segment_urls_without_exporting_auth() {
    let (library, playback, server) = fixture(false).await;
    let mut relay = Relay::start(
        library,
        &playback,
        "profile",
        Ipv4Addr::LOCALHOST,
        Some(Ipv4Addr::LOCALHOST),
        0.0,
    )
    .await
    .unwrap();
    let client = reqwest::Client::new();
    let manifest = client
        .get(relay.media_url())
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(manifest.starts_with("#EXTM3U"));
    assert!(!manifest.contains("test-session"));
    assert!(!manifest.contains("api_key"));
    let segment = manifest
        .lines()
        .find(|line| line.starts_with("http"))
        .unwrap();
    assert!(segment.starts_with(&relay.state.public));
    assert_eq!(
        client
            .get(segment)
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
        "ABCDEFGHIJ"
    );
    let init = manifest
        .lines()
        .find(|line| line.starts_with("#EXT-X-MAP"))
        .unwrap()
        .split('"')
        .nth(1)
        .unwrap();
    assert_eq!(
        client.get(init).send().await.unwrap().status(),
        StatusCode::OK
    );
    relay.close();
    server.abort();
}
