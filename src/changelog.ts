// Release notes shown by "What's new", newest first. Add an entry for each release, in both languages.

export interface Release {
  version: string;
  en: string[];
  ru: string[];
}

export const CHANGELOG: Release[] = [
  {
    version: "0.1.2",
    en: [
      "Check for updates from the tray (menu bar on macOS).",
      "Clearer Russian wording throughout the app.",
      "macOS: the app is now signed, so from this version on the Accessibility permission carries over to updates.",
    ],
    ru: [
      "Пункт «Проверить обновления» в меню значка (в строке меню на macOS).",
      "Русские тексты в программе стали понятнее.",
      "macOS: программа теперь подписана, так что начиная с этой версии разрешение в «Универсальном доступе» сохраняется при обновлениях.",
    ],
  },
  {
    version: "0.1.1",
    en: [
      "macOS support: a universal app for Apple Silicon and Intel, installable with Homebrew.",
      "Experimental Linux support for KDE Plasma 6, as .deb and .rpm packages.",
      "German, French and Spanish layout pairs, converted with the shortcut.",
      "Double Shift is the default convert/undo shortcut again.",
      "Snippets have an on/off switch, and the Snippets page offers ready-made examples.",
      "History is off by default; auto-clear takes any period from 1 to 30 days.",
      "Ctrl+Tab switches settings categories, and Settings sizes itself to the screen.",
    ],
    ru: [
      "Поддержка macOS: универсальная программа для Apple Silicon и Intel, устанавливается через Homebrew.",
      "Экспериментальная поддержка Linux для KDE Plasma 6, пакеты .deb и .rpm.",
      "Пары раскладок для немецкого, французского и испанского, переключение по сочетанию клавиш.",
      "Двойной Shift снова сочетание по умолчанию для переключения и отмены.",
      "Сниппеты можно выключить, а на их странице есть готовые примеры.",
      "История выключена по умолчанию; для автоочистки можно задать любой срок от 1 до 30 дней.",
      "Ctrl+Tab переключает разделы настроек, а окно настроек подстраивается под экран.",
    ],
  },
  {
    version: "0.1.0",
    en: [
      "Fixes words typed in the wrong layout, RU ↔ EN, as you type or with a shortcut.",
      "Typo fixes, snippets and per-app exceptions.",
      "History of recent automatic fixes, stored only on this computer.",
      "Updates from GitHub: stable or beta, checked on startup.",
      "Report a problem or suggest a feature from About.",
    ],
    ru: [
      "Исправляет слова, набранные не в той раскладке, RU ↔ EN, на лету или по сочетанию клавиш.",
      "Исправление опечаток, сниппеты и исключения для программ.",
      "История последних автоматических исправлений, хранится только на этом компьютере.",
      "Обновления с GitHub: стабильные или бета, проверка при запуске.",
      "Сообщить о проблеме или предложить идею можно в разделе «О программе».",
    ],
  },
];
