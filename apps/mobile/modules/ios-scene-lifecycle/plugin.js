const fs = require('node:fs');
const path = require('node:path');
const { IOSConfig, withAppDelegate, withInfoPlist } = require('@expo/config-plugins');

const START_REACT_NATIVE = /\n#if os\(iOS\) \|\| os\(tvOS\)\n {4}window = UIWindow\(frame: UIScreen\.main\.bounds\)\n {4}factory\.startReactNative\([\s\S]*?\n#endif\n/;
const SCENE_STARTUP_COMMENT = '    // SceneDelegate starts React Native in its UIWindow.';
const DEFAULT_METRO_URL = '    return RCTBundleURLProvider.sharedSettings().jsBundleURL(forBundleRoot: ".expo/.virtual-metro-entry")';

module.exports = function withIosSceneLifecycle(config, { metroPort } = {}) {
  config = withInfoPlist(config, (mod) => {
    mod.modResults.UIApplicationSceneManifest = {
      UIApplicationSupportsMultipleScenes: false,
      UISceneConfigurations: {
        UIWindowSceneSessionRoleApplication: [{
          UISceneConfigurationName: 'Default Configuration',
          UISceneDelegateClassName: '$(PRODUCT_MODULE_NAME).SceneDelegate',
        }],
      },
    };
    return mod;
  });

  config = withAppDelegate(config, (mod) => {
    let { contents, language } = mod.modResults;
    if (language !== 'swift') {
      throw new Error('ios-scene-lifecycle requires the Expo SDK 54 Swift AppDelegate.');
    }
    if (!contents.includes(SCENE_STARTUP_COMMENT)) {
      if (!START_REACT_NATIVE.test(contents)) {
        throw new Error('ios-scene-lifecycle could not find the Expo SDK 54 React Native startup block.');
      }
      contents = contents.replace(START_REACT_NATIVE, `\n${SCENE_STARTUP_COMMENT}\n`);
    }
    if (metroPort && contents.includes(DEFAULT_METRO_URL)) {
      contents = contents.replace(DEFAULT_METRO_URL, [
        '    let provider = RCTBundleURLProvider.sharedSettings()',
        '    var host = provider.packagerServerHost()',
        '    if host == nil, let path = Bundle.main.path(forResource: "ip", ofType: "txt") {',
        '      host = (try? String(contentsOfFile: path, encoding: .utf8))?',
        '        .trimmingCharacters(in: .whitespacesAndNewlines)',
        '    }',
        '#if targetEnvironment(simulator)',
        '    host = host ?? "localhost"',
        '#endif',
        '    if let host, !host.isEmpty {',
        `      provider.jsLocation = "\\(host):${metroPort}"`,
        '    }',
        '    return provider.jsBundleURL(forBundleRoot: ".expo/.virtual-metro-entry")',
      ].join('\n'));
    }
    mod.modResults.contents = contents;
    return mod;
  });

  return IOSConfig.XcodeProjectFile.withBuildSourceFile(config, {
    filePath: 'SceneDelegate.swift',
    contents: fs.readFileSync(path.join(__dirname, 'SceneDelegate.swift'), 'utf8'),
    overwrite: true,
  });
};
