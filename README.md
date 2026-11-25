# AIKO

**A**.**I**. **K**ept **O**ffline: an anime desktop companion whose AI runs entirely on your computer.

She sits on your taskbar in a transparent, always-on-top window, chats with a small local AI through [Ollama](https://ollama.com), remembers you, reads your notes, can look at your screen, and talks with an offline voice. Nothing you say to her leaves your computer. Windows, macOS and Linux.

## Install

Every installer asks which brain size you want and downloads only that one (plus the small `nomic-embed-text` model her memory uses):
- **2B** (`qwen3-vl:2b-instruct`, 1.9 GB): runs on any computer, even without a graphics card.
- **4B** (`qwen3-vl:4b-instruct`, 3.3 GB): sharper and follows her personality better. Best with 6 GB+ of GPU memory or an Apple Silicon Mac with 16 GB.

You can switch later with the **2B / 4B** button; the other model downloads the first time you switch.

**Windows 10/11:** download `AIKO-Setup-<version>.exe` from [Releases](../../releases) and run it. It installs WebView2 and Ollama if they're missing. The beta isn't code-signed: choose "More info" and "Run anyway".

**macOS 11+ and Linux (x86_64):**
```bash
curl -fsSL https://raw.githubusercontent.com/aikeptoffline-cyber/aiko/main/installer/install.sh | bash
```
It installs Ollama if needed, downloads your chosen model, and installs the newest AIKO release (`.dmg` on macOS; `.deb`, `.rpm` or `.AppImage` on Linux). Add `-s -- --model 4b` after `bash` to skip the question. You can also download the packages from [Releases](../../releases) yourself. On macOS the beta isn't signed by Apple: the script clears the quarantine flag for you; if you install the `.dmg` by hand, right-click AIKO and choose Open the first time. macOS asks for Screen Recording permission the first time she looks at your screen.

## What she does
- **Talks, with personality:** dry, acidic humour; she questions your plans before agreeing, admits it with a joke when you prove her wrong, and turns gentle when you sound down.
- **Remembers you:** facts you tell her become long-term memories, recalled by meaning. ☰ → forget memories and chat older than a day, week, month or year.
- **Second brain:** drop `.md` or `.txt` notes in `Documents/AIKO Brain` (📚 opens it). She indexes them locally and cites the note she used. Say "note this down" and she writes one.
- **Learns from the web:** "search for…", "research…", or a pasted link runs a real search or reads the page. What she reads on trusted sites (official vendors, Wikipedia, `.gov`/`.edu`, major news; edit `trusted_sites` in `config.json`) is kept as notes in `AIKO Brain/Learned from the web`, with the source and date.
- **Sees your screen:** 👁 looks at all your monitors now; ◎ lets her glance every 45 s (15 s while you game) and react; 📓 is an opt-in screen journal.
- **Pictures:** paste one with Ctrl+V (Cmd+V) or drag it onto her.
- **Voice:** an offline neural voice (Piper "Kristin", public domain), the same on every platform. She downloads it once (21 MB) the first time she speaks. The voice button cycles 🔊 speaks → 🔇 quiet (text only) → 🤐 taped (no voice, no comments of her own).
- **Incognito:** ☰ → 🕶 Incognito: nothing from that chat is kept (no history, memories, web notes or journal). She wears sunglasses while it's on.
- **Feels alive:** she breathes, blinks, sways her arms, follows your cursor, switches between a dozen poses, and wears accessories (tape, sunglasses).

## Where things live
| | Windows | macOS | Linux |
|---|---|---|---|
| Settings, memories, chat, voice | `%APPDATA%\local.companion` | `~/Library/Application Support/local.companion` | `~/.local/share/local.companion` |
| Second brain | `Documents\AIKO Brain` | `~/Documents/AIKO Brain` | `~/Documents/AIKO Brain` |

`config.json` there takes `name`, `chat_model`, `embed_model`, `location` (default place for the weather), `brain_dir`, `trusted_sites` and `forget_after`. Only web search (DuckDuckGo), web pages you ask about, and the weather (Open-Meteo) go online, and only when a question needs them.

## Build from source
1. Install [Rust](https://rustup.rs) and [Ollama](https://ollama.com). Windows also needs the Visual Studio C++ Build Tools; Linux needs the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) plus `libpipewire-0.3-dev libgbm-dev libclang-dev` for screen capture (see `.github/workflows/build.yml`).
2. `ollama pull qwen3-vl:2b-instruct` and `ollama pull nomic-embed-text`.
3. Fetch the 3D libraries for the optional VRM mode: `./scripts/vendor.sh` (or `./setup.ps1` on Windows).
4. `cd src-tauri` and `cargo build --release`. The app is a single file with the UI built in.
   - Windows: if the project sits in a deep folder, linking fails with `LNK1104` (260-character path limit). Build to a short folder with `$env:CARGO_TARGET_DIR="$env:LOCALAPPDATA\companion-build"`, and since that folder isn't named `target`, also point `ESPEAK_NG_DATA_PATH` at the `espeak-ng-data.tar.bz2` that `piper-phonemize-sys` leaves in it.
5. Installers: `npx @tauri-apps/cli@2 build` makes the `.dmg`/`.deb`/`.rpm`/`.AppImage`; Windows uses Inno Setup (`installer/aiko.iss`). Pushing a `v*` tag runs `.github/workflows/build.yml`, which builds all of them and attaches them to the release.

## Art
Her poses live in `art/gen`, generated as edits of one master image so they share a canvas. `art/key.ps1` keys them off the green background into `ui/sprites`; `art/arms.ps1` cuts her arms into separate swinging layers; `art/accessories.ps1` cuts accessories (`art/accessories/<name>.png`, the master pose wearing the item) into overlays. To add a pose, generate it in Gemini as an edit of `art/gen/master.png` on the same #00FF00 background and run the scripts. Drop a `.vrm` file on her window to switch her to 3D.

## Light by design
- The AI model unloads from memory after 2 idle minutes; automatic glances skip the model when the screen hasn't changed.
- Screenshots are cut down to 1024 px; her past replies go back to the model cut short.

## License
AIKO is free software under the [GNU GPL v3.0 or later](LICENSE). Her voice uses [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx) (Apache-2.0) and [piper-phonemize](https://crates.io/crates/piper-phonemize) with espeak-ng (GPL-3.0); the "Kristin" voice was trained on public-domain LibriVox recordings.
