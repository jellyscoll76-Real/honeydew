//! Full app-frame CPU control. Recorder is a fake sink; no hardware playback claims.
use super::*;
use std::{hint::black_box, time::Instant};

#[test]
#[ignore = "controlled release full-app clip count profile"]
fn high_clip_count_full_app() {
    assert!(!cfg!(debug_assertions));
    for hits in [128, 512, 2048, 4096] {
        for kind in ["audio", "audio_long", "missing", "empty", "notes", "notes_long", "drums_steps", "drums_notes"] {
            for view in ["overview", "close_end"] {
                for mode in ["idle", "scroll", "all_selected"] {
                    let fixture = crate::ui::clips::count_profile::fixture(kind, hits);
                    let wav = fixture.wav;
                    let mut rig = Rig::new();
                    rig.app.audio.samples.load(&fixture.song, &|_| (kind != "missing").then(|| wav.clone()));
                    rig.app.song = fixture.song;
                    rig.app.selected_channel = 1;
                    rig.pin_playlist();
                    let duration = hits as f64 * 0.25;
                    if view == "overview" {
                        rig.app.main_screen.zoom = (960.0 / duration as f32 / 38.4).log2();
                    } else {
                        rig.app.main_screen.zoom = 0.0;
                        rig.app.main_screen.scroll_x = (duration - 16.0).max(0.0);
                    }
                    if mode == "all_selected" {
                        let refs = rig.app.song.channels[1].lanes[0].clips.iter().map(|c| crate::ui::clips::ClipRef {
                            channel: 1, lane: 0, id: c.id,
                        }).collect();
                        rig.app.main_screen.clips.set_selection(refs);
                    }
                    let before = rig.app.song.clone();
                    let mut times = Vec::new();
                    let mut tess = Vec::new();
                    for frame in 0..36 {
                        if mode == "scroll" { rig.app.main_screen.scroll_x += 0.01; }
                        let start = Instant::now();
                        rig.frame(vec![egui::Event::PointerMoved(pos2(500.0 + (frame % 8) as f32, 145.0))]);
                        let ui = start.elapsed().as_secs_f64() * 1000.0;
                        let start = Instant::now();
                        black_box(rig.ctx.tessellate(rig.out.shapes.clone(), 1.0));
                        let t = start.elapsed().as_secs_f64() * 1000.0;
                        if frame >= 6 { times.push(ui); tess.push(t); }
                    }
                    // View bookkeeping may change, but frames must not create or delete project clips.
                    assert_eq!(
                        rig.app.song.channels[1].lanes[0].clips.len(),
                        before.channels[1].lanes[0].clips.len()
                    );
                    times.sort_by(f64::total_cmp);
                    tess.sort_by(f64::total_cmp);
                    println!("CLIP_PROFILE {}", serde_json::json!({"phase":"full_app_cpu", "case":kind,"hits":hits,
                        "clips":before.channels[1].lanes[0].clips.len(),"view":view,"mode":mode,"frames":30,
                        "ui_p50_ms":times[15],"ui_p95_ms":times[28],"ui_p99_ms":times[29],
                        "tess_p50_ms":tess[15],"tess_p95_ms":tess[28],
                        "fake_audio_sink":true,"hardware_xruns":null,"gpu_fps":null}));
                }
            }
        }
    }
}
