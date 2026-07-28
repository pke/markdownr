const { withDangerousMod } = require('@expo/config-plugins');
const path = require('node:path');
const fs = require('node:fs');

// iOS is linked automatically by Expo autolinking via
// modules/file-picker/ios/FilePicker.podspec, so no iOS patching is needed.

const withFilePickerAndroid = (config) => {
  return withDangerousMod(config, [
    'android',
    async (cfg) => {
      const destDir = path.join(
        cfg.modRequest.platformProjectRoot,
        'app', 'src', 'main', 'java', 'dev', 'dudesoft', 'filepicker'
      );
      fs.mkdirSync(destDir, { recursive: true });
      const src = path.join(__dirname, 'android', 'src', 'main', 'java', 'dev', 'dudesoft', 'filepicker', 'FilePickerModule.kt');
      fs.copyFileSync(src, path.join(destDir, 'FilePickerModule.kt'));
      return cfg;
    },
  ]);
};

module.exports = (config) => withFilePickerAndroid(config);
