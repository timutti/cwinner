use std::fs;
use std::path::Path;
use crate::audio::SoundKind;
use anyhow::Result;

const SAMPLE_RATE: u32 = 48000;
const PI2: f32 = 2.0 * std::f32::consts::PI;

/// A single note with frequency, start time, duration, and amplitude.
struct Note {
    freq: f32,
    start: f32,
    dur: f32,
    amp: f32,
}

/// Render a sequence of notes into samples with fade-out envelopes.
fn render_notes(notes: &[Note], total_duration: f32) -> Vec<i16> {
    let num_samples = (SAMPLE_RATE as f32 * total_duration) as usize;
    let mut samples = vec![0f32; num_samples];

    for note in notes {
        let start_idx = (SAMPLE_RATE as f32 * note.start) as usize;
        let note_samples = (SAMPLE_RATE as f32 * note.dur) as usize;
        for i in 0..note_samples {
            let idx = start_idx + i;
            if idx >= num_samples { break; }
            let t = i as f32 / SAMPLE_RATE as f32;
            // Smooth fade: quick attack (5ms), then exponential decay
            let attack = (t / 0.005).min(1.0);
            let decay = (-3.0 * t / note.dur).exp();
            let envelope = attack * decay;
            samples[idx] += envelope * note.amp * (PI2 * note.freq * t).sin();
        }
    }

    // Normalize to prevent clipping
    let peak = samples.iter().map(|s| s.abs()).fold(0f32, f32::max);
    let scale = if peak > 0.0 { 0.9 / peak } else { 1.0 };

    samples.iter()
        .map(|&s| (s * scale * i16::MAX as f32) as i16)
        .collect()
}

/// Each sound kind has a unique multi-note melody.
fn sound_notes(kind: &SoundKind) -> (Vec<Note>, f32) {
    match kind {
        // Mini: quick double-tap notification (two short pops)
        SoundKind::Mini => {
            let notes = vec![
                Note { freq: 1318.5, start: 0.0, dur: 0.06, amp: 0.7 },  // E6
                Note { freq: 1568.0, start: 0.08, dur: 0.06, amp: 0.5 }, // G6
            ];
            (notes, 0.2)
        }
        // Milestone: pleasant rising two-note chime
        SoundKind::Milestone => {
            let notes = vec![
                Note { freq: 523.25, start: 0.0, dur: 0.3, amp: 0.8 },  // C5
                Note { freq: 659.25, start: 0.15, dur: 0.4, amp: 0.9 }, // E5
            ];
            (notes, 0.6)
        }
        // Epic: C major chord with a swell (3 simultaneous notes)
        SoundKind::Epic => {
            let notes = vec![
                Note { freq: 261.63, start: 0.0, dur: 0.8, amp: 0.7 },  // C4
                Note { freq: 329.63, start: 0.05, dur: 0.8, amp: 0.6 }, // E4
                Note { freq: 392.00, start: 0.1, dur: 0.8, amp: 0.6 },  // G4
                Note { freq: 523.25, start: 0.15, dur: 0.7, amp: 0.5 }, // C5 (octave)
            ];
            (notes, 1.0)
        }
        // Fanfare: ascending four-note trumpet call
        SoundKind::Fanfare => {
            let notes = vec![
                Note { freq: 523.25, start: 0.0, dur: 0.2, amp: 0.8 },  // C5
                Note { freq: 659.25, start: 0.18, dur: 0.2, amp: 0.8 }, // E5
                Note { freq: 783.99, start: 0.36, dur: 0.2, amp: 0.9 }, // G5
                Note { freq: 1046.5, start: 0.54, dur: 0.6, amp: 1.0 }, // C6 (held)
            ];
            (notes, 1.2)
        }
        // Streak: rapid ascending scale with echo
        SoundKind::Streak => {
            let scale = [523.25, 587.33, 659.25, 783.99, 880.0, 1046.5, 1174.7, 1318.5];
            let mut notes: Vec<Note> = Vec::new();
            for (i, &freq) in scale.iter().enumerate() {
                let start = i as f32 * 0.08;
                notes.push(Note { freq, start, dur: 0.25, amp: 0.7 });
                // Echo at half volume
                notes.push(Note { freq, start: start + 0.12, dur: 0.15, amp: 0.3 });
            }
            // Final held chord
            notes.push(Note { freq: 1046.5, start: 0.7, dur: 0.8, amp: 0.8 }); // C6
            notes.push(Note { freq: 1318.5, start: 0.75, dur: 0.7, amp: 0.6 }); // E6
            (notes, 1.6)
        }
    }
}

pub fn generate_wav(kind: &SoundKind) -> Vec<u8> {
    let (notes, total_duration) = sound_notes(kind);
    let samples = render_notes(&notes, total_duration);
    encode_wav(&samples, SAMPLE_RATE)
}

/// Encode mono samples as stereo WAV (HDMI/DisplayPort requires stereo).
fn encode_wav(samples: &[i16], sample_rate: u32) -> Vec<u8> {
    let num_channels: u16 = 2;
    let bits_per_sample: u16 = 16;
    let byte_rate = sample_rate * num_channels as u32 * bits_per_sample as u32 / 8;
    let block_align = num_channels * bits_per_sample / 8;
    let data_size = (samples.len() as u32) * 2 * num_channels as u32;
    let chunk_size = 36 + data_size;

    let mut buf = Vec::with_capacity(44 + data_size as usize);
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
    // Duplicate each mono sample to left + right channel
    for s in samples {
        let bytes = s.to_le_bytes();
        buf.extend_from_slice(&bytes); // left
        buf.extend_from_slice(&bytes); // right
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

    #[test]
    fn test_sounds_have_distinct_lengths() {
        let mini = generate_wav(&SoundKind::Mini);
        let fanfare = generate_wav(&SoundKind::Fanfare);
        let streak = generate_wav(&SoundKind::Streak);
        // Mini should be much shorter than fanfare/streak
        assert!(mini.len() < fanfare.len(), "Mini should be shorter than Fanfare");
        assert!(fanfare.len() < streak.len(), "Fanfare should be shorter than Streak");
    }
}
