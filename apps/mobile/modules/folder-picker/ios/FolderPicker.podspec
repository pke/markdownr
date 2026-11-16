Pod::Spec.new do |s|
  s.name           = 'FolderPicker'
  s.version        = '1.0.0'
  s.summary        = 'Native folder picker for Markdownr'
  s.description    = 'Picks a folder and enumerates its markdown files, with persisted access.'
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
