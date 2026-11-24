# Auto-Save acceptance — v0.5.30

Enable in Settings → Application → Auto-Save. Default OFF; interval 5:00. Drag the time or enter minutes:seconds (10 seconds to 24 hours); a bare number means seconds.

Auto-Save writes the current project to **My Songs / Auto-saved projects / Auto-save-<session-project-id> / Project.flm**. Each project opened in a session gets its own folder. Open that file from Songs to recover it, then use Save New to give it your desired name. Samples remain external references, as with normal FLM saves; this is not a ZIP sample archive. No recovery copy is automatically deleted.

This is intentionally a recovery save, not an overwrite of your manual project file. It never renames the working song, clears its unsaved flag, changes undo history or stops playback. Existing recent-session crash recovery remains independent, including when this new interval feature is OFF. Disabling cancels pending countdowns; a write already underway is allowed to finish atomically.

## Automated

- Defaults, preference persistence, five-minute and custom interval timing, 5→1 warning, cancellation, late-frame full warning, clean-project no-save, active-edit deferral.
- Real FLM round-trip from the recovery writer; encoding failure and temporary-file IO failure retain the previous recovery bytes.
- Existing project file saves, current-project lifecycle, Piano Roll/Playlist editing and audio render regressions.

## Physical Android checks (not claimed performed in CI)

1. With Auto-Save OFF, edit for more than five minutes: no new recovery folder or countdown should appear. Existing `recent.state` recovery may still update.
2. Enable it at 0:10; close Settings, edit notes and play audio. Confirm the top banner shows 5, 4, 3, 2, 1 before writing, captures no taps, and reports success only after completion. Reopen the resulting FLM and compare notes, envelopes, module state and sample paths.
3. Change to 1:30, restart, and confirm toggle/interval persistence. Disable during countdown: no scheduled snapshot. Change interval: full new interval. Let the app sleep across a deadline: a full five-second warning must appear on return, not an immediate hidden save.
4. Hold/drag a note/fader at the deadline, record audio, and operate a rack: editing and recording should defer the snapshot; an idle rack does not. Confirm playback does not stop or seek. Check dense projects on a low-end device for snapshot-capture latency; background encoding does not prove zero UI delay.
5. Switch projects mid-countdown or during a write. Confirm the old snapshot cannot overwrite the new project's recovery. Test untitled projects; no Save dialog should interrupt.
6. Simulate unavailable/full storage: an error banner must appear, no false success, and the previous recovery and manual file must remain readable. Back up projects before destructive storage tests.
