import {getLocales} from 'expo-localization';

const about_en = `---
title: About Markdownr
author: The Dude
theme: dude
---
# About Markdownr

Markdownr is a fast, lean, focused Markdown viewer for **iPhone** and **iPad**.

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

Made with care & joy for markdown lovers everywhere.

:::quotes

> "Yeah, well, you know, that's just, like, your markdown, man."
> — The Dude

> "The Dude renders."
> — The Dude

> "Smokey, this is not HTML. This is Markdown. There are rules."
> — Walter

> "Am I the only one around here who gives a shit about proper headings?!"
> — Walter

> "My markdown has been commended as being strongly formatted."
> — Maude

> "Does the front matter make you uncomfortable, Mr. Lebowski?"
> — Maude

> "Sometimes you render the markdown, and sometimes, well, the markdown renders you."
> — The Stranger

> "I like your style, Dude. It really ties the document together."
> — The Stranger

:::

[Back to Welcome](markdownr:home)
`;

const about_de = `---
title: Über Markdownr
author: The Dude
theme: dude
---
# Über Markdownr

Markdownr ist ein schneller, kleiner, fokussierter Markdown-Viewer für **iPhone** und **iPad**.

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

Mit viel Sorgfalt & Spaß gemacht für Markdown-Fans überall.

:::quotes

> „Ja, weißt du, das ist halt, wie, dein Markdown, Mann."
> — The Dude

> „The Dude rendert."
> — The Dude

> „Smokey, das ist nicht HTML. Das ist Markdown. Es gibt Regeln."
> — Walter

> „Bin ich der Einzige hier, dem korrekte Überschriften wichtig sind?!"
> — Walter

> „Mein Markdown wurde als ausgesprochen gut formatiert gelobt."
> — Maude

> „Macht Sie das Front Matter etwa verlegen, Mr. Lebowski?"
> — Maude

> „Manchmal renderst du das Markdown, und manchmal, naja, rendert das Markdown dich."
> — The Stranger

> „Ich mag deinen Stil, Dude. Er hält das Dokument wirklich zusammen."
> — The Stranger

:::

[Zurück zur Startseite](markdownr:home)
`;

const about_ru = `---
title: О приложении Markdownr
author: The Dude
theme: dude
---
# О приложении Markdownr

Markdownr — это быстрый, лёгкий и сфокусированный просмотрщик Markdown для **iPhone** и **iPad**.

## Возможности

- **Нативный рендеринг** — плавная прокрутка, мгновенное отображение
- **Тёмная и светлая темы** — следуют системе или переключаются вручную
- **Красивые темы** — Default, Ocean, Forest, Sunset, Lavender и сезонные темы
- **Предложения тем** — документы могут предлагать собственную тему через front matter
- **Оглавление** — автоматически создаваемая панель навигации
- **Полнотекстовый поиск** — подсвеченные результаты с переходом между совпадениями
- **Front Matter** — отображает метаданные YAML в виде оформленного заголовка
- **Поддержка формул** — строчный и блочный LaTeX через KaTeX
- **Масштабирование щипком** — приближайте любой контент

## Философия

> «Чувак невозмутим.»

Как и Чувак, Markdownr не пытается быть всем сразу. Он не редактирует. Он не синхронизирует. У него нет семнадцати боковых панелей. Он просто *показывает markdown* — и делает это хорошо.

Вы открываете файл. Он выглядит прекрасно. Вот и всё. Чувак бы одобрил.

---

Сделано с заботой и радостью для всех любителей markdown по всему миру.

:::quotes

> «Да ну, знаешь, это же просто, типа, твой markdown, чувак.»
> — Чувак

> «Чувак рендерит.»
> — Чувак

> «Смоуки, это не HTML. Это Markdown. Тут есть правила.»
> — Уолтер

> «Я тут единственный, кому не наплевать на правильные заголовки?!»
> — Уолтер

> «Мой markdown хвалили за строгое форматирование.»
> — Мод

> «Вас смущает front matter, мистер Лебовски?»
> — Мод

> «Иногда ты рендеришь markdown, а иногда, ну, markdown рендерит тебя.»
> — Незнакомец

> «Мне нравится твой стиль, Чувак. Он и правда связывает документ воедино.»
> — Незнакомец

:::

[Назад на главную](markdownr:home)
`;

const translations: Record<string, string> = {
  en: about_en,
  de: about_de,
  ru: about_ru,
};

export function getAboutMarkdown(): string {
  const languageCode = getLocales()[0]?.languageCode ?? 'en';
  return translations[languageCode] ?? about_en;
}
