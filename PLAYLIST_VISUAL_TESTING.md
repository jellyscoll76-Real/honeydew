# v0.5.27 Playlist visual acceptance

Automated geometry covers square lamps at 75/100/150/200% scale, collapsed and expanded header widths, and short/normal track heights. Follow-cap bounds cover 180/320/720-point ruler widths and both saved-position limits. Existing control/rack/filename/follow interaction suites also run. These are not physical-device performance results.

On Android, check portrait and landscape at supported UI scales:

- Enable Track activity indicators. Expand/collapse the header column; sliders, mute/solo, track selection and header holds must remain independently usable. Lamps occupy their own gutter; no lamp is drawn on automation/sub-track rows.
- Play an audio clip and a synth. Output flashes white. Enable Match activity indicators to track color: each lamp uses its channel's color. Change that color live. Stop/mute and verify the lamp follows actual output, not merely a scheduled clip. Test live input monitoring too.
- Disable indicators: the gutter disappears. Color matching remains saved independently. Restart and check both preferences.
- Enable Continuous scrolling during playback. Drag the larger white ruler square with a finger, including its padded border; the view anchor changes, never transport timing. Drag to both edges (existing 5–95% range), release, restart and verify the saved anchor. Check seek/range gestures elsewhere, disable the setting, and try a two-finger pinch.
- Import a long filename (also Japanese/emoji); check Playlist header, rack title, preset and module label at narrow widths. Only display labels are shortened; names/project data and clip labels remain intact.
- Open a partial-width and full-width rack/module interface: the top-right BanaForge menu button and its hit target must be absent. Close the rack: it returns and opens Main menu normally.
- Compare responsiveness and audio stability with indicators off/on in the same project. The implementation adds no polling timer or audio processing; real low-end-device timing and touch feel still require this manual check.


## Playlist bar-line display settings

The Playlist's visual timeline grid is independent of the musical snap grid. The default bar-line budget is 24; the
numeric setting accepts 1–256. `Adaptive` suppresses major bars when the visible bar spacing would be under 1 U,
while `Always` keeps major bars visible and still respects the configured line budget. Beat lines are secondary and
only appear when there is at least 0.6 U between them. No half-beat or smaller subdivisions are drawn in the Playlist.

On Android, open SETTINGS → Edit → Playlist grid and verify:

- Set the line limit to 1, 24, then 256; zoom and scroll across short and long projects. Major bars never exceed the
  requested visible-line budget, including at the widest and closest zoom levels.
- At a very wide zoom, compare `Adaptive` and `Always`: Adaptive may hide major bars when they would crowd together;
  Always retains a decimated set. At a close zoom, both show clear major bars, with thinner beat lines between them.
- Change the musical snap/grid size independently. The visual bar and beat lines must not start following snap
  subdivisions, and clip movement, snapping, MIDI note positions and project data must remain unchanged.
- Close and reopen the app and project. Both the numeric limit and visibility choice must persist.
- Compare portrait/landscape and supported UI scales. Check that the major bar lines remain visually stronger than
  beat lines and that there are no dense subdivisions at extreme zoom.
