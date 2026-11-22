#!/usr/bin/env python3
"""Validate and materialize an existing release key. Never generate a replacement.

Only the public certificate digest is printed. Secrets stay in environment variables
and the ephemeral keystore, never step outputs, logs, artifacts or the repository.
"""
import argparse
import base64
import hashlib
import os
from pathlib import Path
import subprocess

REQUIRED = (
    "ANDROID_KEYSTORE_BASE64", "ANDROID_KEYSTORE_PASSWORD",
    "ANDROID_KEY_ALIAS", "ANDROID_KEY_PASSWORD",
)


def prepare(destination, certificate_file):
    missing = [name for name in REQUIRED if not os.environ.get(name)]
    if missing:
        raise ValueError("Release signing unavailable: configure " + ", ".join(missing)
                         + ". No temporary release key will be generated.")
    expected = Path(certificate_file).read_text().strip().lower()
    if len(expected) != 64 or any(c not in "0123456789abcdef" for c in expected):
        raise ValueError("A reviewed SHA-256 release certificate pin is required.")
    try:
        key = base64.b64decode(os.environ[REQUIRED[0]], validate=True)
    except (ValueError, base64.binascii.Error):
        raise ValueError("The configured keystore is not valid base64.") from None
    destination = Path(destination)
    # Never overwrite an existing key, even in a build workspace.
    fd = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    try:
        with os.fdopen(fd, "wb") as stream:
            stream.write(key)
        result = subprocess.run([
            "keytool", "-exportcert", "-keystore", str(destination),
            "-storepass:env", "ANDROID_KEYSTORE_PASSWORD",
            "-alias", os.environ["ANDROID_KEY_ALIAS"],
        ], capture_output=True, check=False)
        if result.returncode:
            raise ValueError("Cannot read the configured signing certificate; check the stored key configuration.")
        actual = hashlib.sha256(result.stdout).hexdigest()
        if actual != expected:
            raise ValueError("Release certificate does not match the approved public pin. "
                             "Do not replace the key or uninstall the app to bypass this check.")
        return actual
    except BaseException:
        destination.unlink(missing_ok=True)
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--keystore", required=True)
    parser.add_argument("--certificate", default="android/release-certificate.sha256")
    args = parser.parse_args()
    try:
        digest = prepare(args.keystore, args.certificate)
    except (OSError, ValueError):
        # Deliberately don't echo tool exceptions/secret-bearing inputs.
        print("::error::Release signing validation failed. Required: the existing release keystore, "
              "all four signing secrets and its matching approved certificate pin. "
              "No replacement key or APK was generated.")
        return 1
    print("Release certificate SHA-256: " + digest)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
