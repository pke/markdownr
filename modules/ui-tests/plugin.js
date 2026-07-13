const { withDangerousMod } = require('@expo/config-plugins');
const { execFileSync } = require('node:child_process');
const path = require('node:path');

// Adds the MarkdownrUITests XCUITest target to the generated Xcode project so it
// survives `expo prebuild --clean`.
//
// Why a dangerous mod that shells out to Ruby instead of `withXcodeProject`:
// node-xcode (Expo's pbxproj serializer) has no `ui-testing` target type, so
// building the target in-memory and letting Expo re-serialize it fails with
// "Invalid target: undefined". Instead we let prebuild finish writing the
// pbxproj, then run inject-target.rb (xcodeproj gem, which handles ui-testing
// targets) against the finished file. withDangerousMod runs late enough that the
// pbxproj is already on disk; the script is idempotent so a re-run is a no-op.
//
// Opt-in: only applied when WITH_UITESTS=1 (see app.config.js), so normal dev/
// prod prebuilds are untouched.
module.exports = function withUITests(config) {
  return withDangerousMod(config, [
    'ios',
    (cfg) => {
      const script = path.join(__dirname, 'inject-target.rb');
      const home = process.env.HOME || '';
      // Mirror the release toolchain env so the xcodeproj gem resolves.
      const env = {
        ...process.env,
        GEM_HOME: process.env.GEM_HOME || path.join(home, '.gem'),
        PATH: `/opt/homebrew/opt/ruby/bin:${path.join(home, '.gem', 'bin')}:${process.env.PATH}`,
      };
      execFileSync('ruby', [script], {
        stdio: 'inherit',
        env,
        cwd: cfg.modRequest.projectRoot,
      });
      return cfg;
    },
  ]);
};
