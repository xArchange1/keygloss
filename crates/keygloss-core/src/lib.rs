//! Keygloss engine core. Pure Rust, no OS calls.
//!
//! - [`platform`]: traits and data types the engine needs from the OS.
//! - [`settings`]: the persisted settings schema.
//! - [`pair`]: the language pair definition (RU↔EN is the only one).
//! - [`data`]: dictionaries, trigram models and word lists of a pair; background warm-up.
//! - [`keymap`]: character conversion between the two layouts.
//! - [`detector`]: wrong-layout detection and the engine's pure decision helpers.
//! - [`buffer`]: the typed-word buffer.
//! - [`exceptions`]: user word exceptions and the "who wins" pair choice.
//! - [`apps`]: built-in per-app policy tables (Windows process names).
//! - [`typo`]: typo fixes (table, mechanical, numeric, two capitals, Caps Lock word).
//! - [`undo`]: learning from undo.
//! - [`resonance`]: anti-resonance guard for automatic conversions.
//! - [`double_space`]: the "double space inserts a period" tail rule.
//! - [`snippets`]: snippet trigger matching, expansion and picker mapping.
//! - [`text_case`]: the case toggle rule.
//! - [`cue`]: the synthesized conversion sound.

pub mod apps;
pub mod buffer;
pub mod cue;
pub mod data;
pub mod detector;
pub mod double_space;
pub mod exceptions;
pub mod keymap;
pub mod pair;
pub mod platform;
pub mod resonance;
pub mod settings;
pub mod snippets;
pub mod text_case;
pub mod typo;
pub mod undo;
