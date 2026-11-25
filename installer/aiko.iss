; Windows installer for AIKO (Inno Setup 6). Build: ISCC.exe installer\aiko.iss
; Installs for the current user only (no admin prompt). Checks for the WebView2 runtime and Ollama and
; downloads whatever is missing, asks about shortcuts and her model size, then downloads the models.

#define AppName "AIKO"
#define AppVersion "2.9.0-beta"
#define AppExe "aiko.exe"
#ifndef BuildExe
  ; local builds use a short target folder (Windows path limit); CI passes /DBuildExe=...
  #define BuildExe GetEnv("LOCALAPPDATA") + "\companion-build\release\aiko.exe"
#endif

[Setup]
AppId={{3B281C9C-824F-421F-965A-E5B1EDF37F17}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
DefaultDirName={localappdata}\Programs\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir=..\release
OutputBaseFilename=AIKO-Setup-{#AppVersion}
SetupIconFile=..\src-tauri\icons\icon.ico
UninstallDisplayIcon={app}\{#AppExe}
WizardStyle=modern
Compression=lzma2
SolidCompression=yes
CloseApplications=yes
; her models are downloaded after install
ExtraDiskSpaceRequired=4000000000

[Files]
Source: "{#BuildExe}"; DestDir: "{app}"; DestName: "{#AppExe}"; Flags: ignoreversion
Source: "setup-deps.ps1"; DestDir: "{app}"; Flags: ignoreversion
Source: "TESTERS.txt"; DestDir: "{app}"; Flags: ignoreversion isreadme

; she was called Companion before 2.7.1 (same AppId, so this upgrades that install)
[InstallDelete]
Type: files; Name: "{app}\companion.exe"
Type: files; Name: "{autoprograms}\Companion.lnk"
Type: files; Name: "{autodesktop}\Companion.lnk"

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Check: WantDesktop

[Run]
Filename: "{tmp}\MicrosoftEdgeWebview2Setup.exe"; Parameters: "/silent /install"; StatusMsg: "Installing the Microsoft WebView2 runtime..."; Check: NeedWebView2
Filename: "{tmp}\OllamaSetup.exe"; Parameters: "/VERYSILENT /SUPPRESSMSGBOXES /NORESTART"; StatusMsg: "Installing Ollama (runs her AI locally)..."; Check: NeedOllama
Filename: "powershell.exe"; Parameters: "-NoProfile -ExecutionPolicy Bypass -File ""{app}\setup-deps.ps1"" -Model {code:ModelName}"; StatusMsg: "Downloading her AI models (this can take a few minutes)..."
Filename: "{app}\{#AppExe}"; Description: "Start {#AppName} now"; Flags: postinstall nowait skipifsilent

[Code]
const
  WebView2Key = 'SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}';

var
  DepsPage: TWizardPage;
  ShortcutChoice, ModelChoice: TInputOptionWizardPage;
  DownloadPage: TDownloadWizardPage;
  MissingWebView2, MissingOllama: Boolean;

function WebView2Installed: Boolean;
var
  V: String;
begin
  Result := (RegQueryStringValue(HKLM32, WebView2Key, 'pv', V) or RegQueryStringValue(HKCU, WebView2Key, 'pv', V))
    and (V <> '') and (V <> '0.0.0.0');
end;

function OllamaInstalled: Boolean;
begin
  Result := FileExists(ExpandConstant('{localappdata}\Programs\Ollama\ollama.exe'))
    or FileExists(ExpandConstant('{commonpf64}\Ollama\ollama.exe'));
end;

function NeedWebView2: Boolean; begin Result := MissingWebView2; end;
function NeedOllama: Boolean; begin Result := MissingOllama; end;
function WantDesktop: Boolean; begin Result := ShortcutChoice.Values[0]; end;

function ModelName(Param: String): String;
begin
  if ModelChoice.Values[1] then Result := 'qwen3-vl:4b-instruct' else Result := 'qwen3-vl:2b-instruct';
end;

function Status(Ok: Boolean): String;
begin
  if Ok then Result := 'already installed' else Result := 'will be downloaded and installed';
end;

procedure InitializeWizard;
begin
  MissingWebView2 := not WebView2Installed;
  MissingOllama := not OllamaInstalled;

  DepsPage := CreateOutputMsgPage(wpWelcome, 'What AIKO needs',
    'AIKO runs her AI entirely on this PC. Setup checked what is already here.',
    'Microsoft WebView2 runtime (draws her window): ' + Status(not MissingWebView2) + #13#10 +
    'Ollama (runs her AI model locally): ' + Status(not MissingOllama) + #13#10 +
    'Her AI models (2 to 3.6 GB): downloaded at the end of setup' + #13#10#13#10 +
    'Nothing you say to her leaves this PC. Only her optional web search and weather tools go online.' + #13#10#13#10 +
    'This is a beta. Windows may warn about an unknown publisher: choose "More info" and then "Run anyway".');

  ShortcutChoice := CreateInputOptionPage(wpSelectDir, 'Shortcuts', 'Where should AIKO appear?',
    'Choose where to put shortcuts to her, then click Next.', True, False);
  ShortcutChoice.Add('Desktop and Start menu');
  ShortcutChoice.Add('Start menu only');
  ShortcutChoice.Values[0] := True;

  ModelChoice := CreateInputOptionPage(ShortcutChoice.ID, 'Her AI brain', 'Which model size should she use?',
    'You can switch later with the 2B / 4B button in her top bar.', True, False);
  ModelChoice.Add('2B: works on any PC, including ones without a graphics card (1.9 GB download)');
  ModelChoice.Add('4B: smarter, best with a graphics card with 6 GB or more (3.3 GB download)');
  ModelChoice.Values[0] := True;

  DownloadPage := CreateDownloadPage(SetupMessage(msgWizardPreparing), SetupMessage(msgPreparingDesc), nil);
end;

function NextButtonClick(CurPageID: Integer): Boolean;
begin
  Result := True;
  if (CurPageID = wpReady) and (MissingWebView2 or MissingOllama) then begin
    DownloadPage.Clear;
    if MissingWebView2 then
      DownloadPage.Add('https://go.microsoft.com/fwlink/p/?LinkId=2124703', 'MicrosoftEdgeWebview2Setup.exe', '');
    if MissingOllama then
      DownloadPage.Add('https://ollama.com/download/OllamaSetup.exe', 'OllamaSetup.exe', '');
    DownloadPage.Show;
    try
      try
        DownloadPage.Download;
      except
        if not DownloadPage.AbortedByUser then
          SuppressibleMsgBox('Downloading a dependency failed: ' + GetExceptionMessage + #13#10 +
            'Check your internet connection and try again.', mbCriticalError, MB_OK, IDOK);
        Result := False;
      end;
    finally
      DownloadPage.Hide;
    end;
  end;
end;

// first install only: start her on the chosen model; later installs keep the user's settings
procedure CurStepChanged(CurStep: TSetupStep);
var
  Cfg: String;
begin
  if CurStep = ssPostInstall then begin
    Cfg := ExpandConstant('{userappdata}\local.companion\config.json');
    if not FileExists(Cfg) then begin
      ForceDirectories(ExtractFileDir(Cfg));
      SaveStringToFile(Cfg, '{ "chat_model": "' + ModelName('') + '" }', False);
    end;
  end;
end;
