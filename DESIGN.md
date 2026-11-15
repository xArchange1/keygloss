---
name: Keygloss
description: Privacy-first RU↔EN wrong-layout fixer; a warm paper desk with real keys on it.
colors:
  paper: "#f3eee7"
  paper-shade: "#e4e5e3"
  ink: "#171819"
  gold: "#8a6500"
  indigo: "#3d48b8"
  on-gold: "#fffdf8"
  raised: "#fbf9f5"
  night-paper: "#2b2d30"
  night-floor: "#0b0c0d"
  night-ink: "#f3eee7"
  night-gold: "#f5c84c"
  night-indigo: "#8e9bff"
  night-on-gold: "#171819"
  night-raised: "#34363a"
typography:
  site-display:
    fontFamily: "Libron, Georgia, serif"
    fontSize: "88px"
    fontWeight: 700
    lineHeight: 1.1
    letterSpacing: "-0.03em"
  site-headline:
    fontFamily: "Libron, Georgia, serif"
    fontSize: "56px"
    fontWeight: 700
    lineHeight: 1.2
    letterSpacing: "-0.03em"
  site-title:
    fontFamily: "Libron, Georgia, serif"
    fontSize: "24px"
    fontWeight: 700
    lineHeight: 1.3
  site-body:
    fontFamily: "Libron, Georgia, serif"
    fontSize: "16px"
    fontWeight: 400
    lineHeight: 1.5
  site-label:
    fontFamily: "Libron, Georgia, serif"
    fontSize: "11.5px"
    fontWeight: 400
    letterSpacing: "0.06em"
  mono:
    fontFamily: "ui-monospace, Cascadia Mono, Consolas, monospace"
    fontSize: "14.5px"
    lineHeight: 1.4
  app-title:
    fontFamily: "Segoe UI Variable Text, Segoe UI, system-ui, sans-serif"
    fontSize: "20px"
    fontWeight: 600
  app-body:
    fontFamily: "Segoe UI Variable Text, Segoe UI, system-ui, sans-serif"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: 1.45
  app-label:
    fontFamily: "Segoe UI Variable Text, Segoe UI, system-ui, sans-serif"
    fontSize: "11px"
    fontWeight: 600
    letterSpacing: "0.08em"
rounded:
  key: "5px"
  control: "6px"
  window: "9px"
  card: "12px"
  feature: "14px"
  section: "40px"
  pill: "99px"
spacing:
  xs: "4px"
  sm: "8px"
  md: "16px"
  lg: "24px"
  xl: "56px"
  section: "96px"
components:
  button-primary:
    backgroundColor: "{colors.gold}"
    textColor: "{colors.on-gold}"
    rounded: "{rounded.pill}"
    height: "40px"
    padding: "0 20px"
  button-secondary:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.ink}"
    rounded: "{rounded.pill}"
    height: "40px"
    padding: "0 20px"
  keycap:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.indigo}"
    rounded: "{rounded.key}"
    padding: "1px 6px"
  toggle-on:
    backgroundColor: "{colors.gold}"
    rounded: "{rounded.pill}"
    width: "38px"
    height: "22px"
  window:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.ink}"
    rounded: "{rounded.window}"
  terminal:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.paper}"
    typography: "{typography.mono}"
    rounded: "{rounded.window}"
    padding: "10px 10px 10px 16px"
  badge:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.indigo}"
    rounded: "{rounded.pill}"
    padding: "2px 10px"
---

# Design System: Keygloss

## Overview

**Creative North Star: "The Writer's Desk"**

Keygloss looks like a warm paper desk with real objects on it: keycaps with a bottom bevel, a sticky note, a luggage tag, small app windows with title bars, a terminal. Everything is something you could press or pick up. The paper is never cold white and the night mode is never pure black; both are gentle vertical gradients, so the surface feels lit rather than flat.

Two typefaces share one palette. The marketing site speaks in Libron, a bookish serif (Cyrillic is filled by Literata under the same family name), and stages the app's UI as tactile objects on the desk. The app itself uses the OS system sans (Segoe UI Variable on Windows) at a compact 13px, because it is a settings window people open rarely and should feel native. The site may be expressive; the app stays quiet and lets brand show only in the gold toggles, the indigo focus rings and the paper gradient.

**Key Characteristics:**
- A warm paper gradient behind everything, light and dark.
- Gold is the single "on" colour: primary buttons, active toggles, the caret.
- Indigo marks keys, focus, badges and links, sparingly.
- Objects carry real physicality: bevels, soft drop shadows, slight rotation on the site.
- Bilingual by construction: every crop and string has an RU twin.

## Colors

A warm, two-accent palette: paper and ink, one gold for action, one indigo for detail. Every theme value lives in `--kg-*` custom properties (`src/styles.css`, mirrored in `site/src/pages/index.astro`); dark mode swaps the whole set through `prefers-color-scheme` or `data-theme`.

### Primary
- **Ledger Gold** (light `gold`, dark `night-gold`): the action colour. Primary pill buttons, toggles that are on, the selected-segment ring, the typing caret, the keycap's second legend. Dark ochre in light mode so it holds contrast as a fill; bright saffron at night.

### Secondary
- **Fountain-Pen Indigo** (light `indigo`, dark `night-indigo`): the detail colour. Keycap legends, focus outlines, status badges and their dots, FAQ links, app section labels.

### Neutral
- **Desk Paper** (`paper` → `paper-shade`; night `night-paper` → `night-floor`): the body gradient, drawn top to bottom and fixed to the viewport.
- **Ink** (`ink`, night `night-ink`): all text. Muted (68%) and faint (58%) text tones, the 13% and 24% hairlines, the title-bar tint and the shadows are all `color-mix` derivations of ink over paper, never separate greys.
- **Raised Card** (`raised`, night `night-raised`): windows, stickers, popovers, selected segments, keycap faces.
- **On-Gold** (`on-gold`, night `night-on-gold`): text and toggle knobs drawn on gold.

### Named Rules
**The Gold-Is-Never-Prose Rule.** Gold is an accent only. Never use it for light-theme body text; at 8px+ fills, borders and glyph accents only.

**The Derive-Don't-Add Rule.** A new tint, line or surface is a `color-mix` of the existing tokens (`--muted`, `--line`, `--hl`, `--panel`). Don't introduce a new hex.

**The Sparing Indigo Rule.** Indigo appears on status dots, badges, keys, focus rings and links. It never fills a large surface.

## Typography

**Display Font:** Libron (with Georgia, serif; Literata covers Cyrillic)
**Body Font:** Libron on the site; Segoe UI Variable Text (with Segoe UI, system-ui) in the app
**Label/Mono Font:** ui-monospace, Cascadia Mono, Consolas for commands

**Character:** On the site, a bookish serif with tight negative tracking at display sizes reads like a hand-set title page; in the app, the native system sans keeps settings plain and fast to scan.

### Hierarchy
- **Display** (site, 700, 88px, 1.1): the hero headline only.
- **Headline** (site, 700, 56px, 1.2): section heads, balanced wrap.
- **Title** (site 700 24px for feature rows; app 600 20px for page titles).
- **Body** (site 400 16px/1.5, feature copy capped at 40ch; app 400 13px/1.45).
- **Label** (site 11.5px uppercase eyebrows at +0.06em; app 11px semibold uppercase section labels at +0.08em in indigo).
- **Demo** (site): the typed-wrong word at 40px struck through in gold, and the fixed word at 72px bold. The before/after is the brand's core image.

### Named Rules
**The Two-Voices Rule.** The serif belongs to the site and the system sans to the app. Never put Libron in the app or Segoe on the site.

## Layout

The site centres on a 1090px column (`--wrap`, with a 24px gutter on each side). The hero is a centred title over a collage of objects placed absolutely on a 1440px canvas (`--x/--y`); the collage scales to 86% below 1400px and 74% below 1240px, and below 1000px it is replaced by a short inline row of objects. Feature rows are two-column grids, 5fr text to 7fr visual, alternating sides, with a 56px gap (40px below 1240px) and 80px between rows; they stack into one column below 1000px. Sections breathe at 96px vertical padding (64px on small screens). A faint 43px dot grid sits behind the hero like graph paper.

The app is a compact settings window: a sidebar, then pages built from a page header, small uppercase section labels and grouped cards of rows (min-height 32px, 12px horizontal padding).

## Elevation & Depth

The system is lifted, not flat: objects sit on the desk with soft, ink-tinted shadows. Shadows are always built from `--kg-text` via `color-mix`, so they warm with the theme.

### Shadow Vocabulary
- **Desk shadow** (`--shadow`: `0 1px 2px` ink 10% + `0 10px 28px -10px` ink 30%): windows, stickers, terminal, popovers.
- **Pill bevel** (inset 1px top highlight + `0 1px 2px` + `0 6px 14px -8px`): buttons; the primary variant tints its drop shadow with gold.
- **Key press depth** (`border-bottom-width` 2–5px instead of a shadow): keycaps and sequence keys.
- **App card** (1px inset top highlight at white 4%, hairline border, ink 3.5% fill): grouped settings cards; the app stays almost flat.

### Named Rules
**The Bevel-Not-Glow Rule.** Depth comes from bottom borders and soft drop shadows. No glows, no coloured blur halos.

## Shapes

Gently rounded and object-like. Keys have 5px corners with a thicker bottom edge; windows 9px with a tinted 26–30px title bar and three dots (gold, pale gold, indigo); feature windows 14px; the install band 40px; buttons, badges, segments and toggles are fully round pills. On the site, objects may tilt by 2–6°; in the app nothing rotates.

## Components

### Buttons
- **Shape:** full pill (99px), 40px tall (36px small).
- **Primary:** gold gradient fill (gold mixed toward on-gold at the top), on-gold text, gold-tinted drop shadow. Hover flattens to solid gold.
- **Secondary:** raised-to-slightly-inked gradient, hairline border, ink text, pill bevel. Hover flattens to raised.
- **Focus:** 2px indigo outline, 2px offset, everywhere.

### Keycaps
- **Style:** raised face, indigo legend, indigo 40% border with a 2px bottom edge (hero keycaps: 46px squares with a 4px bottom edge, Latin legend top-left and Cyrillic in gold bottom-right).

### Toggles and Segments
- **Toggle:** 38×22 pill, gold when on with an on-gold knob, ink 20% when off with a raised knob, 150ms slide.
- **Segmented:** ink 8% track, selected segment raised with a gold ring.

### Cards / Containers
- **Window:** raised body, 9px corners, desk shadow, title bar tinted 8% ink.
- **Sticker:** a small raised card (10px corners) with a faint label and rows split by hairlines; used in the hero collage.
- **App card:** 12px corners, ink 3.5% fill, 10% ink hairline, rows divided at 7%.

### Inputs / Fields
- **Terminal:** ink background, paper text, mono 14.5px, a `$ ` prompt at 60%, and a copy pill that turns gold when done.

### Navigation
- **Menubar:** a sticky 52px bar in paper at 92% with a hairline bottom; muted links that turn ink on hover; the brand link is bold; icon buttons get a 7% ink hover wash.

### Signature: the before/after demo
A large window whose body is a soft gold-to-indigo gradient. It shows the mistyped word struck through in gold, the corrected word huge and bold with a gold caret, and an indigo status badge.

## Do's and Don'ts

### Do:
- **Do** build every new colour as a `color-mix` of the `--kg-*` tokens.
- **Do** give pressable objects a bevel (thicker bottom border) and the desk shadow.
- **Do** ship every image crop and string in both English and Russian.
- **Do** use the indigo 2px outline at 2px offset for every focus state.
- **Do** keep the app compact (13px body) and native-feeling.

### Don't:
- **Don't** use gold for light-theme body text.
- **Don't** use pure white or pure black backgrounds; use the paper gradients.
- **Don't** put the serif in the app or the system sans on the site.
- **Don't** add glows, neon or coloured blur shadows.
- **Don't** use Keyboop's name, icon or visual assets.
