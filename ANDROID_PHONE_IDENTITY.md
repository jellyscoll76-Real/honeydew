# Bana Forge identity check — Android phone only

## Current facts

- The owner has confirmed **installed v0.5.32** in Android Settings. This is user-confirmed version text, not a remotely measured versionCode/certificate.
- Its **installed public signing-certificate fingerprint is still unknown**. Different CI builds can share the same version name.
- No original private key has been recovered. `android/RELEASE_HOLD.md` remains active. No new Bana Forge or custom diagnostic APK was built, signed or distributed for this check.
- Do not uninstall, reinstall, overwrite, clear storage or change permissions/settings of Bana Forge.

## Recommended method: AppVerifier (Android 9 or newer)

AppVerifier is an existing third-party open-source signing-certificate viewer. It is **not Bana Forge**, not an update and not a playtest build. Its separate package is `dev.soupslurpr.appverifier`. Installing this separately named app is not an in-place update to Bana Forge.

Use only the maintainer's official download:

- [Official release page — AppVerifier 13](https://github.com/soupslurpr/AppVerifier/releases/tag/13)
- [Direct existing AppVerifier-13.apk download](https://github.com/soupslurpr/AppVerifier/releases/download/13/AppVerifier-13.apk) (about 2.4 MB)

The maintainer recommends Accrescent as its preferred distribution route; the official README links it: [AppVerifier download guidance](https://github.com/soupslurpr/AppVerifier#download). If you already have that trusted store, you can use it rather than allowing browser APK installation. Do not use random APK mirrors. Downloading through HTTPS from the maintainer is a supply-chain trust choice, not an independent binary audit. Do not try to establish AppVerifier's authenticity by having it verify itself.

### Simple steps

1. Check your Android version under **Settings → About phone → Android version**. AppVerifier 13 needs Android 9+. If your phone is on Android 8 or installation fails, stop and use the alternative section below; do not force installation.
2. Download `AppVerifier-13.apk` from the official link. Open it in your Downloads list. The installer must say **AppVerifier**, never Bana Forge/BanaForge. If it proposes updating/replacing Bana Forge, **cancel**.
3. If Android asks, allow **Install unknown apps** temporarily for the browser/file app you used. This permission is only to install AppVerifier. Disable it again after installation. If a security or organization policy blocks installation, do not disable protections/bypass policy; use an already available trusted store/inspector or stop and report the restriction.
4. Open **AppVerifier**. Review its privacy/license screen if shown. You do not need root, Shizuku, wireless debugging, Accessibility, device administrator, all-files access, photos or project access. Do not grant those for this task. Stop if a download unexpectedly asks for them.
5. Tap **App List**, search for **Bana** or `dev.daijro.banaforge`, then tap the Bana Forge entry. Use the installed-app list, **not Verify APK File**, for this first check.
6. Confirm the package shown is exactly **`dev.daijro.banaforge`**. The long colon-separated hexadecimal line(s) are the **SHA-256 signing-certificate hash(es)**.
7. Tap **Copy Verification Info**, return to this chat and paste it. Or use **Share Verification Info** to a private destination you choose. A cropped screenshot of only Bana Forge's package/hash panel is also fine. Do not share a full installed-app list or any project data.

You may see an unknown/not-in-database status. Bana Forge is not assumed to be in AppVerifier's internal database; that status alone is not a finding about whether your installation is malicious or damaged. We need the displayed public hash, not a green database badge.

AppVerifier's share text contains the package and certificate hashes, **not versionName/versionCode**. You have already supplied versionName 0.5.32. Do not label the expected code 5032 as device-confirmed unless an inspector actually displays it. More than one hash can represent signer history or multiple signers; send all displayed hashes and do not guess which is current.

### What to paste

```text
Version shown in Android Settings: 0.5.32
Selected from installed App List: yes
Package: dev.daijro.banaforge
Signing certificate SHA-256: [paste the actual result here]
```

Do not paste the example/expected hash below as if it was measured. Upper/lower case and colon separators do not matter.

The previously delivered 0.5.32 APK from run **45012769205** had:

```text
2B:F9:DC:96:90:05:46:73:62:DD:2F:9A:80:D4:D4:3E:62:C2:AC:36:8C:71:8B:A7:8F:9C:E2:3D:E6:EC:A1:26
```

- If the installed hash matches, it belongs to that audited signing identity (not proof of byte-for-byte identical APK content).
- If it differs, keep the app installed. It can still display 0.5.32 while belonging to another disposable signing identity. Record the actual result; do not alter the release pin or install an old APK to make it match.
- Neither result recovers a private key or proves an update can install.

## Why this can be read-only

Android's PackageManager exposes public installed-package metadata. A narrowly scoped custom diagnostic could declare a `<queries>` entry for `dev.daijro.banaforge`, read `PackageInfo.versionName` and `longVersionCode`, and on Android 9+ use `GET_SIGNING_CERTIFICATES` / `SigningInfo.apkContentsSigners`, hashing each public certificate's DER bytes with SHA-256. Signing history should be separately labelled. Android 8 needs the legacy public `GET_SIGNATURES` API. None of this requires the target's private keys, passwords, projects or sandbox contents.

A custom app is therefore technically feasible, but would itself need building and signing. **No custom diagnostic or replacement signing key is needed or generated here**: the existing AppVerifier provides the required missing certificate observation. The existing Bana Forge distribution hold is untouched.

Reviewed AppVerifier tag **13** source:

- [Installed App List](https://github.com/soupslurpr/AppVerifier/blob/13/app/src/main/kotlin/dev/soupslurpr/appverifier/ui/AppListScreen.kt): installed user packages and `GET_SIGNING_CERTIFICATES`.
- [Hash computation](https://github.com/soupslurpr/AppVerifier/blob/13/app/src/main/kotlin/dev/soupslurpr/appverifier/ui/VerifyAppViewModel.kt): SHA-256 over public signature bytes; for a single signer it displays certificate history, and for multiple signers the APK signers.
- [Copy/share panel](https://github.com/soupslurpr/AppVerifier/blob/13/app/src/main/kotlin/dev/soupslurpr/appverifier/ui/VerifyAppScreen.kt): explicit package/hash text only.
- [Manifest](https://github.com/soupslurpr/AppVerifier/blob/13/app/src/main/AndroidManifest.xml): declares installed-package visibility, not private-project/file access. This is source review, not independent inspection of every merged permission or reproducible verification of the released binary.
- [Build configuration](https://github.com/soupslurpr/AppVerifier/blob/13/app/build.gradle.kts): minSdk 28 (Android 9).

Installed-app visibility is distinct from app-data access. The diagnostic may enumerate the installed app list locally; that is why the minimum shared output is only Bana Forge's package/hashes. Ordinary sandbox protections do not normally hide the public signing certificate when the package is visible in the same profile.

## If AppVerifier cannot find it or cannot be installed

1. **Profile check:** AppVerifier and Bana Forge must be in the same Android profile (personal/work/private space/secondary user). Use a diagnostic installed in the same profile only if you are authorized. Do not move, reinstall or reset Bana Forge to change profiles. Organization restrictions may require the administrator's help.
2. **Alternative inspector, still phone-only:** the existing open-source **App Manager** documents a **Signatures** tab with certificate **SHA256** values and an App Info page with version metadata. Official release: [App Manager v4.1.1](https://github.com/MuntashirAkon/AppManager/releases/tag/v4.1.1). Use **No root** mode, decline optional elevated/storage/usage permissions and view only Bana Forge's information/Signatures tabs. This is a powerful general manager, **not inherently a read-only app**: do not use uninstall, clear data/cache, freeze/disable, permissions, components, signing or backup/restore actions. If setup cannot proceed without elevated access, stop rather than grant it. [Official signature-tab documentation](https://github.com/MuntashirAkon/AppManager/blob/v4.1.1/docs/raw/en/pages/app-details-page.tex) explains the SHA256 field. We have not device-tested this route.
3. **If installed-package inspection is blocked but a public APK copy is available:** AppVerifier's **Verify APK File** can inspect a local APK selected with the system file picker. Use only an APK exported from the **current installation** by an already trusted, public-APK-only sharing/extraction feature, if your phone provides one. Do not grant access to project folders. Do not use a general data-backup or restore operation. Do not install the exported file. A store URL/share link is not an APK. A previously downloaded APK may differ from what is installed, so its result **does not confirm the current installation's fingerprint**. Clearly state the source of any file inspected.
4. **If the device blocks both inspectors and public-APK export:** there is no guaranteed safe Android-only workaround on every locked-down phone. Keep Bana Forge untouched and report the phone model, Android version and the non-sensitive restriction message. Built-in Settings confirming 0.5.32 is useful but is not the missing fingerprint. We will not bypass policy, root the device, unlock the bootloader or ask for an uninstall.

## Private-key recovery remains separate

Priority is the original key matching the **actual installed 0.5.32 certificate**, not just the version name. The matching key is not inside the APK and another Android app cannot extract it. Previous CI generated it on a hosted runner, outside the phone, and deleted its temporary copy.

Remaining possible sources are independently saved originals: developer computer backups, removable/offline media, vault attachments, renamed/encrypted archives or a separately configured signing service. They are not accessible from this session. No new recovery evidence or key was found by researching Android diagnostics. The earlier connected-storage filename searches found no candidates; that does not establish that every offline or arbitrarily named backup is absent.

On the phone you may privately use **Files** search for existing `.jks`, `.keystore`, `.p12` or `.pfx` backup filenames, including Honeydew/Bana Forge-era downloads. Do not open, send, upload or delete a candidate or share its password. Filename matches are not certificate matches. Report only that a candidate backup exists; it then needs a separately planned secure local/vault validation and recovery process. The known fallback settings cannot recreate the random RSA key.

If no matching private key survives, ordinary in-place updates for this identity cannot be repaired merely by raising the version code or generating another key. Migration stays conditional on tested preservation, never on uninstalling the only installation.

## Parallel phone-only backup precautions (not a completed migration)

Keep the old app and its files intact. Save current projects; where the installed version supports it, export each as **project ZIP → Send** to a durable private destination such as user Documents or a trusted storage provider. Confirm the receiving app saved a real file outside `Android/data/dev.daijro.banaforge/`. Keep two copies where possible. Do not overwrite the originals or import back into the only installation as a test.

A project ZIP is not a complete library/settings backup. Preserve original recordings, unused samples, presets/racks and their dependencies, voicebank archives/folders and any required Original Content separately where existing supported actions allow. Record supported settings/routing with screenshots; no general automatic settings exporter currently exists. Modern Android may block browsing app-specific storage, and a newly signed app cannot read the old sandbox. If an important asset cannot be exported, stop rather than claim a full backup or uninstall to proceed.

Actual restoration still requires disposable copies and a separate test destination. No user migration tests have been performed. Needed implementation remains whole-library inventory/completeness reporting, durable streaming export, portable settings/routing export/import, dependency checks and staged non-overwriting restore, as detailed in `ANDROID_BACKUP_RESTORE_PLAN.md`. A future exporter cannot retroactively run inside this old installation without a compatible signing key.
