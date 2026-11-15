<p align="center">
  <img src="src-tauri/icons/128x128@2x.png" width="96" alt="Keygloss">
</p>

<h1 align="center">Keygloss</h1>

<p align="center">Приватная современная альтернатива Punto Switcher</p>

<p align="center">
  <a href="../../README.md">English</a> ·
  <b>Русский</b>
</p>

<p align="center">
  <a href="https://keygloss.app"><img src="docs/media/banner-ru.webp" alt="Баннер Keygloss: Хватит писать Ghbdtn. Keygloss сам исправит текст и переключит раскладку, пока вы печатаете."></a>
</p>

---

Набираете `ghbdtn`, получаете `привет`, и раскладка переключается сама. RU ↔ EN, на лету или по сочетанию клавиш.

## Возможности

- Исправление опечаток, автозамена и исключения для программ.
- История последних автоматических исправлений, хранится только на этом компьютере.
- Обновления с GitHub, стабильные или бета.
- Поддерживаемые языки: английский и русский.

## Скриншоты

<table>
  <tr>
    <td width="50%"><img src="docs/media/layout-switching-ru.webp" alt="Настройки переключения раскладки в светлой теме"></td>
    <td width="50%"><img src="docs/media/snippets-ru.webp" alt="Страница автозамены в тёмной теме"></td>
  </tr>
  <tr>
    <td align="center"><sub>Переключение раскладки: автоисправление, триггеры и исправления текста</sub></td>
    <td align="center"><sub>Автозамена: сокращения, которые разворачиваются в текст</sub></td>
  </tr>
</table>

## Установка

| Система | Команда |
| --- | --- |
| Windows | `winget install xArchange1.keygloss` |
| macOS | `brew install keygloss`<br><br>Не работает. [Отблагодарите](https://github.com/xArchange1/keygloss#sponsor) на покупку Apple Developer ID. |
| Linux | <code>curl -fsSL https://raw.githubusercontent.com/xArchange1/keygloss/main/packaging/linux/install.sh | sh</code> |

### Linux

Скрипт ставит `.deb` (Debian, Ubuntu) или `.rpm` (Fedora, openSUSE) из последнего выпуска, для x86_64. Чтобы обновиться, запустите его снова: Keygloss сообщает о новых версиях, но сам их не ставит. Пакет можно поставить и вручную из [Releases](https://github.com/xArchange1/keygloss/releases).

- Нужна KDE Plasma 6 на Wayland: через неё Keygloss читает и переключает раскладки. Другие среды и сеансы X11 пока не поддерживаются.
- Keygloss читает клавиатуры напрямую (`/dev/input`) и печатает через виртуальную клавиатуру (`uinput`). Пакет добавляет правило udev, которое даёт к ним доступ пользователю за компьютером. Читать ввод с клавиатуры после этого может и любая запущенная вами программа.
- Выделенный текст читается через Ctrl+C, поля паролей не распознаются. Текст, который не набрать ни в одной раскладке, вставляется через Ctrl+V, а терминалы это сочетание не принимают.

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
