const { withXcodeProject } = require('@expo/config-plugins');
const path = require('path');
const fs = require('fs');

const TARGET = 'MarkdownrUITests';

// ⚠️  WORK IN PROGRESS — NOT registered in app.config.js.
// Adds an XCUITest target to the generated Xcode project so it survives
// `expo prebuild --clean`. node-xcode has no native "ui-testing" target type,
// so this creates a unit-test bundle and fixes up its product type + test host.
// The target it produces currently fails Expo's xcodeproj serializer
// ("Invalid target: undefined") — see README.md. Registering this plugin will
// break `expo prebuild`; finish the target wiring (or add the target in Xcode)
// before enabling it.
module.exports = function withUITests(config) {
  return withXcodeProject(config, (cfg) => {
    const project = cfg.modResults;
    const appName = cfg.modRequest.projectName; // Markdownr / MarkdownrDev
    const iosRoot = cfg.modRequest.platformProjectRoot;

    // 1. Copy the Swift test source into ios/<TARGET>/
    const destDir = path.join(iosRoot, TARGET);
    fs.mkdirSync(destDir, { recursive: true });
    fs.copyFileSync(
      path.join(__dirname, 'MarkdownrUITests', 'RecentsUITests.swift'),
      path.join(destDir, 'RecentsUITests.swift'),
    );

    if (project.pbxTargetByName(TARGET)) return cfg; // already added

    // 2. Create the target (as a test bundle) and its build phases.
    const bundleId = `dev.dudesoft.${appName.toLowerCase()}.uitests`;
    const target = project.addTarget(TARGET, 'unit_test_bundle', TARGET, bundleId);

    project.addBuildPhase([], 'PBXSourcesBuildPhase', 'Sources', target.uuid);
    project.addBuildPhase([], 'PBXFrameworksBuildPhase', 'Frameworks', target.uuid);
    project.addBuildPhase([], 'PBXResourcesBuildPhase', 'Resources', target.uuid);

    // 3. Add the Swift source to a PBXGroup and to the target's Sources phase.
    const group = project.addPbxGroup(['RecentsUITests.swift'], TARGET, TARGET);
    const mainGroup = project.getFirstProject().firstProject.mainGroup;
    project.addToPbxGroup(group.uuid, mainGroup);
    project.addSourceFile(
      path.join(TARGET, 'RecentsUITests.swift'),
      { target: target.uuid },
      group.uuid,
    );

    // 4. Turn the test bundle into a UI-testing bundle targeting the app.
    const configurations = project.pbxXCBuildConfigurationSection();
    for (const key of Object.keys(configurations)) {
      const buildSettings = configurations[key].buildSettings;
      if (!buildSettings || configurations[key].name === undefined) continue;
      if (buildSettings.PRODUCT_BUNDLE_IDENTIFIER === `"${bundleId}"`) {
        buildSettings.PRODUCT_NAME = `"${TARGET}"`;
        buildSettings.TEST_TARGET_NAME = `"${appName}"`;
        buildSettings.SWIFT_VERSION = '5.0';
        buildSettings.IPHONEOS_DEPLOYMENT_TARGET = '15.1';
        buildSettings.CODE_SIGNING_ALLOWED = 'NO';
      }
    }
    // Product type: ui-testing rather than unit-test.
    const nativeTargets = project.pbxNativeTargetSection();
    nativeTargets[target.uuid].productType = '"com.apple.product-type.bundle.ui-testing"';

    // 5. Make the UI test target depend on the app target.
    const appTarget = project.pbxTargetByName(appName);
    if (appTarget) {
      project.addTargetDependency(target.uuid, [appTarget.uuid]);
    }

    return cfg;
  });
};
