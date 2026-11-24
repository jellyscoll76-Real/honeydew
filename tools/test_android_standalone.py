import base64
import hashlib
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from prepare_android_signing import REQUIRED, STANDALONE_REQUIRED, prepare
from verify_android_standalone import PACKAGE, verify, version_code

CERT_BYTES = b"standalone public test certificate"
PIN = hashlib.sha256(CERT_BYTES).hexdigest()
VERSION = "0.5.33"
CURRENT = {
    "package": PACKAGE,
    "version_name": VERSION,
    "version_code": 5033,
    "certificate_sha256": PIN,
    "abis": ["arm64-v8a", "armeabi-v7a"],
}


class StandaloneVerifierTests(unittest.TestCase):
    def test_gradle_standalone_variant_has_its_own_package_and_signer(self):
        root = Path(__file__).resolve().parents[1]
        gradle = (root / "android/app/build.gradle").read_text()
        self.assertIn("applicationIdSuffix = '.standalone'", gradle)
        self.assertIn("signingConfig = signingConfigs.standaloneRelease", gradle)
        self.assertIn("BANAFORGE_STANDALONE_KEYSTORE_FILE", gradle)
        self.assertNotIn("initWith release", gradle)
        dist = (root / "tools/android-dist.sh").read_text()
        self.assertIn("if [ -f android/RELEASE_HOLD.md ]", dist)
        self.assertIn("--standalone-release", dist)
        rust = (root / "src/android.rs").read_text()
        self.assertIn('"dev.daijro.banaforge.standalone"', rust)
        self.assertIn('with_app_id(android_app_id())', rust)
        updater = (root / "src/update.rs").read_text()
        self.assertIn("check_standalone_sounds(auto)", updater)
        self.assertIn("if !apk_updates_enabled()", updater)

    def test_standalone_key_preparation_uses_only_the_separate_secret_namespace(self):
        env = dict(zip(STANDALONE_REQUIRED, [
            base64.b64encode(b"standalone-only-key").decode(),
            "standalone-store-secret",
            "standalone-alias",
            "standalone-key-secret",
        ]))
        with tempfile.TemporaryDirectory() as temp:
            key = Path(temp) / "standalone.jks"
            pin = Path(temp) / "certificate.sha256"
            pin.write_text(PIN)
            with patch.dict(os.environ, env, clear=True), patch(
                "prepare_android_signing.subprocess.run",
                return_value=subprocess.CompletedProcess([], 0, CERT_BYTES, b""),
            ) as run:
                self.assertEqual(prepare(key, pin, standalone=True), PIN)
            self.assertTrue(key.is_file())
            self.assertIn("BANAFORGE_STANDALONE_KEYSTORE_PASSWORD", run.call_args.args[0])
            self.assertNotIn("ANDROID_KEYSTORE_PASSWORD", run.call_args.args[0])

        with patch.dict(os.environ, dict(zip(REQUIRED, ["x", "old-store", "old-alias", "old-key"])), clear=True):
            with self.assertRaisesRegex(ValueError, "BANAFORGE_STANDALONE"):
                prepare(Path("never-created.jks"), Path("pin"), standalone=True)

    def test_expected_identity_version_pin_and_abis_pass(self):
        verify(CURRENT, VERSION, PIN, ["arm64-v8a", "armeabi-v7a"])
        self.assertEqual(version_code(VERSION), 5033)

    def test_rejects_legacy_package(self):
        with self.assertRaisesRegex(ValueError, "package ID"):
            verify({**CURRENT, "package": "dev.daijro.banaforge"}, VERSION, PIN)

    def test_rejects_old_signer_and_uninitialized_pin(self):
        with self.assertRaisesRegex(ValueError, "signer"):
            verify({**CURRENT, "certificate_sha256": "b" * 64}, VERSION, PIN)
        with self.assertRaisesRegex(ValueError, "reviewed standalone"):
            verify(CURRENT, VERSION, "UNSET")

    def test_rejects_missing_abi_and_bad_version(self):
        with self.assertRaisesRegex(ValueError, "ABI"):
            verify({**CURRENT, "abis": ["arm64-v8a"]}, VERSION, PIN,
                   ["arm64-v8a", "armeabi-v7a"])
        with self.assertRaises(ValueError):
            version_code("0.1000.0")

    def test_successor_must_keep_identity_increase_version_and_abis(self):
        previous = {**CURRENT, "version_name": "0.5.32", "version_code": 5032}
        verify(CURRENT, VERSION, PIN, ["arm64-v8a", "armeabi-v7a"], previous)
        with self.assertRaisesRegex(ValueError, "increase versionCode"):
            verify(CURRENT, VERSION, PIN, previous={**previous, "version_code": 5033})
        with self.assertRaisesRegex(ValueError, "package/certificate mismatch"):
            verify(CURRENT, VERSION, PIN, previous={**previous, "certificate_sha256": "b" * 64})
        with self.assertRaisesRegex(ValueError, "drops"):
            verify({**CURRENT, "abis": ["arm64-v8a"]}, VERSION, PIN,
                   previous={**previous, "abis": ["arm64-v8a", "armeabi-v7a"]})


if __name__ == "__main__":
    unittest.main()
