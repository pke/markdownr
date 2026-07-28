const { withAppBuildGradle, withGradleProperties } = require('@expo/config-plugins');

// Android release/distribution setup (same pattern as vcardr/shoutr):
//  - shrink by default: R8 minify + resource shrinking + locale filtering
//    (en/de/ru — AndroidX otherwise drags ~80 locales into resources.arsc)
//  - release APKs signed with the shared upload key from
//    ~/.gradle/gradle.properties (UPLOAD_*) — sideloaded APKs must be
//    properly signed or Android refuses to install them; falls back to the
//    debug key when the properties are absent (CI, other machines)
//  - Eclipse/OSGi-style APK filename with a build-timestamp qualifier:
//    dev.dudesoft.markdownr_1.11.0.v20260728-0430.apk
//
// Distribution happens via GitHub Releases (tools/publish-android-release.sh
// uploads the APK to the version tag; the website links the stable
// releases/latest/download/markdownr.apk asset) — no APK binaries in git.
//
// Everything is appended to the *generated* android/app/build.gradle, so it
// survives `expo prebuild --clean`. Rename only, never relocate the output
// directory — AGP finalizes the listing-redirect path against the default
// build/outputs/apk/<variant>/ and the build fails if it moves.

const MARKER = '// --- markdownr android-release plugin ---';

const GRADLE_SNIPPET = [
  MARKER,
  'android {',
  '    defaultConfig {',
  '        resourceConfigurations += ["en", "de", "ru"]',
  '    }',
  '    signingConfigs {',
  '        upload {',
  '            def uploadStore = findProperty("UPLOAD_STORE_FILE")',
  '            if (uploadStore != null) {',
  '                storeFile file(uploadStore)',
  '                storePassword findProperty("UPLOAD_STORE_PASSWORD")',
  '                keyAlias findProperty("UPLOAD_KEY_ALIAS")',
  '                keyPassword findProperty("UPLOAD_KEY_PASSWORD")',
  '            }',
  '        }',
  '    }',
  '    buildTypes {',
  '        release {',
  '            if (findProperty("UPLOAD_STORE_FILE") != null) {',
  '                signingConfig signingConfigs.upload',
  '            }',
  '        }',
  '    }',
  '    applicationVariants.configureEach { variant ->',
  '        variant.outputs.configureEach {',
  '            def qualifier = new java.text.SimpleDateFormat("yyyyMMdd-HHmm").format(new java.util.Date())',
  '            outputFileName = "${variant.applicationId}_${variant.versionName}.v${qualifier}.apk"',
  '        }',
  '    }',
  '}',
].join('\n');

module.exports = function withAndroidRelease(config) {
  config = withGradleProperties(config, (cfg) => {
    const set = (key, value) => {
      const existing = cfg.modResults.find(
        (item) => item.type === 'property' && item.key === key
      );
      if (existing) existing.value = value;
      else cfg.modResults.push({ type: 'property', key, value });
    };
    set('android.enableMinifyInReleaseBuilds', 'true');
    set('android.enableShrinkResourcesInReleaseBuilds', 'true');
    // arm64-v8a covers every real handset since ~2017; x86/x86_64 are
    // emulator-only and armeabi-v7a is a vanishing 32-bit tail. RN native libs
    // are ~20 MB per ABI, so unlike vcardr (37 KB native, universal APK) the
    // trade-off flips here: single-ABI cuts the website download by ~60%.
    // Also applies to debug builds (all our targets — Pixel 8, Apple-Silicon
    // emulators — are arm64), which speeds those up too.
    set('reactNativeArchitectures', 'arm64-v8a');
    return cfg;
  });

  config = withAppBuildGradle(config, (cfg) => {
    if (!cfg.modResults.contents.includes(MARKER)) {
      cfg.modResults.contents += '\n' + GRADLE_SNIPPET + '\n';
    }
    return cfg;
  });

  return config;
};
