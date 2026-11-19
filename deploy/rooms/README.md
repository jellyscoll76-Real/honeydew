# Honeydew Rooms — Android preview

The APK does not contain a relay account or deployment. Use a Cloudflare **Workers Free** account, enable `workers.dev`, and deploy the SQLite-backed Durable Object with Wrangler from this directory:

```sh
npx wrangler deploy
```

On both Android phones, open **Collab**, enter the deployed `https://…workers.dev` origin and a display name. On the first phone select **Create room from this song**; on the second enter the room code (or read it from the shared invite link) and select **Join room**. Opening the invite link displays the code; it does not deep-link into the app. Use a project without unsaved changes on the joining phone. The preview requires the same Honeydew version and factory sound library on both phones.

Install the soundless, signed ARM64 debug build from the branch's Android APK workflow, extract it from the workflow artifact, and install it alongside the release app. Or build locally with `tools/android-dist.sh --base --abi arm64-v8a` and the Android toolchain from the root README.

## Preview limits

- One Host, one Guest, one room per client session. Guest reconnect reservation: 60 seconds. Host socket-loss recovery: 15 seconds. Room data is temporary (four hours maximum); the app, not the relay, owns the exported ZIP.
- Ordered, committed FLM checkpoints in JSON envelopes; no audio streaming. Checkpoints are capped at 1 MiB. This first implementation sends a whole checkpoint at most about three times per second rather than compact MIDI deltas. The drag lease is conservative: it fences the affected piano-roll clip and pauses whole-checkpoint commits from the other client while active.
- Files transfer separately and are verified by SHA-256 on receipt; up to 6 MiB/sample and 32 MiB/room. Only supported user media files sync. If two different files already have the same project path, the preview stops syncing rather than overwriting either file. Rename the conflicting asset before starting again. Use the existing ZIP export to include user samples.
- The opt-in monitor covers channel solos; clip/sample/rack-specific monitor gestures are not relayed by this preview. The host owns in-room save/export controls; a Guest can save their standalone copy after the room ends.
- No service credentials in the APK. The room code is an invitation, not an account; reconnect tokens are scoped to a seat. Keep the public relay limited to trusted testers until account-level admission/rate controls and two-device testing are complete. Free-plan quota exhaustion returns errors, not extra usage charges; do not enable a paid plan for this preview.

Run `node --test worker.test.mjs` for relay state-machine tests. Before distribution, also test across separate mobile networks: third-seat rejection, simultaneous note edits, airplane-mode recovery before/after 60 seconds, host departure during a pending sample download, independent playback/solo, and reopening the exported ZIP with an empty sample library.
