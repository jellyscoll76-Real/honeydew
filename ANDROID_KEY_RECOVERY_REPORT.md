# Bana Forge — signing recovery and migration assessment

Status: **release hold; no key recovered; no new APK built or distributed**.

This report supplements [ANDROID_UPDATE_SAFETY.md](ANDROID_UPDATE_SAFETY.md). A displayed version alone does not identify the installed signer. The owner's device has not been inspected.

## 1. Recovery investigation and limits

| Location checked | Result and scope |
| --- | --- |
| Repository history | Expanded the shallow checkout to the available full history (333 reachable commits at investigation time). No `.jks`, `.keystore`, `.p12` or `.pfx` path found in that history or current working tree. This is a filename/history check, not proof that no arbitrarily named or encrypted backup exists anywhere. |
| Historical playtest build configuration | Changes from `0baf137` through `8c33db1` generated a temporary release key when signing inputs were absent. The configurations used GitHub-hosted Ubuntu runners. The actual delivered 0.5.32 source is `b75ea8d`; its workflow removed the temporary keystore before uploading only `target/banaforge-android-*.apk`. There is no configured backup/upload of that key. |
| GitHub Actions signing secrets | Direct metadata listing returned HTTP 403 (integration lacks permission), not an empty inventory. The successful public-identity audits 45033530505 and 45034001173 found all four signing inputs absent in the build job. Secret values are not retrievable through the secrets API. An authorized owner must still inspect any vault, organization secrets not granted to this repo, or differently named credentials. |
| Environment-scoped storage | Repository environments API returned no environments. No environment-attached signing store was identified. This does not inspect external vaults. |
| GitHub releases / Actions artifacts | No GitHub Releases listed. Enumerated 260 Actions artifact records: 127 APK artifacts; the others are public audits, formatting, measurement or test logs. None is named as a signing-key backup. The delivered-build upload configuration excludes the temporary key. Not every historical ZIP was downloaded or inspected; artifact names alone cannot rule out a deliberately renamed backup. |
| Actions caches | Listed 24 Rust build caches. The release key lived under `RUNNER_TEMP`, outside configured Rust build/cache paths, and was removed. No configured key cache or backup found. Cache payloads were not downloaded. |
| Connected Google Drive | Filename-only queries for `.jks`, `.keystore`, `.p12`, `.pfx`, `banaforge`, `Bana Forge`, `honeydew`, `signing`, and `release-key` returned no matches, with no further result pages. No file contents downloaded. |
| Connected Dropbox | Active filename queries for `keystore`, `jks`, `banaforge`, and `honeydew` returned no matches or additional pages. Deleted entries, offline backups and arbitrarily named archives are not ruled out. No file contents downloaded. |
| Original developer machine / offline vault / store-managed key | Not accessible or established from this checkout. No claim these were searched. For these two audited CI APKs, an unrelated original developer key would not help: the recovered certificate must match the installed APK exactly. |

No private key was generated, restored to repository secrets or uploaded. No backup contents or secret values were retrieved from secure stores. The old APK's public certificate cannot reconstruct its private key. The old fallback's fixed configuration does not make its randomly generated RSA private key reproducible.

Additional evidence: [audit 45034001173](https://github.com/jellyscoll76-Real/honeydew/actions/runs/45034001173) passed; [Android run 45034001178](https://github.com/jellyscoll76-Real/honeydew/actions/runs/45034001178) stopped at **Prepare existing release signing key (no fallback)** before native compilation. The unrelated broad CI run 45034001184 failed `cargo fmt --check`; it is not a passing whole-repository validation and is not evidence about key recovery.

### Remaining owner-only recovery actions

- Identify the installed package's public signer (see below); do not provide passwords or key bytes in chat.
- Check the developer's original machine, encrypted backups, password-manager/vault attachments, CI credentials under other names, and any authorized organization signing service. A local developer keystore is useful only if its public certificate matches.
- If a candidate exists, the owner should validate it privately and restore the matching original key through the four secure repository secrets described in ANDROID_UPDATE_SAFETY.md. Keep an independently restorable encrypted backup. Never change the public pin just to accept an unrelated key.
- A GitHub-hosted job's deleted temporary filesystem is not a known available recovery source. This investigation found no supported way to recover those removed key files.

## 2. Which installed identities can update normally?

| Exact audited APK | Certificate SHA-256 | Normal successor today |
| --- | --- | --- |
| 0.5.31, code 5031, run 44972042580 | `fd9f7b1dcb2edaf4aca2c4abca9167a3ff55416dad2849cebee919ac0c5a220f` | None demonstrated; requires this identity's private key. |
| 0.5.32, code 5032, run 45012769205 | `2bf9dc969005467362dd2f9a80d4d43e62c2ac368c718ba78f9ce23de6eca126` | None demonstrated; requires this identity's private key. Current safeguards pin this public identity only. |

Both APKs use `dev.daijro.banaforge` and contain both `arm64-v8a` and `armeabi-v7a`. A recovered 0.5.32 key would permit candidate updates for matching 0.5.32 installs, not for the differently signed audited 0.5.31 install. The converse holds for a recovered 0.5.31 key; supporting that lineage requires explicitly reviewed baseline/pin handling, not blindly changing the existing pin. Other builds with the same version text may have different signers.

With Android platform/build tools on an owner-controlled computer, use `adb shell pm path dev.daijro.banaforge`, then `adb pull` the returned base APK path to a local inspection folder. Run `apksigner verify --print-certs installed.apk` and `aapt dump badging installed.apk`. These inspect public identity; they do not extract a private signing key or app data. Device authorization is required. Sharing the public SHA-256 certificate fingerprint and versionCode is sufficient; do not send private credentials. No device was connected for this report.

## 3. Is migration necessary?

**Conditional, not yet executed:** if no matching private key can be recovered, ordinary same-package in-place updating is unavailable for that installed identity. A different package ID would be a separate installation, not a repaired update. No such change is made or authorized by this report.

The least destructive migration candidate is to keep the existing installation and validate exported copies on a spare device first. A separately identified successor could later allow old/new apps side by side on one device, but requires explicit owner approval, separately scoped providers/paths, a persistent signing identity and verified import behavior. A new app cannot read the old app's sandbox automatically. Do not uninstall the old app to make a mismatched APK install.

If neither a key nor a validated export route is available, stop. Do not replace the installation or promise full recovery.

## 4. Data-preservation findings from the current source

These are source-level capabilities, **not completed tests on the installed APK**. Confirm that the specific installed version exposes and successfully performs each operation.

| Data | Preservation route | Limits |
| --- | --- | --- |
| Saved song/project structure | Save `.flm`; preferably export project **ZIP**, then **Send** via Android share sheet to storage outside the app. | The ZIP contains a newly serialized `.flm`, not a raw copy of every project/version. Verify notes, clips, racks, automation, envelopes, tempo, metadata and VocalSynth edits after import; do not assume cross-version fidelity. |
| Referenced user samples/programs | ZIP export gathers readable referenced user files through the sample-cache reader, including supported DW/SFZ dependency traversal. | Not the entire library. Missing/unreadable files can be omitted. Plain/external references may not be portable. A successful ZIP write is not a completeness guarantee. |
| Bundled / Original Content | Restore the same compatible content separately where available and licensed. | Intentionally excluded from project ZIPs, even when reachable by a user path. The exporter reports excluded content; do not assume a soundless APK or fresh install has those sounds. |
| Unused samples, presets, racks, SoundFonts, MIDI, recordings, rendered audio, project versions/recovery files, recently deleted material | Copy/export separately while the old app remains installed, using supported Send actions and/or a verified computer copy of the user library. | ZIP export is per-project, not a whole-library or deleted-file backup. Save current recordings/unsaved edits first. Check any nested dependencies and license restrictions. |
| Voicebanks | Keep/recover the original licensed bank archives/folders separately; reimport and verify aliases/paths/audio. | No evidence that project ZIP export is a complete voicebank backup. Do not infer playable bank samples from stored metadata. |
| User settings | Record screenshots/values and reapply supported controls manually, then restart and verify persistence. Themes have a separate export/import path. | No general settings export/import UI found. Project ZIPs do not include settings. Audio/MIDI devices, permissions and absolute paths require fresh validation. Disabled/stored-only controls do not gain functionality through migration. |
| Internal app state | Preserve by retaining the old installation. | `storage.json` is under the internal app data folder (`BanaForge`); ordinary new apps/file pickers cannot read it. `allowBackup=false` remains unchanged. No reliable ordinary release-app ADB backup/run-as route is established. Do not promise automatic transfer of private preferences, recent-session state, undo history or caches. |

Source references: `src/project_files/export.rs` (`archive`, `user_files_of`), `archive.rs` (single-song ZIP import/relinking), `native.rs` (private-library save and mobile share), `src/ui/views/save_dialog.rs` (Send), `src/ui/main_menu/library.rs` (user folder), `src/settings.rs` (internal settings store).

The normal Android user library is `Android/data/dev.daijro.banaforge/files`; some installations may still use an internal fallback if migration/storage creation failed. **App-specific external storage is not an independent backup and may be deleted on uninstall.** Modern Android/USB file access varies by device. Verify any copy from a computer; do not rely on a file manager being able to browse that folder. A share-sheet temporary URI is not itself a durable backup: the receiving app must actually save the file outside Bana Forge's app-specific storage.

## 5. Proposed migration rehearsal — no uninstall, no new APK yet

1. Keep the old installation intact. Save all unsaved work. Inventory projects, alternate versions, samples/banks, presets and supported settings. Photograph/screenshot settings, including Auto-Save, audio mode/buffer, MIDI, waveform/UI options, latency compensation and custom themes. Do not change audio settings just for backup.
2. Export each project as ZIP with **Send**, saving an independent copy to user-controlled Documents/Downloads or another durable location, not `Android/data/...`. Also keep `.flm` files, original assets, and optional WAV references/stems for audible comparison; WAV/MIDI alone cannot preserve editable racks/effects/automation. Give backup archives unique names because opening an archive reuses its extraction folder name.
3. Verify the saved copies exist independently and transfer them to a second device/computer. Keep two copies. Check ZIP integrity, one expected `.flm`, file counts, sample dependencies, sizes and SHA-256 hashes. Android ARMv7's whole-file cap is 256 MiB (ARM64 512 MiB; web 128 MiB), with additional archive expansion limits; do not assume a large ARM64 export imports on ARMv7. Keep originals and investigate oversize/dependency issues rather than deleting anything.
4. On a spare test environment, open a **copy** of each archive through the normal importer. Never overwrite the only original. Check all warnings, missing assets, voicebank paths/aliases, track counts, notes/timing, tempo, clip edits/envelopes, instruments/effects, automation and settings. Save, close, reopen and compare playback/render references. Separately imported presets/voicebanks/themes must also be validated. Record what could not be transferred.
5. Only after those checks and owner approval can a new-signing migration release be planned. Keep the old installation/data until every important project has been confirmed. This report does not authorize an uninstall or package-ID change.

## 6. Future release gates and outstanding testing

The PR #5 missing-key, public-certificate, package, version and ABI safeguards remain. They passed 10 policy tests plus 3 native-verifier tests locally; audit 45034001173 also passed its policy step. A checked-in `android/RELEASE_HOLD.md` blocks normal APK distribution builds while recovery/migration is unresolved. It is not a substitute for device testing.

Before a release is downloadable:

- Confirm the installed public identity and recover the matching private key, or explicitly approve and validate a separate migration lineage. Back up the persistent signing key securely and verify restoration privately.
- Preserve the package ID for normal updates; verify the final APK's certificate, higher versionCode and real native code for both ABIs. Use one persistent signing key per accepted lineage and reject absent/mismatched credentials. Do not reuse a versionCode for a newly distributed release.
- Use a controlled private test build process first. The current upload workflow's public identity report explicitly says `device_install_tested=false`: it does **not** prove or automate real Android project/settings retention. Define and review the device evidence before clearing the distribution hold; do not mark metadata tests as device evidence.
- Perform predecessor → successor → next-successor installations with saved projects/settings on ARMv7 and ARM64 where available. Run normal `adb install -r`/PackageInstaller updates with no uninstall, storage clear, downgrade or signature bypass. Verify successful relaunch, saved projects, sample availability, settings persistence and playback after each step.
- Test installer cancellation, corrupt/truncated APK rejection, staging interruption/failure and subsequent old-app data access on expendable test devices. Keep public logs/test results without private credentials.
- If migration is needed, complete the export/import rehearsal and a documented per-data-type retention checklist first. Test failed/cancelled imports without touching the originals.
- Attach results for the exact candidate APK hash and predecessor, Android/device/ABI details and next-successor update. Require human review for every release; the current CI does not yet supply this physical-device gate automatically.

No build was dispatched for this investigation. No replacement key, release APK or new downloadable APK link exists. Historical APK downloads must not be advertised as the signing fix or reinstalled over a differently signed app. Requested playtest delivery remains blocked until a safe path is verified.
