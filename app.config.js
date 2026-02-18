const IS_DEV = process.env.APP_VARIANT === 'development';

export default {
  expo: {
    name: IS_DEV ? 'Markdownr Dev' : 'Markdownr',
    slug: 'markdownr',
    version: '1.1.0',
    orientation: 'default',
    icon: IS_DEV ? './assets/icon-dev.png' : './assets/icon.png',
    userInterfaceStyle: 'automatic',
    newArchEnabled: true,
    splash: {
      image: IS_DEV ? './assets/splash-icon-dev.png' : './assets/splash-icon.png',
      resizeMode: 'contain',
      backgroundColor: IS_DEV ? '#333333' : '#410065',
    },
    ios: {
      supportsTablet: true,
      appleTeamId: 'K9VADMGRA7',
      buildNumber: '2',
      bundleIdentifier: IS_DEV
        ? 'dev.dudesoft.markdownr.dev'
        : 'dev.dudesoft.markdownr',
      icon: IS_DEV
        ? './assets/icon-dev.png'
        : {
            light: './assets/icon.png',
            dark: './assets/icon-dark.png',
            tinted: './assets/icon-tinted.png',
          },
      infoPlist: {
        CFBundleDocumentTypes: [
          {
            CFBundleTypeName: 'Markdown Document',
            CFBundleTypeRole: 'Viewer',
            LSHandlerRank: 'Alternate',
            LSItemContentTypes: [
              'net.daringfireball.markdown',
              'public.text',
            ],
          },
        ],
        UTImportedTypeDeclarations: [
          {
            UTTypeConformsTo: ['public.text'],
            UTTypeDescription: 'Markdown Document',
            UTTypeIdentifier: 'net.daringfireball.markdown',
            UTTypeTagSpecification: {
              'public.filename-extension': ['md', 'markdown'],
              'public.mime-type': 'text/markdown',
            },
          },
        ],
        LSSupportsOpeningDocumentsInPlace: true,
        UISupportsDocumentBrowser: true,
        ITSAppUsesNonExemptEncryption: false,
      },
    },
    android: {
      package: IS_DEV
        ? 'dev.dudesoft.markdownr.dev'
        : 'dev.dudesoft.markdownr',
      adaptiveIcon: {
        foregroundImage: IS_DEV
          ? './assets/adaptive-icon-dev.png'
          : './assets/adaptive-icon.png',
        backgroundColor: IS_DEV ? '#333333' : '#410065',
      },
      edgeToEdgeEnabled: true,
      intentFilters: [
        {
          action: 'VIEW',
          category: ['DEFAULT', 'BROWSABLE'],
          data: [
            {
              scheme: 'file',
              mimeType: 'text/markdown',
            },
            {
              scheme: 'content',
              mimeType: 'text/markdown',
            },
          ],
        },
        {
          action: 'VIEW',
          category: ['DEFAULT', 'BROWSABLE'],
          data: [
            {
              scheme: 'file',
              host: '*',
              pathPattern: '.*\\.md',
            },
            {
              scheme: 'content',
              host: '*',
              pathPattern: '.*\\.md',
            },
          ],
        },
        {
          action: 'VIEW',
          category: ['DEFAULT', 'BROWSABLE'],
          data: [
            {
              scheme: 'file',
              host: '*',
              pathPattern: '.*\\.markdown',
            },
            {
              scheme: 'content',
              host: '*',
              pathPattern: '.*\\.markdown',
            },
          ],
        },
      ],
    },
    web: {
      favicon: './assets/favicon.png',
    },
    plugins: ['expo-document-picker'],
  },
};
