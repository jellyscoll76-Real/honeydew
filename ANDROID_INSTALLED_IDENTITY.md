# Identify the installed Bana Forge safely

**Current result: the owner confirmed v0.5.32 in Android Settings; its installed public certificate and numeric versionCode are still unmeasured.** This workspace has no connected Android device/ADB toolchain. The certificate cannot be inferred from the last download, project files, APK filename or repository version. Do not reinstall an old APK to check it.

**Only have an Android phone? Use [the phone-only certificate guide](ANDROID_PHONE_IDENTITY.md)** instead of the computer steps below. It links an existing diagnostic APK, not a new Bana Forge build.

The APK distribution hold remains active. This procedure builds/installs nothing, accesses no app project/settings directories, and needs no private key or keystore password.

## Quick version check on the phone

Open Android **Settings → Apps → Bana Forge → App details** (the location varies by phone) and note the displayed version. This can distinguish 0.5.31 from 0.5.32 where the version is shown, but does **not** establish the signing certificate. A screenshot is optional; crop personal information.

## Public-certificate check using an owner-controlled computer

Requirements: Python 3.9+, Android SDK **Platform Tools** (`adb`) and SDK **Build Tools** (`apksigner`, `aapt`). Use the official Android SDK/Android Studio tools on your computer. No APK extraction helper app, root, bootloader unlock, `run-as`, uninstall or app-data backup command is required.

1. Download `tools/identify_android_install.py` from this branch or use your checkout. It is standalone and has no Python package dependencies.
2. Enable USB debugging temporarily on your phone, connect it by USB and approve **only your own trusted computer**. Do not authorize an unknown computer. Use the personal/profile installation you intend to inspect; managed/work-profile or secondary-user installations need separate review.
3. Run the command below, replacing the SDK directory with your actual local path (not typing the example literally):

```sh
python3 tools/identify_android_install.py --device \
  --tools "/your/Android/Sdk/build-tools/36.0.0" \
  --adb "/your/Android/Sdk/platform-tools/adb" \
  --report banaforge-public-identity.json
```

On Windows, use `py -3` instead of `python3`, put the script path on one line, and pass the Windows paths in quotes; `--adb` should point to `adb.exe`. The script locates `apksigner.bat` and `aapt.exe` in `--tools`. Other installed SDK Build Tools versions may be selected if they can verify the APK; build-tools 36.0.0 matches the CI audit environment.

If several authorized devices are attached, disconnect the others or use `--serial` with the device ID from a **local** `adb devices` listing. The JSON excludes device serials and local/remote file paths. It refuses to overwrite an existing report; choose a new filename for another inspection.

### Exactly what the script does

- `adb devices` to select an authorized device, stopping on ambiguity.
- `adb -s … shell pm path dev.daijro.banaforge` to ask for public installed APK paths.
- `adb -s … pull <base.apk> <temporary computer file>` to copy **only the installed public base APK** off the device. It does not copy user data or change the installed app. The temporary copy is deleted when inspection finishes normally or raises a handled error; a forcibly killed computer process may leave the public APK in its temporary folder.
- Local `apksigner verify --print-certs` and `aapt dump badging` to verify the base APK signature and read version metadata; SHA-256 over the copied public APK.

The script contains no install, uninstall, clear-data, key-generation, app-launch, root or private-storage commands. It is read-only with respect to the installed application. USB authorization/debugging is a separate phone security setting: turn debugging off/revoke the computer afterward if you do not need it.

Multiple/rotated signers and unexpected package/path layouts stop for review. Split APKs are counted, not copied/validated; `base_apk_abis` covers only the base APK and is not a universal-packaging certification. The historical delivered APKs were single universal APKs. A failure does not justify reinstalling or clearing anything.

### What to share back

Only these public report fields are needed:

- `package`
- `version_name`
- `version_code`
- `certificate_sha256`
- `source` and `installed_version_confirmed`
- `matching_audited_identity_run`

The report contains no key bytes, passwords, device serial, project names, settings, or project contents. An APK certificate is public; it cannot reveal/recover the signing private key. Keep your APK/report local unless you want to share the public fields above.

| Audited version | versionCode | Public certificate SHA-256 |
| --- | --- | --- |
| 0.5.31 | 5031 | `fd9f7b1dcb2edaf4aca2c4abca9167a3ff55416dad2849cebee919ac0c5a220f` |
| 0.5.32 | 5032 | `2bf9dc969005467362dd2f9a80d4d43e62c2ac368c718ba78f9ce23de6eca126` |

Other builds of the same version can have different certificates. A matching version/code/certificate identifies the audited signing lineage, not necessarily byte-for-byte identical APK content. No identity match establishes that its private key is available or that an update has passed a device test.

## If you only have a downloaded APK

```sh
python3 tools/identify_android_install.py --apk /path/to/copy.apk \
  --tools /path/to/Android/Sdk/build-tools/36.0.0
```

This inspects a local public APK and explicitly reports `installed_version_confirmed=false`. **It cannot answer what is currently installed on the phone.**

## Original private-key recovery still possible only from existing storage

No matching key has been found. Previous checks are in `ANDROID_KEY_RECOVERY_REPORT.md`. The original developer computer, removable disks, encrypted offline archives and vault attachments are not mounted or remotely accessible here. No new evidence from those locations is available. The owner/developer must check them privately; do not send passwords or keystores through chat.

Search those locations for release/build backups and keystore files (`.jks`, `.keystore`, `.p12`, `.pfx`, including Honeydew/Bana Forge-era names). Include offline media and the backup histories of renamed/deleted files. Do not assume a file is useful merely because of its name. An ordinary Android debug keystore or unrelated developer release key cannot sign updates to these audited CI identities.

A trusted owner can inspect a candidate's **public certificate** locally with `keytool -list -v -keystore <candidate>`; let keytool prompt locally for its password, never put a password on the command line or send it here. For a multi-entry keystore inspect the intended signing entry. Compare the certificate's SHA-256 fingerprint (case/colon formatting does not matter) with the installed APK result. Do not export private keys or modify the candidate. Certificate equality is necessary; a signing trial in a private environment is still required to confirm access to the usable private-key entry.

If the certificate matches, preserve the original, make and privately verify an encrypted backup, and have the authorized repository owner restore the existing four secure signing secrets. Do not remove the release hold yet. If it does not match, stop; do not weaken the pin to accept it. A recovered 0.5.31 key cannot update the differently signed audited 0.5.32 installation, or vice versa.

If no independent backup survives, the deleted ephemeral CI key cannot be reconstructed from the APK, public certificate, known fallback configuration or a regenerated key. Migration then remains conditional on validated exports, not on an uninstall workaround.
