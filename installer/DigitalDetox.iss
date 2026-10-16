; Inno Setup script for the Digital Detox installer.
;
; Build it after `cargo build --release`, from the repository root:
;   iscc /DAppVersion=2.4.0 installer\DigitalDetox.iss
; The installer is written to release\DigitalDetox-v<version>-setup-x64.exe.
;
; Installs per user (no administrator rights, like the app itself), adds a Start menu shortcut
; and optionally a desktop one, and starts the app at the end, also when run silently by winget.

#ifndef AppVersion
  #error Pass the version: iscc /DAppVersion=X.Y.Z installer\DigitalDetox.iss
#endif

#define AppName "Digital Detox"
#define AppExe "DigitalDetox.exe"
#define RepoUrl "https://github.com/telcoray/DigitalDetox"

[Setup]
; Identifies the app across versions, so a new installer upgrades the old install. Never change it.
AppId={{42042902-594B-4431-8ED2-5B76A9614A92}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=telcoray
AppPublisherURL={#RepoUrl}
AppSupportURL={#RepoUrl}/issues
AppUpdatesURL={#RepoUrl}/releases
VersionInfoVersion={#AppVersion}
; Per user: %LOCALAPPDATA%\Programs\Digital Detox, no UAC prompt
PrivilegesRequired=lowest
DefaultDirName={autopf}\{#AppName}
DisableProgramGroupPage=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
WizardStyle=modern
SetupIconFile=..\assets\icon.ico
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppName}
OutputDir=..\release
OutputBaseFilename=DigitalDetox-v{#AppVersion}-setup-x64
Compression=lzma2/max
SolidCompression=yes
; An upgrade closes the running app (it lives in the system tray, so it's usually running)
CloseApplications=force
RestartApplications=no
; English or Italian, following the language of Windows
ShowLanguageDialog=auto

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "italian"; MessagesFile: "compiler:Languages\Italian.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "..\target\release\digital-detox.exe"; DestDir: "{app}"; DestName: "{#AppExe}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Run]
; No "skipifsilent": winget runs the installer silently, and the app should open afterwards too
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall

[UninstallRun]
; The app keeps running in the system tray: close it so its files can be removed
Filename: "{sys}\taskkill.exe"; Parameters: "/IM {#AppExe} /F"; Flags: runhidden; RunOnceId: "CloseApp"

[Code]
// "Start with Windows" is turned on from the app's tray menu, so the installer doesn't know
// about it: remove its entries on uninstall, or Windows would try to start a missing file.
// Only when the entry points to this install, not to a copy of the zip kept somewhere else.
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Command: String;
begin
  if (CurUninstallStep = usUninstall) and
     RegQueryStringValue(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Run', 'DigitalDetox', Command) and
     (Pos(Lowercase(ExpandConstant('{app}\{#AppExe}')), Lowercase(Command)) > 0) then
  begin
    RegDeleteValue(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Run', 'DigitalDetox');
    RegDeleteValue(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run', 'DigitalDetox');
  end;
end;
