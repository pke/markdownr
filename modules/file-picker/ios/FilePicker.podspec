Pod::Spec.new do |s|
  s.name           = 'FilePicker'
  s.version        = '1.0.0'
  s.summary        = 'Native in-place file picker for Markdownr'
  s.description    = 'Picks a document in place with a security-scoped bookmark, enabling external-change detection.'
  s.license        = 'MIT'
  s.author         = 'dudesoft'
  s.homepage       = 'https://dudesoft.dev'
  s.platforms      = {
    :ios => '15.1'
  }
  s.swift_version  = '5.9'
  s.source         = { git: '' }
  s.static_framework = true

  s.dependency 'ExpoModulesCore'

  s.pod_target_xcconfig = {
    'DEFINES_MODULE' => 'YES',
    'SWIFT_COMPILATION_MODE' => 'wholemodule'
  }

  s.source_files = '**/*.{h,m,swift}'
end
