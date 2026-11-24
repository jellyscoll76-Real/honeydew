import base64
import hashlib
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from prepare_android_signing import prepare, REQUIRED
from verify_android_update import verify

CERT = b"public certificate fixture"
DIGEST = hashlib.sha256(CERT).hexdigest()


class SigningTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.pin = Path(self.temp.name) / "pin"
        self.pin.write_text(DIGEST)
        self.key = Path(self.temp.name) / "key.jks"
        self.env = dict(zip(REQUIRED, [base64.b64encode(b"existing-private-key").decode(), "store-secret", "alias", "key-secret"]))

    def invoke(self, env, result=None):
        with patch.dict(os.environ, env, clear=True), patch("prepare_android_signing.subprocess.run", return_value=result or subprocess.CompletedProcess([], 0, CERT, b"")) as run:
            answer = prepare(self.key, self.pin)
            self.assertNotIn("-genkeypair", run.call_args.args[0])
            self.assertNotIn("store-secret", run.call_args.args[0])
            return answer

    def test_missing_and_partial_configuration_fail_without_creating_a_key(self):
        for env in [{}, {REQUIRED[0]: self.env[REQUIRED[0]]}]:
            with self.assertRaises(ValueError):
                self.invoke(env)
            self.assertFalse(self.key.exists())

    def test_existing_key_bytes_and_certificate_remain_identical_across_builds(self):
        for _ in range(2):
            self.assertEqual(self.invoke(self.env), DIGEST)
            self.assertEqual(self.key.read_bytes(), b"existing-private-key")
            self.assertEqual(self.key.stat().st_mode & 0o777, 0o600)
            self.key.unlink()

    def test_bad_base64_does_not_write_a_key(self):
        with self.assertRaises(ValueError):
            self.invoke({**self.env, REQUIRED[0]: "not base64!"})
        self.assertFalse(self.key.exists())

    def test_wrong_certificate_or_bad_password_removes_only_ephemeral_copy(self):
        for result in [subprocess.CompletedProcess([], 0, b"wrong-cert", b""), subprocess.CompletedProcess([], 1, b"", b"private diagnostic")]:
            with self.assertRaises(ValueError):
                self.invoke(self.env, result)
            self.assertFalse(self.key.exists())

    def test_never_overwrites_an_existing_key(self):
        self.key.write_bytes(b"preserve-me")
        with self.assertRaises(FileExistsError):
            self.invoke(self.env)
        self.assertEqual(self.key.read_bytes(), b"preserve-me")


class UpdateTests(unittest.TestCase):
    def metadata(self, version="0.5.33", code=5033):
        return dict(package="dev.daijro.banaforge", certificate_sha256=DIGEST,
                    version_code=code, version_name=version, abis=["armeabi-v7a", "arm64-v8a"])

    def test_two_consecutive_updates_keep_package_signer_and_both_abis(self):
        old = self.metadata("0.5.32", 5032)
        new = self.metadata()
        verify(new, "0.5.33", DIGEST, old, old["abis"])
        verify(self.metadata("0.5.34", 5034), "0.5.34", DIGEST, new, old["abis"])

    def test_package_signer_version_and_abi_regressions_are_rejected(self):
        old = self.metadata("0.5.32", 5032)
        for changes in [dict(package="dev.daijro.banaforge.debug"), dict(certificate_sha256="0" * 64),
                        dict(version_code=5032), dict(version_name="0.5.32"), dict(abis=["arm64-v8a"])]:
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                verify({**self.metadata(), **changes}, "0.5.33", DIGEST, old, old["abis"])

    def test_same_version_rebuild_is_not_a_new_update(self):
        with self.assertRaises(ValueError):
            verify(self.metadata(), "0.5.33", DIGEST, self.metadata())

    def test_minor_and_major_bumps_increase_encoded_version(self):
        verify(self.metadata("0.6.0", 6000), "0.6.0", DIGEST, self.metadata())
        verify(self.metadata("1.0.0", 1000000), "1.0.0", DIGEST, self.metadata("0.6.0", 6000))

    def test_ambiguous_or_overflowing_versions_are_rejected(self):
        for v in ["0.5.33-beta.1", "0.1000.0", "0.5.1000", "9999.0.0"]:
            with self.subTest(v=v), self.assertRaises(ValueError):
                verify(self.metadata(), v, DIGEST)


if __name__ == "__main__":
    unittest.main()
