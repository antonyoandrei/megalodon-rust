//! Progressive enhancement only. Content, routes, search and filters remain server-rendered.
mod airplay;
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{Document, Element, Event, EventTarget, HtmlImageElement, Window};

fn elements(root: &Element, selector: &str) -> Vec<Element> {
    root.query_selector_all(selector)
        .map(|nodes| {
            (0..nodes.length())
                .filter_map(|i| nodes.get(i)?.dyn_into().ok())
                .collect()
        })
        .unwrap_or_default()
}

fn listen(
    target: &EventTarget,
    name: &str,
    capture: bool,
    f: impl FnMut(Event) + 'static,
) -> Result<(), JsValue> {
    let callback = Closure::wrap(Box::new(f) as Box<dyn FnMut(Event)>);
    target.add_event_listener_with_callback_and_bool(
        name,
        callback.as_ref().unchecked_ref(),
        capture,
    )?;
    // These listeners live for this document; ordinary navigation releases the module and DOM.
    callback.forget();
    Ok(())
}

fn set_flag(element: &Element, name: &str, value: bool) {
    let _ = element.set_attribute(name, if value { "true" } else { "false" });
}

fn image_finished(image: &HtmlImageElement, observer: &web_sys::IntersectionObserver) {
    if let Some(frame) = image
        .parent_element()
        .filter(|p| p.has_attribute("data-media"))
    {
        let state = if image.natural_width() > 0 {
            "ready"
        } else {
            "error"
        };
        let _ = frame.set_attribute("data-load-state", state);
        let _ = frame.remove_attribute("aria-busy");
        observer.unobserve(&frame);
    }
}

fn images(document: &Document, root: &Element) -> Result<(), JsValue> {
    let callback = Closure::<dyn FnMut(js_sys::Array, web_sys::IntersectionObserver)>::new(
        |entries: js_sys::Array, _: web_sys::IntersectionObserver| {
            for value in entries.iter() {
                let entry: web_sys::IntersectionObserverEntry = value.unchecked_into();
                set_flag(&entry.target(), "data-in-view", entry.is_intersecting());
            }
        },
    );
    let observer = web_sys::IntersectionObserver::new(callback.as_ref().unchecked_ref())?;
    callback.forget();
    for event_name in ["load", "error"] {
        let observer = observer.clone();
        listen(document.as_ref(), event_name, true, move |event| {
            if let Some(image) = event
                .target()
                .and_then(|t| t.dyn_into::<HtmlImageElement>().ok())
            {
                image_finished(&image, &observer);
            }
        })?;
    }
    for frame in elements(root, "[data-media]") {
        let Some(image) = frame
            .query_selector("img")?
            .and_then(|e| e.dyn_into::<HtmlImageElement>().ok())
        else {
            continue;
        };
        if image.complete() {
            image_finished(&image, &observer);
        } else {
            frame.set_attribute("data-load-state", "loading")?;
            frame.set_attribute("aria-busy", "true")?;
            observer.observe(&frame);
        }
    }
    let root = root.clone();
    let document = document.clone();
    listen(
        document.clone().as_ref(),
        "visibilitychange",
        false,
        move |_| {
            set_flag(&root, "data-page-hidden", document.hidden());
        },
    )
}

fn update_rail(rail: &Element) {
    let Some(window) = rail.parent_element() else {
        return;
    };
    let max = (rail.scroll_width() - rail.client_width()).max(0);
    let position = rail.scroll_left();
    let previous = position > 2;
    let next = position < max - 2;
    set_flag(&window, "data-previous", previous);
    set_flag(&window, "data-next", next);
    if let Some(section) = window.parent_element() {
        for button in elements(&section, "[data-scroll]") {
            let available = if button.get_attribute("data-scroll").as_deref() == Some("previous") {
                previous
            } else {
                next
            };
            if let Some(button) = button.dyn_ref::<web_sys::HtmlButtonElement>() {
                button.set_disabled(!available);
            }
        }
    }
}

fn page_target(
    position: f64,
    stride: f64,
    visible_width: f64,
    maximum: f64,
    direction: f64,
) -> f64 {
    let count = (visible_width / stride).round().max(1.0);
    ((position / stride).round() * stride + direction * stride * count).clamp(0.0, maximum.max(0.0))
}

fn move_rail(window: &Window, rail: &Element, direction: &str) {
    let max = (rail.scroll_width() - rail.client_width()).max(0) as f64;
    let Some(first) = rail.first_element_child() else {
        return;
    };
    let style = window.get_computed_style(rail).ok().flatten();
    let px = |name: &str| {
        style
            .as_ref()
            .and_then(|s| s.get_property_value(name).ok())
            .and_then(|v| v.trim_end_matches("px").parse::<f64>().ok())
            .unwrap_or(0.0)
    };
    let gap = px("column-gap");
    let stride = first.get_bounding_client_rect().width() + gap;
    if stride < 1.0 {
        return;
    }
    let visible = rail.client_width() as f64 - px("padding-left") - px("padding-right") + gap;
    let target = match direction {
        "start" => 0.0,
        "end" => max,
        "previous" => page_target(rail.scroll_left() as f64, stride, visible, max, -1.0),
        _ => page_target(rail.scroll_left() as f64, stride, visible, max, 1.0),
    };
    let reduced = window
        .match_media("(prefers-reduced-motion: reduce)")
        .ok()
        .flatten()
        .is_some_and(|m| m.matches())
        || window
            .document()
            .and_then(|d| d.document_element())
            .is_some_and(|e| e.has_attribute("data-reduce-motion"));
    let options = web_sys::ScrollToOptions::new();
    options.set_left(target);
    options.set_behavior(if reduced {
        web_sys::ScrollBehavior::Instant
    } else {
        web_sys::ScrollBehavior::Smooth
    });
    rail.scroll_to_with_scroll_to_options(&options);
}

fn rails(window: &Window, document: &Document, root: &Element) -> Result<(), JsValue> {
    for rail in elements(root, "[data-rail]") {
        update_rail(&rail);
        let scroll_rail = rail.clone();
        listen(rail.as_ref(), "scroll", false, move |_| {
            update_rail(&scroll_rail)
        })?;
        let key_rail = rail.clone();
        let window = window.clone();
        listen(rail.as_ref(), "keydown", false, move |event| {
            if event
                .target()
                .and_then(|t| t.dyn_into::<Element>().ok())
                .as_ref()
                != Some(&key_rail)
            {
                return;
            }
            let Some(key) = event.dyn_ref::<web_sys::KeyboardEvent>() else {
                return;
            };
            let direction = match key.key().as_str() {
                "ArrowLeft" => "previous",
                "ArrowRight" => "next",
                "Home" => "start",
                "End" => "end",
                _ => return,
            };
            event.prevent_default();
            move_rail(&window, &key_rail, direction);
        })?;
    }
    let root_resize = root.clone();
    listen(window.as_ref(), "resize", false, move |_| {
        for rail in elements(&root_resize, "[data-rail]") {
            update_rail(&rail);
        }
    })?;
    let window = window.clone();
    let document = document.clone();
    listen(document.clone().as_ref(), "click", false, move |event| {
        let Some(target) = event.target().and_then(|t| t.dyn_into::<Element>().ok()) else {
            return;
        };
        if let Ok(Some(button)) = target.closest("button[data-scroll]")
            && let Some(id) = button.get_attribute("aria-controls")
            && let Some(rail) = document.get_element_by_id(&id)
        {
            move_rail(
                &window,
                &rail,
                &button.get_attribute("data-scroll").unwrap_or_default(),
            );
        }
    })
}

fn navigation(window: &Window, document: &Document, root: &Element) -> Result<(), JsValue> {
    let nav_root = root.clone();
    listen(document.as_ref(), "click", false, move |event| {
        if event.default_prevented() {
            return;
        }
        let Some(mouse) = event.dyn_ref::<web_sys::MouseEvent>() else {
            return;
        };
        if mouse.button() != 0
            || mouse.ctrl_key()
            || mouse.meta_key()
            || mouse.shift_key()
            || mouse.alt_key()
        {
            return;
        }
        let Some(target) = event.target().and_then(|t| t.dyn_into::<Element>().ok()) else {
            return;
        };
        if let Ok(Some(link)) = target.closest("a[href]") {
            let href = link.get_attribute("href").unwrap_or_default();
            if href.starts_with('/')
                && !href.starts_with("//")
                && !link.has_attribute("target")
                && !link.has_attribute("download")
            {
                set_flag(&nav_root, "data-navigating", true);
            }
        }
    })?;
    let form_root = root.clone();
    listen(document.as_ref(), "submit", false, move |_| {
        set_flag(&form_root, "data-navigating", true)
    })?;
    let root = root.clone();
    listen(window.as_ref(), "pageshow", false, move |_| {
        let _ = root.remove_attribute("data-navigating");
        for rail in elements(&root, "[data-rail]") {
            update_rail(&rail);
        }
    })
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    if airplay::attach()? {
        return Ok(());
    }
    let window = web_sys::window().ok_or("No window")?;
    let document = window.document().ok_or("No document")?;
    let root = document.document_element().ok_or("No root")?;
    images(&document, &root)?;
    rails(&window, &document, &root)?;
    features(&window, &document, &root)?;
    navigation(&window, &document, &root)?;
    root.set_attribute("data-client-ready", "true")
}

fn feature_busy(root: &Element, target: Option<&str>) {
    for link in elements(root, "[data-feature-target]") {
        if link.get_attribute("data-feature-target").as_deref() == target {
            let _ = link.set_attribute("aria-busy", "true");
        } else {
            let _ = link.remove_attribute("aria-busy");
        }
    }
}

fn features(window: &Window, document: &Document, root: &Element) -> Result<(), JsValue> {
    let Some(hero) = root.query_selector(".home-hero")? else {
        return Ok(());
    };
    let finished_hero = hero.clone();
    listen(hero.as_ref(), "transitionend", false, move |event| {
        if let Some(visual) = event.target().and_then(|t| t.dyn_into::<Element>().ok())
            && visual
                .matches(".hero-slide[data-active='true']>.hero-visual")
                .unwrap_or(false)
        {
            for slide in elements(&finished_hero, ".hero-slide[data-active='false']") {
                let _ = slide.set_attribute("hidden", "");
                let _ = slide.remove_attribute("data-leaving");
            }
        }
    })?;
    let document = document.clone();
    let window = window.clone();
    let root = root.clone();
    listen(document.clone().as_ref(), "click", false, move |event| {
        let Some(mouse) = event.dyn_ref::<web_sys::MouseEvent>() else {
            return;
        };
        if event.default_prevented()
            || mouse.button() != 0
            || mouse.ctrl_key()
            || mouse.meta_key()
            || mouse.shift_key()
            || mouse.alt_key()
        {
            return;
        }
        let Some(target) = event.target().and_then(|t| t.dyn_into::<Element>().ok()) else {
            return;
        };
        let Ok(Some(link)) = target.closest("[data-feature-target]") else {
            return;
        };
        let Some(id) = link.get_attribute("data-feature-target") else {
            return;
        };
        let Some(slide) = document.get_element_by_id(&id) else {
            return;
        };
        event.prevent_default();
        let _ = hero.set_attribute("data-requested-feature", &id);
        if slide.get_attribute("data-active").as_deref() == Some("true") {
            feature_busy(&root, None);
            if let Ok(Some(status)) = hero.query_selector(".hero-status") {
                status.set_text_content(None);
            }
            return;
        }
        feature_busy(&root, Some(&id));
        if let Ok(Some(status)) = hero.query_selector(".hero-status") {
            status.set_text_content(Some("Cargando escena…"));
        }
        let _ = slide.remove_attribute("hidden");
        let root = root.clone();
        let hero = hero.clone();
        let window = window.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let Some(image) = slide
                .query_selector(".hero-media>img")
                .ok()
                .flatten()
                .and_then(|e| e.dyn_into::<HtmlImageElement>().ok())
            else {
                return;
            };
            let _ = image.set_attribute("loading", "eager");
            // Establish the incoming layer at opacity 0 even when its image is cached.
            let _ = slide.get_bounding_client_rect();
            if image.complete() && image.natural_width() == 0 {
                image.set_src(&image.src());
            }
            let decoded = wasm_bindgen_futures::JsFuture::from(image.decode()).await;
            if hero.get_attribute("data-requested-feature").as_deref() != Some(&id) {
                if slide.get_attribute("data-active").as_deref() != Some("true") {
                    let _ = slide.set_attribute("hidden", "");
                }
                return;
            }
            feature_busy(&root, None);
            let status = hero.query_selector(".hero-status").ok().flatten();
            if decoded.is_err() {
                if let Some(status) = status {
                    status.set_text_content(Some(
                        "Escena no disponible. Pulsa de nuevo para reintentar.",
                    ));
                }
                let _ = slide.set_attribute("hidden", "");
                return;
            }
            if let Some(status) = status {
                status.set_text_content(None);
            }
            let reduced = window
                .match_media("(prefers-reduced-motion: reduce)")
                .ok()
                .flatten()
                .is_some_and(|m| m.matches())
                || root.has_attribute("data-reduce-motion");
            for panel in elements(&hero, ".hero-slide") {
                let active = panel.id() == id;
                let was_active = panel.get_attribute("data-active").as_deref() == Some("true");
                set_flag(&panel, "data-leaving", !active && was_active && !reduced);
                set_flag(&panel, "data-active", active);
                set_flag(&panel, "aria-hidden", !active);
                if active {
                    let _ = panel.remove_attribute("inert");
                    let _ = panel.remove_attribute("hidden");
                } else {
                    let _ = panel.set_attribute("inert", "");
                    if reduced || !was_active {
                        let _ = panel.set_attribute("hidden", "");
                    }
                }
            }
            for option in elements(&hero, "[data-feature-target]") {
                if option.get_attribute("data-feature-target").as_deref() == Some(&id) {
                    let _ = option.set_attribute("aria-current", "true");
                } else {
                    let _ = option.remove_attribute("aria-current");
                }
            }
            if let Some(title) = slide.query_selector("h1").ok().flatten() {
                let _ = hero.set_attribute("aria-labelledby", &title.id());
            }
            if let (Ok(history), Some(href)) = (window.history(), link.get_attribute("href")) {
                let _ = history.replace_state_with_url(&JsValue::NULL, "", Some(&href));
            }
        });
    })
}

#[cfg(test)]
mod tests {
    use super::page_target;

    #[test]
    fn carousel_pages_remain_aligned_and_reach_both_ends() {
        assert_eq!(page_target(0.0, 180.0, 900.0, 1620.0, 1.0), 900.0);
        assert_eq!(page_target(900.0, 180.0, 900.0, 1620.0, 1.0), 1620.0);
        assert_eq!(page_target(1620.0, 180.0, 900.0, 1620.0, -1.0), 720.0);
        assert_eq!(page_target(0.0, 180.0, 900.0, 1620.0, -1.0), 0.0);
        assert_eq!(page_target(210.0, 280.0, 310.0, 1500.0, 1.0), 560.0);
    }
}
