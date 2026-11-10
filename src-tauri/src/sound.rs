//! The conversion cue at a volume.

/// Size of the canonical PCM WAV header `cue::switch_cue_wav` writes.
const HEADER: usize = 44;

/// Scales the 16-bit samples of a canonical mono PCM WAV by `volume` (clamped to 0..=1).
pub fn scaled(wav: &[u8], volume: f64) -> Vec<u8> {
    let v = volume.clamp(0.0, 1.0);
    let mut out = wav.to_vec();
    let start = HEADER.min(out.len());
    for s in out[start..].chunks_exact_mut(2) {
        let x = (f64::from(i16::from_le_bytes([s[0], s[1]])) * v).round() as i16;
        s.copy_from_slice(&x.to_le_bytes());
    }
    out
}

/// Plays the cue without blocking the caller.
pub fn play_cue(volume: f64) {
    keygloss_windows::shell::play_wav(scaled(&keygloss_core::cue::switch_cue_wav(), volume));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples(wav: &[u8]) -> Vec<i16> {
        wav[HEADER..].chunks(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect()
    }

    #[test]
    fn scales_samples_and_keeps_the_header() {
        let wav = keygloss_core::cue::switch_cue_wav();
        let half = scaled(&wav, 0.5);
        assert_eq!(half.len(), wav.len());
        assert_eq!(half[..HEADER], wav[..HEADER]);
        for (a, b) in samples(&wav).iter().zip(samples(&half)) {
            assert!((f64::from(*a) * 0.5 - f64::from(b)).abs() <= 0.5);
        }
        assert_eq!(scaled(&wav, 1.0), wav);
        assert_eq!(scaled(&wav, 3.0), wav, "clamped");
        assert!(samples(&scaled(&wav, 0.0)).iter().all(|&s| s == 0));
    }
}
