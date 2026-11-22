# High clip count investigation: controlled profiling results

**Status: profiled, not yet optimized.** No production code was changed by this
investigation. The harness is test-only (`src/ui/clips/count_profile.rs`,
`src/app/tests/count_profile.rs`) and runs in CI via
`.github/workflows/clip-count-profile.yml` (runs 45065032259, 45066092379).

## What was measured

Controlled fixtures with equal musical content: many short clips vs 16 long
clips carrying the same synthesized pulse sequence (6000 PCM frames per pulse
at 48 kHz), at 128 / 512 / 2048 / 4096 logical hits, for audio, missing-file,
empty, Piano Roll notes, and drum (steps/notes) cases. Two views (overview =
all clips visible; close_end = 65 visible, rest offscreen), idle/scroll, and
all-selected modes. Three measurement layers:

1. **UI attribution** — production helpers (layout, ID lookup, selection
   retention, hit scan, paint, tessellation) plus allocation counting.
2. **Full-app headless frames** — complete `BanaForgeApp` frames through the
   test Rig (fake audio sink, no GPU).
3. **Offline audio engine** — 400 blocks × 256 stereo frames at 48 kHz.

**Limits:** Linux CI CPU only. No Android device, no GPU rasterization, no
real playback, no hardware underruns. Full-app numbers are CPU time per frame,
not FPS. Move/edit gestures were not measured. Notes/drums are UI fixture
controls, not acoustic equivalents. Allocation counts include diagnostic
probes. The composite in the harness is not a real frame; the full-app table
is the closest to one.

## Results

### Full-app CPU per frame (ms, p50 UI / p95 UI / p50 tessellation)

| Case | Hits (clips) | Overview idle | Overview all-selected | Close-end idle |
|---|---:|---:|---:|---:|
| audio | 512 (512) | 4.92 / 5.00 / 0.63 | 5.85 / 5.95 / 0.65 | 0.34 / 0.38 / 0.28 |
| audio | 2048 (2048) | 3.22 / 3.56 / 2.17 | 6.62 / 6.96 / 2.24 | 0.45 / 0.50 / 0.28 |
| audio | 4096 (4096) | 10.01 / 10.24 / 3.73 | 22.78 / 23.32 / 3.86 | 0.64 / 0.71 / 0.28 |
| missing | 4096 (4096) | 10.83 / 10.98 / 3.78 | 23.54 / 23.80 / 3.79 | 0.63 / 0.70 / 0.26 |
| empty | 4096 (4096) | 9.33 / 9.56 / 3.74 | 21.75 / 21.91 / 3.79 | 0.58 / 0.64 / 0.27 |
| notes | 4096 (4096) | 9.46 / 9.55 / 3.80 | 21.29 / 21.42 / 3.76 | 0.57 / 0.68 / 0.29 |
| drums_steps | 4096 (4096) | 9.14 / 9.32 / 3.50 | 21.67 / 21.88 / 3.79 | 0.58 / 0.67 / 0.28 |
| audio_long | 4096 (16) | 0.15 / 0.20 / 0.18 | 0.15 / 0.19 / 0.19 | 0.10 / 0.15 / 0.16 |
| notes_long | 4096 (16) | 0.60 / 0.66 / 0.94 | 0.62 / 0.70 / 0.94 | 0.11 / 0.14 / 0.17 |

At 2048 clips every many-clip case is ~3 ms; at 4096, ~9–11 ms UI plus
~3.7 ms tessellation. **Clip count, not content type, drives frame time:**
audio, missing, empty, notes, and drums are nearly identical at 4096.
16 long clips with the same musical content stay flat at ~0.15 ms.
Offscreen clips are already cheap (close-end stays under 0.7 ms at 4096
total clips) — paint culling works. All-selected roughly doubles the cost
and grows superlinearly (22.8 ms at 4096).

### UI attribution, overview/idle (ms per diagnostic frame, medians)

| Case | Hits | Layout | Lookup | All-sel lookup | Paint | Tess | Alloc calls | Requested |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| audio | 512 | 0.004 | 0.078 | 0.078 | 3.784 | 0.327 | 6,692 | 1.2 MB |
| audio | 2048 | 0.011 | 1.182 | 1.174 | 1.717 | 0.970 | 10,279 | 3.5 MB |
| audio | 4096 | 0.043 | 5.043 | 5.033 | 5.781 | 2.136 | 20,521 | 6.9 MB |
| missing | 4096 | 0.045 | 5.072 | 5.086 | 6.789 | 2.283 | 36,905 | 7.9 MB |
| empty | 4096 | 0.048 | 5.115 | 5.107 | 5.756 | 2.318 | 16,425 | 6.9 MB |
| notes | 4096 | 0.052 | 5.025 | 5.018 | 5.702 | 2.345 | 16,425 | 6.9 MB |
| audio_long | 4096 | 0.001 | 0.000 | 0.000 | 0.035 | 0.025 | 697 | 0.2 MB |

- **ID lookup is quadratic** (visible × total): 0.004 → 0.078 → 1.17 → 5.0 ms
  as clips go 128 → 512 → 2048 → 4096. `find()` is a linear scan
  (`src/ui/clips.rs:2146`), and selection retention calls it per selected
  clip every frame (`group.retain(|&r| kept(r))`, `src/ui/clips.rs:521`).
  Clip IDs are unique within a lane, so an index is semantically safe.
- **Paint is the largest single cost at high counts** (~5.7–6.8 ms at 4096,
  nearly identical across content types — per-clip fixed work, not
  waveform-specific). Audio at 512 is an outlier (3.78 ms vs ~0.25 ms for
  other cases): the waveform mesh cache (256 entries / 4 MiB, linear search,
  `stereo_wave_mesh`) thrashes once distinct geometries exceed 256, forcing
  a rebuild per clip per frame. Audio paint then *dips* at 2048 (1.72 ms) —
  clips are narrower at that overview zoom, so meshes have fewer columns.
  The 512-vs-2048 non-monotonicity is reproducible across two runs but the
  exact zoom/LOD mechanism is not yet isolated.
- **Layout is negligible** (0.05 ms at 4096) and **hit-rect scan is
  negligible** in the probe. Offscreen paint culling already works
  (close-end paint is flat ~0.1–0.6 ms regardless of total count).
- **Allocations scale with visible clips**: 16–37k calls and 7–8 MB
  requested per frame at 4096 visible. Secondary, but real allocator
  pressure on a phone.

### Offline audio engine (per 256-frame block, ms)

| Case | Hits | p50 | p95 | p99 | Over 5.33 ms budget |
|---|---:|---:|---:|---:|---:|
| audio | 512 | 0.030 | 0.038 | 0.058 | 0 |
| audio | 2048 | 0.115 | 0.128 | 0.232 | 0 |
| audio | 4096 | 0.235 | 0.261 | 0.480 | 0 |
| missing | 4096 | 0.083 | 0.108 | 0.168 | 0 |
| empty | 4096 | 0.0007 | 0.0007 | 0.0007 | 0 |
| audio_long | 4096 | 0.003 | 0.003 | 0.004 | 0 |

Per-block cost scales with clip slots (missing clips cost engine scan time
too; empty clips are nearly free). No synthetic budget misses on this CPU,
but 4096-clip p99 (0.48 ms) leaves less headroom once a phone CPU's
multiplier is applied. Engine prepare time also grows (14.7 ms at 4096
audio clips). Secondary contributor, not the main lag source.

### Cold asset loading

Reader calls stay at **1 regardless of clip count** (path dedup works).
Cold load: audio 4096 = 1.66 ms, missing 4096 = 1.78 ms, drums ≈ 8.7 ms
flat (one-time kit decode), empty/notes ≈ 0.01 ms. **Asset loading is not
the problem.**

## Responsible subsystems (ranked)

1. **Repeated linear clip-ID lookup** — quadratic for visible-clip
   resolution and selection retention. Worst multiplier: all-selected at
   4096 costs 22.8 ms vs 10.0 ms idle in full-app frames.
2. **Per-visible-clip paint** — shape construction, mesh-cache lookup/rebuild,
   and text per clip; ~5.7–6.8 ms at 4096 visible. Waveform mesh cache
   thrash adds a mid-count audio spike (512 clips).
3. **Tessellation** — ~2.2–2.4 ms at 4096 visible; proportional to shapes.
4. **Allocations** — 16–37k calls / 7–8 MB per frame at 4096 visible.
5. **Audio engine slot scan** — per-block cost scales with clip count;
   secondary on desktop, unknown phone multiplier.
6. **Not responsible:** layout scan (0.05 ms at 4096), offscreen paint
   (already culled), asset loading (deduped, one read), hit-rect scan
   (negligible in probe).

## Safe optimization directions (no behavior change proposed yet)

1. **Index clip lookup per frame** (HashMap from clip ID to index per lane,
   built once per frame; IDs are unique within a lane). Turns selection
   retention and visible-clip resolution from O(S×N)/O(V×N) to O(N) build +
   O(1) lookups. No editing semantics change.
2. **Waveform mesh cache**: replace the linear VecDeque search with a hash
   map keyed by geometry, and/or raise the entry bound with the same byte
   budget. Addresses the 512-clip audio paint spike. Memory bound must stay
   explicit.
3. **Paint LOD**: skip waveform detail (and clip-name text) for sub-pixel
   clips. Verify current behavior first — the 2048 dip suggests partial
   LOD already exists.
4. **Engine slot scan**: faster inactive-slot skipping. Higher risk; needs
   audio regression tests. Lower priority.
5. **Do not** merge clips, change editing behavior, or touch audio quality,
   latency, MIDI timing, or instrument responsiveness.

## What remains before claiming a fix

- Implement (1) and (2), re-run this harness, and confirm the quadratic
  lookup term and the 512-clip paint spike are gone.
- Add move/edit gesture measurement and a zoom sweep to isolate the
  512-vs-2048 paint non-monotonicity.
- Real playback testing on device (the user is phone-only) with hardware
  underrun observation; desktop CPU numbers do not transfer directly.
- Large-arrangement regression tests in CI so clip counts cannot silently
  regress.
