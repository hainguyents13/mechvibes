; Mechvibes Inno Setup Script
; Requires Inno Setup 6.0 or later: https://jrsoftware.org/isinfo.php
;
; Version is NOT hardcoded here - pass it at compile time:
;   ISCC.exe /DAppVersion=0.5.2 installer\windows\mechvibes-setup.iss
; scripts/build-windows-installer.ps1 does this automatically by reading
; Cargo.toml, so there is a single source of truth for the version.

#ifndef AppVersion
  #error AppVersion must be defined, e.g. ISCC /DAppVersion=0.5.2 mechvibes-setup.iss
#endif

#define MyAppName "Mechvibes"
#define MyAppVersion AppVersion
#define MyAppPublisher "Hai Nguyen"
#define MyAppURL "https://github.com/hainguyents13/mechvibes"
#define MyAppExeName "mechvibes.exe"
#define MyAppId "{{8B5F5E5E-5E5E-5E5E-5E5E-5E5E5E5E5E5E}"

; The app was called MechvibesDX before. Upgrades remove what those builds
; left under the old names (see [InstallDelete] and PrepareToInstall).
#define LegacyAppName "MechvibesDX"
#define LegacyExeName "mechvibes-dx.exe"

[Setup]
; NOTE: The value of AppId uniquely identifies this application. It is
; unchanged by the rename, so a MechvibesDX install upgrades in place.
AppId={#MyAppId}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}/issues
AppUpdatesURL={#MyAppURL}/releases
; The install folder keeps the old name. The original Electron Mechvibes
; installs to %LOCALAPPDATA%\Programs\Mechvibes, which is {autopf}\Mechvibes
; for a per-user install, and sharing it would let either uninstaller delete
; the other app's files.
DefaultDirName={autopf}\{#LegacyAppName}
DefaultGroupName={#MyAppName}
; Upgrades would otherwise keep the old "MechvibesDX" Start menu folder.
UsePreviousGroup=no
AllowNoIcons=yes
LicenseFile=..\..\LICENSE
; OutputBaseFilename MUST contain "x64" and end in ".exe" to match the
; in-app auto-updater's asset filter (src/utils/auto_updater.rs).
OutputDir=..\..\dist
OutputBaseFilename=Mechvibes-{#MyAppVersion}-Setup-x64
SetupIconFile=..\..\assets\icon.ico
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayIcon={app}\{#MyAppExeName}
DisableProgramGroupPage=yes
DisableWelcomePage=no
CloseApplications=yes
CloseApplicationsFilter=*.exe
RestartApplications=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "startup"; Description: "Run at Windows startup"; GroupDescription: "Additional options:"; Flags: unchecked

[Files]
; Main executable
Source: "..\..\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion

; Assets folder - MUST be in the same directory as the executable for Dioxus asset!() macro
Source: "..\..\assets\*"; DestDir: "{app}\assets"; Flags: ignoreversion recursesubdirs createallsubdirs

; Soundpacks folder (built-in soundpacks)
Source: "..\..\soundpacks\*"; DestDir: "{app}\soundpacks"; Flags: ignoreversion recursesubdirs createallsubdirs

; No [Files] entry for data/ - it's not part of the source tree (config.json,
; manifest.json, etc. are runtime state, not shipped defaults). The app
; creates data/ itself on first run and falls back to defaults for any
; missing file (see AppConfig::load() in src/state/config.rs), so nothing
; needs to be bundled here.

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\{#MyAppExeName}"; IconIndex: 0
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\{#MyAppExeName}"; IconIndex: 0; Tasks: desktopicon

[Registry]
; Add to startup if task is selected
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "{#MyAppName}"; ValueData: """{app}\{#MyAppExeName}"" --minimized"; Flags: uninsdeletevalue; Tasks: startup

[Run]
; Interactive installs: the usual "run it now" checkbox on the last page.
; `skipifsilent` keeps it off the auto-update path, which has no final page.
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#MyAppName}}"; Flags: nowait postinstall skipifsilent

; Silent installs (i.e. auto-update): the ONE path that brings the app back.
;
; Do not add a `Check: not RmSessionStarted` guard here. That was tried and
; it broke auto-update completely - the app closed and never returned. A
; /LOG trace of a real silent install shows why:
;
;   Found 1 files to register with RestartManager.
;   RestartManager found an application using one of our files: MechvibesDX
;   Shutting down applications using our files.
;   Attempting to restart applications.
;
; CloseApplications=yes uses Restart Manager to close us, so a session is
; ALWAYS started and RmSessionStarted is ALWAYS true here - it says a session
; exists, not that our app will be revived. Restart Manager only revives
; processes that called RegisterApplicationRestart, which this app never
; does, so "Attempting to restart applications" is a no-op for us. The guard
; therefore skipped the only working relaunch on every single silent install.
;
; Double-launch is not a risk worth guarding against anyway: the app takes a
; named single-instance mutex at startup and a second copy exits immediately
; (src/libs/single_instance.rs, main.rs).
;
; `runasoriginaluser`: Setup may be elevated, but the app has to come back as
; the user who owns the tray and audio session, not as admin.
Filename: "{app}\{#MyAppExeName}"; Flags: nowait runasoriginaluser skipifnotsilent

[InstallDelete]
; Shortcuts and WebView2 data from MechvibesDX builds. Their executable is
; removed in PrepareToInstall, once it has exited.
Type: filesandordirs; Name: "{autoprograms}\{#LegacyAppName}"
Type: files; Name: "{autodesktop}\{#LegacyAppName}.lnk"
Type: filesandordirs; Name: "{localappdata}\mechvibes-dx"

[UninstallDelete]
; WebView2 keeps its data in %LOCALAPPDATA%\<executable name>.
Type: filesandordirs; Name: "{localappdata}\mechvibes"
Type: filesandordirs; Name: "{userappdata}\Mechvibes"

[Code]
// An auto-update starts Setup while the old app is still running; the app
// shuts down only after launching it. Restart Manager does not close a
// MechvibesDX build because its executable is not a file Setup installs, so
// wait for it to exit and remove it here. Left in place it would keep
// starting the old version from any pinned shortcut.
function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  LegacyExe: String;
  Attempt: Integer;
begin
  Result := '';
  LegacyExe := ExpandConstant('{app}\{#LegacyExeName}');
  Attempt := 0;
  while FileExists(LegacyExe) and not DeleteFile(LegacyExe) and (Attempt < 40) do
  begin
    Sleep(250);
    Attempt := Attempt + 1;
  end;
  if FileExists(LegacyExe) then
    Result := 'Close {#LegacyAppName}, then run Setup again.';
end;

procedure RefreshIconCache();
var
  ResultCode: Integer;
begin
  Exec('ie4uinit.exe', '-show', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
  begin
    // Create custom soundpacks directory in AppData
    CreateDir(ExpandConstant('{userappdata}\Mechvibes\soundpacks\keyboard'));
    CreateDir(ExpandConstant('{userappdata}\Mechvibes\soundpacks\mouse'));

    // Force Windows to refresh icon cache
    RefreshIconCache();
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  ResultCode: Integer;
begin
  if CurUninstallStep = usPostUninstall then
  begin
    if MsgBox('Do you want to remove all user data including custom soundpacks and settings?', mbConfirmation, MB_YESNO) = IDYES then
    begin
      DelTree(ExpandConstant('{userappdata}\Mechvibes'), True, True, True);
    end;
  end;
end;
