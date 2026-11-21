//! Opt-in interval project recovery saves. Never writes the manual-save path or audio state.
use super::BanaForgeApp;
use eframe::egui::{self, Align2, Id, LayerId, Order, Rect, vec2};

#[derive(Default)]
pub(super) struct Timer {
    config: Option<u32>,
    due: f64,
    warning: Option<f64>,
}
impl Timer {
    /// A late/background frame still starts a full five-second warning, never skips it.
    fn tick(&mut self, now: f64, enabled: bool, seconds: u32, dirty: bool, blocked: bool) -> (bool, Option<u32>) {
        let seconds = seconds.clamp(10, 86400);
        if !enabled { *self = Self::default(); return (false, None); }
        if self.config != Some(seconds) {
            self.config = Some(seconds); self.due = now + f64::from(seconds); self.warning = None;
        }
        if !dirty { self.warning = None; self.due = now + f64::from(seconds); return (false, None); }
        if self.warning.is_none() && now >= self.due - 5.0 { self.warning = Some(now + 5.0); }
        if let Some(end) = self.warning {
            if now >= end && !blocked {
                self.warning = None; self.due = now + f64::from(seconds); return (true, None);
            }
            return (false, Some((end - now).ceil().max(0.0) as u32));
        }
        (false, None)
    }
}

#[derive(Default)]
pub(super) struct AutoSaver {
    timer: Timer,
    folder: Option<std::path::PathBuf>,
    generation: u64,
    worker_generation: u64,
    #[cfg(not(target_arch = "wasm32"))]
    worker: Option<std::thread::JoinHandle<Result<std::path::PathBuf, String>>>,
    status: Option<(String, f64)>,
}
impl AutoSaver {
    pub(super) fn reset_project(&mut self) {
        self.timer = Timer::default(); self.folder = None; self.generation = self.generation.wrapping_add(1);
        self.status = None;
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn finish(&mut self) {
        if let Some(worker) = self.worker.take() { let _ = worker.join(); }
    }
}

impl BanaForgeApp {
    pub(super) fn project_autosave(&mut self, ctx: &egui::Context, m: &crate::metrics::Metrics) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let now = ctx.input(|i| i.time);
            if self.project_autosave.worker.as_ref().is_some_and(|w| w.is_finished()) {
                let result = self.project_autosave.worker.take().unwrap().join().unwrap_or_else(|_| Err("save worker stopped".into()));
                if self.project_autosave.worker_generation == self.project_autosave.generation {
                    let text = match result {
                        Ok(folder) => { self.project_autosave.folder = Some(folder); "Auto-Saved recovery copy · My Songs / Auto-saved projects".into() }
                        Err(error) => format!("Auto-Save failed: {error}"),
                    };
                    self.project_autosave.status = Some((text, now));
                }
            }
            let root = self.files.auto_save_root();
            let busy = self.project_autosave.worker.is_some();
            let blocked = busy || self.recording() || self.param_gesture || self.params_moved
                || ctx.input(|i| i.pointer.any_down()) || self.nav.overlays.iter().any(|o| !matches!(o, crate::ui::views::Overlay::Rack(_) | crate::ui::views::Overlay::BananaEditor { .. } | crate::ui::views::Overlay::KeyboardSettings));
            let (save, countdown) = self.project_autosave.timer.tick(now,
                self.settings.auto_save && root.is_some(), self.settings.auto_save_interval_secs,
                self.files.modified(), blocked);
            if save && let Some(root) = root {
                // Plugin state must be captured on the UI thread; encoding and all disk IO run on the worker.
                let snapshot = crate::project_files::auto_save_snapshot(&self.song);
                let write = self.files.codec.write;
                let folder = self.project_autosave.folder.clone();
                let repaint = ctx.clone();
                self.project_autosave.worker_generation = self.project_autosave.generation;
                self.project_autosave.worker = Some(std::thread::spawn(move || {
                    let result = write_recovery(root, folder, snapshot, write);
                    repaint.request_repaint(); result
                }));
            }
            let notice = countdown.map(|n| if n == 0 { "Auto-Save waiting for current edit / recording…".into() }
                else { format!("Auto-Saving recovery copy in {n}…") })
                .or_else(|| self.project_autosave.worker.as_ref().map(|_| "Auto-Saving recovery copy…".into()))
                .or_else(|| self.project_autosave.status.as_ref().filter(|(_, at)| now - at < 4.0).map(|(s, _)| s.clone()));
            if let Some(text) = notice {
                // Painter only: no widget, focus, modal layer or pointer interception.
                let screen = ctx.content_rect();
                let band = Rect::from_min_size(screen.left_top() + vec2(m.u(0.25), m.u(0.15)), vec2((screen.width() - m.u(0.5)).max(1.0), m.u(0.8)));
                let p = ctx.layer_painter(LayerId::new(Order::Tooltip, Id::new("project auto-save notice")));
                p.rect_filled(band, m.u(0.1), self.theme.colors.module_fill.to_color32());
                crate::ui::widgets::truncated(&p, band.shrink(m.u(0.1)), Align2::CENTER_CENTER, &text,
                    m.font_id(crate::metrics::FontSize::Pt8), self.theme.colors.text.to_color32(), 0);
            }
            if busy { ctx.request_repaint_after_secs(0.2); }
            else if countdown.is_some() { ctx.request_repaint_after_secs(0.5); }
            else if self.settings.auto_save && root_available(&self.files) {
                ctx.request_repaint_after_secs((self.project_autosave.timer.due - 5.0 - now).max(0.25) as f32);
            }
            if let Some((_, since)) = &self.project_autosave.status {
                if now - since < 4.0 { ctx.request_repaint_after_secs((4.0 - now + since).max(0.01) as f32); }
            }
        }
        #[cfg(target_arch = "wasm32")]
        let _ = (ctx, m);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn write_recovery(root: std::path::PathBuf, folder: Option<std::path::PathBuf>, song: crate::song::Song,
    write: fn(&crate::song::Song) -> Result<Vec<u8>, String>) -> Result<std::path::PathBuf, String> {
    use std::io::Write;
    let bytes = write(&song)?; // Encode successfully before touching an existing recovery file.
    if bytes.len() as u64 > crate::project_files::MAX_FILE_BYTES { return Err("Project exceeds the readable file-size limit".into()); }
    let folder = match folder {
        Some(folder) => folder,
        None => {
            std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let serial = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos();
            let folder = root.join(format!("Auto-save-{time}-{serial}"));
            std::fs::create_dir(&folder).map_err(|e| e.to_string())?; // Never reuse someone else's folder.
            folder
        }
    };
    crate::project_files::replace_file(&folder.join("Project.flm"), |f| f.write_all(&bytes)).map_err(|e| e.to_string())?;
    Ok(folder)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn auto_save_default_off_interval_countdown_disable_and_custom_timer() {
        let s = crate::settings::Settings::default(); assert!(!s.auto_save); assert_eq!(s.auto_save_interval_secs, 300);
        let mut t = Timer::default();
        assert_eq!(t.tick(0.0, false, 300, true, false), (false, None));
        assert_eq!(t.tick(999.0, false, 300, true, false), (false, None));
        t.tick(0.0, true, 300, true, false);
        assert_eq!(t.tick(294.0, true, 300, true, false), (false, None));
        for n in 0..5 { assert_eq!(t.tick(295.0 + f64::from(n), true, 300, true, false), (false, Some(5-n))); }
        assert_eq!(t.tick(300.0, true, 300, true, false), (true, None));
        t.tick(595.0, true, 300, true, false);
        assert_eq!(t.tick(600.0, false, 300, true, false), (false, None));
        t.tick(0.0, true, 60, true, false);
        assert_eq!(t.tick(55.0, true, 60, true, false), (false, Some(5)));
        assert_eq!(t.tick(60.0, true, 60, true, false), (true, None));
    }
    #[test]
    fn auto_save_late_frame_and_gestures_never_skip_warning_or_save_mid_edit() {
        let mut t = Timer::default(); t.tick(0.0, true, 10, true, false);
        assert_eq!(t.tick(100.0, true, 10, true, false), (false, Some(5)));
        assert_eq!(t.tick(105.0, true, 10, true, true), (false, Some(0)));
        assert_eq!(t.tick(106.0, true, 10, true, false), (true, None));
        t.tick(111.0, true, 10, true, false);
        assert_eq!(t.tick(116.0, true, 10, false, false), (false, None));
    }
    #[test]
    fn auto_save_worker_roundtrip_and_failed_encode_preserves_previous_copy() {
        let root = std::env::temp_dir().join(format!("banaforge-auto-save-test-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let song = crate::song::Song::default();
        let codec = crate::project_files::Codec::default();
        let folder = write_recovery(root.clone(), None, song.clone(), codec.write).unwrap();
        let before = std::fs::read(folder.join("Project.flm")).unwrap();
        (codec.read)(&before).unwrap();
        fn fail(_: &crate::song::Song) -> Result<Vec<u8>, String> { Err("injected encoder failure".into()) }
        assert!(write_recovery(root.clone(), Some(folder.clone()), song, fail).is_err());
        assert_eq!(std::fs::read(folder.join("Project.flm")).unwrap(), before);
        std::fs::create_dir(folder.join(".Project.flm.tmp")).unwrap();
        assert!(write_recovery(root.clone(), Some(folder.clone()), crate::song::Song::default(), codec.write).is_err());
        assert_eq!(std::fs::read(folder.join("Project.flm")).unwrap(), before);
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn root_available(files: &crate::project_files::ProjectFiles) -> bool { files.disk_user_dir().is_some() }
