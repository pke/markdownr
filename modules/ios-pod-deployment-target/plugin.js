const { withPodfile } = require('@expo/config-plugins');

// Raise every pod's IPHONEOS_DEPLOYMENT_TARGET to at least 15.1 (the app's
// own minimum). Some pods still declare iOS 12.4 (react-native-svg's
// RNSVG-RNSVGFilters), which the iOS 27 SDK rejects outright:
//   "The iOS Simulator deployment target 'IPHONEOS_DEPLOYMENT_TARGET' is set
//    to 12.4, but the range of supported deployment target versions is 15.0…"
//
// Injected into the *generated* ios/Podfile's post_install block, so it
// survives `expo prebuild --clean`.

const MARKER = '# --- markdownr ios-pod-deployment-target plugin ---';
const MIN_TARGET = '15.1';

const SNIPPET = [
  `    ${MARKER}`,
  '    installer.pods_project.targets.each do |target|',
  '      target.build_configurations.each do |build_config|',
  `        if build_config.build_settings['IPHONEOS_DEPLOYMENT_TARGET'].to_f < ${MIN_TARGET}`,
  `          build_config.build_settings['IPHONEOS_DEPLOYMENT_TARGET'] = '${MIN_TARGET}'`,
  '        end',
  '      end',
  '    end',
].join('\n');

module.exports = function withIosPodDeploymentTarget(config) {
  return withPodfile(config, (cfg) => {
    const podfile = cfg.modResults.contents;
    if (podfile.includes(MARKER)) return cfg;
    // After react_native_post_install(...), so nothing it does overrides ours
    const anchor = /react_native_post_install\([\s\S]*?\n\s*\)\n/;
    if (!anchor.test(podfile)) {
      throw new Error('ios-pod-deployment-target: no react_native_post_install call in Podfile');
    }
    cfg.modResults.contents = podfile.replace(anchor, (call) => `${call}${SNIPPET}\n`);
    return cfg;
  });
};
