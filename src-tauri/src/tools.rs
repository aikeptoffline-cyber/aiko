//! Tools the chat model can call: web search (DuckDuckGo), reading a web page, and weather (Open-Meteo).
//! These are the only features that reach the internet, and only when the model decides to use them.

use serde_json::{json, Value};
use std::io::Read;
use std::time::Duration;

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130 Safari/537.36";

pub fn spec() -> Value {
    let tool = |name: &str, desc: &str, arg: &str, arg_desc: &str| json!({
        "type": "function",
        "function": { "name": name, "description": desc, "parameters": {
            "type": "object", "properties": { arg: { "type": "string", "description": arg_desc } }, "required": [arg] } }
    });
    json!([
        tool("web_search", "Search the web with DuckDuckGo. Use it for news, current facts, or anything the user asks you to look up.", "query", "what to search for"),
        tool("web_fetch", "Read the text of a web page, for example a result from web_search.", "url", "full http(s) URL"),
        tool("weather", "Current weather and a 3-day forecast for a place.", "location", "city name"),
        // second brain (handled in main.rs, which owns the index)
        tool("search_brain", "Search the user's own notes and documents by meaning.", "query", "what to look for"),
        json!({ "type": "function", "function": { "name": "save_note",
            "description": "Save a note into the user's second brain, e.g. when they say 'note this down'.",
            "parameters": { "type": "object", "properties": {
                "title": { "type": "string", "description": "short title" },
                "text": { "type": "string", "description": "the note, in markdown" } },
            "required": ["title", "text"] } } }),
    ])
}

/// Runs a tool; failures come back as text so the model can tell the user.
pub fn run(name: &str, args: &Value, default_location: &str) -> String {
    let arg = |k: &str| args[k].as_str().unwrap_or("").trim().to_string();
    let result = match name {
        "web_search" => web_search(&arg("query")),
        "web_fetch" => web_fetch(&arg("url")),
        "weather" => {
            let place = arg("location");
            weather(if place.is_empty() { default_location } else { &place })
        }
        _ => Err(format!("unknown tool {name}")),
    };
    result.unwrap_or_else(|e| format!("error: {e}"))
}

fn get(url: &str) -> Result<String, String> {
    let resp = ureq::get(url).set("User-Agent", UA).timeout(Duration::from_secs(15)).call().map_err(|e| e.to_string())?;
    let mut buf = vec![];
    resp.into_reader().take(400_000).read_to_end(&mut buf).map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

fn web_search(query: &str) -> Result<String, String> {
    if query.is_empty() { return Err("empty query".into()); }
    let html = get(&format!("https://html.duckduckgo.com/html/?q={}", encode(query)))?;
    let mut out = String::new();
    for (n, block) in html.split("class=\"result__a\"").skip(1).take(5).enumerate() {
        let href = between(block, "href=\"", "\"").unwrap_or("");
        // links go through a DuckDuckGo redirect that carries the real URL in `uddg`
        let link = between(href, "uddg=", "&").or_else(|| href.split("uddg=").nth(1)).map(decode).unwrap_or_else(|| href.replace("&amp;", "&"));
        let title = text(between(block, ">", "</a>").unwrap_or(""));
        let snippet = between(block, "result__snippet", "</a>").and_then(|s| s.split_once('>')).map(|(_, s)| text(s)).unwrap_or_default();
        out += &format!("{}. {}\n{}\n{}\n\n", n + 1, title, link, snippet);
    }
    if out.is_empty() { Err("no results".into()) } else { Ok(out) }
}

fn web_fetch(url: &str) -> Result<String, String> {
    if !url.starts_with("http://") && !url.starts_with("https://") { return Err("not an http(s) URL".into()); }
    let mut html = get(url)?;
    for tag in ["script", "style", "noscript"] {
        while let Some(start) = html.find(&format!("<{tag}")) {
            let end = html[start..].find(&format!("</{tag}>")).map(|e| start + e + tag.len() + 3).unwrap_or(html.len());
            html.replace_range(start..end, " ");
        }
    }
    Ok(text(&html).chars().take(4000).collect())
}

fn weather(place: &str) -> Result<String, String> {
    if place.is_empty() { return Err("no location given, ask the user where they are".into()); }
    let geo: Value = serde_json::from_str(&get(&format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={}&count=1", encode(place)))?).map_err(|e| e.to_string())?;
    let g = &geo["results"][0];
    let (lat, lon) = (g["latitude"].as_f64().ok_or("place not found")?, g["longitude"].as_f64().ok_or("place not found")?);
    let f: Value = serde_json::from_str(&get(&format!(
        "https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}\
         &current=temperature_2m,apparent_temperature,relative_humidity_2m,wind_speed_10m,weather_code\
         &daily=weather_code,temperature_2m_max,temperature_2m_min,precipitation_probability_max&forecast_days=3&timezone=auto"))?)
        .map_err(|e| e.to_string())?;
    let (c, d) = (&f["current"], &f["daily"]);
    let mut s = format!(
        "{}, {} (local time {}): {}°C, feels like {}°C, {}, humidity {}%, wind {} km/h.\n",
        g["name"].as_str().unwrap_or(place), g["country"].as_str().unwrap_or(""), c["time"].as_str().unwrap_or(""),
        c["temperature_2m"], c["apparent_temperature"], sky(&c["weather_code"]), c["relative_humidity_2m"], c["wind_speed_10m"]);
    for i in 0..3 {
        s += &format!("{}: {}, {} to {}°C, {}% chance of rain\n", d["time"][i].as_str().unwrap_or(""), sky(&d["weather_code"][i]),
            d["temperature_2m_min"][i], d["temperature_2m_max"][i], d["precipitation_probability_max"][i]);
    }
    Ok(s)
}

/// WMO weather code in words
fn sky(code: &Value) -> &'static str {
    match code.as_i64().unwrap_or(-1) {
        0 => "clear", 1..=3 => "partly cloudy", 45 | 48 => "fog", 51..=57 => "drizzle", 61..=67 => "rain",
        71..=77 => "snow", 80..=82 => "rain showers", 85 | 86 => "snow showers", 95..=99 => "thunderstorm", _ => "unknown",
    }
}

fn between<'a>(s: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let rest = &s[s.find(start)? + start.len()..];
    Some(&rest[..rest.find(end)?])
}

/// HTML fragment to plain text: drop tags, decode common entities, collapse whitespace
fn text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch { '<' => { in_tag = true; out.push(' ') } '>' => in_tag = false, _ if !in_tag => out.push(ch), _ => {} }
    }
    let out = out.replace("&amp;", "&").replace("&quot;", "\"").replace("&#x27;", "'").replace("&#39;", "'")
        .replace("&lt;", "<").replace("&gt;", ">").replace("&nbsp;", " ");
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn encode(s: &str) -> String {
    s.bytes().map(|b| match b {
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
        b' ' => "+".into(),
        _ => format!("%{b:02X}"),
    }).collect()
}

fn decode(s: &str) -> String {
    let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let b = s.as_bytes();
    let mut out = vec![];
    let mut i = 0;
    while i < b.len() {
        match (b[i], b.get(i + 1).copied().and_then(hex), b.get(i + 2).copied().and_then(hex)) {
            (b'%', Some(h), Some(l)) => { out.push(h * 16 + l); i += 3; continue }
            (b'+', _, _) => out.push(b' '),
            (c, _, _) => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_round_trip() {
        assert_eq!(decode(&encode("café & ollama 2?")), "café & ollama 2?");
        assert_eq!(decode("https%3A%2F%2Follama.com%2F"), "https://ollama.com/");
    }

    #[test]
    fn html_to_text() {
        assert_eq!(text("<b>Hi</b>&amp; <i>there</i>\n  you"), "Hi & there you");
        assert_eq!(between("a href=\"x\">T</a>", "href=\"", "\""), Some("x"));
    }
}
