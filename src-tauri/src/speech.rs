//! Her voice: an offline neural voice (Piper "Kristin", public domain) run by sherpa-onnx, the same on
//! Windows, macOS and Linux. The voice model (about 21 MB) is downloaded once into the app data folder;
//! after that everything runs locally. Text is phonemized with espeak-ng (via piper-phonemize, GPL-3.0).

use base64::Engine;
use sherpa_onnx::{GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsModelConfig, OfflineTtsVitsModelConfig};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const VOICE: &str = "vits-piper-en_US-kristin-medium-int8";
const URL: &str = "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/vits-piper-en_US-kristin-medium-int8.tar.bz2";

pub struct Speech {
    dir: PathBuf, // <app data>/voices
    tts: Mutex<Option<std::sync::Arc<OfflineTts>>>,
}

impl Speech {
    pub fn new(app_data: &Path) -> Self {
        Self { dir: app_data.join("voices"), tts: Mutex::new(None) }
    }

    fn voice_dir(&self) -> PathBuf { self.dir.join(VOICE) }

    pub fn installed(&self) -> bool { find_model(&self.voice_dir()).is_some() }

    /// Downloads and unpacks the voice if it isn't here yet; `progress` gets 0-100.
    pub fn install(&self, mut progress: impl FnMut(u32)) -> Result<(), String> {
        if self.installed() { return Ok(()); }
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let resp = ureq::get(URL).call().map_err(|e| format!("voice download failed: {e}"))?;
        let total: u64 = resp.header("Content-Length").and_then(|v| v.parse().ok()).unwrap_or(21_000_000);
        let mut buf = Vec::with_capacity(total as usize);
        let mut reader = resp.into_reader();
        let mut chunk = [0u8; 64 * 1024];
        let mut last = 0;
        loop {
            let n = reader.read(&mut chunk).map_err(|e| e.to_string())?;
            if n == 0 { break; }
            buf.extend_from_slice(&chunk[..n]);
            let pct = (buf.len() as u64 * 100 / total.max(1)).min(100) as u32;
            if pct != last { last = pct; progress(pct); }
        }
        // unpack into a temp folder, then rename, so a half-unpacked voice is never used
        let tmp = self.dir.join(format!("{VOICE}.part"));
        let _ = std::fs::remove_dir_all(&tmp);
        tar::Archive::new(bzip2::read::BzDecoder::new(&buf[..])).unpack(&tmp).map_err(|e| e.to_string())?;
        let inner = tmp.join(VOICE);
        let from = if inner.is_dir() { inner } else { tmp.clone() };
        let _ = std::fs::remove_dir_all(self.voice_dir());
        std::fs::rename(&from, self.voice_dir()).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_dir_all(&tmp);
        if self.installed() { Ok(()) } else { Err("the voice archive had no model in it".into()) }
    }

    fn engine(&self) -> Result<std::sync::Arc<OfflineTts>, String> {
        let mut slot = self.tts.lock().unwrap();
        if let Some(t) = slot.as_ref() { return Ok(t.clone()); }
        let dir = self.voice_dir();
        let model = find_model(&dir).ok_or("voice not installed")?;
        let config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                vits: OfflineTtsVitsModelConfig {
                    model: Some(model.to_string_lossy().into_owned()),
                    tokens: Some(dir.join("tokens.txt").to_string_lossy().into_owned()),
                    noise_scale: 0.667,
                    noise_scale_w: 0.8,
                    length_scale: 1.0,
                    ..Default::default()
                },
                num_threads: 2,
                ..Default::default()
            },
            ..Default::default()
        };
        let tts = std::sync::Arc::new(OfflineTts::create(&config).ok_or("couldn't load the voice")?);
        *slot = Some(tts.clone());
        Ok(tts)
    }

    /// One sentence as a 16-bit mono WAV, base64-encoded for the page to play.
    pub fn say(&self, text: &str) -> Result<String, String> {
        let tts = self.engine()?;
        let lang = match tts.lang() { l if l.is_empty() => "en-us".to_string(), l => l };
        let ph = piper_phonemize::phonemize(text, &lang).ok_or("couldn't phonemize that")?;
        let codepoints: Vec<Vec<i32>> = (0..ph.num_sentences()).filter_map(|i| ph.get_phonemes(i)).filter(|p| !p.is_empty()).collect();
        if codepoints.is_empty() { return Ok(String::new()); }
        let config = GenerationConfig { sid: 0, speed: 1.05, phoneme_codepoints: Some(codepoints), ..Default::default() };
        let audio = tts.generate_with_config("", &config, None::<fn(&[f32], f32) -> bool>).ok_or("speech failed")?;
        Ok(base64::engine::general_purpose::STANDARD.encode(wav(audio.samples(), audio.sample_rate() as u32)))
    }
}

/// The voice's .onnx file, wherever the archive put it.
fn find_model(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path())
        .find(|p| p.extension().is_some_and(|x| x == "onnx"))
        .filter(|_| dir.join("tokens.txt").exists())
}

fn wav(samples: &[f32], rate: u32) -> Vec<u8> {
    let data = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
    for s in samples { out.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes()); }
    out
}

#[cfg(test)]
mod tests {
    /// Downloads the real voice (21 MB) into a temp folder and speaks a sentence: `cargo test -- --ignored`
    #[test]
    #[ignore]
    fn speaks() {
        let dir = std::env::temp_dir().join("aiko-voice-test");
        let s = super::Speech::new(&dir);
        s.install(|_| {}).unwrap();
        let b64 = s.say("Hello! I'm Aiko, and I run entirely on your computer.").unwrap();
        let wav = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64).unwrap();
        assert!(wav.len() > 44 + 22050, "expected over half a second of audio, got {} bytes", wav.len());
        std::fs::write(dir.join("hello.wav"), wav).unwrap();
    }

    #[test]
    fn wav_header() {
        let w = super::wav(&[0.0, 1.0, -1.0], 22050);
        assert_eq!(&w[..4], b"RIFF");
        assert_eq!(w.len(), 44 + 6);
        assert_eq!(&w[44..], &[0, 0, 0xFF, 0x7F, 0x01, 0x80]);
    }
}
