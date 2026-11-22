# Large-project performance investigation

**Investigation status: controlled desktop/CI profiling completed; interaction/device coverage is partial. No production optimization was made.** This is a UI/performance investigation only; it does not change audio behavior. Dataset from CI release profile run 45066092379 (commit d03b42e), decoded to `.cache/clip-count-profile/summary.json` and `resources.txt`. A test-only comparison of a candidate hash index is being added; it is not wired into production.

## Harness, controls and limits

- 128 / 512 / 2048 / 4096 logical hits. Short-clip fixture uses one clip per hit; long fixture carries the same periodic pulse sequence in 16 longer clips. Audio content is repeated samples of the same 48 kHz synthetic pulse sequence, so it compares short vs long clip organization while holding total musical content approximately constant.
- UI fixtures: audio, missing audio, empty clips, Piano Roll notes, drums step and drums note cases. Notes short/long contain the same number of notes; drum fixtures use one hit per short pattern, but the 16-pattern long control does **not** hold drum-hit count constant. Drum conclusions are therefore limited to per-clip UI scaling, not an equivalent rhythmic-sequence A/B.
- Two view scales (overview spans the arrangement; close-end keeps about 65 of 4096 clips visible), idle and simulated scroll-axis movement, plus a worst-case all-selected mode. UI helpers measure layout, per-visible ID lookup, selection identity retention, a rectangle `contains` scan, paint, tessellation and allocation requests.
- Full-app control runs complete headless app frames at both view scales, with fake audio sink and an injected moving pointer. Scroll changes the playlist axis. The all-selected mode sets the selection, but is not a gesture sequence. The hit-test measurement is a small rectangle probe, not a tap/marquee benchmark.
- Offline engine renders 400 blocks × 256 stereo frames at 48 kHz without a device. This is independent of editor visibility and has no real-time audio callback, concurrent UI, Android scheduler, or hardware xrun measurement.
- No actual move/resize/edit gesture benchmark, touch-device latency, GPU rasterization/frame pacing, long playback session, or phone memory/underrun test was performed. These remain open before a device-level performance claim.

## Findings and measurements

### Primary performance scaling: visible short clips and repeated linear lookup

The clearest controlled result is **clips versus content**. With all clips visible, headless full-app UI p50 grows from roughly 0.2 ms at 128 short clips to 0.3–0.6 ms at 512, about 2.2–2.8 ms at 2048 and about 7.5–8.7 ms at 4096 for empty, missing, notes and drums fixtures. Audio has a second waveform-cache cost (below). With 4096 total clips but only ~65 visible at close-end zoom, idle full-app UI p50 is just 0.40–0.72 ms. 16 long audio clips carrying the equivalent pulse content cost ~0.08 ms at 4096 logical hits; 16 long note patterns cost ~0.40 ms (more notes still need rendering). That supports the observation that a large number of individual clips/patterns is expensive; visibility culling helps substantially.

Full-app frame timings (UI p50 / UI p95 / tessellation p50, milliseconds; Linux release/headless, no GPU):

| Fixture | 512 short clips, overview idle | 2048 short clips, overview idle | 4096 short clips, overview idle | 4096 all selected | 4096 close-end idle |
|---|---:|---:|---:|---:|---:|
| Audio (waveform) | 4.25 / 4.34 / 0.43 | 2.77 / 2.94 / 1.95 | 8.51 / 8.79 / 2.34 | 18.77 / 19.09 / 2.68 | 0.72 / 0.82 / 0.26 |
| Missing audio | 0.45 / 0.54 / 0.24 | 2.80 / 2.93 / 1.93 | 8.72 / 8.88 / 2.37 | 18.98 / 19.44 / 2.55 | 0.44 / 0.56 / 0.17 |
| Empty clips | 0.33 / 0.42 / 0.24 | 2.23 / 2.39 / 1.90 | 7.48 / 7.67 / 2.46 | 17.27 / 17.69 / 2.58 | 0.41 / 0.51 / 0.17 |
| Piano Roll notes | 0.42 / 0.51 / 0.32 | 2.33 / 2.44 / 1.95 | 7.67 / 7.83 / 2.63 | 17.45 / 17.70 / 2.42 | 0.42 / 0.52 / 0.18 |
| Drum steps | 0.42 / 0.51 / 0.32 | 2.23 / 2.29 / 1.98 | 7.71 / 7.91 / 2.51 | 17.25 / 17.64 / 2.41 | 0.40 / 0.48 / 0.18 |
| Audio long, 16 clips | 0.08 / 0.14 / 0.12 | 0.08 / 0.16 / 0.11 | 0.08 / 0.17 / 0.11 | 0.08 / 0.18 / 0.11 | 0.05 / 0.15 / 0.11 |
| Notes long, 16 patterns | 0.12 / 0.19 / 0.09 | 0.24 / 0.33 / 0.29 | 0.40 / 0.52 / 0.67 | 0.40 / 0.50 / 0.67 | 0.11 / 0.15 / 0.17 |

The 512-clip audio row is non-monotonic: it is a waveform-mesh-cache miss/thrash regime, not evidence 2048 clips are intrinsically cheaper. Different fixture construction/zoom/LOD changes the apparent point cost. Broadly, both visible clip count and clip-content complexity affect time; short notes cost more than empty clip records.

**Repeated linear lookup is a confirmed algorithmic hot spot in the isolated production-helper profile.** `find(song, ClipRef)` scans a lane's clip vector (`src/ui/clips.rs` near `find`); the visible clip pass resolves each visible ref and selection retention calls `find` for each selected/group ref each frame. With every clip visible/selected in the one-lane fixture, the diagnostic lookup and selection probes are each about 0.004 ms (128), 0.078 ms (512), 1.16–1.18 ms (2048), 5.02–5.15 ms (4096). This is consistent with O(N²) total comparisons (N linear searches of N clips). It is significant but is **not the whole frame cost**: paint and tessellation remain material.

At 4096 visible clips, isolated helper medians are ~0.05 ms layout, ~5.0 ms lookup, ~5.0 ms all-selected retention probe, ~5.7–6.8 ms paint, and ~2.1–2.4 ms tessellation (depends on fixture). The full-app all-selected case grows from ~7.5–8.7 ms idle to ~17.3–19.0 ms p50, supporting selection-retention overhead. The layout pass itself is small; offscreen paint culling is effective; hit `Rect::contains` scanning was negligible in its synthetic probe. **Hit-test performance in actual pointer/touch selection remains unmeasured.**

### Waveforms and allocations

At 512 visible audio clips, isolated audio paint is 3.78 ms, versus ~0.18–0.27 ms for empty/missing/notes. Source inspection found `stereo_wave_mesh` using linear search in an LRU capped at 256 entries / 4 MiB. Distinct geometries beyond that limit can repeatedly evict/rebuild. This is strong evidence for cache churn causing the 512-clip audio spike. The exact zoom/mesh LOD interaction causing the subsequent 2048 dip is not isolated. Cache bytes are explicitly bounded, so cache growth has memory consequences.

At 4096 visible clips, the instrumentation requested ~6.9 MB of allocations per diagnostic audio frame, ~7.9 MB for missing audio and ~6.9 MB for empty/notes, across ~16k–37k allocation/reallocation calls. These are **requested allocation bytes, not live/retained bytes**. Whole profile run peak RSS was 102,888 KiB and process CPU 67.17 seconds, but RSS is process-wide and cannot be assigned to a fixture or candidate renderer. It is not phone memory usage.

### UI versus offline audio engine

The CPU profile separates the UI from the synthetic engine render. At 4096 clips, 400 rendered blocks (each 256 stereo frames, 48 kHz) measured:

| Engine fixture | Prepare time | Block p50 | Block p95 | Block p99 | Blocks > 5.33 ms |
|---|---:|---:|---:|---:|---:|
| Audio clips | 14.712 ms | 0.235 ms | 0.261 ms | 0.480 ms | 0 |
| Missing audio | 16.800 ms | 0.083 ms | 0.108 ms | 0.168 ms | 0 |
| Empty clips | 1.073 ms | 0.0007 ms | 0.0007 ms | 0.0007 ms | 0 |
| Audio long, 16 clips | 0.086 ms | 0.0032 ms | 0.0034 ms | 0.0038 ms | 0 |

Engine slot preparation and per-block scanning scale with clip slots; on this desktop test they are far below the 5.33 ms block budget. This is **not** evidence of phone headroom or absence of device underruns. It does show the measured high-count lag is predominantly UI/frame work, not a synthetic render missing its budget. Do not disable audio instruments/effects when editors are inactive; audio-engine processing is independent of the visible view.

Cold file reads are path-deduplicated to one reader call per audio/missing fixture, irrespective of clip count. Cold audio at 4096 took ~1.7 ms; drum kit cold time ~8.6 ms was a one-off fixture load. Asset loading does not explain steady per-frame lag.

## Proposed directions compared (not implemented)

1. **Build an identity index for clip refs** (candidate hash/set or per-lane ID-to-index map). Likely attacks O(N²) ref resolution and selection retention while preserving clip records and edit semantics. A hash structure adds build/allocation cost and memory: rough order for 4096 refs is 0.1–0.3 MiB depending on table capacity/layout; a full-song index over many lanes costs more. Benchmark candidate build + lookup against linear path first; consider only building when there is a non-empty selection. A dense per-lane ID array may use less memory if IDs are bounded/dense, but IDs can be replaced/renumbered on cross-lane moves, so invalidation/lifetime must be correct. A test-only candidate timing is being added; production remains unchanged.
2. **Waveform mesh caching**: profile LRU key lookup and hit rate; a hash map can reduce linear cache search, while retaining a strict byte/entry cap. Raising the cap could reduce rebuilds but increases retained memory; never assume larger is better. Cache only immutable waveform geometry and keep overlays (selection, playhead, handles) live.
3. **Visible-content work**: visibility culling already prevents most offscreen paint, but current layout and interaction enumeration still scan lane clips. Interval/bucket indexing might reduce work for large sparse lanes, at cost of index maintenance on move/resize/undo/import. Actual pointer hit-testing and edit gesture benchmarks are needed before this direction.
4. **Cached visuals / tiles (user proposal)**: feasible as a display-only layer while project clips/notes remain authoritative and individually editable. Use bounded, zoom/DPI/theme/sample-version keyed tiles; draw selected/hovered clips, handles, playhead and drag previews as live overlay; retain geometry/ref-to-project mapping for hit tests and normal clip-level editing. Invalidation must cover move/resize/delete, waveform/sample updates, lane height/order, zoom, ppp, color/theme and text. A single 1024×256 RGBA tile is 1 MiB at 1× and 4 MiB at 2×; dozens of retained tiles quickly consume 10s–100s of MiB. Use a small LRU or vector/mesh cache rather than caching the whole song. Raster zoom can blur; vector tiles preserve quality but cost more tessellation/memory. This is a broader prototype, not the first fix.
5. **On-demand inactive editors**: current app frame code draws the current playlist/mixer and calls `slots.view` for the active bottom editor; it does not draw every editor view every frame. UI state/resources can remain retained without rendering. More lazy construction or releasing editor-only caches may trim memory but is unlikely to address the measured playlist hot path and risks losing scroll/selection state or causing reload hitches. Audio instruments/effects must remain active regardless of editor visibility.
6. **Audio engine inactive-slot optimization** is lower priority: small desktop cost, behavior-sensitive. Any change needs offline equivalence tests and device underrun/timing tests; it is not justified before UI work is measured.

## Recommended order / open measurements

1. Keep production unchanged while the test-only candidate identity index is measured at all hit counts and all-selected cases. Compare build + lookup total, allocation and full-app frames; if benefit is small, drop it.
2. Instrument waveform-cache hit/miss/eviction/rebuild and test hash lookup under the same 128–4096/zoom workload. Preserve the 4 MiB bound for the first prototype and compare image/shape equality.
3. Add genuine tap/select/marquee/move/resize performance actions, plus denser drum patterns with equivalent event content. Measure scrolling independently from pointer movement and zoom at multiple fixed viewport sizes.
4. Only then prototype bounded visual tiles if paint/tessellation remains the dominant cost. Keep project/edit/audio data fully independent.
5. Run exact Android device tests with screen/frame timing, memory pressure and audio underrun observation during playback + scrolling/editing. None of the desktop CPU results establish this.

No APK was built or distributed. The repo's `android/RELEASE_HOLD.md` remains in force.
