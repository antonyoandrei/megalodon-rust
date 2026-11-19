//! Opt-in render evidence, compiled out of release packages. No input automation.
//! Set MEGALODON_MOTION_DIR and compile with --features motion-capture.
use super::*;
use std::path::PathBuf;

#[derive(Default)]
pub(super) struct Capture {
    sequence: usize,
    active: Option<(PathBuf, Instant, usize)>,
    previous: Option<Arc<ColorImage>>,
}

fn save(path: PathBuf, frame: Arc<ColorImage>) {
    std::thread::spawn(move || {
        let pixels: Vec<u8> = frame.pixels.iter().flat_map(|p| p.to_array()).collect();
        let _ = image::save_buffer(
            path,
            &pixels,
            frame.width() as u32,
            frame.height() as u32,
            image::ColorType::Rgba8,
        );
    });
}

impl Capture {
    pub fn begin(&mut self, _ctx: &Context, key: &ui::SceneKey, reduced: bool) {
        let Some(root) = std::env::var_os("MEGALODON_MOTION_DIR") else {
            return;
        };
        self.sequence += 1;
        let mode = if reduced { "reduced" } else { "normal" };
        let folder = PathBuf::from(root).join(format!("{mode}-{:03}", self.sequence));
        if std::fs::create_dir_all(&folder).is_err() {
            return;
        }
        let _ = std::fs::write(
            folder.join("scene.txt"),
            format!(
                "Destination: {key:?}\nReduced motion: {reduced}\nNative framebuffer samples. File time is request time; capture occurs on the next rendered frame. Recording is not a frame-rate benchmark.\n"
            ),
        );
        if let Some(previous) = &self.previous {
            save(folder.join("before.png"), previous.clone());
        }
        self.active = Some((folder, Instant::now(), 0));
    }

    pub fn frame(&mut self, ctx: &Context) {
        ctx.input(|input| {
            for event in &input.raw.events {
                if let Event::Screenshot {
                    user_data, image, ..
                } = event
                    && let Some(path) = user_data
                        .data
                        .as_ref()
                        .and_then(|d| d.downcast_ref::<PathBuf>())
                {
                    self.previous = Some(image.clone());
                    save(path.clone(), image.clone());
                }
            }
        });
        if let Some((folder, start, sample)) = &mut self.active {
            let ms = start.elapsed().as_millis();
            if ms >= (*sample as u128) * 35 {
                ctx.send_viewport_cmd(ViewportCommand::Screenshot(UserData::new(
                    folder.join(format!("{ms:04}ms.png")),
                )));
                *sample += 1;
            }
            if ms < 420 {
                ctx.request_repaint_after(Duration::from_millis(16));
            } else {
                self.active = None;
            }
        }
    }
}
