//! Full app-frame CPU control. Recorder is a fake sink; no hardware playback claims.
use super::*;
use std::{hint::black_box, time::Instant};

/// Scripted pointer interactions, not isolated helpers. Timings are desktop/headless full-app frames.
#[test]
#[ignore = "controlled release scripted playlist interactions; no APK/device/GPU"]
fn high_clip_count_playlist_interactions() {
    assert!(!cfg!(debug_assertions));
    for kind in ["audio", "audio_long"] {
        let fixture = crate::ui::clips::count_profile::fixture(kind, 4096);
        for action in ["select_tap", "move_drag", "resize_drag", "horizontal_scroll"] {
            let mut samples = Vec::new();
            for _ in 0..5 {
                let mut rig = Rig::new();
                rig.app.audio.samples.load(&fixture.song, &|_| Some(fixture.wav.clone()));
                rig.app.song = fixture.song.clone();
                rig.app.selected_channel = 1;
                rig.pin_playlist();
                let clip_beats = if kind == "audio_long" { 64.0 } else { 0.25 };
                rig.app.main_screen.zoom = (10.0_f32 / (clip_beats * 38.4)).log2();
                rig.frame(vec![]);
                let lane_id = egui::Id::new(("clip_lane", 1_usize, 0_usize));
                let lane = rig.ctx.read_response(lane_id).expect("playlist lane drawn").rect;
                let start = pos2(lane.left() + 5.0, lane.center().y);
                let original_id = fixture.song.channels[1].lanes[0].clips[0].id;
                let original_start = fixture.song.channels[1].lanes[0].clips[0].start_tick;
                let original_len = fixture.song.channels[1].lanes[0].clips[0].visible_ticks();
                let t = Instant::now();
                match action {
                    "select_tap" => {
                        rig.press(start);
                        rig.release(start);
                        rig.frame(vec![]);
                        assert!(rig.app.main_screen.clips.selected.is_some(), "clip tap must select");
                    }
                    "move_drag" => {
                        rig.press(start);
                        rig.drag_to(start + vec2(12.0, 0.0));
                        rig.drag_to(start + vec2(32.0, 0.0));
                        rig.release(start + vec2(32.0, 0.0));
                        let moved = rig.app.song.channels[1].lanes[0].clips.iter()
                            .find(|clip| clip.id == original_id).expect("moved clip remains");
                        assert_ne!(moved.start_tick, original_start, "body drag must move the first clip");
                    }
                    "resize_drag" => {
                        rig.press(start);
                        rig.release(start);
                        rig.frame(vec![]);
                        let id = egui::Id::new(("clip_handle", crate::ui::clips::Part::End));
                        let handle = rig.ctx.read_response(id).expect("selected clip end handle drawn").rect;
                        let from = handle.center();
                        rig.press(from);
                        rig.drag_to(from + vec2(28.0, 0.0));
                        rig.release(from + vec2(28.0, 0.0));
                        let resized = rig.app.song.channels[1].lanes[0].clips.iter()
                            .find(|clip| clip.id == original_id).expect("resized clip remains");
                        assert_ne!(resized.visible_ticks(), original_len, "end-handle drag must resize the clip");
                    }
                    "horizontal_scroll" => {
                        rig.frame(vec![
                            egui::Event::PointerMoved(lane.center()),
                            egui::Event::MouseWheel {
                                unit: egui::MouseWheelUnit::Point,
                                delta: vec2(-80.0, 0.0),
                                modifiers: Default::default(),
                            },
                        ]);
                        assert_ne!(rig.app.main_screen.scroll_x, 0.0, "wheel must scroll the playlist");
                    }
                    _ => unreachable!(),
                }
                samples.push(t.elapsed().as_secs_f64() * 1000.0);
            }
            samples.sort_by(f64::total_cmp);
            println!("CLIP_PROFILE {}", serde_json::json!({
                "phase":"scripted_interaction", "case":kind, "clips":fixture.song.channels[1].lanes[0].clips.len(),
                "action":action, "repeats":samples.len(), "total_ms":samples,
                "median_ms":samples[samples.len()/2], "desktop_headless":true,
                "fake_audio_sink":true, "hardware_xruns":null, "gpu_fps":null
            }));
        }
    }
}

#[test]
#[ignore = "controlled release full-app clip count profile"]
fn high_clip_count_full_app() {
    assert!(!cfg!(debug_assertions));
    for hits in [128, 512, 2048, 4096] {
        for kind in ["audio", "audio_long", "missing", "empty", "notes", "notes_long", "drums_steps", "drums_steps_long", "drums_notes", "drums_notes_long"] {
            for view in ["overview", "close_end"] {
                for mode in ["idle", "scroll", "all_selected", "all_selected_indexed"] {
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
                    if mode.starts_with("all_selected") {
                        let refs = rig.app.song.channels[1].lanes[0].clips.iter().map(|c| crate::ui::clips::ClipRef {
                            channel: 1, lane: 0, id: c.id,
                        }).collect();
                        rig.app.main_screen.clips.set_selection(refs);
                    }
                    let before = rig.app.song.clone();
                    crate::ui::clips::reset_wave_mesh_cache();
                    let mut times = Vec::new();
                    let mut tess = Vec::new();
                    for frame in 0..36 {
                        if mode == "scroll" { rig.app.main_screen.scroll_x += 0.01; }
                        let start = Instant::now();
                        crate::ui::clips::with_profile_indexed_retention(
                            mode == "all_selected_indexed",
                            || rig.frame(vec![egui::Event::PointerMoved(pos2(500.0 + (frame % 8) as f32, 145.0))]),
                        );
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
                    let wave_stats = crate::ui::clips::wave_mesh_profile_stats();
                    let (wave_cache_entries, wave_cache_bytes) = crate::ui::clips::wave_mesh_cache_size();
                    println!("CLIP_PROFILE {}", serde_json::json!({"phase":"full_app_cpu", "case":kind,"hits":hits,
                        "clips":before.channels[1].lanes[0].clips.len(),"view":view,"mode":mode,"frames":30,
                        "ui_p50_ms":times[15],"ui_p95_ms":times[28],"ui_p99_ms":times[29],
                        "tess_p50_ms":tess[15],"tess_p95_ms":tess[28],
                        "wave_cache_hits":wave_stats.hits,"wave_cache_misses":wave_stats.misses,
                        "wave_cache_inserts":wave_stats.inserts,"wave_cache_evictions":wave_stats.evictions,
                        "wave_cache_expired":wave_stats.expired,"wave_cache_bypasses":wave_stats.bypasses,
                        "wave_cache_entries":wave_cache_entries,"wave_cache_bytes":wave_cache_bytes,
                        "candidate_index":"only all_selected_indexed; index build included per frame",
                        "fake_audio_sink":true,"hardware_xruns":null,"gpu_fps":null}));
                }
            }
        }
    }
}
