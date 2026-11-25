#!/usr/bin/env bash
# AIKO installer for macOS and Linux:
#   curl -fsSL https://raw.githubusercontent.com/<owner>/<repo>/main/installer/install.sh | bash
# Installs Ollama if it's missing, asks for her model size (2B or 4B) and downloads only that one,
# then installs the latest AIKO release. Options: --model 2b|4b, --yes (no questions: 2B, no shortcut).
set -euo pipefail

REPO="${AIKO_REPO:-aikeptoffline-cyber/aiko}"
MODEL=""; ASSUME_YES=0
while [ $# -gt 0 ]; do
  case "$1" in
    --model) MODEL="$2"; shift 2 ;;
    --yes|-y) ASSUME_YES=1; shift ;;
    *) echo "unknown option $1"; exit 1 ;;
  esac
done

say()  { printf '\n  %s\n' "$*"; }
fail() { printf '\n  Setup could not finish: %s\n\n' "$*" >&2; exit 1; }
# questions read from the terminal, so this also works when piped from curl
ask()  { local a=""; if [ "$ASSUME_YES" = 1 ] || [ ! -r /dev/tty ]; then echo "$2"; return; fi
         printf '  %s ' "$1" >/dev/tty; read -r a </dev/tty || true; echo "${a:-$2}"; }

OS="$(uname -s)"; ARCH="$(uname -m)"
case "$OS" in Darwin|Linux) ;; *) fail "this script is for macOS and Linux; on Windows use AIKO-Setup.exe" ;; esac
command -v curl >/dev/null || fail "curl is needed"

printf '\n  AIKO setup\n  ----------\n'
say "AIKO runs her AI on this computer with Ollama. Nothing you say to her leaves it."

# 1. model size: only the chosen one is downloaded
if [ -z "$MODEL" ]; then
  say "Which brain size?"
  printf '    1) 2B: works on any computer, even without a graphics card (1.9 GB download)\n'
  printf '    2) 4B: smarter; best with 6 GB+ of GPU memory or an Apple Silicon Mac with 16 GB (3.3 GB download)\n'
  case "$(ask 'Choose 1 or 2 [1]:' 1)" in 2|4b|4B) MODEL=4b ;; *) MODEL=2b ;; esac
fi
case "$MODEL" in 4b|4B) CHAT_MODEL="qwen3-vl:4b-instruct" ;; *) CHAT_MODEL="qwen3-vl:2b-instruct" ;; esac

# 2. Ollama
up() { curl -fsS -m 2 http://localhost:11434/api/tags >/dev/null 2>&1; }
if ! command -v ollama >/dev/null && [ ! -d /Applications/Ollama.app ]; then
  say "Installing Ollama (runs her AI locally)..."
  if [ "$OS" = Linux ]; then
    curl -fsSL https://ollama.com/install.sh | sh
  else
    tmp="$(mktemp -d)"; curl -fL# https://ollama.com/download/Ollama.dmg -o "$tmp/Ollama.dmg"
    mnt="$(hdiutil attach -nobrowse -quiet "$tmp/Ollama.dmg" | tail -1 | cut -f3)"
    cp -R "$mnt/Ollama.app" /Applications/ || fail "couldn't copy Ollama to /Applications"
    hdiutil detach -quiet "$mnt"; rm -rf "$tmp"
  fi
fi
if ! up; then
  say "Starting Ollama..."
  if [ "$OS" = Darwin ] && [ -d /Applications/Ollama.app ]; then open -a Ollama
  else (nohup ollama serve >/dev/null 2>&1 &) fi
  for _ in $(seq 1 30); do up && break; sleep 1; done
  up || fail "Ollama did not start. Start it yourself, then run this script again."
fi
OLLAMA="$(command -v ollama || echo /Applications/Ollama.app/Contents/Resources/ollama)"
for m in "$CHAT_MODEL" nomic-embed-text; do
  say "Downloading $m (only the first time; this can take a few minutes)"
  "$OLLAMA" pull "$m" || fail "downloading $m failed; check your internet connection and run this again"
done

# 3. AIKO itself, from the newest release
say "Finding the newest AIKO release..."
urls="$(curl -fsSL "https://api.github.com/repos/$REPO/releases?per_page=1" | grep -o '"browser_download_url": *"[^"]*"' | cut -d'"' -f4)"
pick() { echo "$urls" | grep -E "$1" | head -1; }
tmp="$(mktemp -d)"
if [ "$OS" = Darwin ]; then
  case "$ARCH" in arm64) url="$(pick '_aarch64\.dmg$')" ;; *) url="$(pick '_x64\.dmg$')" ;; esac
  [ -n "$url" ] || fail "no macOS download in the newest release of $REPO"
  curl -fL# "$url" -o "$tmp/AIKO.dmg"
  mnt="$(hdiutil attach -nobrowse -quiet "$tmp/AIKO.dmg" | tail -1 | cut -f3)"
  dest=/Applications; [ -w "$dest" ] || { dest="$HOME/Applications"; mkdir -p "$dest"; }
  rm -rf "$dest/AIKO.app"; cp -R "$mnt/AIKO.app" "$dest/"
  hdiutil detach -quiet "$mnt"
  # the beta isn't signed by Apple yet: without this, macOS refuses to open it
  xattr -dr com.apple.quarantine "$dest/AIKO.app" 2>/dev/null || true
  APP="$dest/AIKO.app"; DATA="$HOME/Library/Application Support/local.companion"
else
  [ "$ARCH" = x86_64 ] || fail "Linux builds are x86_64 only for now"
  sudo_() { if [ "$(id -u)" = 0 ]; then "$@"; else sudo "$@"; fi; }
  if command -v apt-get >/dev/null && url="$(pick '_amd64\.deb$')" && [ -n "$url" ]; then
    curl -fL# "$url" -o "$tmp/aiko.deb"; sudo_ apt-get install -y "$tmp/aiko.deb"; APP="aiko"
  elif command -v dnf >/dev/null && url="$(pick '\.x86_64\.rpm$')" && [ -n "$url" ]; then
    curl -fL# "$url" -o "$tmp/aiko.rpm"; sudo_ dnf install -y "$tmp/aiko.rpm"; APP="aiko"
  elif command -v zypper >/dev/null && url="$(pick '\.x86_64\.rpm$')" && [ -n "$url" ]; then
    curl -fL# "$url" -o "$tmp/aiko.rpm"; sudo_ zypper --non-interactive install --allow-unsigned-rpm "$tmp/aiko.rpm"; APP="aiko"
  else
    url="$(pick '_amd64\.AppImage$')"; [ -n "$url" ] || fail "no Linux download in the newest release of $REPO"
    mkdir -p "$HOME/.local/bin" "$HOME/.local/share/applications"
    curl -fL# "$url" -o "$HOME/.local/bin/aiko.AppImage"; chmod +x "$HOME/.local/bin/aiko.AppImage"
    printf '[Desktop Entry]\nName=AIKO\nComment=Anime desktop companion, AI kept offline\nExec=%s\nTerminal=false\nType=Application\nCategories=Utility;\n' \
      "$HOME/.local/bin/aiko.AppImage" > "$HOME/.local/share/applications/aiko.desktop"
    APP="$HOME/.local/bin/aiko.AppImage"
  fi
  DATA="${XDG_DATA_HOME:-$HOME/.local/share}/local.companion"
  case "$(ask 'Add a desktop shortcut? [y/N]:' n)" in
    y|Y|yes) desk="$(xdg-user-dir DESKTOP 2>/dev/null || echo "$HOME/Desktop")"; mkdir -p "$desk"
             for f in /usr/share/applications/aiko.desktop "$HOME/.local/share/applications/aiko.desktop"; do
               [ -f "$f" ] && { cp "$f" "$desk/"; chmod +x "$desk/aiko.desktop"; break; }; done ;;
  esac
fi
rm -rf "$tmp"

# 4. first install only: start her on the chosen model (later installs keep the user's settings)
mkdir -p "$DATA"
[ -f "$DATA/config.json" ] || printf '{ "chat_model": "%s" }\n' "$CHAT_MODEL" > "$DATA/config.json"

say "All set. She'll download her voice (21 MB) the first time she speaks."
if [ "$OS" = Darwin ]; then
  say "Starting AIKO. macOS will ask to allow Screen Recording the first time she looks at your screen."
  open "$APP"
else
  say "Starting AIKO (also in your applications menu)."
  (nohup "$APP" >/dev/null 2>&1 &)
fi
echo
