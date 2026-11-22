# Cross-project clipboard and lane actions — v0.5.32

## Investigation

- Playlist clip snapshots were stored in `MainScreen.clips`; `SongEdit::Replaced` discarded that whole state. Rack-module snapshots already survived in `Slots.clipboard`; Piano Roll note and shared drum-hit snapshots already survived in egui context memory.
- Playlist paste used the touched lane for a single matching clip, otherwise old numeric track indices. Those indices are not identities in a different project.
- The six-action playlist toolbar's Copy only filled its clipboard; it never dismissed the temporary row. The shared floating bubbles used circular hit tests and could disappear entirely when their fixed-width row did not fit.

## Behavior

Playlist copies are owned snapshots retained for this app session when a project is replaced. Selection, old cursor, gestures and menus reset independently. Copying new clips replaces the clipboard. App restart persistence is not added.

Cross-project paste retains horizontal placement: the earliest copied start lands on the snapped paste time and the others retain their relative beat offsets. Vertical placement ignores the target cursor: each copied clip finds an existing compatible lane nearest its original source-playlist lane position (including hidden lanes in the ordering). Ties choose the earlier lane. Hidden lanes are eligible only with Show hidden tracks enabled. Notes require a note-capable Instrument/Synth/Rack channel; drums require a drum channel/Drums lane with the referenced pad slots; audio requires an Audio lane. No missing channel, lane, instrument or rack is created.

Audio can overlap existing clips. Paste appends clones with fresh lane-local IDs and preserves loop/visible lengths, offsets, events, sample path and sampler settings, envelopes, lyric/pitch metadata and other clip state. It does not replace destination instruments or copy source rack modules over them. Existing instrument/drum racks supply playback sounds, as they do for ordinary pattern pastes. Sample references remain references to existing library files, not embedded copies; deleting/moving those files can still make them unavailable.

Cross-project automation clips are deliberately skipped: module/pad IDs belong to the source project and a same-numbered destination is not proof of a compatible control binding. Same-project automation paste remains supported. Other existing editor/module clipboards and explicit module Replace operations keep their established meanings; this is not a redesign of the Modules browser.

Same-project clip paste keeps its existing behavior: one clip can use the touched lane of the same kind; groups retain their original tracks. Once copied content has crossed a project replacement, it stays detached from target-project track rekeys/deletions. Deselecting and repeated pasting do not consume it.

## Lane actions

The visible bubbles increase from 1.4 U to 1.5 U where space permits. Each gets an equal, non-overlapping square touch cell, at least 44 logical points in normal layouts. The menu wraps into multiple rows rather than hiding off the side of a narrow phone. Stable IDs/labels and the six operations are retained.

Every playlist lane action dims and slightly grows while pressed. The separate Piano Roll, drum and automation floating toolbar dimensions/behavior are retained. Copy performs the snapshot immediately, gives a 90 ms release flash, then hides the temporary toolbar; input/operations do not sleep or wait for animation. During that brief dismissal it cannot issue another action. Reopening the row restores Copy and all other actions. This affects UI drawing only, not audio callbacks, buffers, scheduling or sample processing.

## Automated checks

`Cross-project clipboard validation` runs:

- Retention across project replacement, deselection, repeated replacements and target track rekeys.
- Mixed audio/note/drum routing into existing lanes despite an incompatible vertical cursor.
- Existing overlapping audio and destination rack invariance; deep copied-clip property comparisons.
- Multiple compatible destinations, repeat paste and unique IDs.
- Missing/hidden destinations as no-ops; safe rejection of project-local automation bindings.
- Actual app project replacement, note clipboard retention, clip paste routing and undo.
- Padded touch hit detection and pressed circle size/color changes.
- Copy release-flash/dismiss/reopen state and layout bounds/target separation at narrow widths.
- Existing playlist, Copy/Duplicate/Delete/Snap/Edit/More, Piano Roll, drum/step grid, rack and Auto-Save regressions.

## Physical Android acceptance (not performed in the sandbox)

1. Install the universal APK on ARMv7 and ARM64 devices. Use small and large phones, portrait/landscape and several UI scales.
2. In A, copy audio with non-default trim/offset/envelopes and a note/drum pattern. Load B, move the vertical cursor below all compatible racks, and paste at a distinct timeline beat. Confirm correct existing destinations, sound and metadata.
3. Repeat with several compatible destinations, no match, hidden lanes, mixed selections, undo/redo, repeated paste and B→C project switches. Verify no tracks/racks are created and no instruments/clips are overwritten. Confirm overlapping audio plays as expected.
4. Exercise normal same-project copy/cut/paste, including groups, keyboard shortcuts and the touch insert-menu Paste.
5. Tap Copy: confirm visible press/release feedback, exactly one copy and toolbar dismissal. Reopen it and verify all six actions, including More submenus and Edit navigation. Tap the padded area around each circle and drag off a button to cancel.
6. Check wrapped toolbars remain visible and clear of editing handles on small screens, including with the keyboard/editor open. Repeat while playing and navigating quickly. Listen for audio regressions; automated geometry tests do not establish real-device touch comfort or frame pacing.

## Regression investigation notes

The initial enlarged shared-toolbar attempt exposed legacy geometry assumptions, so the new sizing/hit cells were scoped to the six playlist lane actions; Piano Roll, drum/step and automation toolbar geometry is unchanged. Playlist tests that intentionally target a grid clip underneath the now-larger toolbar dismiss that overlay first; press ownership still prevents accidental edits through the toolbar.

One pre-existing rack test expected a click on a relative-drag Mix slider to produce a parameter change. The unchanged pre-feature source `9fca7e2` fails that isolated test as well (comparison workflow run 45011125624). The test now performs a real relative drag, keeping the parameter movement and rebuild shortcut in the same frame, so its original SetParam/handover/no-undo assertions remain meaningful. No rack control or audio-engine code was changed to satisfy it.
