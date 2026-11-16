"""Select and prepare the public iOS Simulator runtime for release tests."""

import json
import plistlib
import re
import subprocess
import sys
import time


PUBLIC_UPDATES_URL = "https://gdmf.apple.com/v2/pmv"
CATALOG_URL = "https://devimages-cdn.apple.com/downloads/xcode/simulators/index2.dvtdownloadableindex"
VERSION = r"[0-9]+\.[0-9]+(?:\.[0-9]+)?"
BUILD = r"[0-9]+[A-Z][0-9]+[a-z]?"


def version_key(version):
    parts = tuple(map(int, version.split(".")))
    return parts + (0,) * (3 - len(parts))


def select_runtime(updates, catalog):
    versions = [
        item["ProductVersion"]
        for item in updates["PublicAssetSets"]["iOS"]
        if re.fullmatch(VERSION, item["ProductVersion"])
        and any(model.startswith("iPhone") for model in item["SupportedDevices"])
    ]
    if not versions:
        raise ValueError("Apple's Software Lookup Service contains no public iPhone update.")
    latest_ios = max(versions, key=version_key)

    mappings = [
        item for item in catalog["sdkToSimulatorMappings"]
        if item.get("sdkIdentifier", "").startswith("iphoneos")
        and item.get("groupTag") not in (None, "none")
    ]
    # In a split SDK group Apple marks the standard build as preferred.
    preferred = [item for item in mappings if item.get("affinity") == "preferred"]
    preferred_groups = {item["groupTag"] for item in preferred}
    preferred_builds = {item["simulatorBuildUpdate"] for item in preferred}
    secondary_builds = {
        item["simulatorBuildUpdate"] for item in mappings
        if item["groupTag"] in preferred_groups
    } - preferred_builds

    candidates = []
    for item in catalog["downloadables"]:
        if item.get("category") != "simulator" or item.get("platform") != "com.apple.platform.iphoneos":
            continue
        simulator = item.get("simulatorVersion", {})
        version = simulator.get("version", "")
        build = simulator.get("buildUpdate", "")
        if (
            item.get("name") == f"iOS {version} Simulator Runtime"
            and re.fullmatch(VERSION, version)
            and re.fullmatch(BUILD, build)
            and build not in secondary_builds
            and version_key(version) <= version_key(latest_ios)
        ):
            candidates.append((version, build))
    if not candidates:
        raise ValueError(f"Apple lists no public standard iPhone Simulator runtime for iOS {latest_ios} or earlier.")

    latest = max((version for version, _ in candidates), key=version_key)
    builds = {build for version, build in candidates if version_key(version) == version_key(latest)}
    if len(builds) != 1:
        raise ValueError(f"Apple lists multiple unconfirmed iOS {latest} Simulator builds: {', '.join(sorted(builds))}.")
    return latest_ios, latest, builds.pop()


def runtime_info(runtime_build, runtimes):
    for runtime in runtimes:
        if runtime["buildversion"] != runtime_build or not runtime["isAvailable"]:
            continue
        phones = [
            device for device in runtime.get("supportedDeviceTypes", [])
            if device.get("productFamily") == "iPhone" and "Duo" not in device["name"]
        ]
        if not phones:
            raise ValueError(f"Simulator build {runtime_build} does not support a standard iPhone.")
        device = next((phone for phone in phones if phone["name"] == "iPhone 15"), phones[0])
        return runtime["identifier"], runtime["version"], device["identifier"], device["name"]
    return None


def ensure_runtime(build, installed_runtimes, download, wait=time.sleep):
    info = runtime_info(build, installed_runtimes())
    if info:
        return info

    download_error = None
    try:
        download(build)
    except subprocess.CalledProcessError as error:
        download_error = error

    # CoreSimulator may register the exact build shortly after xcodebuild exits,
    # including when Xcode reports that it did not receive installation status.
    for _ in range(15):
        info = runtime_info(build, installed_runtimes())
        if info:
            return info
        wait(2)
    raise ValueError(f"Simulator build {build} did not become available. Check the selected Xcode.") from download_error


def fetch(url):
    return subprocess.check_output([
        "curl", "--fail", "--silent", "--show-error", "--location",
        "--connect-timeout", "10", "--max-time", "30",
        "--retry", "2", "--retry-max-time", "30", url,
    ])


def installed_runtimes():
    return json.loads(subprocess.check_output(["xcrun", "simctl", "list", "runtimes", "-j"]))["runtimes"]


def download(build):
    print(f"Installing iOS Simulator build {build} with Xcode", file=sys.stderr, flush=True)
    subprocess.run(["xcodebuild", "-downloadPlatform", "iOS", "-buildVersion", build],
                   stdout=sys.stderr, check=True)


def main(expected_build=""):
    try:
        public_ios, runtime_version, build = select_runtime(
            json.loads(fetch(PUBLIC_UPDATES_URL)), plistlib.loads(fetch(CATALOG_URL)),
        )
        print(f"Apple public iOS {public_ios}. Standard iPhone Simulator {runtime_version} ({build}).",
              file=sys.stderr, flush=True)
        if expected_build and expected_build != build:
            raise ValueError(f"Requested build {expected_build} does not match Apple's latest standard build {build}.")
        print("\t".join((build, *ensure_runtime(build, installed_runtimes, download))))
    except (OSError, ValueError, KeyError, plistlib.InvalidFileException, subprocess.CalledProcessError) as error:
        print(f"Could not prepare the public iOS Simulator: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(*sys.argv[1:]))
