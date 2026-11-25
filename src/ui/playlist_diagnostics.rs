//! Debug-build-only Playlist counters and scoped timings for performance investigation.
//!
//! These counters describe the work done by Rust UI code. They do not include egui tessellation after the view returns,
//! GPU execution/presentation, Android scheduling, or allocator/GC activity. They intentionally make no rendering or
//! audio-policy changes.

use std::cell::Cell;
use std::time::Duration;

use eframe::egui::{Color32, Painter, Rect, Stroke, StrokeKind, pos2, vec2};

use crate::metrics::FontSize;
use crate::ui::UiCtx;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Snapshot {
    total_clips: usize,
    examined: usize,
    visible: usize,
    painted: usize,
    enumerate_ms: f32,
    paint_ms: f32,
    pattern_ms: f32,
    pattern_calls: usize,
    waveform_source_hits: usize,
    waveform_source_misses: usize,
    waveform_mesh_hits: usize,
    waveform_mesh_misses: usize,
    piano_roll_ms: f32,
}

thread_local! {
    static FRAME: Cell<Snapshot> = const { Cell::new(Snapshot {
        total_clips: 0,
        examined: 0,
        visible: 0,
        painted: 0,
        enumerate_ms: 0.0,
        paint_ms: 0.0,
        pattern_ms: 0.0,
        pattern_calls: 0,
        waveform_source_hits: 0,
        waveform_source_misses: 0,
        waveform_mesh_hits: 0,
        waveform_mesh_misses: 0,
        piano_roll_ms: 0.0,
    }) };
}

fn millis(duration: Duration) -> f32 {
    duration.as_secs_f32() * 1000.0
}

/// Begin one Playlist frame after its clip candidates have been enumerated and culled.
pub(crate) fn begin_playlist_frame(
    total_clips: usize,
    examined: usize,
    visible: usize,
    enumerate: Duration,
) {
    FRAME.with(|frame| {
        frame.set(Snapshot {
            total_clips,
            examined,
            visible,
            enumerate_ms: millis(enumerate),
            ..Snapshot::default()
        });
    });
}

/// Record time spent painting/setting up the intersecting clip blocks and their interactions.
pub(crate) fn finish_clip_paint(painted: usize, elapsed: Duration) {
    FRAME.with(|frame| {
        let mut snapshot = frame.get();
        snapshot.painted = painted;
        snapshot.paint_ms = millis(elapsed);
        frame.set(snapshot);
    });
}

/// Record the Playlist's repeated notes/drums and automation miniature work.
pub(crate) fn pattern_work(elapsed: Duration) {
    FRAME.with(|frame| {
        let mut snapshot = frame.get();
        snapshot.pattern_ms += millis(elapsed);
        snapshot.pattern_calls += 1;
        frame.set(snapshot);
    });
}

/// Count a cached sample-summary lookup result.
pub(crate) fn waveform_source(hit: bool) {
    FRAME.with(|frame| {
        let mut snapshot = frame.get();
        if hit {
            snapshot.waveform_source_hits += 1;
        } else {
            snapshot.waveform_source_misses += 1;
        }
        frame.set(snapshot);
    });
}

/// Count a waveform mesh lookup result.
pub(crate) fn waveform_mesh(hit: bool) {
    FRAME.with(|frame| {
        let mut snapshot = frame.get();
        if hit {
            snapshot.waveform_mesh_hits += 1;
        } else {
            snapshot.waveform_mesh_misses += 1;
        }
        frame.set(snapshot);
    });
}

/// Record the complete Piano Roll UI call, including its interactions and miniature note drawing.
pub(crate) fn piano_roll_work(elapsed: Duration) {
    FRAME.with(|frame| {
        let mut snapshot = frame.get();
        snapshot.piano_roll_ms = millis(elapsed);
        frame.set(snapshot);
    });
}

/// Paint a compact debug HUD over the Playlist. Values are from the current Playlist frame; times exclude backend/GPU
/// work and the separate Piano Roll number is the editor call from this same app frame when it is visible.
pub(crate) fn paint(painter: &Painter, grid: Rect, cx: &UiCtx) {
    let snapshot = FRAME.with(Cell::get);
    let visible = snapshot.visible;
    let rejected = snapshot.examined.saturating_sub(visible);
    let lines = [
        format!(
            "Playlist clips total {} | examined {} | visible {} | off-screen {} | painted {}",
            snapshot.total_clips, snapshot.examined, visible, rejected, snapshot.painted
        ),
        format!(
            "enumerate {:.2} ms | clip paint {:.2} ms | pattern {:.2} ms / {} calls",
            snapshot.enumerate_ms, snapshot.paint_ms, snapshot.pattern_ms, snapshot.pattern_calls
        ),
        format!(
            "wave source H/M {}/{} | mesh H/M {}/{} | Piano Roll {:.2} ms",
            snapshot.waveform_source_hits,
            snapshot.waveform_source_misses,
            snapshot.waveform_mesh_hits,
            snapshot.waveform_mesh_misses,
            snapshot.piano_roll_ms
        ),
    ];
    let font = cx.m.font_id(FontSize::Pt8);
    let color = cx.theme.colors.text.to_color32();
    let galleys = lines.map(|line| painter.layout_no_wrap(line, font.clone(), color));
    let pad = cx.m.u(0.2);
    let line_h = galleys[0].size().y;
    let width = galleys.iter().map(|g| g.size().x).fold(0.0_f32, f32::max);
    let at = pos2(grid.left() + pad, grid.top() + pad);
    let rect = Rect::from_min_size(
        at,
        vec2(width + 2.0 * pad, 3.0 * line_h + 2.0 * pad),
    );
    painter.rect_filled(rect, cx.m.u(0.15), Color32::from_black_alpha(210));
    painter.rect_stroke(
        rect,
        cx.m.u(0.15),
        Stroke::new(1.0 / cx.m.ppp(), color.gamma_multiply(0.25)),
        StrokeKind::Inside,
    );
    for (row, galley) in galleys.into_iter().enumerate() {
        painter.galley(
            rect.min + vec2(pad, pad + row as f32 * line_h),
            galley,
            color,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playlist_diagnostic_counters_reset_and_accumulate_per_frame() {
        begin_playlist_frame(100, 20, 7, Duration::from_micros(250));
        finish_clip_paint(6, Duration::from_micros(900));
        pattern_work(Duration::from_micros(400));
        pattern_work(Duration::from_micros(300));
        waveform_source(true);
        waveform_source(false);
        waveform_mesh(true);
        waveform_mesh(false);

        let got = FRAME.with(Cell::get);
        assert_eq!(got.total_clips, 100);
        assert_eq!((got.examined, got.visible, got.painted), (20, 7, 6));
        assert!((got.enumerate_ms - 0.25).abs() < 1e-4);
        assert!((got.paint_ms - 0.9).abs() < 1e-4);
        assert!((got.pattern_ms - 0.7).abs() < 1e-4);
        assert_eq!(got.pattern_calls, 2);
        assert_eq!(
            (got.waveform_source_hits, got.waveform_source_misses),
            (1, 1)
        );
        assert_eq!((got.waveform_mesh_hits, got.waveform_mesh_misses), (1, 1));

        begin_playlist_frame(3, 2, 2, Duration::ZERO);
        let reset = FRAME.with(Cell::get);
        assert_eq!(reset.total_clips, 3);
        assert_eq!(reset.pattern_calls, 0);
        assert_eq!(
            (reset.waveform_source_hits, reset.waveform_source_misses),
            (0, 0)
        );
        assert_eq!((reset.waveform_mesh_hits, reset.waveform_mesh_misses), (0, 0));
    }
}
