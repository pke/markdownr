#!/usr/bin/env node

/**
 * Generates iOS Settings.bundle from settings-definition.json
 * - Root.plist (English)
 * - de.lproj/Root.strings (German localization)
 * - ru.lproj/Root.strings (Russian localization)
 *
 * Usage: node scripts/generate-settings-bundle.js
 */

const fs = require('fs');
const path = require('path');

const settingsPath = path.join(__dirname, '..', 'settings-definition.json');
const bundlePath = path.join(__dirname, '..', 'ios', 'MarkdownrDev', 'Settings.bundle');
const outputPath = path.join(bundlePath, 'Root.plist');

// Localizations to emit, keyed by the field suffix used in settings-definition.json
// (titleDe/titlesDe/footerDe, titleRu/titlesRu/footerRu, ...).
const LOCALES = [
  {suffix: 'De', lproj: 'de.lproj'},
  {suffix: 'Ru', lproj: 'ru.lproj'},
];

// Read settings definition
const settings = JSON.parse(fs.readFileSync(settingsPath, 'utf8'));

// Convert setting type to iOS PSSpecifier type
function getSpecifierType(type) {
  switch (type) {
    case 'toggle':
      return 'PSToggleSwitchSpecifier';
    case 'text':
      return 'PSTextFieldSpecifier';
    case 'slider':
      return 'PSSliderSpecifier';
    case 'multiValue':
      return 'PSMultiValueSpecifier';
    default:
      return 'PSToggleSwitchSpecifier';
  }
}

// Convert a setting to plist format
function settingToPlist(setting, indent = '\t\t') {
  const type = getSpecifierType(setting.type);
  let plist = `${indent}<dict>\n`;
  plist += `${indent}\t<key>Type</key>\n`;
  plist += `${indent}\t<string>${type}</string>\n`;
  plist += `${indent}\t<key>Title</key>\n`;
  plist += `${indent}\t<string>${setting.title}</string>\n`;
  plist += `${indent}\t<key>Key</key>\n`;
  plist += `${indent}\t<string>${setting.key}</string>\n`;
  plist += `${indent}\t<key>DefaultValue</key>\n`;

  if (typeof setting.defaultValue === 'boolean') {
    plist += `${indent}\t<${setting.defaultValue}/>\n`;
  } else if (typeof setting.defaultValue === 'number') {
    plist += `${indent}\t<real>${setting.defaultValue}</real>\n`;
  } else {
    plist += `${indent}\t<string>${setting.defaultValue}</string>\n`;
  }

  // Add Values and Titles for multiValue type
  if (setting.type === 'multiValue' && setting.values && setting.titles) {
    plist += `${indent}\t<key>Values</key>\n`;
    plist += `${indent}\t<array>\n`;
    for (const value of setting.values) {
      plist += `${indent}\t\t<string>${value}</string>\n`;
    }
    plist += `${indent}\t</array>\n`;

    plist += `${indent}\t<key>Titles</key>\n`;
    plist += `${indent}\t<array>\n`;
    for (const title of setting.titles) {
      plist += `${indent}\t\t<string>${title}</string>\n`;
    }
    plist += `${indent}\t</array>\n`;
  }

  plist += `${indent}</dict>\n`;
  return plist;
}

// Generate plist content
let plist = `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
\t<key>StringsTable</key>
\t<string>Root</string>
\t<key>PreferenceSpecifiers</key>
\t<array>
`;

// Collect strings for localization, one map per locale (keyed by English source)
const localizations = {};
for (const {suffix} of LOCALES) {
  localizations[suffix] = {};
}

// Add groups and settings
for (const group of settings.groups) {
  // Add group header
  plist += `\t\t<dict>\n`;
  plist += `\t\t\t<key>Type</key>\n`;
  plist += `\t\t\t<string>PSGroupSpecifier</string>\n`;
  plist += `\t\t\t<key>Title</key>\n`;
  plist += `\t\t\t<string>${group.title}</string>\n`;

  // Add footer text if present (this is how iOS shows descriptions)
  if (group.footer) {
    plist += `\t\t\t<key>FooterText</key>\n`;
    plist += `\t\t\t<string>${group.footer}</string>\n`;
  }

  plist += `\t\t</dict>\n`;

  // Collect translations for every locale
  for (const {suffix} of LOCALES) {
    if (group[`title${suffix}`]) {
      localizations[suffix][group.title] = group[`title${suffix}`];
    }
    if (group.footer && group[`footer${suffix}`]) {
      localizations[suffix][group.footer] = group[`footer${suffix}`];
    }
  }

  // Add settings in this group
  for (const setting of group.settings) {
    plist += settingToPlist(setting);

    for (const {suffix} of LOCALES) {
      if (setting[`title${suffix}`]) {
        localizations[suffix][setting.title] = setting[`title${suffix}`];
      }

      // multiValue option titles
      const localizedTitles = setting[`titles${suffix}`];
      if (setting.type === 'multiValue' && setting.titles && localizedTitles) {
        for (let i = 0; i < setting.titles.length; i++) {
          if (localizedTitles[i]) {
            localizations[suffix][setting.titles[i]] = localizedTitles[i];
          }
        }
      }
    }
  }
}

plist += `\t</array>
</dict>
</plist>
`;

// Ensure output directory exists
if (!fs.existsSync(bundlePath)) {
  fs.mkdirSync(bundlePath, { recursive: true });
}

// Write plist
fs.writeFileSync(outputPath, plist);
console.log(`Generated: ${outputPath}`);

// Generate a .strings file per locale
for (const {suffix, lproj} of LOCALES) {
  const lprojPath = path.join(bundlePath, lproj);
  if (!fs.existsSync(lprojPath)) {
    fs.mkdirSync(lprojPath, { recursive: true });
  }

  let stringsContent = `/* ${lproj} localization for Settings.bundle */\n\n`;
  for (const [english, translated] of Object.entries(localizations[suffix])) {
    // Escape quotes in strings
    const escapedEnglish = english.replace(/"/g, '\\"');
    const escapedTranslated = translated.replace(/"/g, '\\"');
    stringsContent += `"${escapedEnglish}" = "${escapedTranslated}";\n`;
  }

  const stringsPath = path.join(lprojPath, 'Root.strings');
  fs.writeFileSync(stringsPath, stringsContent);
  console.log(`Generated: ${stringsPath}`);
}
