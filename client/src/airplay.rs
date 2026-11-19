//! Safari's own video/AirPlay controls. Only progress crosses the temporary LAN relay.
use wasm_bindgen::{JsCast, prelude::*};
pub fn attach() -> Result<bool, JsValue> {
    let Some(window) = web_sys::window() else {
        return Ok(false);
    };
    let Some(document) = window.document() else {
        return Ok(false);
    };
    let Some(element) = document.get_element_by_id("handoff-video") else {
        return Ok(false);
    };
    let video: web_sys::HtmlMediaElement = element.dyn_into()?;
    let start = video
        .get_attribute("data-start")
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    let media = video.clone();
    super::listen(video.as_ref(), "loadedmetadata", false, move |_| {
        if start > 0.0 {
            media.set_current_time(start);
        }
    })?;
    let last = std::rc::Rc::new(std::cell::Cell::new(0.0));
    for name in ["timeupdate", "play", "pause", "seeked", "ended"] {
        let video = video.clone();
        let window = window.clone();
        let last = last.clone();
        super::listen(video.clone().as_ref(), name, false, move |_| {
            let now = js_sys::Date::now();
            if name == "timeupdate" && now - last.get() < 5000.0 {
                return;
            }
            last.set(now);
            let position = video.current_time();
            if !position.is_finite() {
                return;
            }
            let init = web_sys::RequestInit::new();
            init.set_method("POST");
            init.set_body(&JsValue::from_str(&format!(
                "{{\"position\":{position},\"paused\":{},\"ended\":{}}}",
                video.paused(),
                video.ended()
            )));
            let headers = js_sys::Object::new();
            let _ =
                js_sys::Reflect::set(&headers, &"Content-Type".into(), &"application/json".into());
            init.set_headers(&headers);
            let request = window.fetch_with_str_and_init("./progress", &init);
            let document = window.document();
            let media = video.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let response = wasm_bindgen_futures::JsFuture::from(request).await;
                let ok = response
                    .ok()
                    .and_then(|r| r.dyn_into::<web_sys::Response>().ok())
                    .is_some_and(|r| r.ok());
                if !ok {
                    let _ = media.pause();
                    if let Some(status) =
                        document.and_then(|d| d.get_element_by_id("handoff-status"))
                    {
                        status.set_text_content(Some(
                            "La sesión ha terminado. Vuelve a abrir AirPlay desde Megalodon.",
                        ));
                    }
                }
            });
        })?;
    }
    Ok(true)
}
