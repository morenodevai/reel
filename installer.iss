[Setup]
AppName=Reel
AppVersion=2.2.0
AppPublisher=Reel
DefaultDirName={autopf}\Reel
DefaultGroupName=Reel
OutputDir=installer_output
OutputBaseFilename=Reel_2.2.0_x64-setup
Compression=lzma2
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayIcon={app}\reel.exe
SetupIconFile=windows\runner\resources\app_icon.ico
WizardStyle=modern
DisableProgramGroupPage=yes

[Files]
Source: "build\windows\x64\runner\Release\reel.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "build\windows\x64\runner\Release\*.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "build\windows\x64\runner\Release\data\*"; DestDir: "{app}\data"; Flags: ignoreversion recursesubdirs

[Icons]
Name: "{group}\Reel"; Filename: "{app}\reel.exe"
Name: "{autodesktop}\Reel"; Filename: "{app}\reel.exe"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional icons:"

[Run]
Filename: "{app}\reel.exe"; Description: "Launch Reel"; Flags: nowait postinstall skipifsilent
