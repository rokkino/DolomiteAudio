; Inno Setup Script for ZeroLatency Audio Driver Suite
; Generates ZeroAudioSetup.exe with silent driver installation and ASIO registration

[Setup]
AppName=ZeroLatency Audio Driver Suite
AppVersion=1.0.0
AppPublisher=ZeroLatency Audio Technologies
DefaultDirName={autopf}\ZeroAudio
DefaultGroupName=ZeroLatency Audio
Compression=lzma2/ultra64
SolidCompression=yes
ArchitecturesInstallIn64BitMode=x64
PrivilegesRequired=admin
OutputDir=dist
OutputBaseFilename=ZeroAudioSetup

[Files]
Source: "target\release\audio_asio.dll"; DestDir: "{app}"; Flags: restartreplace uninsrestartdelete
Source: "target\release\audio-control.exe"; DestDir: "{app}"; Flags: restartreplace uninsrestartdelete
Source: "target\release\audio-bench.exe"; DestDir: "{app}"; Flags: restartreplace uninsrestartdelete
Source: "ZeroAudio.inf"; DestDir: "{app}"; Flags: restartreplace
Source: "ZeroLatencyAudioCA.cer"; DestDir: "{app}"; Flags: restartreplace
Source: "target\release\audio_kernel.sys"; DestDir: "{sys}\drivers"; Flags: restartreplace uninsrestartdelete

[Icons]
Name: "{group}\ZeroLatency Audio Control Panel"; Filename: "{app}\audio-control.exe"
Name: "{group}\ZeroLatency Audio Benchmark"; Filename: "{app}\audio-bench.exe"
Name: "{group}\Uninstall ZeroLatency Audio"; Filename: "{uninstallexe}"

[Registry]
; ASIO Driver Registration
Root: HKLM64; Subkey: "SOFTWARE\ASIO\ZeroLatency Audio"; ValueType: string; ValueName: "CLSID"; ValueData: "{3E8C1234-A789-4B56-9DEF-0123456789AB}"; Flags: uninsdeletekey
Root: HKLM64; Subkey: "SOFTWARE\ASIO\ZeroLatency Audio"; ValueType: string; ValueName: "Description"; ValueData: "ZeroLatency Rust Kernel ASIO (Sub-Nanosecond Direct)"

; COM InprocServer32 Registration
Root: HKLM64; Subkey: "SOFTWARE\Classes\CLSID\{3E8C1234-A789-4B56-9DEF-0123456789AB}\InprocServer32"; ValueType: string; ValueData: "{app}\audio_asio.dll"; Flags: uninsdeletekey
Root: HKLM64; Subkey: "SOFTWARE\Classes\CLSID\{3E8C1234-A789-4B56-9DEF-0123456789AB}\InprocServer32"; ValueType: string; ValueName: "ThreadingModel"; ValueData: "Both"

[Run]
; Install Certificate into TrustedPublisher
Filename: "certutil.exe"; Parameters: "-addstore -f ""TrustedPublisher"" ""{app}\ZeroLatencyAudioCA.cer"""; Flags: runhidden
; Install and Register PnP Driver
Filename: "pnputil.exe"; Parameters: "-i -a ""{app}\ZeroAudio.inf"""; Flags: runhidden
; Create and Start Kernel Service
Filename: "sc.exe"; Parameters: "create ZeroLatencyAudio type= kernel binPath= ""{sys}\drivers\audio_kernel.sys"""; Flags: runhidden
Filename: "sc.exe"; Parameters: "start ZeroLatencyAudio"; Flags: runhidden
; Run Control Panel after setup
Filename: "{app}\audio-control.exe"; Description: "Launch ZeroLatency Audio Control Panel"; Flags: postinstall nowait skipifsilent

[UninstallRun]
Filename: "sc.exe"; Parameters: "stop ZeroLatencyAudio"; Flags: runhidden
Filename: "sc.exe"; Parameters: "delete ZeroLatencyAudio"; Flags: runhidden
