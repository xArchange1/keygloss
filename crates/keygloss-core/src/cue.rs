//! The conversion ("switch") sounds, synthesized in code as mono 16-bit 44.1 kHz WAVs. The beep
//! is the original's `CueSynth.switchData`: two dry 34 ms bursts of 440 Hz with 22 ms of silence
//! between. The whoosh is Keygloss's own.

const SAMPLE_RATE: u32 = 44_100;
const AMP: f64 = 0.38;

fn append_dry(out: &mut Vec<i16>) {
    let (dur, freq) = (0.034, 440.0);
    let sr = f64::from(SAMPLE_RATE);
    let n = (sr * dur) as usize;
    for i in 0..n {
        let t = i as f64 / sr;
        let attack = (t / 0.007).min(1.0);
        let decay = (-t / (dur * 0.38)).exp();
        let v = (2.0 * std::f64::consts::PI * freq * t).sin() * attack * decay * AMP;
        out.push((v.clamp(-1.0, 1.0) * 32_767.0) as i16);
    }
}

fn append_silence(out: &mut Vec<i16>, dur: f64) {
    let n = (f64::from(SAMPLE_RATE) * dur) as usize;
    out.extend(std::iter::repeat_n(0, n));
}

/// Minimal 16-bit mono PCM RIFF/WAVE.
fn wav(samples: &[i16]) -> Vec<u8> {
    let data_size = (samples.len() * 2) as u32;
    let mut d = Vec::with_capacity(44 + samples.len() * 2);
    d.extend_from_slice(b"RIFF");
    d.extend_from_slice(&(36 + data_size).to_le_bytes());
    d.extend_from_slice(b"WAVEfmt ");
    d.extend_from_slice(&16u32.to_le_bytes());
    d.extend_from_slice(&1u16.to_le_bytes()); // PCM
    d.extend_from_slice(&1u16.to_le_bytes()); // mono
    d.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    d.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes()); // byte rate
    d.extend_from_slice(&2u16.to_le_bytes()); // block align
    d.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    d.extend_from_slice(b"data");
    d.extend_from_slice(&data_size.to_le_bytes());
    for s in samples {
        d.extend_from_slice(&s.to_le_bytes());
    }
    d
}

/// The conversion cue as WAV bytes (about 90 ms).
pub fn switch_cue_wav() -> Vec<u8> {
    let mut s = Vec::new();
    append_dry(&mut s);
    append_silence(&mut s, 0.022);
    append_dry(&mut s);
    wav(&s)
}

/// A whoosh as WAV bytes (260 ms): noise through a band-pass sweeping up from 300 Hz to
/// 2.4 kHz, fading in and out.
pub fn whoosh_wav() -> Vec<u8> {
    let sr = f64::from(SAMPLE_RATE);
    let n = (sr * 0.26) as usize;
    let (mut low, mut band, mut seed) = (0.0, 0.0, 0x2545_F491_u32);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let p = i as f64 / n as f64;
        // xorshift32 noise in -1..1: the same samples every time.
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let noise = f64::from(seed) / f64::from(u32::MAX) * 2.0 - 1.0;
        let f = 2.0 * (std::f64::consts::PI * 300.0 * 8f64.powf(p) / sr).sin();
        low += f * band;
        band += f * (noise - low - 0.5 * band);
        out.push(band * (std::f64::consts::PI * p).sin().powi(2));
    }
    let peak = out.iter().fold(0.0, |m: f64, v| m.max(v.abs()));
    let samples: Vec<i16> = out.iter().map(|v| (v / peak * AMP * 32_767.0) as i16).collect();
    wav(&samples)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u32_at(d: &[u8], i: usize) -> u32 {
        u32::from_le_bytes(d[i..i + 4].try_into().unwrap())
    }
    fn u16_at(d: &[u8], i: usize) -> u16 {
        u16::from_le_bytes(d[i..i + 2].try_into().unwrap())
    }

    #[test]
    fn wav_header_is_sane() {
        let d = switch_cue_wav();
        assert_eq!(&d[0..4], b"RIFF");
        assert_eq!(u32_at(&d, 4) as usize, d.len() - 8);
        assert_eq!(&d[8..16], b"WAVEfmt ");
        assert_eq!(u32_at(&d, 16), 16);
        assert_eq!(u16_at(&d, 20), 1);
        assert_eq!(u16_at(&d, 22), 1);
        assert_eq!(u32_at(&d, 24), 44_100);
        assert_eq!(u32_at(&d, 28), 88_200);
        assert_eq!(u16_at(&d, 32), 2);
        assert_eq!(u16_at(&d, 34), 16);
        assert_eq!(&d[36..40], b"data");
        // 1499 + 970 + 1499 samples, as Int(44100 × duration) in the original.
        assert_eq!(u32_at(&d, 40), 3968 * 2);
        assert_eq!(d.len(), 44 + 3968 * 2);
        let samples: Vec<i16> = d[44..].chunks(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
        assert_eq!(samples[0], 0);
        assert!(samples[1499..1499 + 970].iter().all(|&s| s == 0));
        let peak = samples.iter().map(|s| s.unsigned_abs()).max().unwrap();
        assert!(peak > 5000 && peak < (0.38 * 32_767.0) as u16 + 1);
    }

    #[test]
    fn whoosh_fades_in_and_out_at_the_cue_level() {
        let d = whoosh_wav();
        assert_eq!(u32_at(&d, 4) as usize, d.len() - 8);
        assert_eq!(d.len(), 44 + 11_466 * 2);
        let samples: Vec<i16> = d[44..].chunks(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
        let peak = |s: &[i16]| s.iter().map(|s| s.unsigned_abs()).max().unwrap();
        assert_eq!(peak(&samples), (0.38 * 32_767.0) as u16);
        assert!(peak(&samples[..200]) < 300 && peak(&samples[samples.len() - 200..]) < 300);
        assert_eq!(d, whoosh_wav());
    }
}
