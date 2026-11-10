# Keygloss

Keygloss fixes text typed in the wrong keyboard layout on Windows: `ghbdtn` becomes `привет`,
and the layout switches to match. RU↔EN, automatic or by hotkey.

It is a Tauri rewrite of [Keyboop](https://github.com/iffuno/keyboop) (macOS, MIT) by the
Keyboop contributors. See [THIRD_PARTY.md](THIRD_PARTY.md) for attribution.

## Run

Requires Rust (MSVC), Node and pnpm.

```
pnpm install
pnpm tauri dev
```

## Layout

- `crates/keygloss-core`: OS-agnostic engine, platform traits, settings.
- `crates/keygloss-windows`: Windows implementation of the platform traits.
- `src-tauri`: the app (engine wiring, tray, commands).
- `src`: the frontend (React + TypeScript).

## License

MIT, see [LICENSE](LICENSE).
