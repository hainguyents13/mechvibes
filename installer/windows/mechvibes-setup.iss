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

[Setup]
; NOTE: The value of AppId uniquely identifies this application.
AppId={#MyAppId}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}/issues
AppUpdatesURL={#MyAppURL}/releases
; Not {autopf}\{#MyAppName}: the original Electron Mechvibes installs per-user
; into %LOCALAPPDATA%\Programs\Mechvibes, and sharing that folder would mix
; the two apps' files and let either uninstaller delete the other's. Upgrades
; keep whatever folder they were installed in (UsePreviousAppDir).
DefaultDirName={autopf}\MechvibesDX
DefaultGroupName={#MyAppName}
; Upgrades from MechvibesDX would otherwise reuse its Start Menu folder name.
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
; Tells Explorer to pick up the .mvtheme file type after install and uninstall.
ChangesAssociations=yes

[InstallDelete]
; Leftovers from when the app was called MechvibesDX (0.8.x and earlier).
; Listing the old exe here also makes CloseApplications close a copy of it
; that is still running, since Restart Manager covers [InstallDelete] files.
Type: files; Name: "{app}\mechvibes-dx.exe"
Type: filesandordirs; Name: "{autoprograms}\MechvibesDX"
Type: files; Name: "{autodesktop}\MechvibesDX.lnk"
; WebView2 cache, named after the exe by Dioxus.
Type: filesandordirs; Name: "{localappdata}\mechvibes-dx"

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
; mechvibes:// links from the sound pack library ("Install in Mechvibes").
; HKCU on purpose: Mechvibes 2 registered the same scheme there, and an entry
; in HKCU wins over one in HKLM, so it has to be replaced where it lives.
Root: HKCU; Subkey: "Software\Classes\mechvibes"; ValueType: string; ValueName: ""; ValueData: "URL:{#MyAppName}"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\mechvibes"; ValueType: string; ValueName: "URL Protocol"; ValueData: ""
Root: HKCU; Subkey: "Software\Classes\mechvibes\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"",0"
Root: HKCU; Subkey: "Software\Classes\mechvibes\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""
; .mvtheme theme files from the Share button: double-clicking one opens it in
; the app, which asks before putting it on. HKA, so an install for everyone
; registers it for everyone; no older app owns this extension. On uninstall
; only this app's values go, and the .mvtheme key only if nothing else is in it.
Root: HKA; Subkey: "Software\Classes\.mvtheme"; ValueType: string; ValueName: ""; ValueData: "Mechvibes.Theme"; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKA; Subkey: "Software\Classes\.mvtheme"; ValueType: string; ValueName: "Content Type"; ValueData: "application/x-mechvibes-theme"; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKA; Subkey: "Software\Classes\.mvtheme\OpenWithProgids"; ValueType: string; ValueName: "Mechvibes.Theme"; ValueData: ""; Flags: uninsdeletevalue uninsdeletekeyifempty
Root: HKA; Subkey: "Software\Classes\Mechvibes.Theme"; ValueType: string; ValueName: ""; ValueData: "{#MyAppName} theme"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Mechvibes.Theme\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"",0"
Root: HKA; Subkey: "Software\Classes\Mechvibes.Theme\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""
Root: HKA; Subkey: "Software\Classes\Applications\{#MyAppExeName}\SupportedTypes"; ValueType: string; ValueName: ".mvtheme"; ValueData: ""; Flags: uninsdeletekey

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
;   RestartManager found an application using one of our files: Mechvibes
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

[UninstallDelete]
; WebView2 cache only. Settings and sound packs in {userappdata}\Mechvibes
; are deleted only when the user says so (CurUninstallStepChanged): entries
; here run before that question is asked, so listing that folder here would
; delete it whatever the answer. The original Electron app keeps its data in
; the same folder, too.
Type: filesandordirs; Name: "{localappdata}\{#MyAppName}"

[Code]
const
  UninstallKeyRoot = 'Software\Microsoft\Windows\CurrentVersion\Uninstall';

// The original Electron Mechvibes (2.x) is a separate install. Left in place
// it keeps running alongside this app, and every key plays twice. Its
// electron-builder installer registers an uninstall entry whose
// QuietUninstallString is '"<dir>\Uninstall Mechvibes.exe" /currentuser /S'
// (or /allusers), under HKCU or HKLM depending on how it was installed.
function FindElectronMechvibes(var RootKey: Integer; var SubKey, Command: String): Boolean;
var
  Roots: array[0..1] of Integer;
  Names: TArrayOfString;
  R, I: Integer;
  Version, Quiet: String;
begin
  Result := False;
  Roots[0] := HKCU;
  Roots[1] := HKLM;
  for R := 0 to 1 do
    if RegGetSubkeyNames(Roots[R], UninstallKeyRoot, Names) then
      for I := 0 to GetArrayLength(Names) - 1 do
      begin
        SubKey := UninstallKeyRoot + '\' + Names[I];
        if RegQueryStringValue(Roots[R], SubKey, 'DisplayVersion', Version) and
           (Copy(Version, 1, 2) = '2.') and
           RegQueryStringValue(Roots[R], SubKey, 'QuietUninstallString', Quiet) and
           (Pos('\uninstall mechvibes.exe"', Lowercase(Quiet)) > 0) then
        begin
          RootKey := Roots[R];
          Command := Quiet;
          Result := True;
          Exit;
        end;
      end;
end;

// Runs the Electron app's own silent uninstaller, which keeps its user data
// (electron-builder's default). ShellExec rather than Exec so an all-users
// install can ask for elevation. An NSIS uninstaller hands off to a copy of
// itself and returns at once, so success is its uninstall entry going away.
function RemoveElectronMechvibes(RootKey: Integer; SubKey, Command: String): Boolean;
var
  ExePath, Params: String;
  ErrorCode, Attempt: Integer;
begin
  ExePath := Copy(Command, 2, Pos('"', Copy(Command, 2, Length(Command))) - 1);
  Params := Trim(Copy(Command, Length(ExePath) + 3, Length(Command)));
  if not ShellExec('', ExePath, Params, '', SW_HIDE, ewWaitUntilTerminated, ErrorCode) then
  begin
    Result := False;
    Exit;
  end;
  Attempt := 0;
  while RegKeyExists(RootKey, SubKey) and (Attempt < 60) do
  begin
    Sleep(500);
    Attempt := Attempt + 1;
  end;
  Result := not RegKeyExists(RootKey, SubKey);
end;

function HasSwitch(const Name: String): Boolean;
var
  I: Integer;
begin
  Result := False;
  for I := 1 to ParamCount do
    if CompareText(ParamStr(I), '/' + Name) = 0 then
    begin
      Result := True;
      Exit;
    end;
end;

// Offered before anything is installed: the Electron uninstaller removes a
// desktop shortcut named Mechvibes.lnk, which is also this app's.
// A silent install never asks. The in-app updater leaves Mechvibes 2 alone;
// winget, which upgrades the old HaiNguyen.Mechvibes package silently, passes
// /REMOVEMECHVIBES2 so that upgrade does not leave both apps playing.
function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  RootKey: Integer;
  SubKey, Command: String;
begin
  Result := '';
  if not FindElectronMechvibes(RootKey, SubKey, Command) then
    Exit;
  if WizardSilent then
  begin
    if HasSwitch('REMOVEMECHVIBES2') then
      if RemoveElectronMechvibes(RootKey, SubKey, Command) then
        Log('Removed Mechvibes 2')
      else
        Log('Mechvibes 2 could not be removed');
    Exit;
  end;
  if MsgBox('The previous Mechvibes (version 2) is still installed. With both installed, ' +
      'every key plays twice.' + #13#10#13#10 +
      'Remove version 2 now? Only the old app is removed, nothing else.',
      mbConfirmation, MB_YESNO) = IDYES then
    if not RemoveElectronMechvibes(RootKey, SubKey, Command) then
      MsgBox('Mechvibes 2 could not be removed automatically. You can remove it ' +
        'later in Windows Settings > Apps.', mbInformation, MB_OK);
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
    // Defaults to No, and a silent uninstall keeps the data, so a reinstall
    // finds everything where it was.
    if SuppressibleMsgBox('Also delete your settings and imported sound packs?' + #13#10#13#10 +
      'Choose No to keep them for when you install Mechvibes again.',
      mbConfirmation, MB_YESNO or MB_DEFBUTTON2, IDNO) = IDYES then
    begin
      DelTree(ExpandConstant('{userappdata}\Mechvibes'), True, True, True);
    end;
  end;
end;
