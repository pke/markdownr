import i18n from 'i18next';
import {initReactI18next} from 'react-i18next';
import {getLocales} from 'expo-localization';

// react-i18next setup for Markdownr UI chrome.
// Resources are bundled (no async backend), so translations are available
// synchronously on first render. The initial language follows the device
// locale via expo-localization, with an English fallback. `en` is the source
// of truth for keys — see i18next.d.ts for the compile-time key typing.

export const en = {
  mode: {auto: 'Auto', dark: 'Dark', light: 'Light'},
  view: {rendered: 'Rendered', source: 'Source'},
  menu: {open: 'Open', openFolder: 'Open Folder', search: 'Search'},
  frontMatter: {moreInfo: 'More info', hide: 'hide'},
  // Welcome-page action suffixes (appended after the tappable link text)
  suffix: {
    toDarkMode: ' dark mode 🌙',
    toLightMode: ' light mode ☀️',
    toAutoMode: ' auto mode ⚙️',
    colorThemes: ' color themes:',
  },
  alert: {openFolderMessage: 'Select all files in this folder to enable link navigation.'},
  search: {
    title: 'Search',
    placeholder: 'Search in markdown...',
    line: 'Line',
    noResults: 'No results found',
    recentSearches: 'Recent Searches',
  },
  recentFiles: {
    title: 'Recent Files',
    empty: 'No recent files. Open a markdown file to see it here.',
    unavailableTitle: 'File unavailable',
    unavailableMessage: 'This file was removed by iOS to free up storage space. Open the original file again to re-cache it.',
  },
  drawer: {contents: 'Contents', showAll: 'Show All', about: 'About'},
  theme: {suggested: 'Suggested theme:', custom: 'Custom theme'},
  error: {title: 'Something went wrong', tryAgain: 'Try Again'},
  common: {
    clearAll: 'Clear All',
    delete: 'Delete',
    done: 'Done',
    apply: 'Apply',
    dismiss: 'Dismiss',
    cancel: 'Cancel',
  },
} as const;

// Same shape as the English source, but with plain-string leaves so a
// translation isn't forced to equal the English literal — while still being
// checked for completeness (every key must be present).
type Translations<T> = {[K in keyof T]: T[K] extends string ? string : Translations<T[K]>};

const de: Translations<typeof en> = {
  mode: {auto: 'Auto', dark: 'Dunkel', light: 'Hell'},
  view: {rendered: 'Gerendert', source: 'Quelltext'},
  menu: {open: 'Öffnen', openFolder: 'Ordner öffnen', search: 'Suchen'},
  frontMatter: {moreInfo: 'Mehr Infos', hide: 'ausblenden'},
  suffix: {
    toDarkMode: ' Dunkelmodus 🌙',
    toLightMode: ' Hellmodus ☀️',
    toAutoMode: ' Automatik ⚙️',
    colorThemes: ' Farbthemen:',
  },
  alert: {openFolderMessage: 'Wähle alle Dateien in diesem Ordner aus, um die Link-Navigation zu aktivieren.'},
  search: {
    title: 'Suchen',
    placeholder: 'In Markdown suchen …',
    line: 'Zeile',
    noResults: 'Keine Ergebnisse gefunden',
    recentSearches: 'Letzte Suchen',
  },
  recentFiles: {
    title: 'Zuletzt geöffnet',
    empty: 'Keine zuletzt geöffneten Dateien. Öffne eine Markdown-Datei, damit sie hier erscheint.',
    unavailableTitle: 'Datei nicht verfügbar',
    unavailableMessage: 'Diese Datei wurde von iOS entfernt, um Speicherplatz freizugeben. Öffne die Originaldatei erneut, um sie neu zu laden.',
  },
  drawer: {contents: 'Inhalt', showAll: 'Alle anzeigen', about: 'Über'},
  theme: {suggested: 'Empfohlenes Thema:', custom: 'Eigenes Thema'},
  error: {title: 'Etwas ist schiefgelaufen', tryAgain: 'Erneut versuchen'},
  common: {
    clearAll: 'Alle löschen',
    delete: 'Löschen',
    done: 'Fertig',
    apply: 'Anwenden',
    dismiss: 'Verwerfen',
    cancel: 'Abbrechen',
  },
};

const ru: Translations<typeof en> = {
  mode: {auto: 'Авто', dark: 'Тёмная', light: 'Светлая'},
  view: {rendered: 'Просмотр', source: 'Исходник'},
  menu: {open: 'Открыть', openFolder: 'Открыть папку', search: 'Поиск'},
  frontMatter: {moreInfo: 'Подробнее', hide: 'скрыть'},
  suffix: {
    toDarkMode: ' тёмный режим 🌙',
    toLightMode: ' светлый режим ☀️',
    toAutoMode: ' авторежим ⚙️',
    colorThemes: ' цветовые темы:',
  },
  alert: {openFolderMessage: 'Выберите все файлы в этой папке, чтобы включить переходы по ссылкам.'},
  search: {
    title: 'Поиск',
    placeholder: 'Поиск в Markdown…',
    line: 'Строка',
    noResults: 'Ничего не найдено',
    recentSearches: 'Недавние запросы',
  },
  recentFiles: {
    title: 'Недавние файлы',
    empty: 'Нет недавних файлов. Откройте файл Markdown, и он появится здесь.',
    unavailableTitle: 'Файл недоступен',
    unavailableMessage: 'Этот файл был удалён системой iOS для освобождения места. Откройте исходный файл ещё раз, чтобы восстановить его.',
  },
  drawer: {contents: 'Содержание', showAll: 'Показать все', about: 'О приложении'},
  theme: {suggested: 'Рекомендуемая тема:', custom: 'Своя тема'},
  error: {title: 'Что-то пошло не так', tryAgain: 'Повторить'},
  common: {
    clearAll: 'Очистить всё',
    delete: 'Удалить',
    done: 'Готово',
    apply: 'Применить',
    dismiss: 'Закрыть',
    cancel: 'Отмена',
  },
};

export const resources = {
  en: {translation: en},
  de: {translation: de},
  ru: {translation: ru},
};

/** Map the device locale to a supported UI language, falling back to English. */
export function currentLanguage(): 'en' | 'de' | 'ru' {
  const code = getLocales()[0]?.languageCode;
  return code === 'de' || code === 'ru' ? code : 'en';
}

i18n.use(initReactI18next).init({
  resources,
  lng: currentLanguage(),
  fallbackLng: 'en',
  interpolation: {escapeValue: false}, // React already escapes
  returnNull: false,
});

export default i18n;
