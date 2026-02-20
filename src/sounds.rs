use std::fs;
use std::path::Path;
use crate::audio::SoundKind;
use anyhow::Result;

/// Frequencies and durations for each sound type
fn params(kind: &SoundKind) -> (f32, f32) {
    match kind {
        SoundKind::Mini      => (880.0, 0.3),   // A5, short blip
        SoundKind::Milestone => (523.25, 0.8),   // C5, medium chime
        SoundKind::Epic      => (659.25, 1.2),   // E5, triumphant
        SoundKind::Fanfare   => (783.99, 1.5),   // G5, fanfare
        SoundKind::Streak    => (1046.5, 1.5),   // C6, streak celebration
    }
}

pub fn generate_wav(kind: &SoundKind) -> Vec<u8> {
    let (freq, duration) = params(kind);
    let sample_rate: u32 = 44100;
    let num_samples = (sample_rate as f32 * duration) as usize;

    let mut samples: Vec<i16> = Vec::with_capacity(num_samples);
    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        // Sine wave with linear fade-out envelope
        let envelope = 1.0 - (t / duration);
        let sample = (envelope * 0.95 * i16::MAX as f32
            * (2.0 * std::f32::consts::PI * freq * t).sin()) as i16;
        samples.push(sample);
    }

    encode_wav(&samples, sample_rate)
}

fn encode_wav(samples: &[i16], sample_rate: u32) -> Vec<u8> {
    let num_channels: u16 = 1;
    let bits_per_sample: u16 = 16;
    let byte_rate = sample_rate * num_channels as u32 * bits_per_sample as u32 / 8;
    let block_align = num_channels * bits_per_sample / 8;
    let data_size = (samples.len() * 2) as u32;
    let chunk_size = 36 + data_size;

    let mut buf = Vec::with_capacity(44 + samples.len() * 2);
    // RIFF header
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&chunk_size.to_le_bytes());
    buf.extend_from_slice(b"WAVE");
    // fmt chunk
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes());         // chunk size
    buf.extend_from_slice(&1u16.to_le_bytes());          // PCM format
    buf.extend_from_slice(&num_channels.to_le_bytes());
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&byte_rate.to_le_bytes());
    buf.extend_from_slice(&block_align.to_le_bytes());
    buf.extend_from_slice(&bits_per_sample.to_le_bytes());
    // data chunk
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_size.to_le_bytes());
    for s in samples {
        buf.extend_from_slice(&s.to_le_bytes());
    }
    buf
}

pub fn extract_all_sounds(dest: &Path) -> Result<()> {
    fs::create_dir_all(dest)?;
    for kind in [SoundKind::Mini, SoundKind::Milestone, SoundKind::Epic,
                 SoundKind::Fanfare, SoundKind::Streak] {
        let filename = format!("{}.wav", kind.name());
        let path = dest.join(filename);
        if !path.exists() {
            fs::write(&path, generate_wav(&kind))?;
        }
    }
    Ok(())
}

/// Returns a temp WAV path for the given sound, generating it if needed.
pub fn ensure_sound_file(kind: &SoundKind) -> Result<std::path::PathBuf> {
    let tmp_dir = std::env::temp_dir().join("cwinner");
    fs::create_dir_all(&tmp_dir)?;
    let path = tmp_dir.join(format!("{}.wav", kind.name()));
    if !path.exists() {
        fs::write(&path, generate_wav(kind))?;
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_wav_is_valid_wav() {
        let wav = generate_wav(&SoundKind::Mini);
        // WAV header: "RIFF" magic
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert!(wav.len() > 44); // header + data
    }

    #[test]
    fn test_all_sounds_generate() {
        for kind in [SoundKind::Mini, SoundKind::Milestone, SoundKind::Epic,
                     SoundKind::Fanfare, SoundKind::Streak] {
            let wav = generate_wav(&kind);
            assert!(wav.len() > 100, "{:?} generated empty WAV", kind);
        }
    }

    #[test]
    fn test_extract_all_sounds_creates_files() {
        let tmp = tempfile::tempdir().unwrap();
        extract_all_sounds(tmp.path()).unwrap();
        for name in ["mini.wav", "milestone.wav", "epic.wav", "fanfare.wav", "streak.wav"] {
            assert!(tmp.path().join(name).exists(), "{} missing", name);
        }
    }
}
