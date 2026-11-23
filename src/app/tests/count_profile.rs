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
        for action in [
            "select_tap",
            "move_drag",
            "resize_drag",
            "horizontal_scroll",
        ] {
            let mut samples = Vec::new();
            let mut effects = Vec::new();
            let mut handle_rect = None;
            let mut resize_details = None;
            for _ in 0..5 {
                let mut rig = Rig::new();
                rig.app
                    .audio
                    .samples
                    .load(&fixture.song, &|_| Some(fixture.wav.clone()));
                rig.app.song = fixture.song.clone();
                rig.app.selected_channel = 1;
                rig.pin_playlist();
                let clip_beats = if kind == "audio_long" { 64.0 } else { 0.25 };
                rig.app.main_screen.zoom = (10.0_f32 / (clip_beats * 38.4)).log2();
                rig.frame(vec![]);
                let lane_id = egui::Id::new(("clip_lane", 1_usize, 0_usize));
                let lane = rig
                    .ctx
                    .read_response(lane_id)
                    .expect("playlist lane drawn")
                    .rect;
                let clip_index = if kind == "audio_long" { 0 } else { 20 };
                let start = pos2(
                    lane.left() + clip_index as f32 * 10.0 + 5.0,
                    lane.center().y,
                );
                let target = &fixture.song.channels[1].lanes[0].clips[clip_index];
                let original_id = target.id;
                let original_start = target.start_tick;
                let original_len = target.visible_ticks();
                if action == "move_drag" || action == "resize_drag" {
                    rig.press(start);
                    rig.release(start);
                    for _ in 0..4 {
                        rig.frame(vec![]);
                    }
                    assert!(
                        rig.app.main_screen.clips.selected.is_some(),
                        "clip must be selected before drag"
                    );
                    if action == "resize_drag" {
                        // Keep the intentionally floating action row from covering a sub-pixel dense clip's handle.
                        rig.app.main_screen.clips.row.hide();
                        rig.frame(vec![]);
                    }
                }
                let mut effect = false;
                let mut current_handle_rect = None;
                let mut current_resize_details = None;
                let t = Instant::now();
                match action {
                    "select_tap" => {
                        rig.press(start);
                        rig.release(start);
                        rig.frame(vec![]);
                        effect = rig.app.main_screen.clips.selected.is_some();
                    }
                    "move_drag" => {
                        rig.press(start);
                        rig.drag_to(start + vec2(12.0, 0.0));
                        rig.drag_to(start + vec2(32.0, 0.0));
                        rig.release(start + vec2(32.0, 0.0));
                        effect = rig.app.song.channels[1].lanes[0]
                            .clips
                            .iter()
                            .find(|clip| clip.id == original_id)
                            .is_some_and(|clip| clip.start_tick != original_start);
                    }
                    "resize_drag" => {
                        let id = egui::Id::new(("clip_handle", crate::ui::clips::Part::End));
                        if let Some(response) = rig.ctx.read_response(id) {
                            let handle = response.rect;
                            current_handle_rect = Some(format!("{:?}", handle));
                            let from = handle.center();
                            rig.press(from);
                            let captured = rig
                                .ctx
                                .read_response(id)
                                .is_some_and(|r| r.is_pointer_button_down_on());
                            rig.drag_to(from + vec2(28.0, 0.0));
                            rig.release(from + vec2(28.0, 0.0));
                            let result_len = rig.app.song.channels[1].lanes[0]
                                .clips
                                .iter()
                                .find(|clip| clip.id == original_id)
                                .map(|clip| clip.visible_ticks());
                            let resized = result_len.is_some_and(|len| len != original_len);
                            current_resize_details = Some(serde_json::json!({
                                "captured":captured, "old_visible_ticks":original_len,
                                "new_visible_ticks":result_len, "press_at":format!("{:?}", from),
                                "lane_rect":format!("{:?}", lane), "handle_rect":format!("{:?}", handle)
                            }));
                            effect = captured && resized;
                        }
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
                        effect = rig.app.main_screen.scroll_x != 0.0;
                    }
                    _ => unreachable!(),
                }
                samples.push(t.elapsed().as_secs_f64() * 1000.0);
                effects.push(effect);
                if current_handle_rect.is_some() {
                    handle_rect = current_handle_rect;
                }
                if current_resize_details.is_some() {
                    resize_details = current_resize_details;
                }
            }
            samples.sort_by(f64::total_cmp);
            println!(
                "CLIP_PROFILE {}",
                serde_json::json!({
                    "phase":"scripted_interaction", "case":kind, "clips":fixture.song.channels[1].lanes[0].clips.len(),
                    "action":action, "repeats":samples.len(), "total_ms":samples,
                    "median_ms":samples[samples.len()/2], "effect_observed":effects,
                    "handle_response_rect":handle_rect, "resize_details":resize_details,
                    "desktop_headless":true,
                    "fake_audio_sink":true, "hardware_xruns":null, "gpu_fps":null
                })
            );
        }
    }
}

#[test]
#[ignore = "diagnose adjacent dense short-clip end-handle capture across spacings and touch points"]
fn high_clip_count_dense_resize_hit_test_diagnostic() {
    assert!(!cfg!(debug_assertions));
    for isolate_following_clips in [false, true] {
        let case = if isolate_following_clips {
            "following_clips_out_of_view"
        } else {
            "dense_adjacent"
        };
        let fixture = crate::ui::clips::count_profile::fixture("audio", 4096);
        let target_index = 20;
        let target_id = fixture.song.channels[1].lanes[0].clips[target_index].id;
        let mut source = fixture.song.clone();
        if isolate_following_clips {
            for (i, clip) in source.channels[1].lanes[0]
                .clips
                .iter_mut()
                .enumerate()
                .skip(target_index + 1)
            {
                clip.start_tick = 100_000_000 + i as u32 * 1000;
            }
        }
        for clip_width in [6.0_f32, 10.0, 16.0] {
            for handle_fraction in [0.2_f32, 0.5, 0.8] {
                let mut results = Vec::new();
                for _ in 0..5 {
                    let mut rig = Rig::new();
                    rig.app
                        .audio
                        .samples
                        .load(&source, &|_| Some(fixture.wav.clone()));
                    rig.app.song = source.clone();
                    rig.app.selected_channel = 1;
                    rig.pin_playlist();
                    rig.app.main_screen.zoom = (clip_width / (0.25 * 38.4)).log2();
                    rig.frame(vec![]);
                    let lane_id = egui::Id::new(("clip_lane", 1_usize, 0_usize));
                    let lane = rig
                        .ctx
                        .read_response(lane_id)
                        .expect("playlist lane drawn")
                        .rect;
                    let start = pos2(
                        lane.left() + target_index as f32 * clip_width + clip_width / 2.0,
                        lane.center().y,
                    );
                    rig.press(start);
                    rig.release(start);
                    for _ in 0..4 {
                        rig.frame(vec![]);
                    }
                    assert_eq!(
                        rig.app.main_screen.clips.selected.map(|r| r.id),
                        Some(target_id)
                    );
                    rig.app.main_screen.clips.row.hide();
                    rig.frame(vec![]);
                    let handle_id = egui::Id::new(("clip_handle", crate::ui::clips::Part::End));
                    let response = rig
                        .ctx
                        .read_response(handle_id)
                        .expect("end handle response registered");
                    let handle = response.rect;
                    let from = pos2(
                        handle.left() + handle.width() * handle_fraction,
                        handle.center().y,
                    );
                    let before = rig.app.song.channels[1].lanes[0]
                        .clips
                        .iter()
                        .find(|clip| clip.id == target_id)
                        .unwrap()
                        .visible_ticks();
                    rig.press(from);
                    let captured = rig
                        .ctx
                        .read_response(handle_id)
                        .is_some_and(|r| r.is_pointer_button_down_on());
                    rig.drag_to(from + vec2(28.0, 0.0));
                    rig.release(from + vec2(28.0, 0.0));
                    let after = rig.app.song.channels[1].lanes[0]
                        .clips
                        .iter()
                        .find(|clip| clip.id == target_id)
                        .unwrap()
                        .visible_ticks();
                    assert!(
                        captured,
                        "end handle did not capture: {case}, width={clip_width}, fraction={handle_fraction}"
                    );
                    assert_ne!(
                        before, after,
                        "end handle did not resize: {case}, width={clip_width}, fraction={handle_fraction}"
                    );
                    results.push(serde_json::json!({
                        "captured":captured, "resized":before != after, "before_ticks":before,
                        "after_ticks":after, "press_at":format!("{:?}",from),
                        "handle_response_rect":format!("{:?}",handle), "lane_rect":format!("{:?}",lane)
                    }));
                }
                println!(
                    "CLIP_PROFILE {}",
                    serde_json::json!({
                        "phase":"resize_hit_test_diagnostic", "case":case, "clips":source.channels[1].lanes[0].clips.len(),
                        "target_index":target_index, "clip_width_px":clip_width,
                        "handle_fraction":handle_fraction, "repeats":results
                    })
                );
            }
        }
    }
}

#[test]
#[ignore = "controlled release full-app clip count profile"]
fn high_clip_count_full_app() {
    assert!(!cfg!(debug_assertions));
    for hits in [128, 512, 2048, 4096] {
        for kind in [
            "audio",
            "audio_long",
            "missing",
            "empty",
            "notes",
            "notes_long",
            "drums_steps",
            "drums_steps_long",
            "drums_notes",
            "drums_notes_long",
        ] {
            for view in ["overview", "close_end"] {
                for mode in ["idle", "scroll", "all_selected", "all_selected_indexed"] {
                    let fixture = crate::ui::clips::count_profile::fixture(kind, hits);
                    let wav = fixture.wav;
                    let mut rig = Rig::new();
                    rig.app
                        .audio
                        .samples
                        .load(&fixture.song, &|_| (kind != "missing").then(|| wav.clone()));
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
                        let refs = rig.app.song.channels[1].lanes[0]
                            .clips
                            .iter()
                            .map(|c| crate::ui::clips::ClipRef {
                                channel: 1,
                                lane: 0,
                                id: c.id,
                            })
                            .collect();
                        rig.app.main_screen.clips.set_selection(refs);
                    }
                    let before = rig.app.song.clone();
                    crate::ui::clips::reset_wave_mesh_cache();
                    let mut times = Vec::new();
                    let mut tess = Vec::new();
                    for frame in 0..36 {
                        if mode == "scroll" {
                            rig.app.main_screen.scroll_x += 0.01;
                        }
                        let start = Instant::now();
                        crate::ui::clips::with_profile_indexed_retention(
                            mode == "all_selected_indexed",
                            || {
                                rig.frame(vec![egui::Event::PointerMoved(pos2(
                                    500.0 + (frame % 8) as f32,
                                    145.0,
                                ))])
                            },
                        );
                        let ui = start.elapsed().as_secs_f64() * 1000.0;
                        let start = Instant::now();
                        black_box(rig.ctx.tessellate(rig.out.shapes.clone(), 1.0));
                        let t = start.elapsed().as_secs_f64() * 1000.0;
                        if frame >= 6 {
                            times.push(ui);
                            tess.push(t);
                        }
                    }
                    // View bookkeeping may change, but frames must not create or delete project clips.
                    assert_eq!(
                        rig.app.song.channels[1].lanes[0].clips.len(),
                        before.channels[1].lanes[0].clips.len()
                    );
                    times.sort_by(f64::total_cmp);
                    tess.sort_by(f64::total_cmp);
                    let wave_stats = crate::ui::clips::wave_mesh_profile_stats();
                    let (wave_cache_entries, wave_cache_bytes) =
                        crate::ui::clips::wave_mesh_cache_size();
                    println!(
                        "CLIP_PROFILE {}",
                        serde_json::json!({"phase":"full_app_cpu", "case":kind,"hits":hits,
                        "clips":before.channels[1].lanes[0].clips.len(),"view":view,"mode":mode,"frames":30,
                        "ui_p50_ms":times[15],"ui_p95_ms":times[28],"ui_p99_ms":times[29],
                        "tess_p50_ms":tess[15],"tess_p95_ms":tess[28],
                        "wave_cache_hits":wave_stats.hits,"wave_cache_misses":wave_stats.misses,
                        "wave_cache_inserts":wave_stats.inserts,"wave_cache_evictions":wave_stats.evictions,
                        "wave_cache_expired":wave_stats.expired,"wave_cache_bypasses":wave_stats.bypasses,
                        "wave_cache_entries":wave_cache_entries,"wave_cache_bytes":wave_cache_bytes,
                        "candidate_index":"only all_selected_indexed; index build included per frame",
                        "fake_audio_sink":true,"hardware_xruns":null,"gpu_fps":null})
                    );
                }
            }
        }
    }
}

#[test]
#[ignore = "release-only complete-frame linear vs retained-position paint lookup A/B"]
fn high_clip_count_paint_lookup_ab() {
    assert!(!cfg!(debug_assertions));
    let hits = 4096;
    for (view, sparse) in [("dense_overview", false), ("sparse_close_end", true)] {
        let fixture = crate::ui::clips::count_profile::fixture("empty", hits);
        let duration = hits as f64 * 0.25;
        let mut shape_counts = Vec::new();
        for (lookup, linear) in [("linear", true), ("retained_position", false)] {
            let mut rig = Rig::new();
            rig.app.song = fixture.song.clone();
            rig.app.selected_channel = 1;
            rig.pin_playlist();
            if sparse {
                rig.app.main_screen.zoom = 0.0;
                rig.app.main_screen.scroll_x = (duration - 16.0).max(0.0);
            } else {
                rig.app.main_screen.zoom = (960.0 / duration as f32 / 38.4).log2();
            }
            let mut full_frame_ms = Vec::new();
            let mut final_shape_count = 0;
            for frame in 0..36 {
                let started = Instant::now();
                crate::ui::clips::with_profile_linear_paint_lookup(linear, || {
                    rig.frame(vec![egui::Event::PointerMoved(pos2(
                        500.0 + (frame % 8) as f32,
                        145.0,
                    ))]);
                });
                let shapes = rig.out.shapes.clone();
                black_box(rig.ctx.tessellate(shapes.clone(), 1.0));
                let elapsed = started.elapsed().as_secs_f64() * 1000.0;
                if frame >= 6 {
                    full_frame_ms.push(elapsed);
                }
                final_shape_count = shapes.len();
            }
            full_frame_ms.sort_by(f64::total_cmp);
            let visible_clips = fixture.song.channels[1].lanes[0]
                .clips
                .iter()
                .filter(|clip| {
                    rig.ctx
                        .read_response(egui::Id::new(("clip", 1_usize, 0_usize, clip.id)))
                        .is_some()
                })
                .count();
            shape_counts.push(final_shape_count);
            println!(
                "CLIP_PROFILE {}",
                serde_json::json!({
                    "phase":"full_app_paint_lookup_ab", "view":view, "clips":hits,
                    "visible_clips":visible_clips,
                    "lookup":lookup, "frames":30, "full_frame_p50_ms":full_frame_ms[15],
                    "full_frame_p95_ms":full_frame_ms[28], "shape_count":final_shape_count,
                    "shape_count_only_equivalence":true, "fake_audio_sink":true,
                    "hardware_xruns":null, "gpu_fps":null
                })
            );
        }
        assert_eq!(
            shape_counts[0], shape_counts[1],
            "render shape counts differ in {view}"
        );
    }
}
