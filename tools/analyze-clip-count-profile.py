#!/usr/bin/env python3
"""Summarize headless measurements without converting CPU helper timings to device FPS."""
import collections
import json
from pathlib import Path
import statistics
import sys

source, destination = Path(sys.argv[1]), Path(sys.argv[2])
rows = [json.loads(line.split('CLIP_PROFILE ', 1)[1]) for line in source.read_text().splitlines() if 'CLIP_PROFILE {' in line]
assert rows, 'No measurements found'
destination.mkdir(parents=True, exist_ok=True)
(destination / 'data.json').write_text(json.dumps(rows, indent=2) + '\n')
lines = ['# High clip count: controlled headless attribution', '',
         'Linux release build; test-only allocation instrumentation. Production frame-local selection indexing is under test.',
         '**Not Android FPS, GPU timing, full-app input latency or hardware xrun measurements.**',
         'Paint includes production clip lookup, cached asset lookup, egui shape construction and text; tessellation is separate.',
         'Layout and explicit lookup/selection/hit probes are independent diagnostic passes, not additive full-app phases.',
         'The composite p50/p95/p99 includes extra all-selected and lookup probes even in idle mode. Do not treat it as a real frame.',
         'Scroll changes the axis only; selection measures actual ID resolution, not complete gestures/edit rebuilds.', '',
         'A/B synthesize the same pulse sequence (6000 PCM frames per pulse at 48 kHz); long clips concatenate pulses.',
         'Durations/hit counts match within a comparison; notes/drums are UI fixture controls, not acoustic equivalents.',
         'Each UI entry is the median across three runs of 24 warm samples; fixture construction and cold decode are excluded.', '',
         '## UI helpers (milliseconds per diagnostic frame)', '',
         '| Case | Hits | Clips | View | Motion | Visible | Layout | Lookup | All-selected lookup | Index build | Indexed lookup | Indexed all-selected | Rect hit scan | Paint | Tessellate | Alloc calls | Requested KiB |',
         '|---|---:|---:|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|']
groups = collections.defaultdict(list)
for row in rows:
    if row['phase'] == 'ui_attribution':
        groups[(row['case'], row['hits'], row['view'], row['motion'])].append(row)
for (case, hits, view, motion), group in groups.items():
    med = lambda key: statistics.median(r[key] for r in group)
    fields = [med(k) for k in ['layout_ms','lookup_ms','all_selected_lookup_ms','index_build_ms','indexed_lookup_ms','all_selected_indexed_ms','hit_rect_scan_ms','paint_ms','tessellate_ms']]
    lines.append(f'| {case} | {hits} | {med("clips"):g} | {view} | {motion} | {med("visible"):g} | ' +
                 ' | '.join(f'{v:.4f}' for v in fields) + f' | {med("alloc_calls_per_frame"):g} | {med("requested_alloc_bytes_per_frame")/1024:.1f} |')
lines += ['', '## Offline audio CPU (not hardware playback)', '',
          'One 400-block pass per fixture, 256 stereo frames per block at 48 kHz. No device driver, concurrent UI or Android scheduler.',
          'Wall blocks exceeding 5.333 ms are reported as synthetic budget misses, **not observed underruns**.',
          'The first ~2.13 seconds are rendered; inactive future clip slots remain in the engine. This does not cover every clip boundary or a long session.', '',
          '| Case | Hits | Clips | Prepare ms | Block p50 ms | p95 ms | p99 ms | Synthetic budget misses | PCM energy | RSS KiB |',
          '|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|']
for r in rows:
    if r['phase'] == 'offline_audio':
        lines.append(f'| {r["case"]} | {r["hits"]} | {r["clips"]} | {r["prepare_ms"]:.3f} | {r["p50_ms"]:.4f} | {r["p95_ms"]:.4f} | {r["p99_ms"]:.4f} | {r["over_budget_blocks"]} | {r["energy"]:.3f} | {r["rss_kib"]} |')
lines += ['', '## Full-app headless frames (milliseconds per frame; fake audio sink, no GPU)', '',
          'Complete BanaForgeApp frames via the test Rig: UI pass plus separate egui tessellation. Recorder is a fake sink; **not hardware playback, device FPS or input latency**.', '',
          '| Case | Hits | Clips | View | Mode | UI p50 | UI p95 | UI p99 | Tess p50 | Tess p95 |', '|---|---:|---:|---|---|---:|---:|---:|---:|---:|']
for r in rows:
    if r['phase'] == 'full_app_cpu':
        lines.append(f"| {r['case']} | {r['hits']} | {r['clips']} | {r['view']} | {r['mode']} | {r['ui_p50_ms']:.3f} | {r['ui_p95_ms']:.3f} | {r['ui_p99_ms']:.3f} | {r['tess_p50_ms']:.3f} | {r['tess_p95_ms']:.3f} |")
lines += ['', 'Cold asset read counts, lazy summary timings, per-case RSS, process CPU time, shape/vertex counts and allocation requests are in data.json.',
          'Allocation bytes count requests/reallocations, not live memory; RSS is process-wide with retained allocator/cache memory, not isolated fixture ownership.',
          'resources.txt records whole-test CPU/peak RSS, excluding compilation. No claims of musical-output equivalence or actual playback stability follow from these measurements.']
(destination / 'REPORT.md').write_text('\n'.join(lines) + '\n')

# Compact medians for the annotation channel (full data.json stays in the artifact).
summary = {'ui_attribution': {}, 'full_app_cpu': {}, 'offline_audio': [], 'cold_assets': []}
for (case, hits, view, motion), group in groups.items():
    med = lambda key: round(statistics.median(r[key] for r in group), 4)
    summary['ui_attribution'][f'{case}|{hits}|{view}|{motion}'] = {
        'clips': med('clips'), 'visible': med('visible'),
        'layout_ms': med('layout_ms'), 'lookup_ms': med('lookup_ms'),
        'all_selected_lookup_ms': med('all_selected_lookup_ms'),
        'index_build_ms': med('index_build_ms'), 'indexed_lookup_ms': med('indexed_lookup_ms'),
        'all_selected_indexed_ms': med('all_selected_indexed_ms'),
        'hit_rect_scan_ms': med('hit_rect_scan_ms'), 'paint_ms': med('paint_ms'),
        'tessellate_ms': med('tessellate_ms'),
        'alloc_calls_per_frame': med('alloc_calls_per_frame'),
        'requested_alloc_bytes_per_frame': med('requested_alloc_bytes_per_frame'),
        'repeats': len(group)}
for r in rows:
    if r['phase'] == 'full_app_cpu':
        summary['full_app_cpu'][f"{r['case']}|{r['hits']}|{r['view']}|{r['mode']}"] = {
            'clips': r['clips'], 'ui_p50_ms': round(r['ui_p50_ms'], 4),
            'ui_p95_ms': round(r['ui_p95_ms'], 4), 'ui_p99_ms': round(r['ui_p99_ms'], 4),
            'tess_p50_ms': round(r['tess_p50_ms'], 4), 'tess_p95_ms': round(r['tess_p95_ms'], 4)}
    elif r['phase'] == 'offline_audio':
        summary['offline_audio'].append({k: (round(v, 4) if isinstance(v, float) else v)
            for k, v in r.items() if k != 'phase'})
    elif r['phase'] == 'cold_assets':
        summary['cold_assets'].append({k: (round(v, 4) if isinstance(v, float) else v)
            for k, v in r.items() if k != 'phase'})
(destination / 'summary.json').write_text(json.dumps(summary, separators=(',', ':')) + '\n')
print(f'Summarized {len(rows)} measurements')
