[Setup]
AppId={{667285C4-2954-457A-9BFD-A9421995974F}
AppName=Reel
AppVersion=1.0.0
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
; Adjust this path to match your build output location
Source: "..\build\windows\x64\runner\Release\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs

[Icons]
Name: "{group}\Reel"; Filename: "{app}\reel.exe"
Name: "{autodesktop}\Reel"; Filename: "{app}\reel.exe"; Tasks: desktopicon

[UninstallDelete]
Type: files; Name: "{app}\*"
Type: dirifempty; Name: "{app}"

[Run]
Filename: "{app}\reel.exe"; Description: "Launch Reel"; Flags: nowait postinstall skipifsilent
