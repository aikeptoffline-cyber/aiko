# AIKO 2.9.0 beta

`AIKO-Setup-2.9.0-beta.exe` is the installer. It checks for and installs WebView2 and Ollama, asks about shortcuts and her model size, downloads her models, and installs the app per user with no admin needed. Tester notes are in `installer/TESTERS.txt`. The app itself is one file, with the UI and sprites built in. It needs Windows 10 or 11 (WebView2 is built into Windows 11).

Before the first run:
1. Install Ollama: https://ollama.com
2. `ollama pull qwen3-vl:2b-instruct`
3. `ollama pull nomic-embed-text`

Memory, settings (`config.json`) and an optional `model.vrm` are kept in `%APPDATA%\local.companion`.

## New in 2.9.0 beta
- **macOS and Linux:** `.dmg` for Apple Silicon and Intel Macs; `.deb`, `.rpm` and `.AppImage` for Linux, plus `installer/install.sh`, which installs Ollama, asks 2B or 4B and downloads only that model, then installs AIKO.
- **Her own voice, everywhere:** an offline neural voice (Piper "Kristin", public domain) instead of the operating system's voices, so she sounds the same on every platform, Linux included. It downloads once (21 MB) the first time she speaks.
- **Voice button:** 🔊 speaks, 🔇 quiet (text only), 🤐 taped (tape over her mouth, no comments of her own; typing to her just gets a muffled "Mmmph!").
- **☰ menu:** incognito chat (she wears sunglasses; nothing is kept), forget memories and chat older than a day/week/month/year, and quit.
- **Buttons** run down her right edge; **accessories** (tape, sunglasses) are overlays that fit every pose.
- **Open source:** GPL-3.0.

## New in 2.8.2 beta
- **Learns from the web:** when she reads a page or search results from a trusted site, she keeps the facts as a note in `Documents\AIKO Brain\Learned from the web`, with the source link and date, and recalls them in later chats. Other sites help with the answer but are never kept. Edit `trusted_sites` in `config.json` to change the list (official vendors, Wikipedia, `.gov`/`.edu`, major news by default).
- **Less stubborn:** she still questions your plans and opinions, but no longer insists a real app or fact "doesn't exist". She offers to look it up, trusts your screen, pictures and web pages over her own memory, and admits it with a joke when you correct her.
- **Better web use:** "research X", "find X" and "can you look up X" now run a real search, and any link you paste is read for her instead of guessed at.

## New in 2.8.1 beta
- **Show her pictures:** paste one with Ctrl+V, or drag pictures onto her (up to 4). They wait as thumbnails by your message box until you send; click one to remove it. You can send a picture without any text.
- **Honest memory:** she now learns only from your own words, never from her replies, screen looks or pictures, so she stops saving made-up "facts" about you.
- **Head fix:** turning her head no longer shows a still copy of it behind her.

## New in 2.8.0 beta
- **New layout:** her window is exactly as tall as she is. The chat box sits in front of her lower half, and her replies show two lines at a time, following along as she talks.
- **💬 Chat history:** read the whole conversation any time. The reply box also stays up longer for long replies, and never hides while your mouse is on it.
- **All your monitors:** when you ask her to look (👁), she sees every screen, labelled left to right. Automatic glances look at the monitor your mouse is on.
- **Game reactions:** with ◎ on, she notices when you're playing a game, glances every 15 seconds, and reacts like a friend watching: sad when you die, happy when you win.
- **New sad face**, for when things go wrong.

## New in 2.7.2 beta
- **Fixed replies looping forever.** A reply could get stuck repeating the same lines for thousands of characters, and the loop was saved into her history and memories. Replies now have a repetition penalty and a length cap, and a reply is stopped the moment it starts repeating itself. Only real facts about you are saved as memories.

## New in 2.7.1 beta
- **Renamed to AIKO** (A.I. Kept Offline). Installing it over Companion upgrades in place: settings and memories are kept, the old shortcuts are replaced, and `Documents\Companion Brain` moves to `Documents\AIKO Brain` on first start.

## New in 2.7.0 beta
- **Windows installer** for testers: dependency checks, a choice of Desktop + Start menu or Start menu only, a choice of 2B or 4B (2B works without a graphics card), and the model download at the end.
- **App icon** made from her art.

## New in v2.6
- **Brain size switch:** the **2B / 4B** button switches her AI model and remembers the choice. 4B (3.3 GB of GPU memory) follows her personality better and gives cleaner memories. 2B (1.9 GB) is lighter and faster. The first switch downloads the model, with progress shown in her status line.
- **Real weather and searches:** weather questions, and messages starting with "search for" or "look up", are now run by the app and she answers from the real data. Before, she sometimes made up the weather.
- **More reliable memory:** the step that pulls out facts can no longer run away, and she keeps any facts that finished.

## New in v2.5
- **Voice:** she speaks her replies in a female voice from the Windows speech engine, sentence by sentence as they arrive, and her mouth moves while she talks. 🔊/🔇 turns it on or off. She only uses voices installed on your PC (Microsoft Zira by default); add a nicer one in Windows Settings > Time & language > Speech.
- **New personality:** a sharp tongue and dry, acidic humour. She assumes you might be wrong, questions your plans, and pushes back before agreeing, then actually helps. When you sound down, she drops the sarcasm and is kind.

## New in v2.4
- **Memory and notes actually save now.** She uses `qwen3-vl:2b-instruct`, which answers directly. The plain `qwen3-vl:2b` thinks first and often ran out of room, so memories came back empty. Replies are also about 5× faster. If you're upgrading, run `ollama pull qwen3-vl:2b-instruct`.
- **"Note this down", "write that down" and "take a note"** are saved into your second brain by the app itself, so her confirmation is always true.
- A stray mood tag in the middle of a reply after she uses a tool is now hidden.

## New in v2.3
- **Screen journal (opt-in):** click 📓 and, whenever your screen changes, she adds one line about what you're doing (app and task only) to `Documents\AIKO Brain\Screen journal\<date>.md`. Her second brain indexes it, so you can ask what you worked on earlier. It's off by default, and the file is yours to read, edit or delete.

## New in v2.2
- **Second brain:** drop `.md` or `.txt` notes in `Documents\AIKO Brain` (the 📚 button opens it). She indexes them locally as vectors and recalls the relevant passages while you chat, naming the note. She can also search them on purpose, and save new notes when you say "note this down". Type `/brain` for stats. To use another folder, set `"brain_dir"` in `config.json`.

## New in v2.1
- **Arms:** her arms sway from the shoulder on her resting pose, and more while she talks.
- **Talking:** she lip-syncs in her teasing and angry moods, and while showing you her tablet.
- **Web tools:** she can search the web (DuckDuckGo), read web pages, and check the weather (Open-Meteo). Set a default place with `"location": "<city>"` in `config.json`.
- **Resizing:** she always fits the window when it's resized, instead of being cut off.

Older builds are kept under their git tags, for example `git checkout v2.0 -- release`.






