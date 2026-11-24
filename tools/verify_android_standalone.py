#!/usr/bin/env python3
"""Verify a signed standalone APK against its permanent package and certificate pin.

This is package/signing/metadata evidence only, not a device-install or migration test.
"""
import argparse
import json
from pathlib import Path
import re

from verify_android_update import inspect

PACKAGE = "dev.daijro.banaforge.standalone"
CERT_RE = re.compile(r"^[0-9a-f]{64}$")
SUPPORTED_ABIS = {"arm64-v8a", "armeabi-v7a"}


def version_code(version):
    parts = version.split(".")
    if len(parts) != 3 or not all(part.isdigit() for part in parts):
        raise ValueError("Expected MAJOR.MINOR.PATCH version.")
    major, minor, patch = map(int, parts)
    code = major * 1_000_000 + minor * 1_000 + patch
    if minor >= 1000 or patch >= 1000 or not 0 < code <= 2_100_000_000:
        raise ValueError("Version is outside the supported Android version-code encoding.")
    return code


def verify(current, version, certificate, required_abis=(), previous=None):
    pin = certificate.strip().lower()
    if not CERT_RE.fullmatch(pin):
        raise ValueError("A reviewed standalone SHA-256 certificate pin is required.")
    expected_code = version_code(version)
    if current["package"] != PACKAGE:
        raise ValueError("APK package ID is not the approved standalone ID.")
    if current["version_name"] != version or current["version_code"] != expected_code:
        raise ValueError("APK version does not match Cargo.toml and Android version-code policy.")
    if current["certificate_sha256"].lower() != pin:
        raise ValueError("APK signer does not match the standalone certificate pin.")
    required = set(required_abis)
    if not required <= SUPPORTED_ABIS or not required <= set(current["abis"]):
        raise ValueError("APK lacks a supported required native ABI.")
    if previous:
        if previous.get("package") != PACKAGE or previous.get("certificate_sha256", "").lower() != pin:
            raise ValueError("Standalone predecessor package/certificate mismatch.")
        if current["version_code"] <= int(previous["version_code"]):
            raise ValueError("A standalone successor must increase versionCode.")
        if not set(previous.get("abis", ())) <= set(current["abis"]):
            raise ValueError("Standalone successor drops a previously supported ABI.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("apk", type=Path)
    parser.add_argument("--tools", type=Path, required=True)
    parser.add_argument("--cargo", type=Path, default=Path("Cargo.toml"))
    parser.add_argument("--certificate", type=Path, default=Path("android/standalone-release-certificate.sha256"))
    parser.add_argument("--abis", default="all")
    parser.add_argument("--previous-identity", type=Path)
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()

    import tomllib
    version = tomllib.loads(args.cargo.read_text())["package"]["version"]
    required = ["arm64-v8a", "armeabi-v7a"] if args.abis == "all" else args.abis.split(",")
    current = inspect(args.apk, args.tools)
    previous = json.loads(args.previous_identity.read_text()) if args.previous_identity else None
    if previous and previous.get("status") == "awaiting-first-verified-release":
        previous = None
    verify(current, version, args.certificate.read_text(), required, previous)
    report = {
        "status": "verified",
        "package": current["package"],
        "version_name": current["version_name"],
        "version_code": current["version_code"],
        "certificate_sha256": current["certificate_sha256"],
        "abis": current["abis"],
        "previous_identity_checked": previous is not None,
        "device_install_tested": False,
        "migration_tested": False,
    }
    text = json.dumps(report, indent=2) + "\n"
    if args.report:
        args.report.write_text(text)
    print(text)


if __name__ == "__main__":
    main()
