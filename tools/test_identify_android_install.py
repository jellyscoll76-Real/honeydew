import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from identify_android_install import (
    KNOWN, PACKAGE, base_path, choose_device, inspect_device, main,
    parse_identity, report_for,
)


class PublicIdentityTests(unittest.TestCase):
    def identity(self, version="0.5.32", code=5032, digest=None):
        digest = digest or next(key[2] for key in KNOWN if key[0] == version)
        return parse_identity(
            f"Signer #1 certificate SHA-256 digest: {digest}",
            f"package: name='{PACKAGE}' versionCode='{code}' versionName='{version}'\n"
            "native-code: 'arm64-v8a' 'armeabi-v7a'\n",
        )

    def test_both_audited_versions_are_identified_by_version_code_and_signer(self):
        for version, code in [("0.5.31", 5031), ("0.5.32", 5032)]:
            result = report_for(self.identity(version, code), "authorized_device_base_apk", 0)
            self.assertTrue(result["installed_version_confirmed"])
            self.assertIsNotNone(result["matching_audited_identity_run"])

    def test_same_version_different_signer_is_not_the_audited_identity(self):
        result = report_for(self.identity(digest="a" * 64), "authorized_device_base_apk")
        self.assertIsNone(result["matching_audited_identity_run"])
        self.assertEqual(result["version_name"], "0.5.32")

    def test_local_apk_never_confirms_installed_version(self):
        result = report_for(self.identity(), "local_apk_not_installed_confirmation")
        self.assertFalse(result["installed_version_confirmed"])

    def test_wrong_package_missing_metadata_or_multiple_signers_fail(self):
        cert = "Signer #1 certificate SHA-256 digest: " + "a" * 64
        for certificate, badging in [
            (cert, "package: name='wrong.package' versionCode='5032' versionName='0.5.32'"),
            (cert, "not package metadata"),
            (cert + "\nSigner #2 certificate SHA-256 digest: " + "b" * 64,
             f"package: name='{PACKAGE}' versionCode='5032' versionName='0.5.32'"),
        ]:
            with self.subTest(badging=badging), self.assertRaises(ValueError):
                parse_identity(certificate, badging)

    def test_device_selection_fails_closed_on_ambiguous_or_unauthorized_devices(self):
        self.assertEqual(choose_device("List of devices attached\none\tdevice\n"), "one")
        self.assertEqual(choose_device("one\tdevice\ntwo\tdevice\n", "two"), "two")
        for devices in ["", "one\tunauthorized\n", "one\tdevice\ntwo\tdevice\n"]:
            with self.assertRaises(ValueError):
                choose_device(devices)
        with self.assertRaises(ValueError):
            choose_device("one\tdevice\n", "missing")

    def test_only_one_safe_base_path_is_allowed_and_splits_are_reported(self):
        self.assertEqual(base_path("package:/data/app/~~abc/pkg-abc/base.apk\n"
                                   "package:/data/app/~~abc/pkg-abc/split_config.arm64.apk\n"),
                         ("/data/app/~~abc/pkg-abc/base.apk", 1))
        for output in ["", "package:relative/base.apk", "package:/data/app/../base.apk",
                       "package:/data/app/x;bad/base.apk", "package:/data/app/x/split.apk",
                       "package:/data/user/0/dev.daijro.banaforge/base.apk",
                       "package:/storage/emulated/0/Android/data/dev.daijro.banaforge/base.apk",
                       "package:/data/app/x/base.apk\npackage:/data/app/y/base.apk"]:
            with self.subTest(output=output), self.assertRaises(ValueError):
                base_path(output)

    def test_device_commands_are_read_only_and_temporary_apk_is_cleaned(self):
        commands = []
        copies = []

        def run(command):
            commands.append(command)
            if command == ["adb", "devices"]:
                return "List of devices attached\nlocal-device\tdevice\n"
            if command[3:] == ["shell", "pm", "path", PACKAGE]:
                return "package:/data/app/x/base.apk\n"
            if command[3] == "pull":
                destination = Path(command[-1])
                copies.append(destination)
                destination.write_bytes(b"public APK fixture")
                return ""
            self.fail("Unexpected device command")

        with patch("identify_android_install.run_text", side_effect=run), patch(
            "identify_android_install.inspect_apk", return_value=self.identity()
        ):
            result, splits = inspect_device("adb", Path("unused-tools"))
        self.assertEqual(len(commands), 3)
        self.assertEqual(result["version_code"], 5032)
        self.assertEqual(splits, 0)
        self.assertFalse(copies[0].exists())
        self.assertNotIn("local-device", json.dumps(report_for(result, "authorized_device_base_apk")))

    def test_report_never_overwrites_an_existing_file(self):
        with tempfile.TemporaryDirectory() as folder:
            destination = Path(folder) / "report.json"
            destination.write_text("keep original")
            with patch("sys.argv", ["identify_android_install.py", "--apk", "copy.apk", "--tools", "sdk",
                                     "--report", str(destination)]), patch(
                "identify_android_install.inspect_apk", return_value=self.identity()
            ), patch("sys.stderr"):
                self.assertEqual(main(), 1)
            self.assertEqual(destination.read_text(), "keep original")


if __name__ == "__main__":
    unittest.main()
