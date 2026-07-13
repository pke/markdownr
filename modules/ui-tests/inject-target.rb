#!/usr/bin/env ruby
# Add a UI-test bundle target to the generated ios/ project using the xcodeproj
# gem. node-xcode (what Expo's withXcodeProject serializes with) has no
# ui-testing target type, so this runs *after* prebuild has finished writing the
# pbxproj — invoked by plugin.js (withDangerousMod) or standalone. Idempotent.
# ios/ is gitignored.
#
#   ruby modules/ui-tests/inject-target.rb
require 'xcodeproj'
require 'fileutils'

ROOT     = File.expand_path('../..', __dir__)
# Auto-detect the app project (MarkdownrDev.xcodeproj dev / Markdownr.xcodeproj
# prod), ignoring Pods.xcodeproj — works for either variant.
PROJ     = Dir[File.join(ROOT, 'ios', '*.xcodeproj')]
             .reject { |p| File.basename(p) == 'Pods.xcodeproj' }.first
abort 'no app .xcodeproj under ios/ — run expo prebuild first' unless PROJ
APP      = File.basename(PROJ, '.xcodeproj')
TARGET   = 'MarkdownrUITests'
BUNDLEID = "dev.dudesoft.#{APP.downcase}.uitests"
SRC_DIR  = File.join(__dir__, 'MarkdownrUITests')
DST_DIR  = File.join(ROOT, 'ios', TARGET)
SWIFTS   = %w[DeepLinkUITests.swift RecentsUITests.swift]

# 1. Copy the (tracked) Swift sources into ios/<TARGET>/ (gitignored).
FileUtils.mkdir_p(DST_DIR)
SWIFTS.each { |f| FileUtils.cp(File.join(SRC_DIR, f), File.join(DST_DIR, f)) }

project = Xcodeproj::Project.open(PROJ)
app_target = project.targets.find { |t| t.name == APP } or abort "no #{APP} target"

test_target = project.targets.find { |t| t.name == TARGET }
if test_target.nil?
  test_target = project.new_target(
    :ui_test_bundle, TARGET, :ios, '15.1', project.products_group, :swift
  )
  group = project.main_group.new_group(TARGET, "#{TARGET}")
  SWIFTS.each { |f| test_target.add_file_references([group.new_file(f)]) }
  test_target.add_dependency(app_target)
  puts "created target #{TARGET}"
else
  puts "target #{TARGET} exists — refreshing build settings"
end

# Always (re)apply settings — PRODUCT_NAME is the fix for the empty
# `-Runner.app` / `.xctest` product name that collided the build.
test_target.build_configurations.each do |c|
  c.build_settings.merge!(
    'PRODUCT_NAME'              => TARGET,
    'PRODUCT_BUNDLE_IDENTIFIER' => BUNDLEID,
    'TEST_TARGET_NAME'          => APP,
    'CODE_SIGNING_ALLOWED'      => 'NO',
    'CODE_SIGNING_REQUIRED'     => 'NO',
    'SWIFT_VERSION'             => '5.0',
    'TARGETED_DEVICE_FAMILY'    => '1,2',
    'GENERATE_INFOPLIST_FILE'   => 'YES',
    'CURRENT_PROJECT_VERSION'   => '1',
    'MARKETING_VERSION'         => '1.0',
  )
end
project.save

# 2. Wire the test target into the shared MarkdownrDev scheme's test action.
scheme_path = File.join(
  Xcodeproj::XCScheme.shared_data_dir(PROJ).to_s, "#{APP}.xcscheme"
)
abort "no shared scheme at #{scheme_path}" unless File.exist?(scheme_path)
scheme = Xcodeproj::XCScheme.new(scheme_path)
tt = project.targets.find { |t| t.name == TARGET }
already = scheme.test_action.testables.any? do |t|
  t.buildable_references.any? { |r| r.target_name == TARGET }
end
unless already
  ref = Xcodeproj::XCScheme::TestAction::TestableReference.new(tt)
  scheme.test_action.add_testable(ref)
  scheme.save_as(PROJ, APP, true)
  puts "wired #{TARGET} into #{APP} scheme test action"
end
