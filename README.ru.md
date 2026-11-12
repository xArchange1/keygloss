<p align="center">
  <img src="src-tauri/icons/128x128@2x.png" width="96" alt="Keygloss">
</p>

<h1 align="center">Keygloss</h1>

<p align="center">Приватная современная альтернатива Punto Switcher</p>

<p align="center">
  <a href="https://github.com/xArchange1/keygloss/releases/latest">Скачать</a> ·
  <a href="https://github.com/xArchange1/keygloss/blob/main/README.md">English</a>
</p>

---

Набираете `ghbdtn`, получаете `привет`, и раскладка переключается сама. RU ↔ EN, на лету или по сочетанию клавиш.

## Возможности

- Исправление опечаток, автозамена и исключения для программ.
- История последних автоматических исправлений, хранится только на этом компьютере.
- Обновления с GitHub, стабильные или бета.

## Установка

| Система | Команда |
| --- | --- |
| Windows | `winget install xArchange1.keygloss` |
| macOS | `brew install keygloss` |

## Сборка из исходников

Нужны Rust (MSVC), Node и pnpm.

```
pnpm install
pnpm tauri dev
```

## Лицензия

MIT, см. [LICENSE](LICENSE).
