fastlane documentation
----

# Installation

Make sure you have the latest version of the Xcode command line tools installed:

```sh
xcode-select --install
```

For _fastlane_ installation instructions, see [Installing _fastlane_](https://docs.fastlane.tools/#installing-fastlane)

# Available Actions

## iOS

### ios upload_metadata

```sh
[bundle exec] fastlane ios upload_metadata
```

Upload metadata to App Store Connect (without screenshots)

### ios submit_review

```sh
[bundle exec] fastlane ios submit_review
```

Submit the current version for App Store review

### ios upload_beta

```sh
[bundle exec] fastlane ios upload_beta
```

Build and upload to TestFlight

### ios upload_screenshots

```sh
[bundle exec] fastlane ios upload_screenshots
```

Upload screenshots to App Store Connect

### ios generate_release_notes

```sh
[bundle exec] fastlane ios generate_release_notes
```

Generate release notes from git commits since last tag. Review and edit before running release.

### ios release

```sh
[bundle exec] fastlane ios release
```

Full release: prebuild, build, upload to TestFlight, upload metadata, and submit for review

----

This README.md is auto-generated and will be re-generated every time [_fastlane_](https://fastlane.tools) is run.

More information about _fastlane_ can be found on [fastlane.tools](https://fastlane.tools).

The documentation of _fastlane_ can be found on [docs.fastlane.tools](https://docs.fastlane.tools).
