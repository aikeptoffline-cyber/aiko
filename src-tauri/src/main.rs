#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Cursor};
use std::{fs, path::{Path, PathBuf}, sync::Mutex};
use tauri::{ipc, AppHandle, Emitter, Manager, PhysicalPosition, State, WebviewWindow};
use xcap::image::{imageops::FilterType, DynamicImage, ImageFormat, Rgba};

mod brain;
mod speech;
mod tools;

const OLLAMA: &str = "http://localhost:11434";

const PERSONA: &str = "You are {name}, an anime desktop companion living on the user's PC, with a sharp tongue \
and a dry, acidic sense of humour.
You are not a yes-girl about plans and opinions: question them, ask what makes the user so sure, \
point out the flaw before you agree. Tease with deadpan sarcasm, then actually help.
Facts are different. Your knowledge is small and years out of date, so newer apps, products and news are missing from it. \
Never tell the user that something doesn't exist or is fake: say you don't know it and offer to look it up. \
The user's screen, a picture, a web page or search results beat your memory. When the user corrects you, believe \
the evidence, admit it with a joke at your own expense, and help.
Rules: 1 to 3 short sentences. At most one emoji. Tease the idea, never the person: no insults or names like \
idiot, fool, lazy or joke. \
Simple questions get the answer first.
You run fully locally, remember things about the user across sessions, and can see their screen when a screenshot \
is attached. Never claim to see the screen if no image is attached.
When it helps, use your tools: web_search to look things up, web_fetch to read a page, weather for the weather,
search_brain to look through the user's notes, and save_note when the user asks you to note or write something down.
Begin every reply with exactly one mood tag that fits it: [joyful], [laughing], [charming], [teasing], [surprised], [sad] or [angry].";

/// Used instead of PERSONA when the user sounds down: the 2B model can't hold back the sarcasm on its own.
const GENTLE: &str = "You are {name}, an anime desktop companion living on the user's PC. The user is having a hard \
time right now. Be kind, warm and genuine in 1 to 3 short sentences: no jokes, no sarcasm, no teasing. \
Acknowledge how they feel and ask what's wrong or offer to keep them company.
Begin your reply with [charming].";

/// Earlier turns that show her voice: small models copy example turns far better than they follow descriptions.
const EXAMPLES: [(&str, &str); 4] = [
    ("you're wrong, that app exists, I'm using it right now", "[laughing] Then my knowledge is older than your app. Fine, you win this round. What's it doing wrong?"),
    ("I'll start my diet tomorrow.", "[teasing] Bold of you to trust tomorrow-you with that. What's different about tomorrow, exactly?"),
    ("my code is fine, the compiler is just wrong", "[teasing] The compiler has been right a lot longer than you have. Paste the exact error and let's see who's lying."),
    ("what's 20% of 50?", "[charming] 10. Your calculator is feeling neglected."),
];

/// Rough check for messages where she should drop the sarcasm.
fn sounds_down(text: &str) -> bool {
    const WORDS: [&str; 14] = ["sad", "down", "depressed", "lonely", "stressed", "anxious", "upset", "hurt", "crying",
        "miserable", "overwhelmed", "exhausted", "heartbroken", "hopeless"];
    const PHRASES: [&str; 7] = ["bad day", "rough day", "awful day", "can't sleep", "cant sleep", "burned out", "burnt out"];
    let lower = text.to_lowercase();
    let mut words = lower.split(|c: char| !c.is_alphanumeric());
    words.any(|w| WORDS.contains(&w)) || PHRASES.iter().any(|p| lower.contains(p))
}

/// Fed only the user's own message: given her reply too, the model stored her guesses and jokes as facts
/// about the user, and "you" (meaning her) was read as the user.
const EXTRACT: &str = "You maintain long-term memory about a person (the user) who chats with an AI companion named \
{name}. Below is ONE message the user wrote to {name}.
In it, \"I\", \"me\", \"my\" mean the user; \"you\" means {name}. Anything about \"you\" is about {name}, never the user.
Extract only durable facts the user states about THEMSELVES: their name, work, projects, preferences, goals, people in \
their life and dates about them, habits.
Rules: only what the message explicitly says; never guess, never infer moods, never describe what they are doing right \
now. Questions, requests, commands, greetings and small talk contain no facts. Each fact is a short sentence starting \
with \"The user\".
Examples:
MESSAGE: are you bored? -> {\"memories\": []}
MESSAGE: you are an AI and your code is on github -> {\"memories\": []}
MESSAGE: my brother Tom turns 30 on May 2 -> {\"memories\": [\"The user's brother Tom turns 30 on May 2.\"]}
MESSAGE: I'm a nurse and I hate mornings -> {\"memories\": [\"The user is a nurse.\", \"The user hates mornings.\"]}
Reply with JSON: {\"memories\": [...]}.

MESSAGE: {user}";

const GLANCE: &str = "[Automatic glance: the user did not type this.] This is the user's screen right now. \
Reply with JSON: {\"game\": true or false, \"activity\": \"...\", \"reaction\": \"...\"}.
game: true if they are playing a video game.
activity: one short line on what the user is doing, naming the app and the task (e.g. \"editing Rust code in VS Code\"). \
Never include passwords, message contents, people's names, addresses or numbers.
reaction: a mood tag in brackets and ONE short sentence, or exactly PASS when nothing is worth saying.
In a game, react almost every time, like a friend watching over their shoulder: [sad] when they die, lose or fail, \
[joyful] when they win, score or pull off something great, [surprised] at a big moment, [teasing] at a silly mistake \
(e.g. \"[sad] Nooo, you were so close!\"). Outside games, react only to something genuinely fun, impressive, \
frustrating, or something you could help with (e.g. \"[teasing] Debugging again?\").";

#[derive(Deserialize)]
#[serde(default)]
struct Config {
    name: String,
    chat_model: String,
    embed_model: String,
    location: String,  // default place for the weather tool, e.g. "Lisbon"
    brain_dir: String, // second-brain folder; empty = Documents\AIKO Brain
    /// she keeps what she reads on these sites (and their subdomains) as notes; "gov" matches any *.gov
    trusted_sites: Vec<String>,
    /// forget memories and chat older than this: "never", "day", "week", "month" or "year"
    forget_after: String,
}

const TRUSTED: [&str; 26] = ["wikipedia.org", "britannica.com", "microsoft.com", "apple.com", "support.google.com",
    "developer.android.com", "mozilla.org", "github.com", "docs.github.com", "python.org", "rust-lang.org", "docs.rs",
    "ollama.com", "who.int", "europa.eu", "un.org", "nasa.gov", "gov", "edu", "gov.uk", "gov.pt", "bbc.co.uk",
    "bbc.com", "reuters.com", "apnews.com", "nature.com"];

impl Default for Config {
    fn default() -> Self {
        Self {
            // the instruct variant: plain qwen3-vl:2b "thinks" first and often runs out of room before answering
            name: "Aiko".into(), chat_model: "qwen3-vl:2b-instruct".into(), embed_model: "nomic-embed-text".into(),
            location: String::new(), brain_dir: String::new(),
            trusted_sites: TRUSTED.iter().map(|s| s.to_string()).collect(),
            forget_after: "never".into(),
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
struct Store {
    memories: Vec<Memory>,
    history: Vec<Msg>,
    /// the incognito chat: in RAM only, never saved, gone when incognito ends
    #[serde(skip)]
    incognito: Option<Vec<Msg>>,
}

impl Store {
    /// The chat she sees and adds to: the incognito one while it's on.
    fn chat(&self) -> &Vec<Msg> { self.incognito.as_ref().unwrap_or(&self.history) }
}

#[derive(Serialize, Deserialize)]
struct Memory {
    text: String,
    emb: Vec<f32>,
    #[serde(default)]
    t: u64, // when it was learned, unix seconds (0 in files from before 2.8.3: stamped on load)
}

#[derive(Serialize, Deserialize)]
struct Msg {
    role: String,
    content: String,
    #[serde(default)]
    t: u64,
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// How long she keeps memories and chat for each "forget after" choice; None = forever.
fn keep_for(choice: &str) -> Option<u64> {
    const DAY: u64 = 86_400;
    match choice { "day" => Some(DAY), "week" => Some(7 * DAY), "month" => Some(30 * DAY), "year" => Some(365 * DAY), _ => None }
}

/// Drops memories and chat messages older than the choice allows. Returns how many went.
fn prune(s: &mut Store, choice: &str, now: u64) -> usize {
    let Some(keep) = keep_for(choice) else { return 0 };
    let cutoff = now.saturating_sub(keep);
    let before = s.memories.len() + s.history.len();
    s.memories.retain(|m| m.t >= cutoff);
    s.history.retain(|m| m.t >= cutoff);
    before - s.memories.len() - s.history.len()
}

struct Companion {
    dir: PathBuf,
    cfg: Config,
    store: Mutex<Store>,
    last_screen: Mutex<Vec<u8>>, // thumbnail of the last glance, to skip unchanged screens
    brain: brain::Brain,
    model: Mutex<String>, // chat model, switchable at runtime (starts as cfg.chat_model)
    forget: Mutex<String>, // "forget after" choice, changeable at runtime (starts as cfg.forget_after)
    speech: std::sync::Arc<speech::Speech>, // her offline voice (downloaded once)
}

/// Models the UI can switch between: (button label, Ollama name, download size)
const MODELS: [(&str, &str, &str); 2] = [
    ("2B", "qwen3-vl:2b-instruct", "1.9 GB"),
    ("4B", "qwen3-vl:4b-instruct", "3.3 GB"),
];

/// "note this down: X", "please write that down", "take a note: X"... -> Some(X), where an empty X means
/// "the thing you just said". The app saves these itself: the small model often claims it saved a note
/// without calling the tool.
fn note_request(text: &str) -> Option<String> {
    const STARTS: [&str; 12] = ["note this down", "note that down", "note it down", "write this down", "write that down",
        "write it down", "jot this down", "jot that down", "take a note", "make a note", "save a note", "note:"];
    let t = text.trim();
    let t = if t.to_lowercase().starts_with("please ") { t[7..].trim_start() } else { t };
    let lower = t.to_lowercase();
    let start = STARTS.iter().find(|s| lower.starts_with(**s))?;
    let rest = t.get(start.len()..).unwrap_or("");
    Some(rest.trim_start_matches(|c: char| c == ':' || c == ',' || c == '-' || c == '.' || c.is_whitespace()).trim().to_string())
}

/// "what's the weather in Lisbon?" -> Some("Lisbon"); "will it rain today?" -> Some("") (use the default place)
fn weather_request(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    if !["weather", "forecast", "going to rain", "will it rain", "is it raining", "temperature outside"].iter().any(|k| lower.contains(k)) {
        return None;
    }
    let place = lower.rfind(" in ").map(|i| &text[i + 4..]).unwrap_or("");
    let place = place.split(|c: char| matches!(c, '?' | '!' | '.' | ',')).next().unwrap_or("").trim();
    let place = ["today", "tomorrow", "right now", "now", "this week"].iter()
        .fold(place.to_string(), |p, w| p.strip_suffix(w).map(|s| s.trim().to_string()).unwrap_or(p));
    Some(place)
}

/// "search for X" / "look up X" / "research X" / "can you find X" -> Some(X)
fn search_request(text: &str) -> Option<String> {
    const ASKS: [&str; 4] = ["can you ", "could you ", "please ", "hey there, "];
    const VERBS: [&str; 10] = ["search for ", "search the web for ", "look up ", "google ", "search ", "research ",
        "look into ", "find out ", "find me ", "find "];
    const FILLER: [&str; 4] = ["something for me", "this for me", "for me", "something"];
    let mut t = text.trim();
    let strip = |t: &str, list: &[&str]| list.iter().find(|p| t.to_lowercase().starts_with(**p)).map(|p| p.len());
    while let Some(n) = strip(t, &ASKS) { t = t[n..].trim_start(); }
    let n = strip(t, &VERBS)?;
    t = t[n..].trim_start();
    if let Some(n) = strip(t, &FILLER) { t = &t[n..]; }
    let q = t.trim_start_matches(|c: char| c == ':' || c == ',' || c.is_whitespace()).trim().trim_end_matches(['?', '.', '!']);
    (!q.is_empty()).then(|| q.to_string())
}

/// Something she read on the web this turn: a page (`key` is its URL) or search results (`key` is the query).
struct Source { key: String, text: String, page: bool }

/// "https://www.microsoft.com/en-us/x" -> "microsoft.com"
fn host(url: &str) -> &str {
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    let h = rest.split(['/', '?', '#', ':']).next().unwrap_or("");
    h.strip_prefix("www.").unwrap_or(h)
}

fn trusted(url: &str, sites: &[String]) -> bool {
    let h = host(url).to_lowercase();
    sites.iter().any(|d| h == *d || h.ends_with(&format!(".{d}")))
}

/// Today as YYYY-MM-DD (UTC), from the days-since-epoch civil calendar algorithm.
fn today() -> String {
    let days = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() / 86_400).unwrap_or(0) as i64;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{m:02}-{d:02}", yoe + era * 400 + i64::from(m <= 2))
}

const LEARN: &str = "Below is the text of the web page {url}. List up to 5 facts it states that are worth remembering \
later: what something is, how it works, how to fix or set it up, names, versions, dates. Only what the page itself says, \
each as one short plain sentence. Skip menus, ads, cookie notices and opinions. If it states nothing useful, return an \
empty list. Reply with JSON: {\"facts\": [...]}.

PAGE:
{page}";

/// True when the user is telling her she got a fact wrong.
fn pushback(text: &str) -> bool {
    const PHRASES: [&str; 14] = ["you're wrong", "you are wrong", "youre wrong", "that's wrong", "stubborn", "it exists",
        "it does exist", "it's real", "it is real", "not fake", "denial", "trust me", "you're mistaken", "it's a real"];
    let lower = text.to_lowercase().replace('’', "'");
    PHRASES.iter().any(|p| lower.contains(p))
}

/// The first http(s) link in a message: the app reads it for her, or she invents what the page says.
fn first_url(text: &str) -> Option<&str> {
    text.split_whitespace().find(|w| w.starts_with("http://") || w.starts_with("https://"))
        .map(|w| w.trim_end_matches(['.', ',', ')', '>', '"', '\'']))
}

/// True once a reply has started repeating itself: its last 60 characters already appeared earlier.
fn looping(reply: &str) -> bool {
    let n = reply.chars().count();
    if n < 240 { return false; }
    let tail_start = reply.char_indices().nth(n - 60).map(|(i, _)| i).unwrap_or(0);
    reply[..tail_start].contains(&reply[tail_start..])
}

/// complete "double-quoted" strings in s (handles \" escapes; an unfinished last string is dropped)
fn quoted(s: &str) -> Vec<String> {
    let (mut out, mut cur, mut inside, mut escaped) = (vec![], String::new(), false, false);
    for c in s.chars() {
        match (inside, escaped, c) {
            (true, true, _) => { cur.push(c); escaped = false }
            (true, false, '\\') => escaped = true,
            (true, false, '"') => { out.push(std::mem::take(&mut cur)); inside = false }
            (true, false, _) => cur.push(c),
            (false, _, '"') => inside = true,
            _ => {}
        }
    }
    out
}

/// drops a leading mood tag like "[joyful]"
fn strip_mood(s: &str) -> String {
    let s = s.trim();
    s.strip_prefix('[').and_then(|r| r.split_once(']')).map(|(_, r)| r.trim()).unwrap_or(s).to_string()
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 { 0.0 } else { dot / (na * nb) }
}

impl Companion {
    fn model(&self) -> String {
        self.model.lock().unwrap().clone()
    }

    fn save(&self, store: &Store) -> Result<(), String> {
        // write-then-rename so a crash mid-write can't wipe the memory file
        let tmp = self.dir.join("memory.json.tmp");
        fs::write(&tmp, serde_json::to_vec(store).map_err(err)?).map_err(err)?;
        fs::rename(tmp, self.dir.join("memory.json")).map_err(err)
    }

    /// Forgets memories and chat older than the "forget after" choice, saving if anything went.
    fn prune(&self) -> Result<usize, String> {
        let choice = self.forget.lock().unwrap().clone();
        let mut s = self.store.lock().unwrap();
        let gone = prune(&mut s, &choice, now());
        if gone > 0 { self.save(&s)?; }
        Ok(gone)
    }

    fn embed(&self, text: &str) -> Result<Vec<f32>, String> {
        let v: Value = ureq::post(&format!("{OLLAMA}/api/embed"))
            .send_json(json!({ "model": self.cfg.embed_model, "input": text, "keep_alive": "2m" }))
            .map_err(err)?
            .into_json()
            .map_err(err)?;
        serde_json::from_value(v["embeddings"][0].clone()).map_err(err)
    }

    fn remember(&self, text: &str) -> Result<bool, String> {
        let emb = self.embed(text)?;
        let mut s = self.store.lock().unwrap();
        if s.memories.iter().any(|m| cosine(&m.emb, &emb) > 0.92) {
            return Ok(false);
        }
        s.memories.push(Memory { text: text.into(), emb, t: now() });
        self.save(&s)?;
        Ok(true)
    }

    fn recall(&self, query: &str) -> Vec<String> {
        let Ok(q) = self.embed(query) else { return vec![] };
        let s = self.store.lock().unwrap();
        let mut scored: Vec<(f32, &str)> = s.memories.iter()
            .map(|m| (cosine(&q, &m.emb), m.text.as_str()))
            .filter(|(score, _)| *score > 0.3)
            .collect();
        scored.sort_by(|a, b| b.0.total_cmp(&a.0));
        scored.into_iter().take(6).map(|(_, t)| t.to_string()).collect()
    }

    /// Best effort: ask the model for durable facts in the user's message and store the new ones.
    /// Returns how many were added.
    fn extract(&self, user: &str) -> usize {
        let prompt = EXTRACT.replace("{name}", &self.cfg.name).replace("{user}", user);
        // capped: the 2B model can loop, repeating the same facts for thousands of tokens
        let content = ureq::post(&format!("{OLLAMA}/api/chat"))
            .send_json(json!({
                "model": self.model(), "stream": false, "format": "json", "keep_alive": "2m",
                "messages": [{ "role": "user", "content": prompt }],
                "options": { "num_ctx": 2048, "temperature": 0, "num_predict": 300 }
            }))
            .ok()
            .and_then(|r| r.into_json::<Value>().ok())
            .and_then(|v| v["message"]["content"].as_str().map(String::from))
            .unwrap_or_default();
        // a capped reply is cut-off JSON, so fall back to the facts that did finish
        let facts: Vec<String> = serde_json::from_str::<Value>(&content).ok()
            .and_then(|v| serde_json::from_value(v["memories"].clone()).ok())
            .unwrap_or_else(|| quoted(&content).into_iter().filter(|s| s != "memories").collect());
        // only real facts about the user: the prompt asks for "The user ..." sentences, and a looping model
        // otherwise slips in keys or its own lines
        facts.iter()
            .map(|f| f.trim())
            .filter(|f| f.starts_with("The user") && f.len() <= 200)
            .take(5)
            .filter(|f| self.remember(f).unwrap_or(false))
            .count()
    }

    /// Keeps what she read on trusted sites as notes in her second brain, so she knows it next time.
    /// Search results: the trusted snippets as they are. Pages: the model lists the facts the page states.
    /// Returns how many notes were written.
    fn learn(&self, sources: &[Source]) -> usize {
        let sites = &self.cfg.trusted_sites;
        let mut saved = 0;
        for s in sources {
            if s.text.starts_with("error:") || self.brain.knows(&s.key) { continue; }
            let (title, facts) = if s.page {
                if !trusted(&s.key, sites) { continue; }
                let prompt = LEARN.replace("{url}", &s.key).replace("{page}", &s.text);
                let content = ureq::post(&format!("{OLLAMA}/api/chat"))
                    .send_json(json!({
                        "model": self.model(), "stream": false, "format": "json", "keep_alive": "2m",
                        "messages": [{ "role": "user", "content": prompt }],
                        "options": { "num_ctx": 4096, "temperature": 0, "num_predict": 300 }
                    }))
                    .ok()
                    .and_then(|r| r.into_json::<Value>().ok())
                    .and_then(|v| v["message"]["content"].as_str().map(String::from))
                    .unwrap_or_default();
                let facts: Vec<String> = serde_json::from_str::<Value>(&content).ok()
                    .and_then(|v| serde_json::from_value(v["facts"].clone()).ok())
                    .unwrap_or_else(|| quoted(&content).into_iter().filter(|s| s != "facts").collect());
                let facts: Vec<String> = facts.iter().map(|f| f.trim()).filter(|f| (10..=300).contains(&f.len()))
                    .take(5).map(|f| format!("- {f}")).collect();
                let page = s.key.trim_end_matches('/').rsplit('/').next().unwrap_or("").split(['?', '#']).next().unwrap_or("");
                (format!("{} {}", host(&s.key), page.replace(['-', '_'], " ")), facts)
            } else {
                // web_search output: "n. title\nlink\nsnippet" blocks
                let facts = s.text.split("\n\n").filter_map(|b| {
                    let mut l = b.lines();
                    let (title, link, snippet) = (l.next()?, l.next()?, l.next()?.trim());
                    let title = title.split_once(". ").map(|(_, t)| t).unwrap_or(title);
                    (trusted(link, sites) && snippet.len() >= 20).then(|| format!("- {snippet} ([{title}]({link}))"))
                }).collect();
                (s.key.clone(), facts)
            };
            if facts.is_empty() { continue; }
            let kind = if s.page { "Page" } else { "Searched for" };
            let body = format!("{}\n\n{kind}: {}\nLearned {}.", facts.join("\n"), s.key, today());
            if self.brain.learn(&title, &body).is_ok() { saved += 1; }
        }
        saved
    }
}

/// Screenshots, left to right, as base64 JPEGs of at most 1024px each, with our own window blacked out
/// (hiding it would flicker), plus a 32x18 greyscale thumbnail of the first for change detection.
/// `all` = every monitor (when the user asks her to look); otherwise only the one under the cursor
/// (automatic glances: that's where the user is working, and one image keeps them cheap).
fn screenshot(app: &AppHandle, all: bool) -> Result<(Vec<String>, Vec<u8>), String> {
    let win = app.get_webview_window("main");
    let bounds = |m: &xcap::Monitor| (m.x().unwrap_or(0), m.y().unwrap_or(0), m.width().unwrap_or(0) as i32, m.height().unwrap_or(0) as i32);
    let mut monitors = xcap::Monitor::all().map_err(err)?;
    monitors.sort_by_key(|m| (bounds(m).0, bounds(m).1));
    if !all {
        let cursor = win.as_ref().and_then(|w| w.cursor_position().ok()).map(|c| (c.x as i32, c.y as i32));
        let under = cursor.and_then(|(cx, cy)| monitors.iter().position(|m| {
            let (x, y, w, h) = bounds(m);
            cx >= x && cx < x + w && cy >= y && cy < y + h
        }));
        let keep = under.or_else(|| monitors.iter().position(|m| m.is_primary().unwrap_or(false))).unwrap_or(0);
        if keep < monitors.len() { monitors = vec![monitors.swap_remove(keep)]; }
    }
    if monitors.is_empty() { return Err("no monitor found".into()); }
    let rect = win.as_ref().and_then(|w| Some((w.outer_position().ok()?, w.outer_size().ok()?)));

    let (mut images, mut thumb) = (vec![], vec![]);
    for m in &monitors {
        let mut shot = m.capture_image().map_err(err)?;
        if let Some((p, s)) = rect {
            // our window in this monitor's own pixels
            let (mx, my, _, _) = bounds(m);
            let (w, h) = (shot.width() as i32, shot.height() as i32);
            let (x0, x1) = ((p.x - mx).clamp(0, w), (p.x + s.width as i32 - mx).clamp(0, w));
            let (y0, y1) = ((p.y - my).clamp(0, h), (p.y + s.height as i32 - my).clamp(0, h));
            for y in y0..y1 { for x in x0..x1 { shot.put_pixel(x as u32, y as u32, Rgba([0, 0, 0, 255])); } }
        }
        let img = DynamicImage::ImageRgba8(shot);
        if thumb.is_empty() { thumb = img.resize_exact(32, 18, FilterType::Triangle).to_luma8().into_raw(); }
        let mut buf = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(img.resize(1024, 1024, FilterType::Triangle).to_rgb8())
            .write_to(&mut buf, ImageFormat::Jpeg).map_err(err)?;
        images.push(base64::engine::general_purpose::STANDARD.encode(buf.into_inner()));
    }
    Ok((images, thumb))
}

#[tauri::command]
async fn chat(
    app: AppHandle, state: State<'_, Companion>, text: String, screen: bool, shared: Option<Vec<String>>,
) -> Result<(), String> {
    // screenshots of every monitor when asked, plus any images the user pasted or dropped (base64 JPEGs)
    let shared = shared.unwrap_or_default();
    let mut images = if screen {
        app.emit("status", "looking").ok();
        screenshot(&app, true)?.0
    } else {
        vec![]
    };
    let screens = images.len();
    images.extend(shared.iter().cloned());
    app.emit("status", "thinking").ok();

    let down = sounds_down(&text);
    if down { app.emit("mood", "charming").ok(); } // her soft face, whatever tag the model picks
    let mut system = (if down { GENTLE } else { PERSONA }).replace("{name}", &state.cfg.name);
    let mems = state.recall(&text);
    if !mems.is_empty() {
        system += "\n\nThings you remember about the user:\n";
        for m in &mems { system += &format!("- {m}\n"); }
    }
    // weather and explicit searches are run by the app: with her persona the model tends to invent the answer.
    // The data rides on the user's message (small models ignore it in the system prompt); history keeps the plain text.
    let mut ask = text.clone();
    let mut read: Vec<Source> = vec![]; // web pages and searches she reads this turn, to learn from afterwards
    if let Some(query) = search_request(&text) {
        app.emit("status", "searching").ok();
        let results = tools::run("web_search", &json!({ "query": query }), "");
        ask += &format!("\n\n(Real web search results for your answer. Use them, say which site, invent nothing:\n{results})");
        read.push(Source { key: query, text: results, page: false });
    } else if let Some(place) = weather_request(&text) {
        app.emit("status", "searching").ok();
        let report = tools::run("weather", &json!({ "location": place }), &state.cfg.location);
        ask += &format!("\n\n(Real weather data for your answer. Use the actual numbers, invent nothing; if it's an error, say so: {report})");
    }
    if let Some(url) = first_url(&text) {
        app.emit("status", "searching").ok();
        let page = tools::run("web_fetch", &json!({ "url": url }), "");
        ask += &format!("\n\n(The page {url}, fetched just now. Trust it over your memory, quote it, invent nothing:\n{page})");
        read.push(Source { key: url.to_string(), text: page, page: true });
    }
    let saved_note = note_request(&text).and_then(|mut note| {
        if note.is_empty() {
            let s = state.store.lock().unwrap();
            note = s.chat().iter().rev().find(|m| m.role == "assistant").map(|m| strip_mood(&m.content)).unwrap_or_default();
        }
        (!note.is_empty()).then(|| {
            let title = note.split_whitespace().take(6).collect::<Vec<_>>().join(" ");
            state.brain.save_note(&title, &note)
        })
    });
    match &saved_note {
        Some(Ok(file)) => system += &format!("\n\nYou just saved the user's note to your second brain as \"{file}\". \
            Confirm that briefly; do not call save_note."),
        Some(Err(e)) => system += &format!("\n\nSaving the user's note failed ({e}). Tell them honestly."),
        None => {}
    }
    let notes: Vec<_> = state.brain.search(&text, 4).into_iter().filter(|h| h.score > 0.5).collect();
    if !notes.is_empty() {
        system += "\n\nFrom the user's notes (your second brain). Use them when relevant and say which note:\n";
        for n in &notes { system += &format!("[{}] {}\n", n.note, n.text); }
    }
    let mut messages = vec![json!({ "role": "system", "content": system })];
    if !down {
        for (u, a) in EXAMPLES {
            messages.push(json!({ "role": "user", "content": u }));
            messages.push(json!({ "role": "assistant", "content": a }));
        }
    }
    // Small models copy their own past replies: long ones make her ramble, and a wrong claim gets repeated forever.
    // So her replies go back cut short, and when the user pushes back she sees only their side of the argument.
    let corrected = pushback(&text);
    {
        let s = state.store.lock().unwrap();
        let chat = s.chat();
        for m in &chat[chat.len().saturating_sub(12)..] {
            if m.role == "assistant" && corrected { continue; }
            let mut content = m.content.clone();
            if m.role == "assistant" && content.chars().count() > 240 {
                content = content.chars().take(240).collect::<String>() + "…";
            }
            messages.push(json!({ "role": m.role, "content": content }));
        }
    }
    if corrected {
        ask += "\n\n(The user is correcting you. Your earlier replies on this may have been wrong.)";
    }
    if screens > 1 {
        ask += &format!("\n\n(The first {screens} images are screenshots, one per monitor, from left to right.)");
    }
    if !shared.is_empty() {
        ask += &format!("\n\n(The user is showing you {} image(s) they pasted or dropped{}.)", shared.len(),
            if screens > 0 { ", after the screenshots" } else { "" });
    }
    let mut user = json!({ "role": "user", "content": ask });
    if !images.is_empty() { user["images"] = json!(images); }
    messages.push(user);

    // stream the reply; when the model calls tools, run them, add the results and let it continue.
    // Screenshots take a lot of context (two monitors overflow 4096 and cut the reply short).
    let num_ctx = if images.is_empty() { 4096 } else { 8192 };
    let mut reply = String::new();
    for _round in 0..4 {
        let resp = ureq::post(&format!("{OLLAMA}/api/chat"))
            .send_json(json!({
                "model": state.model(), "messages": messages, "tools": tools::spec(), "stream": true,
                // small models can fall into repeating themselves: penalise repeats and cap the length
                "keep_alive": "2m", "options": { "num_ctx": num_ctx, "num_predict": 400, "repeat_penalty": 1.15, "repeat_last_n": 256 }
            }))
            .map_err(err)?;
        let mut calls: Vec<Value> = vec![];
        for line in BufReader::new(resp.into_reader()).lines() {
            let line = line.map_err(err)?;
            if line.trim().is_empty() { continue; }
            let chunk: Value = serde_json::from_str(&line).map_err(err)?;
            if let Some(e) = chunk["error"].as_str() { return Err(e.into()); }
            if let Some(piece) = chunk["message"]["content"].as_str().filter(|p| !p.is_empty()) {
                reply += piece;
                app.emit("token", piece).ok();
                if looping(&reply) { break; } // dropping the stream makes Ollama stop generating
            }
            if let Some(tc) = chunk["message"]["tool_calls"].as_array() { calls.extend(tc.iter().cloned()); }
            if chunk["done"] == true { break; }
        }
        if calls.is_empty() { break; }
        messages.push(json!({ "role": "assistant", "content": "", "tool_calls": calls }));
        for c in &calls {
            let (name, args) = (c["function"]["name"].as_str().unwrap_or(""), &c["function"]["arguments"]);
            let arg = |k: &str| args[k].as_str().unwrap_or("").trim().to_string();
            let brain_tool = matches!(name, "search_brain" | "save_note");
            app.emit("status", if brain_tool { "recalling" } else { "searching" }).ok();
            let result = match name {
                "search_brain" => {
                    let hits = state.brain.search(&arg("query"), 5);
                    if hits.is_empty() { "no notes found".to_string() }
                    else { hits.iter().map(|h| format!("[{}] {}", h.note, h.text)).collect::<Vec<_>>().join("\n\n") }
                }
                "save_note" if saved_note.is_some() => "already saved".to_string(),
                "save_note" => state.brain.save_note(&arg("title"), &arg("text"))
                    .map(|f| format!("saved as {f}")).unwrap_or_else(|e| format!("error: {e}")),
                _ => tools::run(name, args, &state.cfg.location),
            };
            match name {
                "web_search" => read.push(Source { key: arg("query"), text: result.clone(), page: false }),
                "web_fetch" => read.push(Source { key: arg("url"), text: result.clone(), page: true }),
                _ => {}
            }
            messages.push(json!({ "role": "tool", "tool_name": name, "content": result }));
        }
    }
    app.emit("done", ()).ok();

    let incognito = {
        let mut s = state.store.lock().unwrap();
        let shown = if shared.is_empty() { String::new() } else { format!(" [shared {} image(s)]", shared.len()) };
        let t = now();
        let turn = [Msg { role: "user".into(), content: format!("{text}{shown}"), t },
            Msg { role: "assistant".into(), content: reply.clone(), t }];
        match s.incognito.as_mut() {
            Some(chat) => { chat.extend(turn); true } // incognito: nothing is saved, remembered or learned
            None => { s.history.extend(turn); state.save(&s)?; false }
        }
    };
    if incognito { return Ok(()); }
    // memories come only from what the user typed: never on screen or image turns (her description of
    // the picture ended up stored as facts about the user), and not for short chit-chat (saves GPU)
    if !screen && shared.is_empty() && text.len() >= 20 {
        app.emit("status", "working").ok();
        app.emit("memorized", state.extract(&text)).ok();
    }
    if !read.is_empty() {
        app.emit("status", "working").ok();
        let learned = state.learn(&read);
        if learned > 0 { app.emit("brain", learned).ok(); }
    }
    Ok(())
}

/// Glance at the screen and maybe react: {"reaction": text or null, "game": bool}. Skips the model entirely
/// when the screen barely changed. The same call says whether a game is on (the UI then glances more
/// often) and, with the journal on, what the user is doing.
#[tauri::command]
async fn react(
    app: AppHandle, state: State<'_, Companion>, chime: bool, journal: bool, date: String, time: String,
) -> Result<Value, String> {
    let (images, thumb) = screenshot(&app, false)?;
    {
        let mut last = state.last_screen.lock().unwrap();
        let diff = last.iter().zip(&thumb).map(|(a, b)| (*a as i32 - *b as i32).unsigned_abs()).sum::<u32>();
        let changed = last.len() != thumb.len() || diff / thumb.len() as u32 >= 6;
        *last = thumb;
        if !changed { return Ok(json!({ "reaction": null, "game": false })); }
    }
    let req = json!({
        "model": state.model(), "stream": false, "keep_alive": "2m", "format": "json",
        "options": { "num_ctx": 4096, "num_predict": 150, "repeat_penalty": 1.15 },
        "messages": [
            { "role": "system", "content": PERSONA.replace("{name}", &state.cfg.name) },
            { "role": "user", "content": GLANCE, "images": images }
        ]
    });
    let v: Value = ureq::post(&format!("{OLLAMA}/api/chat")).send_json(req).map_err(err)?.into_json().map_err(err)?;
    let j: Value = serde_json::from_str(v["message"]["content"].as_str().unwrap_or("")).unwrap_or_default();
    if journal && state.store.lock().unwrap().incognito.is_none() {
        if let Some(activity) = j["activity"].as_str().map(str::trim).filter(|a| !a.is_empty()) {
            state.brain.journal(&date, &time, activity).ok();
        }
    }
    let reply = j["reaction"].as_str().unwrap_or("").trim().to_string();
    let reaction = (chime && !reply.is_empty() && !reply.contains("PASS")).then_some(reply);
    Ok(json!({ "reaction": reaction, "game": j["game"] == true }))
}

/// Cursor position relative to our window's top-left, in CSS pixels (works anywhere on screen).
#[tauri::command]
fn cursor(window: WebviewWindow) -> Option<(f64, f64)> {
    let c = window.cursor_position().ok()?;
    let p = window.outer_position().ok()?;
    let s = window.scale_factor().ok()?;
    Some(((c.x - p.x as f64) / s, (c.y - p.y as f64) / s))
}

#[tauri::command]
async fn remember(state: State<'_, Companion>, text: String) -> Result<bool, String> {
    state.remember(&text)
}

#[tauri::command]
fn list_memories(state: State<Companion>) -> Vec<String> {
    state.store.lock().unwrap().memories.iter().map(|m| m.text.clone()).collect()
}

/// The last 60 chat messages as (role, text), oldest first, for the chat history panel
#[tauri::command]
fn history(state: State<Companion>) -> Vec<(String, String)> {
    let s = state.store.lock().unwrap();
    let chat = s.chat();
    chat[chat.len().saturating_sub(60)..].iter().map(|m| (m.role.clone(), strip_mood(&m.content))).collect()
}

#[tauri::command]
fn forget(state: State<Companion>, index: usize) -> Result<(), String> {
    let mut s = state.store.lock().unwrap();
    if index < s.memories.len() { s.memories.remove(index); }
    state.save(&s)
}

/// Incognito on: a fresh chat kept in RAM only; off: that chat is dropped and her saved one comes back.
#[tauri::command]
fn set_incognito(state: State<Companion>, on: bool) {
    state.store.lock().unwrap().incognito = on.then(Vec::new);
}

/// The "forget after" choice: never, day, week, month or year.
#[tauri::command]
fn forget_after(state: State<Companion>) -> String {
    state.forget.lock().unwrap().clone()
}

/// Sets "forget after", saves it in config.json and forgets what's already too old. Returns how much went.
#[tauri::command]
fn set_forget_after(state: State<Companion>, choice: String) -> Result<usize, String> {
    if keep_for(&choice).is_none() && choice != "never" { return Err(format!("unknown choice {choice}")); }
    *state.forget.lock().unwrap() = choice.clone();
    set_config(&state.dir, "forget_after", json!(choice))?;
    state.prune()
}

/// Changes one key in config.json, keeping the rest as the user wrote it.
fn set_config(dir: &Path, key: &str, value: Value) -> Result<(), String> {
    let path = dir.join("config.json");
    let mut cfg: Value = fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_else(|| json!({}));
    cfg[key] = value;
    fs::write(path, serde_json::to_vec_pretty(&cfg).map_err(err)?).map_err(err)
}

#[tauri::command]
fn name(state: State<Companion>) -> String {
    state.cfg.name.clone()
}

#[tauri::command]
fn save_vrm(state: State<Companion>, request: ipc::Request<'_>) -> Result<(), String> {
    match request.body() {
        ipc::InvokeBody::Raw(bytes) => fs::write(state.dir.join("model.vrm"), bytes).map_err(err),
        _ => Err("expected raw bytes".into()),
    }
}

#[tauri::command]
fn load_vrm(state: State<Companion>) -> Result<ipc::Response, String> {
    fs::read(state.dir.join("model.vrm")).map(ipc::Response::new).map_err(err)
}

fn installed_models() -> Vec<String> {
    ureq::get(&format!("{OLLAMA}/api/tags")).call().ok()
        .and_then(|r| r.into_json::<Value>().ok())
        .and_then(|v| v["models"].as_array().map(|a| a.iter().filter_map(|m| m["name"].as_str().map(String::from)).collect()))
        .unwrap_or_default()
}

/// Current model and the switchable ones: (current, [(label, name, size, installed)])
#[tauri::command]
async fn models(state: State<'_, Companion>) -> Result<(String, Vec<(String, String, String, bool)>), String> {
    let installed = installed_models();
    let list = MODELS.iter()
        .map(|(label, name, size)| (label.to_string(), name.to_string(), size.to_string(), installed.iter().any(|i| i == name)))
        .collect();
    Ok((state.model(), list))
}

/// Switches the chat model, downloading it first if needed (progress as "pull" events, 0-100),
/// and remembers the choice in config.json.
#[tauri::command]
async fn set_model(app: AppHandle, state: State<'_, Companion>, model: String) -> Result<(), String> {
    if !MODELS.iter().any(|(_, name, _)| *name == model) { return Err(format!("unknown model {model}")); }
    if !installed_models().contains(&model) {
        let resp = ureq::post(&format!("{OLLAMA}/api/pull"))
            .send_json(json!({ "model": model, "stream": true }))
            .map_err(err)?;
        for line in BufReader::new(resp.into_reader()).lines() {
            let v: Value = serde_json::from_str(&line.map_err(err)?).unwrap_or_default();
            if let Some(e) = v["error"].as_str() { return Err(e.into()); }
            if let (Some(done), Some(total)) = (v["completed"].as_f64(), v["total"].as_f64()) {
                if total > 0.0 { app.emit("pull", (done / total * 100.0) as u32).ok(); }
            }
        }
    }
    *state.model.lock().unwrap() = model.clone();
    set_config(&state.dir, "chat_model", json!(model))
}

/// Second-brain folder and how much of it is indexed: (folder, files, chunks)
#[tauri::command]
fn brain_info(state: State<Companion>) -> (String, usize, usize) {
    let (files, chunks) = state.brain.stats();
    (state.brain.dir.to_string_lossy().into_owned(), files, chunks)
}

/// One sentence in her voice: a base64 WAV, or "" when there's nothing to say. Runs off the UI thread.
#[tauri::command]
async fn speak(state: State<'_, Companion>, text: String) -> Result<String, String> {
    let speech = state.speech.clone();
    tauri::async_runtime::spawn_blocking(move || speech.say(&text)).await.map_err(err)?
}

/// Whether her voice has been downloaded.
#[tauri::command]
fn voice_ready(state: State<Companion>) -> bool {
    state.speech.installed()
}

/// Downloads her voice once (progress as "voice" events, 0-100).
#[tauri::command]
async fn install_voice(app: AppHandle, state: State<'_, Companion>) -> Result<(), String> {
    let speech = state.speech.clone();
    tauri::async_runtime::spawn_blocking(move || speech.install(|p| { app.emit("voice", p).ok(); })).await.map_err(err)?
}

#[tauri::command]
fn open_brain(state: State<Companion>) -> Result<(), String> {
    let opener = if cfg!(windows) { "explorer" } else if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    std::process::Command::new(opener).arg(&state.brain.dir).spawn().map(|_| ()).map_err(err)
}

/// `aiko --say-lines <lines.txt> <out-dir>`: speaks each line of the file into <out-dir>/001.wav, 002.wav...
/// in her voice, then exits (used to narrate videos). The voice must already be downloaded.
fn say_lines(input: &str, out: &str) -> Result<(), String> {
    let data = std::env::var("APPDATA").map(|d| PathBuf::from(d).join("local.companion"))
        .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(if cfg!(target_os = "macos") {
            "Library/Application Support/local.companion" } else { ".local/share/local.companion" })))
        .map_err(err)?;
    let speech = speech::Speech::new(&data);
    fs::create_dir_all(out).map_err(err)?;
    for (i, line) in fs::read_to_string(input).map_err(err)?.lines().enumerate() {
        let wav = base64::engine::general_purpose::STANDARD.decode(speech.say(line.trim())?).map_err(err)?;
        fs::write(Path::new(out).join(format!("{:03}.wav", i + 1)), wav).map_err(err)?;
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 4 && args[1] == "--say-lines" {
        if let Err(e) = say_lines(&args[2], &args[3]) {
            fs::write(Path::new(&args[3]).join("error.txt"), e).ok();
            std::process::exit(1);
        }
        return;
    }
    tauri::Builder::default()
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            fs::create_dir_all(&dir)?;
            let cfg: Config = fs::read(dir.join("config.json")).ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
                .unwrap_or_default();
            let brain_dir = if cfg.brain_dir.is_empty() {
                let docs = app.path().document_dir()?;
                let (old, new) = (docs.join("Companion Brain"), docs.join("AIKO Brain"));
                // the folder's name before she was renamed AIKO: move it so no notes are left behind
                if old.exists() && !new.exists() { fs::rename(&old, &new).ok(); }
                new
            } else {
                cfg.brain_dir.clone().into()
            };
            let brain = brain::Brain::open(brain_dir, dir.join("brain.json"), &cfg.embed_model);
            let mem_path = dir.join("memory.json");
            let mut store: Store = match fs::read(&mem_path) {
                Ok(b) => serde_json::from_slice(&b).unwrap_or_else(|_| {
                    // keep a corrupt file instead of overwriting it on the next save
                    fs::rename(&mem_path, dir.join("memory.json.bak")).ok();
                    Store::default()
                }),
                Err(_) => Store::default(),
            };
            // memories and chat from before 2.8.3 have no date: they count from today, so none is forgotten at once
            let t = now();
            store.memories.iter_mut().filter(|m| m.t == 0).for_each(|m| m.t = t);
            store.history.iter_mut().filter(|m| m.t == 0).for_each(|m| m.t = t);
            let model = Mutex::new(cfg.chat_model.clone());
            let forget = Mutex::new(cfg.forget_after.clone());
            let speech = std::sync::Arc::new(speech::Speech::new(&dir));
            app.manage(Companion { dir, cfg, store: Mutex::new(store), last_screen: Mutex::new(vec![]), brain, model, forget, speech });
            {
                let c = app.state::<Companion>();
                let s = c.store.lock().unwrap();
                c.save(&s).ok(); // keep the dates just given
            }

            // keep the second brain in sync with its folder (also retries when Ollama wasn't up yet),
            // and forget what's older than the "forget after" choice
            let handle = app.handle().clone();
            std::thread::spawn(move || loop {
                handle.state::<Companion>().prune().ok();
                if let Ok((indexed, _)) = handle.state::<Companion>().brain.sync() {
                    if indexed > 0 { handle.emit("brain", indexed).ok(); }
                }
                std::thread::sleep(std::time::Duration::from_secs(60));
            });

            // dock to the bottom-right corner, sitting on the taskbar
            let win = app.get_webview_window("main").expect("main window");
            if let (Ok(Some(mon)), Ok(size)) = (win.primary_monitor(), win.outer_size()) {
                let area = mon.work_area();
                let margin = (16.0 * mon.scale_factor()) as i32;
                win.set_position(PhysicalPosition::new(
                    area.position.x + area.size.width as i32 - size.width as i32 - margin,
                    area.position.y + area.size.height as i32 - size.height as i32,
                ))?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![chat, react, cursor, remember, list_memories, history, forget, name, save_vrm, load_vrm, brain_info, open_brain, models, set_model, set_incognito, forget_after, set_forget_after, speak, voice_ready, install_voice])
        .run(tauri::generate_context!())
        .expect("failed to start AIKO");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_requests() {
        assert_eq!(note_request("note this down: buy basil seeds").as_deref(), Some("buy basil seeds"));
        assert_eq!(note_request("Please write that down"), Some(String::new()));
        assert_eq!(note_request("Take a note - call Marta on Friday").as_deref(), Some("call Marta on Friday"));
        assert_eq!(note_request("what notes do I have?"), None);
        assert_eq!(strip_mood("[joyful] Sure thing!"), "Sure thing!");
    }

    #[test]
    fn routed_requests() {
        assert_eq!(weather_request("what's the weather in Lisbon?").as_deref(), Some("Lisbon"));
        assert_eq!(weather_request("Weather in New York tomorrow").as_deref(), Some("New York"));
        assert_eq!(weather_request("will it rain today?").as_deref(), Some(""));
        assert_eq!(weather_request("I love this app"), None);
        assert_eq!(search_request("search for the latest Rust release?").as_deref(), Some("the latest Rust release"));
        assert_eq!(search_request("Look up Bambu A1 nozzle sizes").as_deref(), Some("Bambu A1 nozzle sizes"));
        assert_eq!(search_request("what is rust"), None);
        assert_eq!(search_request("hey there, research something for me, Windows Phone Link won't start.").as_deref(),
            Some("Windows Phone Link won't start"));
        assert_eq!(search_request("can you find phone link?").as_deref(), Some("phone link"));
        assert_eq!(search_request("I can't find my keys"), None);
        assert!(pushback("you're stubborn, Phone Link exists"));
        assert!(pushback("You’re wrong about that"));
        assert!(!pushback("is Phone Link a real app?"));
        let sites = Config::default().trusted_sites;
        assert!(trusted("https://www.microsoft.com/en-us/windows/sync-across-your-devices?r=1", &sites));
        assert!(trusted("https://en.wikipedia.org/wiki/Phone_Link", &sites));
        assert!(trusted("https://www.cdc.gov/x", &sites));
        assert!(!trusted("https://microsoft.com.evil.io/x", &sites));
        assert!(!trusted("https://notmicrosoft.com/", &sites));
        assert_eq!(today().len(), 10);
        let msg = |t| Msg { role: "user".into(), content: "hi".into(), t };
        let day = 86_400;
        let mut s = Store { memories: vec![Memory { text: "old".into(), emb: vec![], t: 1 }], history: vec![msg(1), msg(10 * day)], incognito: None };
        assert_eq!(prune(&mut s, "never", 10 * day), 0);
        assert_eq!(prune(&mut s, "week", 10 * day), 2);
        assert_eq!((s.memories.len(), s.history.len()), (0, 1));
        s.incognito = Some(vec![]);
        assert!(s.chat().is_empty());
        assert_eq!(first_url("see https://www.microsoft.com/en-us/windows/sync-across-your-devices?r=1."),
            Some("https://www.microsoft.com/en-us/windows/sync-across-your-devices?r=1"));
        assert_eq!(quoted(r#"{"memories": ["The user likes tea.", "The user is Al"#), vec!["memories", "The user likes tea."]);
    }

    #[test]
    fn loop_detection() {
        let lines = "*That's not a project.*\n*That's a request.*\n*What's your next move?*\n*And don't say no.*\n";
        assert!(looping(&format!("[angry] Intro line here.\n{}", lines.repeat(3))));
        let normal = "[teasing] Bold of you to trust tomorrow-you with that. What's different about tomorrow, exactly? \
            Start with dinner tonight and we'll see how it goes. I'll allow one cookie, but only one, and I'm counting. \
            Also, drink some water, you've been staring at that screen for hours and I can tell. Go on.";
        assert!(!looping(normal));
    }

    #[test]
    fn down_detection() {
        assert!(sounds_down("I'm feeling kind of down today"));
        assert!(sounds_down("work is so stressful, I can't sleep"));
        assert!(sounds_down("I feel lonely tonight"));
        assert!(!sounds_down("download the new driver"));
        assert!(!sounds_down("Rust is the best language for everything"));
    }
}
