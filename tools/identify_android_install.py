#!/usr/bin/env python3
"""Read only Bana Forge's public APK identity; never install or read app data.

Requires Python 3.9+, Android SDK Build Tools (apksigner/aapt), and, for
--device, SDK Platform Tools (adb) with an owner-authorized USB device.
No keytool, keystore, passwords, root, run-as, app launch or storage access.
"""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys
import tempfile

PACKAGE = "dev.daijro.banaforge"
KNOWN = {
    ("0.5.31", 5031, "fd9f7b1dcb2edaf4aca2c4abca9167a3ff55416dad2849cebee919ac0c5a220f"):
        44972042580,
    ("0.5.32", 5032, "2bf9dc969005467362dd2f9a80d4d43e62c2ac368c718ba78f9ce23de6eca126"):
        45012769205,
}


def run_text(command):
    # No shell and no command/tool diagnostics echoed into the public report.
    result = subprocess.run(command, capture_output=True, text=True, timeout=120)
    if result.returncode:
        raise ValueError("Android tool failed. Check local SDK/device access; no install or data-reset command was used.")
    return result.stdout


def tool(directory, name):
    path = Path(directory) / name
    if sys.platform == "win32":
        path = path.with_suffix(".bat" if name == "apksigner" else ".exe")
    if not path.is_file():
        raise ValueError("Missing SDK Build Tool: " + name)
    return str(path.resolve())


def parse_identity(certificate, badging):
    digests = re.findall(r"Signer #\d+ certificate SHA-256 digest: ([0-9a-fA-F]{64})", certificate)
    if len(digests) != 1:
        raise ValueError("Expected one verified APK signer; multiple/rotated signers require separate review.")
    package = re.search(r"^package: name='([^']+)' versionCode='(\d+)' versionName='([^']+)'", badging, re.M)
    if not package or package[1] != PACKAGE:
        raise ValueError("Not the expected Bana Forge release package. Do not infer identity from a filename.")
    abis = re.search(r"^native-code: (.+)$", badging, re.M)
    return {
        "package": package[1], "version_name": package[3], "version_code": int(package[2]),
        "certificate_sha256": digests[0].lower(),
        "base_apk_abis": re.findall(r"'([^']+)'", abis[1]) if abis else [],
    }


def inspect_apk(apk, tools):
    apk = Path(apk).resolve()
    certificate = run_text([tool(tools, "apksigner"), "verify", "--print-certs", str(apk)])
    badging = run_text([tool(tools, "aapt"), "dump", "badging", str(apk)])
    identity = parse_identity(certificate, badging)
    digest = hashlib.sha256()
    with apk.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    identity["base_apk_sha256"] = digest.hexdigest()
    return identity


def choose_device(output, serial=None):
    devices = []
    for line in output.splitlines():
        fields = line.split()
        if len(fields) >= 2 and fields[1] == "device":
            devices.append(fields[0])
    if serial is not None:
        if serial.startswith("-") or serial not in devices:
            raise ValueError("Selected device is not connected and authorized.")
        return serial
    if len(devices) != 1:
        raise ValueError("Connect exactly one authorized device, or select it locally with --serial. No device was inspected.")
    return devices[0]


def base_path(output):
    paths = []
    for line in output.splitlines():
        if not line.startswith("package:"):
            continue
        path = line[len("package:"):].strip()
        public_install = path.startswith("/data/app/") or re.match(
            r"^/mnt/expand/[A-Za-z0-9-]+/app/", path
        )
        if (not public_install or not re.fullmatch(r"/[A-Za-z0-9_./+=@~-]+\.apk", path)
                or ".." in PurePosixPath(path).parts):
            raise ValueError("Unexpected package path; stopping without copying files.")
        paths.append(path)
    bases = [path for path in paths if PurePosixPath(path).name == "base.apk"]
    if len(bases) != 1 or len(set(paths)) != len(paths):
        raise ValueError("Expected exactly one installed base.apk; package missing or layout requires manual review.")
    return bases[0], len(paths) - 1


def inspect_device(adb, tools, serial=None):
    selected = choose_device(run_text([adb, "devices"]), serial)
    prefix = [adb, "-s", selected]
    path, splits = base_path(run_text(prefix + ["shell", "pm", "path", PACKAGE]))
    # Pull only public executable APK bytes into a temporary computer directory.
    # Never traverse /data/user, Android/data, app settings, or user project files.
    with tempfile.TemporaryDirectory(prefix="banaforge-public-apk-") as folder:
        destination = Path(folder) / "base.apk"
        run_text(prefix + ["pull", path, str(destination)])
        identity = inspect_apk(destination, tools)
    return identity, splits


def report_for(identity, source, splits=None):
    key = (identity["version_name"], identity["version_code"], identity["certificate_sha256"])
    run = KNOWN.get(key)
    return {
        **identity,
        "source": source,
        "installed_version_confirmed": source == "authorized_device_base_apk",
        "matching_audited_identity_run": run,
        "identity_status": "matches_audited_identity" if run else "not_an_exact_audited_identity_match",
        "installed_split_apk_count": splits,
        "scope": "Public base APK only; split contents, private key availability, update acceptance and data retention are NOT verified.",
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--device", action="store_true", help="Read an owner-authorized connected device")
    source.add_argument("--apk", type=Path, help="Inspect a local APK; does NOT establish what is installed")
    parser.add_argument("--tools", type=Path, required=True, help="SDK build-tools/<version> directory")
    parser.add_argument("--adb", default="adb", help="Path to SDK platform-tools/adb (or adb.exe)")
    parser.add_argument("--serial", help="Select device locally; excluded from the public report")
    parser.add_argument("--report", type=Path, help="Create a new public JSON file; never overwrite")
    args = parser.parse_args()
    if args.serial and not args.device:
        parser.error("--serial requires --device")
    try:
        if args.device:
            identity, splits = inspect_device(args.adb, args.tools, args.serial)
            source_name = "authorized_device_base_apk"
        else:
            identity, splits = inspect_apk(args.apk, args.tools), None
            source_name = "local_apk_not_installed_confirmation"
        text = json.dumps(report_for(identity, source_name, splits), indent=2) + "\n"
        if args.report:
            with args.report.open("x", encoding="utf-8") as stream:
                stream.write(text)
        print(text, end="")
        return 0
    except (OSError, ValueError, subprocess.SubprocessError):
        print("Inspection could not complete. Check SDK paths, USB authorization, package presence and that the report filename is unused. No install, uninstall, storage reset or key generation was requested.", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
