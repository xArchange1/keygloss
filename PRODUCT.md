# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

The product is a Tauri 2 desktop app (Windows, macOS, experimental Linux) whose UI is a web frontend (`src/`), plus a marketing site in Astro (`site/`). Neither is a native iOS/Android surface.

## Users

People who type in both Russian and English every day on a desktop computer (Windows, macOS, or KDE Plasma on Linux) and keep typing in the wrong layout (`ghbdtn` instead of `привет`). Many are switching from Punto Switcher. Their job: keep typing without stopping to delete and retype text entered in the wrong layout.

## Product Purpose

Keygloss detects RU↔EN wrong-layout text as you type, or on a hotkey, converts it, and switches the layout to match. It also fixes common typos, expands snippets, and lets you set per-app exceptions. Success means the user forgets the wrong-layout problem exists.

## Positioning

In priority order:

1. Free and open source (MIT): no ads, no account, public code.
2. Cross-platform: Windows, macOS, and KDE Plasma 6 on Wayland.
3. Privacy: fully local. No telemetry, typed text is never logged, and the only network calls are update checks to GitHub.
4. A modern UI with light and dark themes, plus typo fixes and snippets on top of layout switching.

## Operating Context

- The app runs in the background through a global keyboard hook and a tray icon. Users touch its settings window rarely.
- It shows a non-activating banner and a picker over other apps.
- The marketing site (keygloss.app) is where people evaluate it and install it: winget on Windows, a Homebrew cask on macOS, a curl install script on Linux.
- Both the app and the site are bilingual (English and Russian).

## Capabilities and Constraints

- Languages: English and Russian only.
- macOS:
  - the build is not notarized, so Gatekeeper blocks the first launch and the user has to allow it in Privacy & Security;
  - the app needs the Accessibility permission.
- Linux:
  - only KDE Plasma 6 on Wayland, x86_64 `.deb`/`.rpm`;
  - no self-update;
  - password fields are not detected.
- Updates come from GitHub releases, on a stable or a beta channel.
- History (the last 200 automatic fixes) is off by default and stored only on this computer.
- No voice features of any kind (dictation, microphone).

## Brand Commitments

- Name: Keygloss. Icon: `site/public/img/keygloss.svg`, `site/public/img/icon.png`.
- Never use Keyboop's icon, copy or endpoints. Its name and links appear only to credit it (README) and to pitch Keygloss as its alternative (the site's meta tags and `robots.txt`). Keygloss is a rewrite of Keyboop (MIT), and the attribution stays in `LICENSE` and `THIRD_PARTY.md`.
- Sponsorship goes through Boosty (`https://boosty.to/xarchange1/donate`) or a T-Bank transfer (`https://t.tb.ru/pm_short/2Jf29Z7cIgW`) and helps fund an Apple Developer ID.

## Evidence on Hand

- App screenshots in light and dark themes, EN and RU:
  - `docs/media/layout-switching*.webp`
  - `docs/media/snippets*.webp`
  - copies in `site/public/img/`
- README banners: `docs/media/banner-{en,ru}.webp`.
- The GitHub star count, fetched live on the site.
- Do not invent:
  - testimonials, user or download counts, press or benchmarks;
  - support for languages other than RU/EN or desktops other than KDE.

## Product Principles

1. Free and open first: never add anything that costs money, needs an account, or hides code.
2. Every platform claim must hold; state the limits (notarization, KDE-only) plainly.
3. Typing stays on the machine: no feature may send or log typed text.
4. Invisible by default: the app works in the background and stays out of the user's way.
5. Bilingual parity: every user-facing string exists in English and Russian.
