//! Second brain: the user's notes and documents (.md / .txt) in a folder, split into chunks and embedded
//! locally (nomic-embed-text through Ollama) so she can recall them by meaning. The index is a JSON file;
//! only new or changed files get re-embedded.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::{fs, sync::Mutex, time::UNIX_EPOCH};

const OLLAMA: &str = "http://localhost:11434";
const CHUNK: usize = 800; // characters per chunk
const LEARNED: &str = "Learned from the web"; // subfolder for what she keeps from trusted sites
const MAX_FILE: u64 = 2_000_000; // bigger files are skipped

#[derive(Serialize, Deserialize)]
struct Chunk {
    text: String,
    emb: Vec<f32>,
}

#[derive(Serialize, Deserialize)]
struct Doc {
    path: String,
    mtime: u64,
    chunks: Vec<Chunk>,
}

pub struct Hit {
    pub score: f32,
    pub note: String,
    pub text: String,
}

pub struct Brain {
    pub dir: PathBuf,
    index: PathBuf,
    model: String,
    docs: Mutex<Vec<Doc>>,
    syncing: Mutex<()>,
}

impl Brain {
    pub fn open(dir: PathBuf, index: PathBuf, model: &str) -> Self {
        fs::create_dir_all(&dir).ok();
        let docs = fs::read(&index).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        Self { dir, index, model: model.into(), docs: Mutex::new(docs), syncing: Mutex::new(()) }
    }

    /// Embeds new or changed files and forgets deleted ones. Returns (files indexed, files removed).
    pub fn sync(&self) -> Result<(usize, usize), String> {
        let _one_at_a_time = self.syncing.lock().unwrap();
        let mut found = vec![];
        walk(&self.dir, &mut found);
        let known: Vec<(String, u64)> = self.docs.lock().unwrap().iter().map(|d| (d.path.clone(), d.mtime)).collect();

        // embedding is slow, so it happens without holding the index lock
        let mut fresh = vec![];
        for (path, mtime) in &found {
            if known.iter().any(|(p, m)| p == path && m == mtime) { continue; }
            let text = fs::read(path).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
            let pieces = chunk(text.trim_start_matches('\u{feff}')); // Windows editors often add a BOM
            let inputs: Vec<String> = pieces.iter().map(|c| format!("search_document: {}\n{c}", note_name(path))).collect();
            let embs = embed(&self.model, &inputs)?;
            fresh.push(Doc { path: path.clone(), mtime: *mtime, chunks: pieces.into_iter().zip(embs).map(|(text, emb)| Chunk { text, emb }).collect() });
        }
        let removed = known.iter().filter(|(p, _)| !found.iter().any(|(f, _)| f == p)).count();
        if fresh.is_empty() && removed == 0 { return Ok((0, 0)); }

        let mut docs = self.docs.lock().unwrap();
        docs.retain(|d| found.iter().any(|(p, m)| *p == d.path && *m == d.mtime));
        let indexed = fresh.len();
        docs.extend(fresh);
        // write-then-rename so a crash mid-write can't corrupt the index
        let tmp = self.index.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec(&*docs).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        fs::rename(tmp, &self.index).map_err(|e| e.to_string())?;
        Ok((indexed, removed))
    }

    pub fn search(&self, query: &str, k: usize) -> Vec<Hit> {
        let Ok(q) = embed(&self.model, &[format!("search_query: {query}")]) else { return vec![] };
        let docs = self.docs.lock().unwrap();
        let mut scored: Vec<(f32, usize, usize)> = docs.iter().enumerate()
            .flat_map(|(di, d)| d.chunks.iter().enumerate().map(move |(ci, c)| (di, ci, c)))
            .map(|(di, ci, c)| (crate::cosine(&q[0], &c.emb), di, ci))
            .collect();
        scored.sort_by(|a, b| b.0.total_cmp(&a.0));
        scored.into_iter().take(k)
            .map(|(score, di, ci)| Hit { score, note: note_name(&docs[di].path), text: docs[di].chunks[ci].text.clone() })
            .collect()
    }

    /// Writes a new note (never overwrites) and indexes it right away. Returns its file name.
    pub fn save_note(&self, title: &str, text: &str) -> Result<String, String> {
        self.write_note(&self.dir, title, text)
    }

    /// A note of facts she read on a trusted site, kept apart from the user's own notes.
    pub fn learn(&self, title: &str, text: &str) -> Result<String, String> {
        let dir = self.dir.join(LEARNED);
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        self.write_note(&dir, title, text)
    }

    /// True if she already learned from this page or search (its source line is in a learned note).
    pub fn knows(&self, source: &str) -> bool {
        let Ok(files) = fs::read_dir(self.dir.join(LEARNED)) else { return false };
        files.flatten().any(|f| fs::read_to_string(f.path())
            .is_ok_and(|t| t.lines().any(|l| l.split_once(": ").is_some_and(|(_, k)| k == source))))
    }

    fn write_note(&self, dir: &Path, title: &str, text: &str) -> Result<String, String> {
        let safe: String = title.chars().take(80).map(|c| if c.is_alphanumeric() || " -_".contains(c) { c } else { '_' }).collect();
        let safe = match safe.trim() { "" => "note", s => s };
        let mut path = dir.join(format!("{safe}.md"));
        for n in 2.. {
            if !path.exists() { break; }
            path = dir.join(format!("{safe} {n}.md"));
        }
        fs::write(&path, format!("# {title}\n\n{text}\n")).map_err(|e| e.to_string())?;
        self.sync()?;
        Ok(path.file_name().unwrap().to_string_lossy().into_owned())
    }

    /// Appends "- HH:MM activity" to Screen journal/<date>.md, skipping repeats of the last entry.
    /// Date and time come from the UI, which knows the local time zone. The minute sync indexes it.
    pub fn journal(&self, date: &str, time: &str, activity: &str) -> Result<(), String> {
        let date: String = date.chars().filter(|c| c.is_ascii_digit() || *c == '-').collect();
        let dir = self.dir.join("Screen journal");
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join(format!("{date}.md"));
        let old = fs::read_to_string(&path).unwrap_or_else(|_| format!("# Screen journal {date}\n\n"));
        let activity = activity.replace('\n', " ");
        if old.lines().last().is_some_and(|l| l.ends_with(&activity)) { return Ok(()); }
        fs::write(&path, format!("{old}- {time} {activity}\n")).map_err(|e| e.to_string())
    }

    /// (files, chunks) in the index
    pub fn stats(&self) -> (usize, usize) {
        let docs = self.docs.lock().unwrap();
        (docs.len(), docs.iter().map(|d| d.chunks.len()).sum())
    }
}

fn note_name(path: &str) -> String {
    Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

/// .md / .txt files under dir, with modification times
fn walk(dir: &Path, out: &mut Vec<(String, u64)>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let path = e.path();
        let Ok(meta) = e.metadata() else { continue };
        if meta.is_dir() { walk(&path, out); continue; }
        let ext = path.extension().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
        if !["md", "markdown", "txt"].contains(&ext.as_str()) || meta.len() > MAX_FILE { continue; }
        let mtime = meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
        out.push((path.to_string_lossy().into_owned(), mtime));
    }
}

/// Paragraph-aware chunks of about CHUNK characters; long paragraphs are cut into CHUNK-sized pieces.
fn chunk(text: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    for para in text.split("\n\n").map(str::trim).filter(|p| !p.is_empty()) {
        if cur.chars().count() + para.chars().count() > CHUNK && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        if para.chars().count() > CHUNK {
            let chars: Vec<char> = para.chars().collect();
            out.extend(chars.chunks(CHUNK).map(|c| c.iter().collect::<String>()));
        } else {
            if !cur.is_empty() { cur.push_str("\n\n"); }
            cur.push_str(para);
        }
    }
    if !cur.is_empty() { out.push(cur); }
    out
}

fn embed(model: &str, inputs: &[String]) -> Result<Vec<Vec<f32>>, String> {
    let mut out = vec![];
    for batch in inputs.chunks(16) {
        let v: Value = ureq::post(&format!("{OLLAMA}/api/embed"))
            .send_json(json!({ "model": model, "input": batch, "keep_alive": "2m" }))
            .map_err(|e| e.to_string())?
            .into_json()
            .map_err(|e| e.to_string())?;
        let embs: Vec<Vec<f32>> = serde_json::from_value(v["embeddings"].clone()).map_err(|e| e.to_string())?;
        out.extend(embs);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn journal_appends_and_skips_repeats() {
        let dir = std::env::temp_dir().join("companion-brain-test");
        fs::remove_dir_all(&dir).ok();
        let brain = Brain::open(dir.clone(), dir.join("index.json"), "unused");
        brain.journal("2026-11-03", "15:40", "editing Rust code in VS Code").unwrap();
        brain.journal("2026-11-03", "15:41", "editing Rust code in VS Code").unwrap();
        brain.journal("2026-11-03", "15:45", "reading docs in Chrome").unwrap();
        let text = fs::read_to_string(dir.join("Screen journal").join("2026-11-03.md")).unwrap();
        assert_eq!(text, "# Screen journal 2026-11-03\n\n- 15:40 editing Rust code in VS Code\n- 15:45 reading docs in Chrome\n");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn chunks_respect_size_and_paragraphs() {
        let text = format!("a\n\nb\n\n{}", "x".repeat(2000));
        let c = chunk(&text);
        assert_eq!(c[0], "a\n\nb");
        assert!(c.iter().all(|p| p.chars().count() <= CHUNK));
        assert_eq!(c.iter().map(|p| p.chars().count()).sum::<usize>(), 4 + 2000);
    }
}
