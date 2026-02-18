import {getLocales} from 'expo-localization';

const about_en = `---
title: About Markdownr
author: The Dude
theme: dude
---
# About Markdownr

> "Yeah, well, you know, that's just, like, your markdown, man."
> — The Dude

---

Markdownr is a lean, focused Markdown viewer for **iPhone** and **iPad**.

Built with **React Native** and **Expo**, powered by the blazing-fast \`md4c\` parser through **Nitro Modules** for native C++ performance.

## Features

- **Native Rendering** — smooth scrolling, instant display
- **Dark & Light Mode** — follows your system or toggle manually
- **Beautiful Themes** — Default, Ocean, Forest, Sunset, Lavender, and seasonal themes
- **Theme Suggestions** — documents can suggest their own theme via front matter
- **Table of Contents** — auto-generated navigation drawer
- **Full-Text Search** — highlighted results with match navigation
- **Front Matter** — displays YAML metadata as a styled header
- **Math Support** — inline and block LaTeX via KaTeX
- **Pinch to Zoom** — zoom in on any content

## The Philosophy

> "The Dude abides."

Like The Dude, Markdownr doesn't try to be everything. It doesn't edit. It doesn't sync. It doesn't have seventeen sidebars. It just *views markdown*, and it does it well.

You open a file. It looks beautiful. That's it. The Dude would approve.

---

> "This aggression will not stand, man."
> — on overly complicated markdown editors

---

Made with care for markdown lovers everywhere.

[Back to Welcome](markdownr:home)
`;

const about_de = `---
title: Über Markdownr
author: The Dude
theme: dude
---
# Über Markdownr

> „Ja, weißt du, das ist halt, wie, dein Markdown, Mann."
> — The Dude

---

Markdownr ist ein schlanker, fokussierter Markdown-Viewer für **iPhone** und **iPad**.

Gebaut mit **React Native** und **Expo**, angetrieben vom blitzschnellen \`md4c\`-Parser über **Nitro Modules** für native C++-Performance.

## Funktionen

- **Natives Rendering** — flüssiges Scrollen, sofortige Anzeige
- **Dark & Light Mode** — folgt deinem System oder manuell umschalten
- **Schöne Themes** — Default, Ocean, Forest, Sunset, Lavender und saisonale Themes
- **Theme-Vorschläge** — Dokumente können ihr eigenes Theme per Front Matter vorschlagen
- **Inhaltsverzeichnis** — automatisch generierte Navigationsleiste
- **Volltextsuche** — hervorgehobene Ergebnisse mit Treffernavigation
- **Front Matter** — zeigt YAML-Metadaten als gestalteten Header
- **Mathe-Unterstützung** — Inline- und Block-LaTeX via KaTeX
- **Pinch to Zoom** — in jeden Inhalt hineinzoomen

## Die Philosophie

> „The Dude bleibt gelassen."

Wie The Dude versucht Markdownr nicht, alles zu sein. Es editiert nicht. Es synchronisiert nicht. Es hat keine siebzehn Seitenleisten. Es *zeigt einfach Markdown an* — und das richtig gut.

Du öffnest eine Datei. Sie sieht wunderschön aus. Das war's. The Dude wäre einverstanden.

---

> „Diese Aggression wird nicht geduldet, Mann."
> — über übermäßig komplizierte Markdown-Editoren

---

Mit Liebe gemacht für Markdown-Fans überall.

[Zurück zur Startseite](markdownr:home)
`;

const translations: Record<string, string> = {
  en: about_en,
  de: about_de,
};

export function getAboutMarkdown(): string {
  const languageCode = getLocales()[0]?.languageCode ?? 'en';
  return translations[languageCode] ?? about_en;
}
