#!/usr/bin/env python3
"""Check actual signed APK identity and optional predecessor update compatibility.

This is an artifact gate, not proof of installation or retained runtime data.
"""
import argparse
import json
from pathlib import Path
import re
import subprocess

PACKAGE = "dev.daijro.banaforge"


def inspect(apk, tools):
    cert = subprocess.check_output([str(tools / "apksigner"), "verify", "--print-certs", str(apk)], text=True)
    digests = re.findall(r"Signer #\d+ certificate SHA-256 digest: ([0-9a-fA-F]{64})", cert)
    if len(digests) != 1:
        raise ValueError("Expected one verified release signer; signing rotation is not automatic.")
    badging = subprocess.check_output([str(tools / "aapt"), "dump", "badging", str(apk)], text=True)
    match = re.search(r"^package: name='([^']+)' versionCode='(\d+)' versionName='([^']+)'", badging, re.M)
    abis = re.search(r"^native-code: (.+)$", badging, re.M)
    if not match or not abis:
        raise ValueError("Missing APK package/version/ABI metadata.")
    return dict(package=match[1], version_code=int(match[2]), version_name=match[3],
                certificate_sha256=digests[0].lower(), abis=re.findall(r"'([^']+)'", abis[1]))


def verify(current, expected_version, certificate, previous=None, required_abis=()):
    parts = expected_version.split(".")
    if len(parts) != 3 or not all(p.isdigit() for p in parts):
        raise ValueError("Expected a stable MAJOR.MINOR.PATCH release version.")
    major, minor, patch = map(int, parts)
    code = major * 1000000 + minor * 1000 + patch
    if minor >= 1000 or patch >= 1000 or not 0 < code <= 2100000000:
        raise ValueError("Version is outside the supported Android version-code encoding.")
    if current["package"] != PACKAGE:
        raise ValueError("Release application ID changed.")
    if current["version_name"] != expected_version or current["version_code"] != code:
        raise ValueError("APK version does not match Cargo.toml and the Android encoding.")
    if current["certificate_sha256"] != certificate.strip().lower():
        raise ValueError("APK release signer does not match the approved certificate.")
    if not set(required_abis) <= set(current["abis"]):
        raise ValueError("APK lacks a required native ABI.")
    if previous:
        if current["package"] != previous["package"] or current["certificate_sha256"] != previous["certificate_sha256"]:
            raise ValueError("Predecessor package/signature mismatch: Android cannot update in place.")
        if current["version_code"] <= previous["version_code"]:
            raise ValueError("An update must have a strictly higher versionCode.")
        if not set(previous["abis"]) <= set(current["abis"]):
            raise ValueError("Update drops an ABI supported by the predecessor.")


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("apk", type=Path)
    p.add_argument("--tools", type=Path, required=True)
    p.add_argument("--cargo", type=Path, default=Path("Cargo.toml"))
    p.add_argument("--certificate", type=Path, default=Path("android/release-certificate.sha256"))
    previous_group = p.add_mutually_exclusive_group()
    previous_group.add_argument("--previous", type=Path)
    previous_group.add_argument("--previous-identity", type=Path,
                                help="Reviewed public identity manifest from the last delivered APK")
    p.add_argument("--abis", default="all")
    p.add_argument("--report", type=Path)
    args = p.parse_args()
    import tomllib
    version = tomllib.loads(args.cargo.read_text())["package"]["version"]
    current = inspect(args.apk, args.tools)
    previous = inspect(args.previous, args.tools) if args.previous else (
        json.loads(args.previous_identity.read_text()) if args.previous_identity else None
    )
    required = ["arm64-v8a", "armeabi-v7a"] if args.abis == "all" else args.abis.split(",")
    if not set(required) <= {"arm64-v8a", "armeabi-v7a"}:
        raise ValueError("Unsupported ABI selection.")
    verify(current, version, args.certificate.read_text(), previous, required)
    report = dict(current=current, previous=previous, predecessor_checked=previous is not None,
                  predecessor_source="APK" if args.previous else ("reviewed public manifest" if previous else None),
                  device_install_tested=False)
    text = json.dumps(report, indent=2) + "\n"
    if args.report:
        args.report.write_text(text)
    print(text)


if __name__ == "__main__":
    main()
