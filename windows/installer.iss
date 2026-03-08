[Setup]
AppId={{667285C4-2954-457A-9BFD-A9421995974F}
AppName=Reel
; Keep in sync with pubspec.yaml version
AppVersion=2.2.0
AppPublisher=Reel
DefaultDirName={autopf}\Reel
DefaultGroupName=Reel
OutputDir=.
OutputBaseFilename=Reel_Setup
Compression=lzma2
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayName=Reel
UninstallDisplayIcon={app}\reel.exe
WizardStyle=modern
MinVersion=10.0

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional icons:"

[Files]
; Flutter build output
Source: "..\build\windows\x64\runner\Release\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs
; Bundled dependencies (ffprobe, ffmpeg, AI model) — deployed to %APPDATA%\Reel by the app on first launch
Source: "..\bundle\bin\*"; DestDir: "{app}\data\bin"; Flags: ignoreversion skipifsourcedoesntexist
Source: "..\bundle\models\*"; DestDir: "{app}\data\models"; Flags: ignoreversion skipifsourcedoesntexist

[Icons]
Name: "{group}\Reel"; Filename: "{app}\reel.exe"
Name: "{autodesktop}\Reel"; Filename: "{app}\reel.exe"; Tasks: desktopicon

[UninstallDelete]
Type: files; Name: "{app}\*"
Type: dirifempty; Name: "{app}"
Type: filesandordirs; Name: "{app}\data"
Type: filesandordirs; Name: "{userappdata}\Reel\bin"
Type: filesandordirs; Name: "{userappdata}\Reel\models"

[Run]
Filename: "{app}\reel.exe"; Description: "Launch Reel"; Flags: nowait postinstall skipifsilent
