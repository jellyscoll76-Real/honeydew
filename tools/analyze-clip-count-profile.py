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
         'Linux release build; test-only allocation and waveform-cache instrumentation. Candidate HashSet indexing is measured only in test builds; production remains unchanged.',
         '**Not Android FPS, GPU timing, physical-touch latency or hardware xrun measurements.** Scripted interaction times are synthetic headless desktop event sequences.',
         'Paint includes production clip lookup, cached asset lookup, egui shape construction and text; tessellation is separate.',
         'Layout and explicit lookup/selection/hit probes are independent diagnostic passes, not additive full-app phases.',
         'The composite p50/p95/p99 includes extra all-selected and lookup probes even in idle mode. Do not treat it as a real frame.',
         'Scroll changes the axis only; selection measures actual ID resolution, not complete gestures/edit rebuilds.', '',
         'A/B synthesize the same pulse sequence (6000 PCM frames per pulse at 48 kHz); long clips concatenate pulses.',
         'Durations/hit counts match within a comparison; short and 16-long drum patterns carry the same ordered drum events (four pads), while notes/drums remain UI fixture controls rather than acoustic engine equivalents.',
         'Each UI entry is the median across three runs of 24 warm samples; fixture construction and cold decode are excluded.', '',
         '## UI helpers (milliseconds per diagnostic frame)', '',
         '| Case | Hits | Clips | View | Motion | Visible | Layout | Lookup | All-selected lookup | Candidate index build | Indexed lookup | Indexed all-selected | Rect hit scan | Paint | Tessellate | Indexed paint+tess p50 | Index capacity | Index alloc bytes | Alloc calls | Requested KiB |',
         '|---|---:|---:|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|']
groups = collections.defaultdict(list)
for row in rows:
    if row['phase'] == 'ui_attribution':
        groups[(row['case'], row['hits'], row['view'], row['motion'])].append(row)
for (case, hits, view, motion), group in groups.items():
    med = lambda key: statistics.median(r[key] for r in group)
    fields = [med(k) for k in ['layout_ms','lookup_ms','all_selected_lookup_ms','index_build_ms','indexed_lookup_ms','all_selected_indexed_ms','hit_rect_scan_ms','paint_ms','tessellate_ms','indexed_paint_tess_p50_ms']]
    lines.append(f'| {case} | {hits} | {med("clips"):g} | {view} | {motion} | {med("visible"):g} | ' +
                 ' | '.join(f'{v:.4f}' for v in fields) +
                 f' | {med("indexed_map_capacity"):g} | {med("indexed_map_requested_alloc_bytes_per_frame"):g} | {med("alloc_calls_per_frame"):g} | {med("requested_alloc_bytes_per_frame")/1024:.1f} |')
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
          'Complete BanaForgeApp frames via the test Rig: UI pass plus separate egui tessellation. Recorder is a fake sink; **not hardware playback, device FPS or physical input latency**.', '',
          '| Case | Hits | Clips | View | Mode | UI p50 | UI p95 | UI p99 | Tess p50 | Tess p95 | Cache hits | misses | evictions |', '|---|---:|---:|---|---|---:|---:|---:|---:|---:|---:|---:|---:|']
for r in rows:
    if r['phase'] == 'full_app_cpu':
        lines.append(f"| {r['case']} | {r['hits']} | {r['clips']} | {r['view']} | {r['mode']} | {r['ui_p50_ms']:.3f} | {r['ui_p95_ms']:.3f} | {r['ui_p99_ms']:.3f} | {r['tess_p50_ms']:.3f} | {r['tess_p95_ms']:.3f} | {r['wave_cache_hits']} | {r['wave_cache_misses']} | {r['wave_cache_evictions']} |")
lines += ['', 'For all-selected modes, `all_selected_indexed` is a test-only full-frame candidate HashSet path; it builds the set each frame. The additional `indexed_paint_tess` pass builds a per-lane ID->position HashMap, resolves and paints the same visible clips, then tessellates; map build and allocation are included. Compare against baseline paint+tessellate at identical fixture/view; this is still a helper pass, not a full-app indexed renderer. Equal shape/vertex counts and per-ID assertions are checked, not pixel/GPU equivalence.', '',

          '## Scripted full-app interactions (milliseconds per complete synthetic event sequence)', '',
          'Five repeats each at 4096 clips; sequence timing includes egui app frames and rig input dispatch. This is Linux desktop/headless diagnostic timing, not physical touch latency or Android performance.', '',
          '| Case | Clips | Action | Median | Five totals (ms) | Effect each repeat | Resize diagnostic |', '|---|---:|---|---:|---|---|---|']
for r in rows:
    if r['phase'] == 'scripted_interaction':
        lines.append(f"| {r['case']} | {r['clips']} | {r['action']} | {r['median_ms']:.3f} | {', '.join(f'{x:.3f}' for x in r['total_ms'])} | {', '.join('yes' if x else 'no' for x in r['effect_observed'])} | {json.dumps(r['resize_details'], separators=(',', ':')) if r.get('resize_details') else '—'} |")
lines += ['', '### Dense resize hit-test diagnostic', '',
          'The diagnostic repeats the original clip target and center point, then moves only the later clips out of the viewport while keeping the target at the same screen coordinates. This tests whether the original center is covered by adjacent clip hit responses; it is not a general UI fix or a production change.', '']
for r in rows:
    if r['phase'] == 'resize_hit_test_diagnostic':
        lines.append(f"- {r['case']} (target index {r['target_index']}): " + '; '.join(
            f"captured={'yes' if x['captured'] else 'no'}, resized={'yes' if x['resized'] else 'no'} ({x['before_ticks']}→{x['after_ticks']} ticks) at {x['press_at']}"
            for x in r['repeats']))
lines += ['', '### Test-only persistent-map maintenance estimate (4096 clips)', '',
          'Each timing includes the existing clip operation plus a map update using ID->vector-position entries. Delete adjusts all later positions to preserve lane order; cross-lane move includes removal, destination collision/re-key, and updates to both lane maps. Same-lane start movement leaves positions stable. Project-load timing rebuilds the map from an already-loaded in-memory song, excluding parsing/assets.', '',
          '| Build | Load rebuild | Add+insert | Copy+insert | Delete+stable reindex | Same-lane move index work | Cross-lane move+updates |',
          '|---:|---:|---:|---:|---:|---:|---:|']
for r in rows:
    if r['phase'] == 'index_maintenance':
        lines.append('| ' + ' | '.join(f'{r[k]:.4f}' for k in [
            'build_ms','load_rebuild_ms_excluding_parse_and_assets','add_plus_insert_ms','copy_plus_insert_ms',
            'delete_plus_stable_order_reindex_ms','same_lane_start_move_index_work_ms','cross_lane_move_plus_two_map_updates_ms']) + ' |')
lines += ['', 'These are isolated Linux/release microbenchmarks, not complete project-edit end-to-end costs. They do not prove persistent index correctness; invalidation must still cover every production mutation path and load/undo/replacement.', '',
          'Cold asset read counts, lazy summary timings, per-case RSS, process CPU time, shape/vertex counts and allocation requests are in data.json.',
          'Allocation bytes count requests/reallocations, not live memory; RSS is process-wide with retained allocator/cache memory, not isolated fixture ownership.',
          'resources.txt records whole-test CPU/peak RSS, excluding compilation. No claims of musical-output equivalence or actual playback stability follow from these measurements.']
(destination / 'REPORT.md').write_text('\n'.join(lines) + '\n')

# Compact medians for the annotation channel (full data.json stays in the artifact).
summary = {'ui_attribution': {}, 'full_app_cpu': {}, 'offline_audio': [], 'cold_assets': [], 'resize_hit_test_diagnostic': [], 'index_maintenance': []}
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
        'indexed_paint_tess_p50_ms': med('indexed_paint_tess_p50_ms'),
        'indexed_paint_tess_p95_ms': med('indexed_paint_tess_p95_ms'),
        'indexed_map_capacity': med('indexed_map_capacity'),
        'indexed_map_alloc_calls_per_frame': med('indexed_map_alloc_calls_per_frame'),
        'indexed_map_requested_alloc_bytes_per_frame': med('indexed_map_requested_alloc_bytes_per_frame'),
        'alloc_calls_per_frame': med('alloc_calls_per_frame'),
        'requested_alloc_bytes_per_frame': med('requested_alloc_bytes_per_frame'),
        'repeats': len(group)}
for r in rows:
    if r['phase'] == 'full_app_cpu':
        summary['full_app_cpu'][f"{r['case']}|{r['hits']}|{r['view']}|{r['mode']}"] = {
            'clips': r['clips'], 'ui_p50_ms': round(r['ui_p50_ms'], 4),
            'ui_p95_ms': round(r['ui_p95_ms'], 4), 'ui_p99_ms': round(r['ui_p99_ms'], 4),
            'tess_p50_ms': round(r['tess_p50_ms'], 4), 'tess_p95_ms': round(r['tess_p95_ms'], 4),
            'wave_cache_hits': r['wave_cache_hits'], 'wave_cache_misses': r['wave_cache_misses'],
            'wave_cache_inserts': r['wave_cache_inserts'], 'wave_cache_evictions': r['wave_cache_evictions'],
            'wave_cache_expired': r['wave_cache_expired'], 'wave_cache_bypasses': r['wave_cache_bypasses'],
            'wave_cache_entries': r['wave_cache_entries'], 'wave_cache_bytes': r['wave_cache_bytes']}
    elif r['phase'] == 'scripted_interaction':
        summary.setdefault('scripted_interaction', []).append({k: (round(v, 4) if isinstance(v, float) else v)
            for k, v in r.items() if k != 'phase'})
    elif r['phase'] == 'offline_audio':
        summary['offline_audio'].append({k: (round(v, 4) if isinstance(v, float) else v)
            for k, v in r.items() if k != 'phase'})
    elif r['phase'] == 'cold_assets':
        summary['cold_assets'].append({k: (round(v, 4) if isinstance(v, float) else v)
            for k, v in r.items() if k != 'phase'})
    elif r['phase'] == 'resize_hit_test_diagnostic':
        summary['resize_hit_test_diagnostic'].append({k: v for k, v in r.items() if k != 'phase'})
    elif r['phase'] == 'index_maintenance':
        summary['index_maintenance'].append({k: v for k, v in r.items() if k != 'phase'})
(destination / 'summary.json').write_text(json.dumps(summary, separators=(',', ':')) + '\n')
print(f'Summarized {len(rows)} measurements')
