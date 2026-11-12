//! Plays the conversion sound (a synthesized one or the user's file) at a volume.

use std::fs::File;
use std::io::{BufReader, Cursor};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::mpsc::{Receiver, RecvError, RecvTimeoutError, Sender, channel};
use std::time::Duration;

use keygloss_core::cue;
use keygloss_core::settings::Sound;
use rodio::{Decoder, DeviceSinkBuilder, Player, Source};
use tauri::{AppHandle, Manager};

/// The app's copy of the custom sound, in the config dir. No extension: the format is read
/// from the content.
const CUSTOM_FILE: &str = "custom-sound";
/// A custom sound is cut off after this long.
const CUSTOM_LIMIT: Duration = Duration::from_secs(5);
/// The output gain at 100% volume: full gain was far too loud, so the slider spans 0..=20% of it.
const MAX_GAIN: f64 = 0.2;

struct Cue {
    sound: Sound,
    volume: f64,
    custom: PathBuf,
}

pub fn custom_file(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|dir| dir.join(CUSTOM_FILE))
}

/// The file as audio; `None` if it is missing or not WAV or MP3.
pub fn decode(path: &Path) -> Option<Decoder<BufReader<File>>> {
    Decoder::try_from(File::open(path).ok()?).ok()
}

/// The player gain for a volume setting (0..=1).
fn gain(volume: f64) -> f32 {
    (volume.clamp(0.0, 1.0) * MAX_GAIN) as f32
}

/// Queues what `cue` plays; `None` plays nothing.
fn append(player: &Player, cue: &Cue) -> Option<()> {
    match cue.sound {
        Sound::Off => return None,
        Sound::Beep => player.append(Decoder::try_from(Cursor::new(cue::switch_cue_wav())).ok()?),
        Sound::Whoosh => player.append(Decoder::try_from(Cursor::new(cue::whoosh_wav())).ok()?),
        Sound::Custom => player.append(decode(&cue.custom)?.take_duration(CUSTOM_LIMIT)),
    }
    Some(())
}

/// Plays one cue to its end, or until a newer one arrives. Returns the next cue.
fn run(cue: &Cue, next: &Receiver<Cue>) -> Result<Cue, RecvError> {
    if let Ok(mut out) = DeviceSinkBuilder::open_default_sink() {
        out.log_on_drop(false);
        let player = Player::connect_new(out.mixer());
        player.set_volume(gain(cue.volume));
        if append(&player, cue).is_some() {
            // One more round after the queue empties lets the device play out its buffer.
            let mut done = false;
            while !done {
                done = player.empty();
                match next.recv_timeout(Duration::from_millis(100)) {
                    Ok(cue) => return Ok(cue),
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => return Err(RecvError),
                }
            }
        }
    }
    next.recv()
}

/// Plays `sound` without blocking the caller; a newer sound cuts an older one off. A missing
/// or unreadable custom file plays nothing.
pub fn play(app: &AppHandle, sound: Sound, volume: f64) {
    static CUES: OnceLock<Sender<Cue>> = OnceLock::new();
    let Some(custom) = custom_file(app) else { return };
    if sound == Sound::Off || volume <= 0.0 {
        return;
    }
    let cues = CUES.get_or_init(|| {
        let (tx, rx) = channel::<Cue>();
        let _ = std::thread::Builder::new().name("keygloss-sound".into()).spawn(move || {
            let mut cue = rx.recv();
            while let Ok(c) = cue {
                cue = run(&c, &rx);
            }
        });
        tx
    });
    let _ = cues.send(Cue { sound, volume, custom });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_volume_plays_at_a_fifth_of_full_gain() {
        assert_eq!(gain(1.0), 0.2);
        assert_eq!(gain(0.5), 0.1);
        assert_eq!(gain(2.0), 0.2);
        assert_eq!(gain(-1.0), 0.0);
    }

    #[test]
    fn decodes_the_built_in_sounds_and_refuses_other_files() {
        let dir = std::env::temp_dir().join(format!("keygloss-sound-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (wav, text) = (dir.join("a"), dir.join("b"));
        std::fs::write(&wav, cue::whoosh_wav()).unwrap();
        std::fs::write(&text, "not audio").unwrap();
        let samples = decode(&wav).unwrap();
        assert_eq!((samples.channels().get(), samples.sample_rate().get()), (1, 44_100));
        assert_eq!(samples.count(), 11_466);
        assert!(decode(&text).is_none());
        assert!(decode(&dir.join("missing")).is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
