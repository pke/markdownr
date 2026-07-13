const { withDangerousMod } = require('@expo/config-plugins');
const path = require('path');
const fs = require('fs');

// iOS is linked automatically by Expo autolinking via
// modules/folder-picker/ios/FolderPicker.podspec, so no iOS patching is needed.

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

module.exports = (config) => withFolderPickerAndroid(config);
