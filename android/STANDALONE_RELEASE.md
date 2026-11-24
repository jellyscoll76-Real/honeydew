# Standalone Android release preparation

This is a separate app lineage, not an update to `dev.daijro.banaforge`.

- **Permanent application ID:** `dev.daijro.banaforge.standalone` (Gradle release build type `standaloneRelease`).
- **Display name:** BanaForge Standalone.
- **Rust persistence namespace:** selected with the `standalone-android` Cargo feature so eframe settings do not reuse the legacy package namespace.
- **Updater isolation:** the standalone build never checks/offers/installs the legacy APK feed; sound-library version checks remain separate. A standalone APK update feed must be added and independently verified before self-updates are enabled.
- **Provider authorities:** use `${applicationId}`, therefore the standalone Documents and Update providers do not collide with the installed app.
- **Signer:** only `BANAFORGE_STANDALONE_*` variables are used by the standalone signing config and preparation path. It cannot fall back to `ANDROID_KEYSTORE_*` or the legacy certificate pin.
- **Build entry point:** `tools/android-dist.sh --standalone-release --abi all`. It still stops while `android/RELEASE_HOLD.md` exists; there is no bypass. `.github/workflows/android-standalone.yml` has the same hold check and only offers a manually dispatched, verified build artifact.

## Key creation and custody — owner-controlled secure environment only

No key was generated for this preparation. Do not create the production key in a disposable runner, repository, or shared workspace. An authorized owner should create it once on a trusted, owner-controlled system using an interactive password prompt, for example:

```sh
keytool -genkeypair -alias banaforge-standalone \
  -keyalg RSA -keysize 3072 -validity 10000 \
  -keystore /secure/offline/path/banaforge-standalone.jks
```

Keep the private keystore and its separately protected passwords in an encrypted, independently restorable vault/offline backup. Do not put key bytes or passwords in Git, logs, workflow outputs or chat. Record and compare the public certificate SHA-256 fingerprint; review it, then replace the `UNSET` contents of `standalone-release-certificate.sha256` with the lowercase 64-hex digest. Configure these separate GitHub Actions secrets through secure repository settings:

- `BANAFORGE_STANDALONE_KEYSTORE_BASE64`
- `BANAFORGE_STANDALONE_KEYSTORE_PASSWORD`
- `BANAFORGE_STANDALONE_KEY_ALIAS`
- `BANAFORGE_STANDALONE_KEY_PASSWORD`

The preparation tool writes a mode-0600 temporary keystore outside the repository, validates its public certificate against the standalone pin, refuses to overwrite an existing path, and removes the temporary copy on script exit. CI removes it with the ephemeral runner. The public pin does not contain or reconstruct the private key.

## Artifact gates

`verify_android_standalone.py` checks the actual signed APK's package ID, Cargo version/versionCode, certificate digest and requested ABIs. The initial baseline is explicitly `awaiting-first-verified-release`; after the first reviewed release, preserve its generated identity report as the next baseline so every successor must keep the same package/signer, increase versionCode and retain ABIs. The distribution script also verifies APK signature, 16 KB alignment, bundled sound payload and native libraries. Unit tests exercise standalone secret isolation and identity/version/ABI rejection.

These checks are metadata/package evidence, not device evidence. Before removing the release hold or giving out an APK, the owner must review the documented migration limitations and complete a copy-only import rehearsal on a spare device. Keep the old installation and its data untouched. Verify project ZIP import, referenced-asset playback, missing/excluded-asset reporting and persistent settings behavior. The current project ZIP route is not a full user-library/settings migration; see `ANDROID_BACKUP_RESTORE_PLAN.md`.

## Current blockers

- `standalone-release-certificate.sha256` is deliberately `UNSET`; the permanent private key, public pin and secure backup are not yet established.
- No standalone APK or download link exists.
- The release hold remains active and blocks both local distribution and the standalone workflow.
- This workspace has no Android device evidence. Java/Android build tools and device tools must be available in the secure build environment; this session does not handle the production key.
