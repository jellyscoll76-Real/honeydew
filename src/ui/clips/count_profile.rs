//! Test-only attribution harness. Runs production helpers, not a GPU/device benchmark.
use super::*;
use banaforge_audio::device::AudioRenderer;
use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
    hint::black_box,
    time::Instant,
};

#[cfg(target_os = "linux")]
mod allocations {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;
    thread_local! {
        static COUNTS: Cell<Option<(usize, usize)>> = const { Cell::new(None) };
    }
    struct Counting;
    #[global_allocator]
    static ALLOCATOR: Counting = Counting;
    fn record(n: usize) {
        let _ = COUNTS.try_with(|c| {
            if let Some((calls, bytes)) = c.get() {
                c.set(Some((calls + 1, bytes + n)));
            }
        });
    }
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, l: Layout) -> *mut u8 {
            record(l.size());
            unsafe { System.alloc(l) }
        }
        unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
            record(l.size());
            unsafe { System.alloc_zeroed(l) }
        }
        unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
            unsafe { System.dealloc(p, l) }
        }
        unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
            record(n);
            unsafe { System.realloc(p, l, n) }
        }
    }
    pub fn start() {
        COUNTS.with(|c| c.set(Some((0, 0))));
    }
    pub fn end() -> (usize, usize) {
        COUNTS.with(|c| c.replace(None).unwrap())
    }
}

fn cpu_ms() -> f64 {
    let mut t = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // Process CPU time includes any workers; it is not Android utilization.
    unsafe {
        libc::clock_gettime(libc::CLOCK_PROCESS_CPUTIME_ID, &mut t);
    }
    t.tv_sec as f64 * 1000.0 + t.tv_nsec as f64 / 1e6
}
fn rss_kib() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .unwrap()
        .lines()
        .find(|l| l.starts_with("VmRSS:"))
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap()
}
fn quantile(v: &[f64], q: f64) -> f64 {
    v[((v.len() - 1) as f64 * q).round() as usize]
}

pub(crate) struct Fixture {
    pub(crate) song: Song,
    pub(crate) wav: Vec<u8>,
    logical_hits: usize,
    kind: &'static str,
}
pub(crate) fn fixture(kind: &'static str, hits: usize) -> Fixture {
    let mut song = Song::default();
    let channel_kind = match kind {
        "notes" | "notes_long" => ChannelKind::Instrument,
        "drums_steps" | "drums_notes" | "drums_steps_long" | "drums_notes_long" => {
            ChannelKind::Drums
        }
        _ => ChannelKind::AudioClip,
    };
    song.add_channel(channel_kind);
    let lane = &mut song.channels[1].lanes[0];
    if kind.starts_with("drums_notes") {
        lane.grid = DrumGrid::Notes;
    }
    let count = if kind.ends_with("long") { 16 } else { hits };
    let per_clip = hits / count;
    let pcm: Vec<f32> = (0..6000 * per_clip)
        .map(|i| {
            let x = (i % 6000) as f32;
            (x * 0.04).sin() * (-x / 500.0).exp() * 0.4
        })
        .collect();
    let wav = banaforge_audio::wav_writer::float32_wav(48000, &[pcm]);
    for i in 0..count {
        let c = lane.add_clip(
            (i * per_clip * PPQ as usize / 4) as u32,
            per_clip as f64 * 0.25,
        );
        match kind {
            "notes" | "notes_long" => {
                c.events = (0..per_clip)
                    .map(|j| Event::Note {
                        pos: (j * PPQ as usize / 4) as u32,
                        len_beats: 0.125,
                        key: 60 + ((i * per_clip + j) % 12) as u8,
                        velocity: 100,
                        pan: 64,
                        slide: false,
                    })
                    .collect();
            }
            "drums_steps" | "drums_notes" | "drums_steps_long" | "drums_notes_long" => {
                // The 16-long-clip control carries the same ordered drum hits as the dense short-clip case.
                c.events = (0..per_clip)
                    .map(|j| Event::Drum {
                        pos: (j * PPQ as usize / 4) as u32,
                        len_beats: 0.125,
                        pad: ((i * per_clip + j) % 4) as u8,
                        velocity: 100,
                        pan: 64,
                        value: 0,
                        repeat: 0,
                    })
                    .collect();
            }
            "empty" => {}
            _ => {
                let mut slot = crate::song::SamplerSlot::default();
                slot.file.path = Some(crate::flm::chunk::ContentPath::Shorthand {
                    root: crate::flm::chunk::ContentRoot::User,
                    relative: "My Samples/profile.wav".into(),
                });
                c.file.sample = Some(Box::new(slot));
                c.file.source_len_beats = per_clip as f64 * 0.25;
            }
        }
    }
    Fixture {
        song,
        wav,
        logical_hits: hits,
        kind,
    }
}

#[test]
#[ignore = "release-only controlled clip count attribution; no APK/device/GPU"]
fn high_clip_count_profile() {
    assert!(!cfg!(debug_assertions), "profile optimized builds only");
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
            let f = fixture(kind, hits);
            let reads = Cell::new(0usize);
            let reader = |_: &crate::flm::chunk::ContentPath| {
                reads.set(reads.get() + 1);
                (kind != "missing").then(|| f.wav.clone())
            };
            let mut cache = SampleCache::default();
            let start = Instant::now();
            cache.load(&f.song, &reader);
            // Include lazy waveform summary in the separately reported cold preparation, not warm frames.
            for c in &f.song.channels[1].lanes[0].clips {
                if let Some(slot) = &c.file.sample {
                    black_box(cache.waveform(slot, &reader));
                }
            }
            println!(
                "CLIP_PROFILE {}",
                serde_json::json!({"phase":"cold_assets", "case":kind,
                "hits":hits,"clips":f.song.channels[1].lanes[0].clips.len(),"ms":start.elapsed().as_secs_f64()*1000.0,
                "reader_calls":reads.get(),"rss_kib":rss_kib()})
            );
            let files = ProjectFiles::with_provider(Box::new(
                crate::project_files::mem::MemFiles::default(),
            ));
            for view in ["overview", "close_start", "close_end", "offscreen"] {
                for motion in ["idle", "scroll"] {
                    for repeat in 0..3 {
                        profile_ui(&f, &mut cache, &files, view, motion, repeat);
                    }
                }
            }
            // Real offline engine, no sound device. Audio sources A/B are the same synthesized pulses.
            // Instrument/drum fixtures are UI controls, not equivalent acoustic fixtures.
            if matches!(kind, "audio" | "audio_long" | "missing" | "empty") {
                let start = Instant::now();
                let mut engine =
                    crate::audio_engine::SongAudioEngine::with_samples(&f.song, 48000.0, &cache)
                        .unwrap();
                let prepare = start.elapsed().as_secs_f64() * 1000.0;
                engine.play();
                let mut output = [0.0_f32; 512];
                let mut times = Vec::new();
                let mut energy = 0.0_f64;
                let cpu = cpu_ms();
                for _ in 0..400 {
                    let start = Instant::now();
                    engine.render(&mut output, 2);
                    times.push(start.elapsed().as_secs_f64() * 1000.0);
                    energy += output.iter().map(|&v| f64::from(v).powi(2)).sum::<f64>();
                }
                let cpu = cpu_ms() - cpu;
                if kind.starts_with("audio") {
                    assert!(energy > 0.0, "audible fixture must render nonzero PCM");
                }
                times.sort_by(f64::total_cmp);
                println!(
                    "CLIP_PROFILE {}",
                    serde_json::json!({"phase":"offline_audio", "case":kind,"hits":hits,
                    "clips":f.song.channels[1].lanes[0].clips.len(),"prepare_ms":prepare,
                    "block_frames":256,"deadline_ms":256.0/48.0,"p50_ms":quantile(&times,0.5),
                    "p95_ms":quantile(&times,0.95),"p99_ms":quantile(&times,0.99),
                    "over_budget_blocks":times.iter().filter(|&&t| t>256.0/48.0).count(),
                    "cpu_ms":cpu,"energy":energy,"hardware_xruns":null,"rss_kib":rss_kib()})
                );
            }
        }
    }
}

#[test]
#[ignore = "test-only ID index build and mutation maintenance costs; Linux/release"]
fn high_clip_count_index_maintenance() {
    assert!(!cfg!(debug_assertions));
    let fixture = fixture("empty", 4096);
    let mut build_ms = Vec::new();
    let mut add_ms = Vec::new();
    let mut copy_ms = Vec::new();
    let mut delete_ms = Vec::new();
    let mut same_lane_move_ms = Vec::new();
    let mut cross_lane_move_ms = Vec::new();
    let mut load_index_ms = Vec::new();
    for _ in 0..9 {
        let lane = &fixture.song.channels[1].lanes[0];
        let start = Instant::now();
        let index: HashMap<u32, usize> = lane
            .clips
            .iter()
            .enumerate()
            .map(|(i, c)| (c.id, i))
            .collect();
        build_ms.push(start.elapsed().as_secs_f64() * 1000.0);
        black_box(index);

        let mut lane = fixture.song.channels[1].lanes[0].clone();
        let mut index: HashMap<u32, usize> = lane
            .clips
            .iter()
            .enumerate()
            .map(|(i, c)| (c.id, i))
            .collect();
        let start = Instant::now();
        let id = add_clip(&mut lane, 99_000_000.0, 16.0, 4);
        index.insert(id, lane.clips.len() - 1);
        add_ms.push(start.elapsed().as_secs_f64() * 1000.0);
        black_box(index);

        let mut lane = fixture.song.channels[1].lanes[0].clone();
        let mut index: HashMap<u32, usize> = lane
            .clips
            .iter()
            .enumerate()
            .map(|(i, c)| (c.id, i))
            .collect();
        let source_id = lane.clips[100].id;
        let start = Instant::now();
        let id = copy_clip(&mut lane, source_id, 16.0).unwrap();
        index.insert(id, lane.clips.len() - 1);
        copy_ms.push(start.elapsed().as_secs_f64() * 1000.0);
        black_box(index);

        let mut lane = fixture.song.channels[1].lanes[0].clone();
        let mut index: HashMap<u32, usize> = lane
            .clips
            .iter()
            .enumerate()
            .map(|(i, c)| (c.id, i))
            .collect();
        let removed = 2048usize;
        let id = lane.clips[removed].id;
        let start = Instant::now();
        assert!(delete_clip(&mut lane, id));
        index.remove(&id);
        for position in index.values_mut() {
            if *position > removed {
                *position -= 1;
            }
        }
        delete_ms.push(start.elapsed().as_secs_f64() * 1000.0);
        black_box(index);

        let mut lane = fixture.song.channels[1].lanes[0].clone();
        let id = lane.clips[100].id;
        let index: HashMap<u32, usize> = lane
            .clips
            .iter()
            .enumerate()
            .map(|(i, c)| (c.id, i))
            .collect();
        let start = Instant::now();
        lane.clips[100].start_tick += 16;
        black_box(index.get(&id));
        same_lane_move_ms.push(start.elapsed().as_secs_f64() * 1000.0);

        let mut song = fixture.song.clone();
        song.add_channel(crate::song::ChannelKind::AudioClip);
        for i in 0..32 {
            song.channels[2].lanes[0].add_clip(i * 16, 1.0);
        }
        let source = &song.channels[1].lanes[0];
        let moving_id = source.clips[21].id;
        let from_index = 21usize;
        let mut from: HashMap<u32, usize> = source
            .clips
            .iter()
            .enumerate()
            .map(|(i, c)| (c.id, i))
            .collect();
        let mut to: HashMap<u32, usize> = song.channels[2].lanes[0]
            .clips
            .iter()
            .enumerate()
            .map(|(i, c)| (c.id, i))
            .collect();
        let source_ref = ClipRef {
            channel: 1,
            lane: 0,
            id: moving_id,
        };
        let start = Instant::now();
        let moved = move_clips(&mut song, &[(source_ref, (2, 0))])[0];
        from.remove(&moving_id);
        for position in from.values_mut() {
            if *position > from_index {
                *position -= 1;
            }
        }
        to.insert(moved.id, song.channels[2].lanes[0].clips.len() - 1);
        cross_lane_move_ms.push(start.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(moved.channel, 2);
        black_box((from, to));

        // Loading/replacing a project invalidates the old map; this measures rebuilding from the loaded song only,
        // not file parsing or asset decoding.
        let loaded = fixture.song.clone();
        let start = Instant::now();
        let loaded_index: HashMap<u32, usize> = loaded.channels[1].lanes[0]
            .clips
            .iter()
            .enumerate()
            .map(|(i, c)| (c.id, i))
            .collect();
        load_index_ms.push(start.elapsed().as_secs_f64() * 1000.0);
        black_box(loaded_index);
    }
    let median = |v: &mut Vec<f64>| {
        v.sort_by(f64::total_cmp);
        quantile(v, 0.5)
    };
    println!(
        "CLIP_PROFILE {}",
        serde_json::json!({
            "phase":"index_maintenance", "clips":4096, "repeats":9,
            "build_ms":median(&mut build_ms), "load_rebuild_ms_excluding_parse_and_assets":median(&mut load_index_ms),
            "add_plus_insert_ms":median(&mut add_ms), "copy_plus_insert_ms":median(&mut copy_ms),
            "delete_plus_stable_order_reindex_ms":median(&mut delete_ms),
            "same_lane_start_move_index_work_ms":median(&mut same_lane_move_ms),
            "cross_lane_move_plus_two_map_updates_ms":median(&mut cross_lane_move_ms),
            "cross_lane_target_id_collision":true,
            "persistent_index_correctness_tested":false
        })
    );
}

fn profile_ui(
    f: &Fixture,
    cache: &mut SampleCache,
    files: &ProjectFiles,
    view: &str,
    motion: &str,
    repeat: usize,
) {
    let ctx = egui::Context::default();
    let indexed_ctx = egui::Context::default();
    let theme = crate::theme::theme_by_id(0);
    let m = crate::ui::offscreen::metrics();
    let cx = UiCtx {
        theme: &theme,
        m: &m,
    };
    let grid = Rect::from_min_size(pos2(0.0, 0.0), vec2(1024.0, 96.0));
    let lane = &f.song.channels[1].lanes[0];
    let n = lane.clips.len();
    let total_beats = f.logical_hits as f32 * 0.25;
    let beat_w = if view == "overview" {
        1024.0 / total_beats
    } else {
        64.0
    };
    let base = match view {
        "close_end" => -(total_beats - 16.0) * beat_w,
        "offscreen" => -(total_beats + 32.0) * beat_w,
        _ => 0.0,
    };
    let refs: Vec<_> = lane
        .clips
        .iter()
        .map(|c| ClipRef {
            channel: 1,
            lane: 0,
            id: c.id,
        })
        .collect();
    let mut totals: Vec<f64> = Vec::new();
    let mut sums = [0.0_f64; 9];
    let mut indexed_paint_totals = Vec::new();
    let mut indexed_alloc_calls = 0usize;
    let mut indexed_alloc_bytes = 0usize;
    let mut indexed_capacity = 0usize;
    let mut calls = 0usize;
    let mut bytes = 0usize;
    let mut visible = 0;
    let mut shapes = 0;
    let mut vertices = 0;
    let mut cpu = 0.0;
    // Isolated production helpers from Clips::show: no copy/paste/drag mutations in this attribution pass.
    for frame in 0..28 {
        let axis = TimeAxis {
            origin: base
                - if motion == "scroll" {
                    (frame % 12) as f32 * 0.7
                } else {
                    0.0
                },
            beat_w,
        };
        let measured = frame >= 4;
        if measured {
            allocations::start();
        }
        let cpu_start = cpu_ms();
        let total = Instant::now();
        let start = Instant::now();
        let mut blocks: Vec<_> = lane
            .clips
            .iter()
            .map(|c| {
                (
                    ClipRef {
                        channel: 1,
                        lane: 0,
                        id: c.id,
                    },
                    clip_rect(c, grid, axis),
                )
            })
            .filter(|(_, r)| r.intersects(grid))
            .collect();
        blocks.sort_by_key(|&(r, _)| Some(r) == refs.last().copied());
        let layout = start.elapsed().as_secs_f64() * 1000.0;
        visible = blocks.len();
        let start = Instant::now();
        for &(r, _) in &blocks {
            black_box(find(&f.song, r));
        }
        let lookup = start.elapsed().as_secs_f64() * 1000.0;
        let start = Instant::now();
        // Worst selection retention: production group.retain's kept()/find, all clips selected.
        for &r in &refs {
            black_box(find(&f.song, r));
        }
        let selected = start.elapsed().as_secs_f64() * 1000.0;
        let start = Instant::now();
        // Candidate only: a test-local hash index compared against production linear find; production is untouched.
        let indexed: HashSet<_> = lane
            .clips
            .iter()
            .map(|clip| (1usize, 0usize, clip.id))
            .collect();
        let index_build = start.elapsed().as_secs_f64() * 1000.0;
        let start = Instant::now();
        for &(r, _) in &blocks {
            black_box(indexed.contains(&(r.channel, r.lane, r.id)));
        }
        let indexed_lookup = start.elapsed().as_secs_f64() * 1000.0;
        let start = Instant::now();
        for &r in &refs {
            black_box(indexed.contains(&(r.channel, r.lane, r.id)));
        }
        let indexed_selected = start.elapsed().as_secs_f64() * 1000.0;
        let start = Instant::now();
        for &(_, r) in &blocks {
            black_box(r.contains(pos2(512.0 + (frame % 4) as f32, 48.0)));
        }
        let hit = start.elapsed().as_secs_f64() * 1000.0;
        let start = Instant::now();
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(grid),
                time: Some(frame as f64 / 60.0),
                ..Default::default()
            },
            |ctx| {
                let painter = ctx
                    .layer_painter(egui::LayerId::background())
                    .with_clip_rect(grid);
                for &(r, rect) in &blocks {
                    let c = find(&f.song, r).unwrap();
                    let wave = audio_wave(cache, files, c, f.song.tempo_bpm);
                    paint_clip(
                        &painter,
                        &cx,
                        &f.song.channels[1],
                        lane,
                        c,
                        wave.as_ref().map(|(s, w)| (s, *w)),
                        Look::default(),
                        clip_box(rect, &m),
                        axis,
                    );
                }
            },
        );
        let paint = start.elapsed().as_secs_f64() * 1000.0;
        shapes = output.shapes.len();
        let start = Instant::now();
        let primitives = ctx.tessellate(output.shapes, 1.0);
        vertices = primitives
            .iter()
            .map(|p| match &p.primitive {
                egui::epaint::Primitive::Mesh(m) => m.vertices.len(),
                _ => 0,
            })
            .sum();
        black_box(primitives);
        let tess = start.elapsed().as_secs_f64() * 1000.0;
        let baseline_total_ms = total.elapsed().as_secs_f64() * 1000.0;

        if measured {
            let a = allocations::end();
            calls += a.0;
            bytes += a.1;
            cpu += cpu_ms() - cpu_start;
        }
        // Test-only frame-local per-lane ID->position index, covering visible resolution, paint and tessellation.
        // Build and map allocation are included; production continues to use find(song, ClipRef).
        let indexed_total = Instant::now();
        if measured {
            allocations::start();
        }
        let mut indexed = HashMap::with_capacity(lane.clips.len());
        for (position, clip) in lane.clips.iter().enumerate() {
            indexed.insert(clip.id, position);
        }
        let capacity = indexed.capacity();
        if measured {
            let a = allocations::end();
            indexed_alloc_calls += a.0;
            indexed_alloc_bytes += a.1;
            indexed_capacity = capacity;
        }
        let indexed_output = indexed_ctx.run(
            egui::RawInput {
                screen_rect: Some(grid),
                time: Some(frame as f64 / 60.0),
                ..Default::default()
            },
            |ctx| {
                let painter = ctx
                    .layer_painter(egui::LayerId::background())
                    .with_clip_rect(grid);
                for &(r, rect) in &blocks {
                    let clip = lane
                        .clips
                        .get(*indexed.get(&r.id).expect("indexed visible clip"))
                        .filter(|clip| clip.id == r.id)
                        .expect("index resolves requested clip ID");
                    let wave = audio_wave(cache, files, clip, f.song.tempo_bpm);
                    paint_clip(
                        &painter,
                        &cx,
                        &f.song.channels[1],
                        lane,
                        clip,
                        wave.as_ref().map(|(s, w)| (s, *w)),
                        Look::default(),
                        clip_box(rect, &m),
                        axis,
                    );
                }
            },
        );
        let indexed_shapes = indexed_output.shapes.len();
        let indexed_primitives = indexed_ctx.tessellate(indexed_output.shapes, 1.0);
        let indexed_vertices: usize = indexed_primitives
            .iter()
            .map(|p| match &p.primitive {
                egui::epaint::Primitive::Mesh(mesh) => mesh.vertices.len(),
                _ => 0,
            })
            .sum();
        black_box(indexed_primitives);
        assert_eq!(
            (indexed_shapes, indexed_vertices),
            (shapes, vertices),
            "indexed lookup must paint same shape/vertex counts"
        );
        if measured {
            indexed_paint_totals.push(indexed_total.elapsed().as_secs_f64() * 1000.0);
            totals.push(baseline_total_ms);
            for (s, v) in sums.iter_mut().zip([
                layout,
                lookup,
                selected,
                index_build,
                indexed_lookup,
                indexed_selected,
                hit,
                paint,
                tess,
            ]) {
                *s += v;
            }
        }
    }
    totals.sort_by(f64::total_cmp);
    indexed_paint_totals.sort_by(f64::total_cmp);
    println!(
        "CLIP_PROFILE {}",
        serde_json::json!({"phase":"ui_attribution","case":f.kind,"hits":f.logical_hits,
        "clips":n,"view":view,"motion":motion,"repeat":repeat,"frames":24,"visible":visible,
        "p50_ms":quantile(&totals,0.5),"p95_ms":quantile(&totals,0.95),"p99_ms":quantile(&totals,0.99),
        "layout_ms":sums[0]/24.0,"lookup_ms":sums[1]/24.0,"all_selected_lookup_ms":sums[2]/24.0,
        "index_build_ms":sums[3]/24.0,"indexed_lookup_ms":sums[4]/24.0,"all_selected_indexed_ms":sums[5]/24.0,
        "hit_rect_scan_ms":sums[6]/24.0,"paint_ms":sums[7]/24.0,"tessellate_ms":sums[8]/24.0,
        "indexed_paint_tess_p50_ms":quantile(&indexed_paint_totals,0.5),
        "indexed_paint_tess_p95_ms":quantile(&indexed_paint_totals,0.95),
        "indexed_map_capacity":indexed_capacity,
        "indexed_map_alloc_calls_per_frame":indexed_alloc_calls/24,
        "indexed_map_requested_alloc_bytes_per_frame":indexed_alloc_bytes/24,
        "alloc_calls_per_frame":calls/24,"requested_alloc_bytes_per_frame":bytes/24,
        "shapes":shapes,"vertices":vertices,"cpu_ms_per_frame":cpu/24.0,"rss_kib":rss_kib(),
        "gpu_fps":null,"hardware_xruns":null,"includes_full_app":false})
    );
}
