# APK distribution hold

Do not build or distribute another APK until the owner has reviewed
`ANDROID_KEY_RECOVERY_REPORT.md` and approved a verified safe update/migration path.

No matching original private signing key has been recovered. Never generate a
replacement as an update fix. Keep existing installations and data intact.

The normal distribution script and both APK-producing CI jobs stop while this
file exists. There is deliberately no command-line bypass. Clearing this hold
requires a reviewed change and a controlled private validation plan, not merely
adding new signing secrets. Before public delivery, retain device evidence for
the exact candidate APK: in-place updates, project/settings retention, both ARM
ABIs where available, and cancelled/failed update behavior. Metadata tests are
not device evidence. See the recovery report for the separate migration case.
