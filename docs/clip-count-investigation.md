# Large-project performance investigation

**Investigation status: controlled desktop/CI profiling completed; interaction/device coverage is partial. No production optimization remains.** This is a UI/performance investigation only; it does not change audio behavior. Baseline complete dataset: CI release profile run 45066092379 (commit d03b42e), `.cache/clip-count-profile/summary.json`. A second run on commit 6578d98 (45069564983) also compared a candidate hash index entirely inside the test harness; no production indexing was used. The new summary is `.cache/clip-count-profile/summary-testindex.json`.

## Harness, controls and limits

- 128 / 512 / 2048 / 4096 logical hits. Short-clip fixture uses one clip per hit; long fixture carries the same periodic pulse sequence in 16 longer clips. Audio content is repeated samples of the same 48 kHz synthetic pulse sequence, so it compares short vs long clip organization while holding total musical content approximately constant.
- UI fixtures: audio, missing audio, empty clips, Piano Roll notes, drums step and drums note cases. Notes short/long contain the same number of notes; drum fixtures use one hit per short pattern, but the 16-pattern long control does **not** hold drum-hit count constant. Drum conclusions are therefore limited to per-clip UI scaling, not an equivalent rhythmic-sequence A/B.
- Two view scales (overview spans the arrangement; close-end keeps about 65 of 4096 clips visible), idle and simulated scroll-axis movement, plus a worst-case all-selected mode. UI helpers measure layout, per-visible ID lookup, selection identity retention, a rectangle `contains` scan, paint, tessellation and allocation requests.
- Full-app control runs complete headless app frames at both view scales, with fake audio sink and an injected moving pointer. Scroll changes the playlist axis. The all-selected mode sets the selection, but is not a gesture sequence. The hit-test measurement is a small rectangle probe, not a tap/marquee benchmark.
- Offline engine renders 400 blocks × 256 stereo frames at 48 kHz without a device. This is independent of editor visibility and has no real-time audio callback, concurrent UI, Android scheduler, or hardware xrun measurement.
- No actual move/resize/edit gesture benchmark, touch-device latency, GPU rasterization/frame pacing, long playback session, or phone memory/underrun test was performed. These remain open before a device-level performance claim.

## Findings and measurements

### Primary performance scaling: visible short clips and repeated linear lookup

The clearest controlled result is **clips versus content**. With all clips visible, headless full-app UI p50 grows from roughly 0.2 ms at 128 short clips to 0.3–0.6 ms at 512, about 2.8–3.2 ms at 2048 and about 9.2–10.2 ms at 4096 for empty, missing, notes and drums fixtures in the latest run. Audio has a second waveform-cache cost (below). With 4096 total clips but only ~65 visible at close-end zoom, idle full-app UI p50 is just 0.40–0.72 ms. 16 long audio clips carrying the equivalent pulse content cost ~0.08 ms at 4096 logical hits; 16 long note patterns cost ~0.40 ms (more notes still need rendering). That supports the observation that a large number of individual clips/patterns is expensive; visibility culling helps substantially.

Full-app frame timings (UI p50 / UI p95 / tessellation p50, milliseconds; Linux release/headless, no GPU):

| Fixture | 512 short clips, overview idle | 2048 short clips, overview idle | 4096 short clips, overview idle | 4096 all selected | 4096 close-end idle |
|---|---:|---:|---:|---:|---:|
| Audio (waveform) | 3.79 / 3.88 / 0.45 | 3.05 / 3.14 / 1.61 | 9.97 / 10.09 / 2.31 | 24.12 / 24.75 / 2.53 | 0.59 / 0.64 / 0.20 |
| Missing audio | 0.56 / 0.62 / 0.37 | 3.22 / 3.35 / 1.18 | 10.23 / 10.35 / 2.32 | 24.80 / 25.05 / 2.50 | 0.57 / 0.61 / 0.19 |
| Empty clips | 0.43 / 0.49 / 0.38 | 2.77 / 2.84 / 1.19 | 9.33 / 9.44 / 2.35 | 23.63 / 23.88 / 2.49 | 0.56 / 0.62 / 0.20 |
| Piano Roll notes | 0.54 / 0.59 / 0.49 | 2.84 / 2.97 / 1.19 | 9.38 / 9.48 / 2.34 | 23.49 / 23.59 / 2.46 | 0.56 / 0.62 / 0.21 |
| Drum steps | 0.55 / 0.60 / 0.48 | 2.79 / 2.88 / 1.17 | 9.24 / 9.34 / 2.28 | 23.41 / 23.55 / 2.37 | 0.53 / 0.58 / 0.20 |
| Audio long, 16 clips | 0.10 / 0.13 / 0.13 | 0.10 / 0.13 / 0.13 | 0.10 / 0.16 / 0.13 | 0.10 / 0.14 / 0.13 | 0.06 / 0.09 / 0.11 |
| Notes long, 16 patterns | 0.16 / 0.18 / 0.20 | 0.32 / 0.37 / 0.44 | 0.53 / 0.61 / 0.77 | 0.53 / 0.60 / 0.76 | 0.06 / 0.12 / 0.12 |

The 512-clip audio row is non-monotonic: it is a waveform-mesh-cache miss/thrash regime, not evidence 2048 clips are intrinsically cheaper. Different fixture construction/zoom/LOD changes the apparent point cost. Broadly, both visible clip count and clip-content complexity affect time; short notes cost more than empty clip records. In the same latest run, overview scrolling at 4096 clips was about the same as idle (~9.2–10.3 ms UI p50); at close-end zoom with ~65 visible, idle was ~0.53–0.59 ms for notes/drums/missing/empty (audio ~0.59 ms), with simulated scroll ~0.53–1.03 ms. This is axis/pointer simulation, not a full finger-driven scroll benchmark. All-selected close-end still cost ~14.7–14.8 ms p50 with 4096 total clips: selection retention visits the selected refs even when most are offscreen.

**Repeated linear lookup is a confirmed algorithmic hot spot in the isolated production-helper profile.** `find(song, ClipRef)` scans a lane's clip vector (`src/ui/clips.rs` near `find`); the visible clip pass resolves each visible ref and selection retention calls `find` for each selected/group ref each frame. With every clip visible/selected in the one-lane fixture, the diagnostic lookup and selection probes are each about 0.004 ms (128), 0.078 ms (512), 1.16–1.18 ms (2048), 5.02–5.15 ms (4096). This is consistent with O(N²) total comparisons (N linear searches of N clips). It is significant but is **not the whole frame cost**: paint and tessellation remain material.

At 4096 visible clips, latest isolated helper medians are ~0.05–0.06 ms layout, ~7.3 ms lookup, ~7.3 ms all-selected retention probe, ~8.1–9.0 ms paint, and ~2.2–2.3 ms tessellation (depends on fixture). The full-app all-selected case grows from ~9.2–10.2 ms idle to ~23.4–24.8 ms p50, supporting selection-retention overhead. A previous CI run was lower (~7.5–8.7 ms idle, ~17.3–19.0 ms selected); the inter-run spread shows absolute timings vary, so treat the repeated shape/trend and within-run comparisons as stronger than cross-run deltas. The layout pass itself is small; offscreen paint culling is effective; hit `Rect::contains` scanning was negligible in its synthetic probe. **Hit-test performance in actual pointer/touch selection remains unmeasured.**

### Waveforms and allocations

At 512 visible audio clips, isolated audio paint is 3.78 ms, versus ~0.18–0.27 ms for empty/missing/notes. Source inspection found `stereo_wave_mesh` using linear search in an LRU capped at 256 entries / 4 MiB. Distinct geometries beyond that limit can repeatedly evict/rebuild. This is strong evidence for cache churn causing the 512-clip audio spike. The exact zoom/mesh LOD interaction causing the subsequent 2048 dip is not isolated. Cache bytes are explicitly bounded, so cache growth has memory consequences.

At 4096 visible clips, the latest diagnostic instrumentation requested ~7.0 MB of allocations per frame for audio/empty/notes and ~8.0 MB for missing audio, across ~16k–37k allocation/reallocation calls. The candidate HashSet probe itself adds some test-only allocations; these are **requested allocation bytes, not live/retained bytes**. Whole profile peak RSS was 103,256 KiB (latest run; prior 102,888 KiB) and process CPU 90.59 s (latest; prior 67.17 s). RSS is process-wide and cannot be assigned to a fixture or candidate renderer. It is not phone memory usage.

### UI versus offline audio engine

The CPU profile separates the UI from the synthetic engine render. At 4096 clips, 400 rendered blocks (each 256 stereo frames, 48 kHz) measured:

| Engine fixture | Prepare time | Block p50 | Block p95 | Block p99 | Blocks > 5.33 ms |
|---|---:|---:|---:|---:|---:|
| Audio clips | 16.630 ms | 0.252 ms | 0.274 ms | 0.517 ms | 0 |
| Missing audio | 18.581 ms | 0.253 ms | 0.278 ms | 0.521 ms | 0 |
| Empty clips | 1.173 ms | 0.0008 ms | 0.0008 ms | 0.0008 ms | 0 |
| Audio long, 16 clips | 0.097 ms | 0.0033 ms | 0.0033 ms | 0.008 ms | 0 |

Engine slot preparation and per-block scanning scale with clip slots; on this desktop test they are far below the 5.33 ms block budget. This is **not** evidence of phone headroom or absence of device underruns. It does show the measured high-count lag is predominantly UI/frame work, not a synthetic render missing its budget. Do not disable audio instruments/effects when editors are inactive; audio-engine processing is independent of the visible view.

Cold file reads are path-deduplicated to one reader call per audio/missing fixture, irrespective of clip count. In the latest run, cold audio/missing at 4096 short clips took ~1.6–1.8 ms; a single 4096-hit long fixture's large source decode took ~12.5 ms; drum fixture setup was ~8.3 ms (one-off fixture load, with no audio-file read). Asset loading does not explain steady per-frame lag.

## Proposed directions compared (not implemented)

1. **Build an identity index for clip refs** (candidate hash/set or per-lane ID-to-index map). It can attack O(N²) ref resolution and selection retention while preserving clip records and edit semantics. The test-only HashSet candidate measured at 4096 refs: ~0.076 ms to build and ~0.057 ms for 4096 selected-ref contains checks (~0.133 ms total) versus ~7.32 ms for the same repeated linear `find` probe, about 55× less in this isolated helper. At 512 refs candidate build+checks ~0.015 ms vs ~0.108 ms; at 128 both are ~0.004 ms, so indexing is not worthwhile at every size. This is **not a full-app A/B** and the candidate is not used by production. Memory was not instrumented; rough HashSet order for 4096 refs is 0.1–0.3 MiB depending on table capacity/layout, with a full-song index over many lanes larger. A dense per-lane ID array may use less memory if IDs are bounded/dense, but IDs can be replaced/renumbered on cross-lane moves, so invalidation/lifetime must be correct. Only implement if an actual full-app A/B confirms benefit, including memory and edits.
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
