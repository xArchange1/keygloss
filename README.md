<p align="center">
  <img src="src-tauri/icons/128x128@2x.png" width="96" alt="Keygloss">
</p>

<h1 align="center">Keygloss</h1>

<p align="center">Privacy-first modern Punto-Switcher alternative</p>

<p align="center">
  <a href="https://github.com/xArchange1/keygloss/releases/latest">Download</a> ·
  <a href="https://github.com/xArchange1/keygloss/blob/main/README.ru.md">Русский</a>
</p>

---

Type `ghbdtn`, get `привет`, and the layout switches to match. RU ↔ EN, as you type or with a hotkey.

## Features

- Typo fixes, snippets and per-app exceptions.
- History of recent automatic fixes, stored only on this computer.
- Updates from GitHub, stable or beta.
- Supported languages: English and Russian.

## Screenshots

## Install

| Platform | Command |
| --- | --- |
| Windows | `winget install xArchange1.keygloss` |
| macOS | `brew install keygloss` |

## Development

Requires Rust (MSVC), Node and pnpm.

```
pnpm install
pnpm tauri dev
```

## Privacy

Keygloss processes your typing only on this computer and collects no usage data or telemetry. It connects to the internet only:

- to check GitHub for a newer release on startup, and to download it. Turn this off in Settings → General ("Check for updates on startup").
- when you click a link, which opens your web browser.

## Credits

Inspired by [Keyboop](https://keyboop.com) ([GitHub](https://github.com/iffuno/keyboop)). Its author doesn't plan Windows or Linux versions, so Keygloss fills the gap.

## License

MIT, see [LICENSE](LICENSE).
