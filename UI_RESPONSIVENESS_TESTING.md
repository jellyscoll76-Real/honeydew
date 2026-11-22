# UI responsiveness — v0.5.31

## Scope and baseline-first process

Baseline commit `601dd99`, release profile run [44969333352](https://github.com/jellyscoll76-Real/honeydew/actions/runs/44969333352), completed before optimization. The ignored `ui_responsiveness_profile` tests use the same input and timing procedure before/after. Build time is excluded. The workflow retains host details, raw timings and `/usr/bin/time -v` process resources as artifacts.

Baseline headless Linux results (milliseconds):

| Workload | Median | p95 | p99 |
| --- | ---: | ---: | ---: |
| Piano Roll, 1,000 notes | 0.041152 | 0.069144 | 0.083715 |
| Piano Roll, 10,000 notes | 0.245589 | 0.264477 | 0.266049 |
| Piano Roll, 50,000 notes | 0.590818 | 0.602906 | 0.645700 |
| Label, 128 Unicode characters | 0.002935 | 0.003375 | — |
| Label, 2,048 characters | 1.620666 | 1.648548 | — |
| Label, 8,192 characters | 27.056642 | 27.238195 | — |

None of the 120 measured Piano Roll frames per size exceeded 16.667 ms on this runner. This is **not** evidence of Android 60 FPS: the harness exercises egui input/layout/paint generation, not GPU presentation, Android input dispatch or vsync. Labels isolate truncation, excluding font layout. Process CPU/RSS includes the test harness, not a running Android session. These are targeted benchmarks, not full-app touch latency or long-session measurements.

## Final release comparison

Optimized source `d69fb42` (production code identical to `9d6465a`), [run 44971530168](https://github.com/jellyscoll76-Real/honeydew/actions/runs/44971530168), passed both regression groups and profiling:

| Workload | Median ms | p95 ms | p99 ms |
| --- | ---: | ---: | ---: |
| Piano Roll, 1,000 notes | 0.039946 | 0.048528 | 0.056607 |
| Piano Roll, 10,000 notes | 0.123451 | 0.135911 | 0.147505 |
| Piano Roll, 50,000 notes | 0.552628 | 0.576006 | 0.579174 |
| Label, 128 characters | 0.000500 | 0.000886 | — |
| Label, 2,048 characters | 0.004239 | 0.004462 | — |
| Label, 8,192 characters | 0.016178 | 0.018561 | — |

The long-label complexity improvement is clear (about 27 ms to 0.016 ms for the isolated largest case). Dense-roll timing is lower in this final run, but separate shared CI hosts and very short frame durations limit confidence in percentages. No measured roll frame exceeded 16.667 ms. An intermediate geometric-rejection-only attempt produced mixed roll results (50k median 0.736216 ms); it was refined to reuse the existing prefix index rather than claiming that attempt improved performance.

The first new external-follow test incorrectly expected a replaced note to stay selected. Established follow semantics preserve only unchanged selected events. The test was corrected to keep a second, unchanged selected note while extending the first; production selection behavior was not changed to satisfy the test. Final targeted controls and release Piano Roll/text regression groups pass.

## Changes

- Long-label truncation maintains head/tail widths while removing characters, replacing repeated slice summation (quadratic) with linear work. Unicode, tail retention, end ellipsis and existing short-label behavior are retained. Width accumulation uses f64 to reduce cancellation.
- Piano Roll property lanes reuse the sorted-start/prefix-end viewport index for stable events, then reject offscreen stems/caps before value lookup and shape allocation. Preview events fall back to a full scan. Accepted external edits invalidate/rebuild the index even when event count is unchanged, while retaining existing selection-follow semantics. Notes crossing viewport boundaries and minimum-width stems remain visible. This does not alter events, selection, hit testing, expression curves, playback or audio.
- No audio callback, scheduling, buffers, audio quality, engine locks, sample processing, MIDI timing or repaint cadence changes. Existing external-edit detection remains intact. No forced 60 Hz cap or perpetual repaint loop was introduced.

## Automated checks

The profiling workflow also runs text and Piano Roll regression tests. The existing targeted workflow covers Auto-Save, project files, editor gestures, undo/save serialization, settings, envelopes, offline rendering and audio device tests. A universal Android release build checks real ARMv7 and ARM64 native packaging. Consult the associated Actions conclusions; a dispatched/canceled run is not validation.

## Device playtest required (not performed in the sandbox)

Use the same physical device/project/settings and equivalent thermal state for before/after comparisons. Retain the previous APK/project backup. Test with the performance overlay both off and on; the existing FPS counter excludes gaps over 250 ms and must not be used alone to rule out stalls.

1. Record Android frame timelines/jank, CPU and memory using Perfetto/Android profiling tools. Measure presentation/input latency on the device, not headless test time. Repeat at 60 Hz and higher supported refresh rates.
2. Rapidly navigate browser/settings/playlist/Piano Roll and switch patterns/tracks. Check immediate pressed feedback, dialogs, long Unicode filenames and dense lists.
3. Continuously swipe, scroll and pinch zoom. Drag/resize notes, box-select, edit graph values and undo/redo in sparse and 1k/10k/50k-note clips. Include long notes crossing both viewport edges, selected notes, expression curves and narrow zoom levels.
4. Repeat while playing a multi-track project and playing the instrument live. Listen for crackles/dropouts; compare underruns, callback load and MIDI timing. Confirm no quality/buffer/latency settings changed. Stop if audio degrades.
5. Run an extended editing/playback session; compare memory after repeated navigation and project reloads, not only at startup. Exercise Auto-Save off and manually enabled, including its five-second warning and recovery copies.

Do not claim device FPS, zero dropped frames, touch-to-display latency, audio latency or long-session stability until these physical-device measurements are collected.
