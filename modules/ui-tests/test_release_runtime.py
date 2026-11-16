import subprocess
import unittest
from unittest.mock import Mock

from release_runtime import ensure_runtime, select_runtime


RELEASES = {
    "PublicAssetSets": {
        "iOS": [
            {"ProductVersion": "27.2.1", "SupportedDevices": ["iPhone16,1"]},
            {"ProductVersion": "27.3", "SupportedDevices": ["iPad16,1"]},
        ],
    },
    "AssetSets": {"iOS": [{"ProductVersion": "27.3", "SupportedDevices": ["iPhone16,1"]}]},
}


def image(version, build, qualifier=""):
    return {
        "category": "simulator",
        "platform": "com.apple.platform.iphoneos",
        "name": f"iOS {version}{qualifier} Simulator Runtime",
        "simulatorVersion": {"version": version, "buildUpdate": build},
    }


def mapping(build, group, preferred=False):
    return {
        "sdkIdentifier": "iphoneos27.2",
        "simulatorBuildUpdate": build,
        "groupTag": group,
        "affinity": "preferred" if preferred else "available",
    }


def installed(build="24B8117", model="iPhone 15"):
    return {
        "buildversion": build,
        "isAvailable": True,
        "version": "27.2",
        "identifier": "com.apple.CoreSimulator.SimRuntime.iOS-27-2",
        "supportedDeviceTypes": [
            {"productFamily": "iPhone", "name": model, "identifier": "phone"}
        ],
    }


class SelectionTests(unittest.TestCase):
    def test_uses_public_standard_build_even_with_newer_device_patch(self):
        catalog = {
            "downloadables": [
                image("27.0", "24A434"),
                image("27.1", "24A94425"),
                image("27.2", "24B91016s"),
                image("27.2", "24B8117"),
                image("27.2.1", "24B150", " Release Candidate"),
                image("27.3", "24C5037i", " beta"),
                image("27.3", "24C80"),
            ],
            "sdkToSimulatorMappings": [
                mapping("24A434", "old", preferred=True),
                mapping("24A94425", "old"),
                mapping("24B8117", "new", preferred=True),
                mapping("24B91016s", "new"),
            ],
        }
        self.assertEqual(select_runtime(RELEASES, catalog), ("27.2.1", "27.2", "24B8117"))
        catalog["downloadables"] = catalog["downloadables"][:3]
        self.assertEqual(select_runtime(RELEASES, catalog), ("27.2.1", "27.0", "24A434"))

    def test_refuses_to_guess_between_unidentified_variants(self):
        catalog = {
            "downloadables": [image("27.2", "24B8117"), image("27.2", "24B91016s")],
            "sdkToSimulatorMappings": [],
        }
        with self.assertRaisesRegex(ValueError, "multiple"):
            select_runtime(RELEASES, catalog)


class InstallationTests(unittest.TestCase):
    def test_reuses_installed_standard_runtime(self):
        download = Mock()
        info = ensure_runtime("24B8117", lambda: [installed()], download)
        self.assertEqual(info[1:], ("27.2", "phone", "iPhone 15"))
        download.assert_not_called()

    def test_requests_exact_missing_build_and_checks_registration(self):
        download = Mock()
        runtimes = Mock(side_effect=[[installed("24B91016s", "iPhone Duo")], [installed()]])
        self.assertEqual(ensure_runtime("24B8117", runtimes, download)[-1], "iPhone 15")
        download.assert_called_once_with("24B8117")

    def test_accepts_runtime_registered_after_xcode_reports_an_error(self):
        download = Mock(side_effect=subprocess.CalledProcessError(1, ["xcodebuild"]))
        runtimes = Mock(side_effect=[[], [], [installed()]])
        self.assertEqual(ensure_runtime("24B8117", runtimes, download, wait=Mock())[-1], "iPhone 15")

    def test_stops_when_only_duo_is_supported(self):
        download = Mock()
        with self.assertRaisesRegex(ValueError, "standard iPhone"):
            ensure_runtime("24B8117", lambda: [installed(model="iPhone Duo")], download)
        download.assert_not_called()

    def test_stops_when_exact_build_does_not_register(self):
        download = Mock()
        with self.assertRaisesRegex(ValueError, "24B8117 did not become available"):
            ensure_runtime("24B8117", lambda: [installed("24A434")], download, wait=Mock())
        download.assert_called_once_with("24B8117")


if __name__ == "__main__":
    unittest.main()
