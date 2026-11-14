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
- Поддерживаемые языки: английский и русский.

## Скриншоты

## Установка

| Система | Команда |
| --- | --- |
| Windows | `winget install xArchange1.keygloss` |
| macOS | `brew install keygloss` |

## Разработка

Нужны Rust (MSVC), Node и pnpm.

```
pnpm install
pnpm tauri dev
```

## Конфиденциальность

Keygloss обрабатывает ввод только на этом компьютере и не собирает данные об использовании и телеметрию. В интернет он выходит только:

- чтобы проверить на GitHub наличие новой версии при запуске и скачать её. Это отключается в Настройки → Общие («Проверять обновления при запуске»).
- когда вы нажимаете на ссылку, которая открывается в браузере.

## Благодарности

Вдохновлено [Keyboop](https://keyboop.com) ([GitHub](https://github.com/iffuno/keyboop)). Его автор не планирует версии для Windows и Linux, поэтому появился Keygloss.

## Лицензия

MIT, см. [LICENSE](LICENSE).
