# Android update identity — v0.5.33 preparation (release blocked)

## Verified cause in the delivered playtest APKs

The original `Android APK` workflow generated a fresh RSA release keystore whenever any of four signing secrets was missing, signed the APK, and removed the temporary keystore at the end. A successful signature/ZIP/native-library check did **not** establish continuity with the previously delivered signer. The tagged-release workflow already required secrets, but the playtest workflow did not.

[Audit run 45033530505](https://github.com/jellyscoll76-Real/honeydew/actions/runs/45033530505) downloaded and inspected the actual previously delivered APK artifacts using `apksigner verify --print-certs` and `aapt dump badging`:

| Field | Delivered 0.5.31 | Delivered 0.5.32 |
| --- | --- | --- |
| Package | `dev.daijro.banaforge` | `dev.daijro.banaforge` |
| versionCode | 5031 | 5032 |
| versionName | 0.5.31 | 0.5.32 |
| Native ABIs | ARM64 + ARMv7 | ARM64 + ARMv7 |
| Certificate SHA-256 | `fd9f7b1dcb2edaf4aca2c4abca9167a3ff55416dad2849cebee919ac0c5a220f` | `2bf9dc969005467362dd2f9a80d4d43e62c2ac368c718ba78f9ce23de6eca126` |
| Source run | 44972042580 | 45012769205 |

All four signing-secret inputs were absent in the audit's build environment. The certificates differ: **these two delivered APKs cannot update one another in place under ordinary Android signature checks**. Their package, increasing version code and universal native packaging are not the cause of that mismatch. Other playtest builds, even of the same displayed version, may have other disposable certificates; the device's actual installed APK identity must be checked if it differs from these delivered artifacts.

## Hard blocker and data safety

The original private key is not available in the current CI environment. No secure backup was discovered or created by this task. The old workflow deleted its temporary keys; Android certificates/APKs contain the public certificate, not a recoverable private signing key. External backups may still exist and must be checked by the owner.

**Do not uninstall, clear app storage, change the package ID, replace the signing key or bypass Android's signature enforcement.** Keep the current installation and its data. There is no honest versionCode-only fix for an already installed APK signed by an unavailable private key. A replacement key is not an in-place update solution. If the matching private key cannot be recovered, the requested ordinary in-place upgrade for that installation cannot be delivered under these constraints. Any future migration decision requires separate owner approval and a verified data-preservation plan.

No project directory, preference filename, storage permission, backup setting, database, project format, migration path or audio processing code is changed by this task. `android:allowBackup` is not changed: enabling it would not repair signing continuity or retroactively recover keys/data.

## Safeguards implemented

- Remove disposable release-key fallback. Missing/partial signing configuration fails before expensive APK compilation/publication. Debug builds retain their separate `.debug` application ID and are not substitutes for release updates.
- Use the same existing-keystore validator in the playtest and tagged-release workflows. Secrets are passed through environment variables, not GitHub step outputs. Temporary copies are mode 0600, never overwrite another key, are cleaned on validation failure and at workflow completion, and are never uploaded.
- Pin the public certificate of the last delivered compact 0.5.32 APK in `android/release-certificate.sha256`. This is an identity lock, **not a newly generated key or a claim its private key exists**. Do not change the pin merely to make a different key pass. If the installed APK is a different build, establish its public identity and review the matching recovered key first.
- Verify actual packaged APK signature, package ID, Cargo version name, numeric version code and requested native ABIs on all distribution-script release builds. CI additionally checks the reviewed predecessor identity in `android/release-baseline.json`, including strictly higher code and no ABI drop. Default distribution remains universal ARMv7 + ARM64.
- Keep a durable public predecessor manifest rather than relying indefinitely on expiring Actions artifacts. After each distributed release, review its verified identity report and advance the baseline manifest to that release before the next one. Never update the baseline to unverified or merely dispatched builds.
- Keep `versionCode = major * 1,000,000 + minor * 1,000 + patch`; require minor/patch below 1000 and code in 1…2,100,000,000. The prepared maintenance version is 0.5.33 / 5033, not a released APK. Every newly distributed update must advance the version; do not relabel/reuse an old code for different release content.

## Secure recovery and backup procedure — owner action required

1. Locate the original keystore **matching the APK actually installed**. Compare its exported public certificate SHA-256 with the audited/device APK certificate. A keystore merely named `banaforge-release.jks` is not evidence of a match.
2. Do not send private keys, keystore contents or passwords in chat, Git, logs or public artifacts. An authorized repository owner must configure Actions secrets through GitHub's secure settings: `ANDROID_KEYSTORE_BASE64`, `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS`, `ANDROID_KEY_PASSWORD`.
3. Make an encrypted offline or access-controlled vault backup of the original keystore and separately protected recovery credentials. Verify restoration in a private environment by checking the certificate digest and the ability to sign a private test artifact. GitHub Secrets alone is not an exportable key backup. Record ownership/recovery access without publishing private material.
4. Once identity is restored, rerun the universal release build on this branch with `abi=all`, `build_type=release`, `include_sounds=false`. A mismatched key must still fail; do not weaken the pin to bypass it.

## Validation status and required device testing

Completed: actual old-APK public-identity audit; 10 signing/update-policy unit tests; 3 native-packaging verifier tests; shell syntax check. Unit tests cover absent/partial secrets, invalid base64, wrong certificate/password, preservation of existing key files, private-copy permissions/cleanup, repeated use of identical key bytes, package/version/ABI rejection and two consecutive metadata update transitions. **These are not installation or real-project retention tests.**

Blocked/not performed: building a correctly signed successor APK, installing over the released APK, runtime project/settings retention, a second subsequent actual installation, ARMv7/ARM64 device updates, and interrupted/failed-install retention. No emulator/device results or secure-backup completion are claimed.

After recovering the key, test with non-critical project copies on both ABIs where available:

1. Install the exact predecessor APK on a test device and record package, signer, version code and supported ABIs. Create multiple saved projects, sample references and identifiable persistent settings. Record file hashes/preferences without resetting storage.
2. Build and verify the successor with the same signing certificate and a higher code. Install normally or use `adb install -r successor.apk` (no uninstall, `pm clear`, downgrade flag or signature bypass). Relaunch and verify projects, playback, settings, permissions and storage paths.
3. Advance the reviewed baseline, bump to the next patch and repeat with another identically signed build. Do not claim two-version update reliability from the Python metadata test alone.
4. Cancel an installer confirmation before commit; test a corrupt/truncated APK and a failed download on a test device. Check that the previous installed version still opens with unchanged data. Check interruption during staging/installation using the device's PackageInstaller logs; platform rejection/rollback expectations must be verified, not assumed to be an observed test result.
5. Retain public identity/test reports for both builds, never private keys. Only deliver an APK as an in-place update after identity verification and clearly distinguish device-tested from untested compatibility.
