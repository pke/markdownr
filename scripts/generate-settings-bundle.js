#!/usr/bin/env node

/**
 * Generates iOS Settings.bundle from settings-definition.json
 * - Root.plist (English)
 * - de.lproj/Root.strings (German localization)
 *
 * Usage: node scripts/generate-settings-bundle.js
 */

const fs = require('fs');
const path = require('path');

const settingsPath = path.join(__dirname, '..', 'settings-definition.json');
const bundlePath = path.join(__dirname, '..', 'ios', 'MarkdownrDev', 'Settings.bundle');
const outputPath = path.join(bundlePath, 'Root.plist');
const deLprojPath = path.join(bundlePath, 'de.lproj');
const deStringsPath = path.join(deLprojPath, 'Root.strings');

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

// Collect strings for localization
const localizations = {};

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

  // Collect German translations
  if (group.titleDe) {
    localizations[group.title] = group.titleDe;
  }
  if (group.footer && group.footerDe) {
    localizations[group.footer] = group.footerDe;
  }

  // Add settings in this group
  for (const setting of group.settings) {
    plist += settingToPlist(setting);

    // Collect German translation
    if (setting.titleDe) {
      localizations[setting.title] = setting.titleDe;
    }

    // Collect German translations for multiValue titles
    if (setting.type === 'multiValue' && setting.titles && setting.titlesDe) {
      for (let i = 0; i < setting.titles.length; i++) {
        if (setting.titlesDe[i]) {
          localizations[setting.titles[i]] = setting.titlesDe[i];
        }
      }
    }
  }
}

plist += `\t</array>
</dict>
</plist>
`;

// Ensure output directories exist
if (!fs.existsSync(bundlePath)) {
  fs.mkdirSync(bundlePath, { recursive: true });
}
if (!fs.existsSync(deLprojPath)) {
  fs.mkdirSync(deLprojPath, { recursive: true });
}

// Write plist
fs.writeFileSync(outputPath, plist);
console.log(`Generated: ${outputPath}`);

// Generate German .strings file
let stringsContent = '/* German localization for Settings.bundle */\n\n';
for (const [english, german] of Object.entries(localizations)) {
  // Escape quotes in strings
  const escapedEnglish = english.replace(/"/g, '\\"');
  const escapedGerman = german.replace(/"/g, '\\"');
  stringsContent += `"${escapedEnglish}" = "${escapedGerman}";\n`;
}

fs.writeFileSync(deStringsPath, stringsContent);
console.log(`Generated: ${deStringsPath}`);
