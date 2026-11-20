; Instalator Bieżni (Inno Setup 6/7). Budowanie: uv run python packaging/zbuduj.py
; Instalacja na koncie użytkownika (bez administratora), dane w %LOCALAPPDATA%\Bieznia
; zostają przy aktualizacji i odinstalowaniu.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

[Setup]
AppId={{6F4C2D8E-3B1A-4E5F-9C7D-2A8B1E0F4D63}
AppName=Bieżnia
AppVersion={#AppVersion}
AppVerName=Bieżnia {#AppVersion}
AppPublisher=damnit.com.pl Damian Skrzypiec
AppPublisherURL=https://damnit.com.pl
AppSupportURL=https://github.com/Damianejroo/bieznia-olek/issues
AppUpdatesURL=https://github.com/Damianejroo/bieznia-olek/releases
AppCopyright=© 2026 damnit.com.pl Damian Skrzypiec
VersionInfoVersion={#AppVersion}
DefaultDirName={localappdata}\Programs\Bieznia
DefaultGroupName=Bieżnia
DisableProgramGroupPage=yes
DisableDirPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\dist
OutputBaseFilename=Bieznia-Setup-{#AppVersion}
SetupIconFile=bieznia.ico
UninstallDisplayIcon={app}\Bieznia.exe
UninstallDisplayName=Bieżnia
WizardStyle=modern
Compression=lzma2/ultra64
SolidCompression=yes
CloseApplications=yes
RestartApplications=no
LicenseFile=..\LICENSE

[Languages]
Name: "polish"; MessagesFile: "compiler:Languages\Polish.isl"

[Tasks]
Name: "desktopicon"; Description: "Skrót na pulpicie"; GroupDescription: "Skróty:"

[Files]
Source: "..\build\dist\Bieznia\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[InstallDelete]
; przy aktualizacji usuń stare biblioteki, żeby nie mieszały się z nowymi
Type: filesandordirs; Name: "{app}\_internal"

[Icons]
Name: "{autoprograms}\Bieżnia"; Filename: "{app}\Bieznia.exe"
Name: "{autodesktop}\Bieżnia"; Filename: "{app}\Bieznia.exe"; Tasks: desktopicon

[Run]
; zwykła instalacja: pole "Uruchom Bieżnię" na końcu
Filename: "{app}\Bieznia.exe"; Description: "Uruchom Bieżnię"; Flags: nowait postinstall skipifsilent
; aktualizacja z aplikacji (tryb cichy): uruchom nową wersję od razu
Filename: "{app}\Bieznia.exe"; Flags: nowait; Check: WizardSilent
