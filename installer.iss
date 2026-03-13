[Setup]
AppId={{667285C4-2954-457A-9BFD-A9421995974F}
AppName=Reel
; Keep in sync with pubspec.yaml version
AppVersion=2.6.0
AppPublisher=Reel
DefaultDirName={autopf}\Reel
DefaultGroupName=Reel
OutputDir=installer_output
OutputBaseFilename=Reel_2.6.0_x64-setup
Compression=lzma2
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayName=Reel
UninstallDisplayIcon={app}\reel.exe
SetupIconFile=windows\runner\resources\app_icon.ico
WizardStyle=modern
MinVersion=10.0
DisableProgramGroupPage=yes
; Auto-close the app on install/uninstall so files aren't locked
CloseApplications=force
CloseApplicationsFilter=reel.exe

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional icons:"

[Files]
; Flutter build output
Source: "build\windows\x64\runner\Release\reel.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "build\windows\x64\runner\Release\*.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "build\windows\x64\runner\Release\data\*"; DestDir: "{app}\data"; Flags: ignoreversion recursesubdirs
; Bundled dependencies (ffprobe, ffmpeg, AI model) — deployed to %APPDATA%\Reel by the app on first launch
Source: "bundle\bin\*"; DestDir: "{app}\data\bin"; Flags: ignoreversion
Source: "bundle\models\*"; DestDir: "{app}\data\models"; Flags: ignoreversion

[Icons]
Name: "{group}\Reel"; Filename: "{app}\reel.exe"
Name: "{autodesktop}\Reel"; Filename: "{app}\reel.exe"; Tasks: desktopicon

[InstallDelete]
; Clean previous install completely before writing new files
Type: filesandordirs; Name: "{app}\data"
Type: files; Name: "{app}\*.dll"
Type: files; Name: "{app}\*.exe"

[UninstallDelete]
; Remove everything in the install directory
Type: filesandordirs; Name: "{app}\data"
Type: files; Name: "{app}\*.dll"
Type: files; Name: "{app}\*.exe"
Type: files; Name: "{app}\unins*"
Type: dirifempty; Name: "{app}"
; Remove deployed runtime assets (but NOT user config/db/logs)
Type: filesandordirs; Name: "{userappdata}\Reel\bin"
Type: filesandordirs; Name: "{userappdata}\Reel\models"

[Code]
// Kill reel.exe before uninstall to prevent locked file errors
function InitializeUninstall(): Boolean;
var
  ResultCode: Integer;
begin
  Exec('taskkill.exe', '/F /IM reel.exe', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Result := True;
end;

[Run]
Filename: "{app}\reel.exe"; Description: "Launch Reel"; Flags: nowait postinstall
