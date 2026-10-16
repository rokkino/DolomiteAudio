; DolomiteAudioRT installer (Inno Setup 6)
; Installs the user-mode ASIO driver + control panel. No kernel driver, no
; certificates, nothing outside {app} except the ASIO / COM registration.

#ifndef MyAppVersion
  #define MyAppVersion "2.2.0"
#endif
#define MyAppName "DolomiteAudioRT"
#define MyClsid "{{3E8C1234-A789-4B56-9DEF-0123456789AB}"

[Setup]
AppId={{7B1E6C52-2F4A-4D7E-9C3B-D0105A0D10A0}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher=rokkino
AppPublisherURL=https://github.com/rokkino/DolomiteAudio
AppSupportURL=https://github.com/rokkino/DolomiteAudio/issues
DefaultDirName={autopf}\DolomiteAudio
DefaultGroupName=DolomiteAudio
DisableProgramGroupPage=yes
LicenseFile=LICENSE-MIT
Compression=lzma2/ultra64
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=admin
CloseApplications=yes
RestartApplications=no
OutputDir=dist
OutputBaseFilename=DolomiteAudioRT-Setup-{#MyAppVersion}
UninstallDisplayName={#MyAppName} ASIO driver
WizardStyle=modern
MinVersion=10.0

[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl"
Name: "it"; MessagesFile: "compiler:Languages\Italian.isl"

[Tasks]
Name: "autostart"; Description: "Icona nell'area di notifica all'avvio di Windows / Tray icon at Windows startup"; Flags: checkedonce

[Files]
Source: "target\release\audio_asio.dll"; DestDir: "{app}"; Flags: ignoreversion restartreplace uninsrestartdelete
Source: "target\release\audio-control.exe"; DestDir: "{app}"; Flags: ignoreversion restartreplace uninsrestartdelete
Source: "target\release\asio_selftest.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "LICENSE-MIT"; DestDir: "{app}"; Flags: ignoreversion
Source: "LICENSE-APACHE"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\DolomiteAudio Control Panel"; Filename: "{app}\audio-control.exe"; Parameters: "gui"
Name: "{group}\Uninstall DolomiteAudio"; Filename: "{uninstallexe}"

[Registry]
; remove registrations left by older builds
Root: HKLM64; Subkey: "SOFTWARE\ASIO\ZeroLatency Audio"; Flags: deletekey
Root: HKLM64; Subkey: "SOFTWARE\ASIO\ZeroLatency Rust Kernel ASIO"; Flags: deletekey
; ASIO driver list (read by every ASIO host)
Root: HKLM64; Subkey: "SOFTWARE\ASIO\{#MyAppName}"; ValueType: string; ValueName: "CLSID"; ValueData: "{#MyClsid}"; Flags: uninsdeletekey
Root: HKLM64; Subkey: "SOFTWARE\ASIO\{#MyAppName}"; ValueType: string; ValueName: "Description"; ValueData: "{#MyAppName}"
; COM in-proc server
Root: HKLM64; Subkey: "SOFTWARE\Classes\CLSID\{#MyClsid}"; ValueType: string; ValueData: "{#MyAppName}"; Flags: uninsdeletekey
Root: HKLM64; Subkey: "SOFTWARE\Classes\CLSID\{#MyClsid}\InprocServer32"; ValueType: string; ValueData: "{app}\audio_asio.dll"
Root: HKLM64; Subkey: "SOFTWARE\Classes\CLSID\{#MyClsid}\InprocServer32"; ValueType: string; ValueName: "ThreadingModel"; ValueData: "Apartment"
; tray icon at login (same value the tray menu toggles)
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "DolomiteAudio"; ValueData: """{app}\audio-control.exe"" tray"; Tasks: autostart; Flags: uninsdeletevalue

[Run]
Filename: "{app}\audio-control.exe"; Parameters: "gui"; Description: "Apri il pannello DolomiteAudio / Open the control panel"; Flags: postinstall nowait skipifsilent runasoriginaluser

[UninstallRun]
Filename: "{sys}\taskkill.exe"; Parameters: "/f /im audio-control.exe"; Flags: runhidden; RunOnceId: "KillTray"

[UninstallDelete]
Type: dirifempty; Name: "{app}"
