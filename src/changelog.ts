// Release notes shown by "What's new", newest first. Add an entry for each release, in both languages.

export interface Release {
  version: string;
  en: string[];
  ru: string[];
}

export const CHANGELOG: Release[] = [
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
      "Исправление опечаток, автозамена и исключения для программ.",
      "История последних автоматических исправлений, хранится только на этом компьютере.",
      "Обновления с GitHub: стабильные или бета, проверка при запуске.",
      "Сообщить о проблеме или предложить идею можно в разделе «О программе».",
    ],
  },
];
