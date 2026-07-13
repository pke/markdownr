const { withDangerousMod } = require('@expo/config-plugins');
const path = require('path');
const fs = require('fs');

const FILE_UUID = 'AA1F2D3E4B5C6D7E8F900000';
const BUILD_UUID = 'AA1F2D3E4B5C6D7E8F901234';
const FILE_NAME = 'FolderPickerModule.swift';

const withFolderPickerIOS = (config) => {
  return withDangerousMod(config, [
    'ios',
    async (cfg) => {
      const appName = cfg.modRequest.projectName;
      const platformRoot = cfg.modRequest.platformProjectRoot;

      // Copy Swift source file
      const src = path.join(__dirname, 'ios', FILE_NAME);
      const destDir = path.join(platformRoot, appName);
      fs.copyFileSync(src, path.join(destDir, FILE_NAME));

      // Patch pbxproj to include the file
      const pbxprojPath = path.join(platformRoot, `${appName}.xcodeproj`, 'project.pbxproj');
      let pbx = fs.readFileSync(pbxprojPath, 'utf8');

      // Patch the [Expo] Configure project shell script to inject FolderPickerModule after generation
      const injectScript = `\\n\\n# Inject local FolderPickerModule (not auto-linked)\\nPROVIDER=\\"\\${SRCROOT}/Pods/Target Support Files/Pods-${appName}/ExpoModulesProvider.swift\\"\\nif ! grep -q 'FolderPickerModule' \\"$PROVIDER\\"; then\\n  sed -i '' 's/SymbolModule\\\\.self$/SymbolModule.self,\\\\n      FolderPickerModule.self/' \\"$PROVIDER\\"\\nfi\\n`;
      pbx = pbx.replace(
        /shellScript = "(# This script configures Expo modules[^"]+)";/,
        (match, script) => {
          if (script.includes('FolderPickerModule')) return match;
          return `shellScript = "${script}${injectScript}";`;
        }
      );

      if (!pbx.includes(FILE_UUID)) {
        // PBXBuildFile entry
        pbx = pbx.replace(
          '/* End PBXBuildFile section */',
          `\t\t${BUILD_UUID} /* ${FILE_NAME} in Sources */ = {isa = PBXBuildFile; fileRef = ${FILE_UUID} /* ${FILE_NAME} */; };\n/* End PBXBuildFile section */`
        );

        // PBXFileReference entry (insert after AppDelegate.swift reference)
        pbx = pbx.replace(
          /(\t\t[A-Z0-9]+ \/\* AppDelegate\.swift \*\/ = \{[^}]+\};)/,
          `$1\n\t\t${FILE_UUID} /* ${FILE_NAME} */ = {isa = PBXFileReference; lastKnownFileType = sourcecode.swift; name = ${FILE_NAME}; path = ${appName}/${FILE_NAME}; sourceTree = "<group>"; };`
        );

        // PBXGroup children (insert after AppDelegate.swift child)
        pbx = pbx.replace(
          /(\t\t\t\t[A-Z0-9]+ \/\* AppDelegate\.swift \*\/,)/,
          `$1\n\t\t\t\t${FILE_UUID} /* ${FILE_NAME} */,`
        );

        // PBXSourcesBuildPhase files (insert after AppDelegate.swift build file)
        pbx = pbx.replace(
          /(\t\t\t\t[A-Z0-9]+ \/\* AppDelegate\.swift in Sources \*\/,)/,
          `$1\n\t\t\t\t${BUILD_UUID} /* ${FILE_NAME} in Sources */,`
        );

        fs.writeFileSync(pbxprojPath, pbx);
      }

      return cfg;
    },
  ]);
};

const withFolderPickerAndroid = (config) => {
  return withDangerousMod(config, [
    'android',
    async (cfg) => {
      const destDir = path.join(
        cfg.modRequest.platformProjectRoot,
        'app', 'src', 'main', 'java', 'dev', 'dudesoft', 'folderpicker'
      );
      fs.mkdirSync(destDir, { recursive: true });
      const src = path.join(__dirname, 'android', 'src', 'main', 'java', 'dev', 'dudesoft', 'folderpicker', 'FolderPickerModule.kt');
      fs.copyFileSync(src, path.join(destDir, 'FolderPickerModule.kt'));
      return cfg;
    },
  ]);
};

module.exports = (config) => {
  config = withFolderPickerIOS(config);
  config = withFolderPickerAndroid(config);
  return config;
};
