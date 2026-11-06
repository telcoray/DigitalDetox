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

[Messages]
italian.WizardSelectTasks=Opzioni aggiuntive
italian.SelectTasksDesc=Personalizza l'installazione di Digital Detox.
italian.SelectTasksLabel2=Scegli le opzioni che preferisci, quindi seleziona Avanti.

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "autostart"; Description: "{cm:StartWithWindows}"; GroupDescription: "{cm:AdditionalOptions}"; Flags: unchecked

[CustomMessages]
english.StartWithWindows=Start Digital Detox when I sign in to Windows
english.AdditionalOptions=Additional options:
italian.StartWithWindows=Avvia Digital Detox all'accesso a Windows
italian.AdditionalOptions=Opzioni aggiuntive:

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
const
  RunKey = 'Software\Microsoft\Windows\CurrentVersion\Run';
  ApprovedKey = 'Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run';

function OwnsStartupEntry: Boolean;
var
  Command: String;
begin
  Result := RegQueryStringValue(HKCU, RunKey, 'DigitalDetox', Command) and
    (Pos(Lowercase('"' + ExpandConstant('{app}\{#AppExe}') + '"'), Lowercase(Command)) = 1);
end;

procedure InitializeStartupTask;
var
  Approval: AnsiString;
  Enabled: Boolean;
begin
  // Follow the current tray/Windows setting on upgrades, rather than the last install choice.
  // Leave explicit command-line task choices to Inno Setup.
  if (ExpandConstant('{param:TASKS|}') <> '') or
     (ExpandConstant('{param:MERGETASKS|}') <> '') then
    Exit;
  Enabled := OwnsStartupEntry;
  if RegQueryBinaryValue(HKCU, ApprovedKey, 'DigitalDetox', Approval) then
    if Length(Approval) > 0 then
      Enabled := Enabled and ((Ord(Approval[1]) mod 2) = 0);
  if Enabled then
    WizardSelectTasks('autostart')
  else
    WizardSelectTasks('!autostart');
end;

// Initialize only when the tasks page is reached: {app} is not available in InitializeWizard.
// ShouldSkipPage also runs during silent setup. Remember the choice when navigating back.
var
  StartupTaskInitialized: Boolean;

function ShouldSkipPage(PageID: Integer): Boolean;
begin
  Result := False;
  if (PageID = wpSelectTasks) and not StartupTaskInitialized then
  begin
    InitializeStartupTask;
    StartupTaskInitialized := True;
    Log('Startup task initialized after destination selection.');
  end;
end;

procedure RemoveStartupEntry;
begin
  // Do not remove the entry of a portable copy kept somewhere else.
  if OwnsStartupEntry then
  begin
    RegDeleteValue(HKCU, RunKey, 'DigitalDetox');
    RegDeleteValue(HKCU, ApprovedKey, 'DigitalDetox');
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep <> ssPostInstall then
    Exit;
  if WizardIsTaskSelected('autostart') then
  begin
    if not RegWriteStringValue(HKCU, RunKey, 'DigitalDetox',
      '"' + ExpandConstant('{app}\{#AppExe}') + '" --background') then
      RaiseException('Could not enable Digital Detox at Windows sign-in.');
    // Match the app's switch: clear a previous Windows Startup disable flag.
    RegDeleteValue(HKCU, ApprovedKey, 'DigitalDetox');
  end
  else
    RemoveStartupEntry;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
    RemoveStartupEntry;
end;
