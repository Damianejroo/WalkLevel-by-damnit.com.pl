; Instalator Bieżni (Inno Setup 6/7). Budowanie: uv run python packaging/zbuduj.py
; Jedno kliknięcie: bez pytań i bez umowy, pasek postępu, potem aplikacja sama się otwiera.
; Instalacja na koncie użytkownika (bez administratora). Dane w %LOCALAPPDATA%\Bieznia
; zostają przy aktualizacji i odinstalowaniu.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#define UninstallKey "Software\Microsoft\Windows\CurrentVersion\Uninstall\{6F4C2D8E-3B1A-4E5F-9C7D-2A8B1E0F4D63}_is1"

[Setup]
AppId={{6F4C2D8E-3B1A-4E5F-9C7D-2A8B1E0F4D63}
AppName=Bieżnia Gamingowa
AppVersion={#AppVersion}
AppVerName=Bieżnia Gamingowa {#AppVersion} by damnit.com.pl
AppPublisher=damnit.com.pl Damian Skrzypiec
AppPublisherURL=https://damnit.com.pl
AppSupportURL=https://github.com/Damianejroo/bieznia-olek/issues
AppUpdatesURL=https://github.com/Damianejroo/bieznia-olek/releases
AppCopyright=© 2026 damnit.com.pl Damian Skrzypiec
VersionInfoVersion={#AppVersion}
DefaultDirName={localappdata}\Programs\Bieznia
DefaultGroupName=Bieżnia Gamingowa
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\dist
OutputBaseFilename=Bieznia-Setup-{#AppVersion}
SetupIconFile=bieznia.ico
UninstallDisplayIcon={app}\Bieznia.exe
UninstallDisplayName=Bieżnia Gamingowa
WizardStyle=modern
WizardSmallImageFile=instalator-55.bmp,instalator-64.bmp,instalator-83.bmp,instalator-110.bmp
Compression=lzma2/ultra64
SolidCompression=yes
CloseApplications=yes
RestartApplications=no
; jedyny ekran: krótki tekst i przycisk Instaluj (Inno zawsze pokazuje jeden ekran)
DisableWelcomePage=yes
DisableDirPage=yes
DisableProgramGroupPage=yes
DisableFinishedPage=yes

[Languages]
Name: "polish"; MessagesFile: "compiler:Languages\Polish.isl"

[Messages]
WizardReady=Instalacja Bieżni Gamingowej
ReadyLabel1=Bieżnia Gamingowa {#AppVersion} by damnit.com.pl zostanie zainstalowana na tym komputerze.
ReadyLabel2b=Kliknij Instaluj. Nie są potrzebne uprawnienia administratora, a po instalacji aplikacja otworzy się sama.

[Files]
Source: "..\build\dist\Bieznia\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[InstallDelete]
; przy aktualizacji usuń stare biblioteki, żeby nie mieszały się z nowymi
Type: filesandordirs; Name: "{app}\_internal"
; skróty ze starszych wersji (nazwa "Bieżnia")
Type: files; Name: "{autoprograms}\Bieżnia.lnk"
Type: files; Name: "{autodesktop}\Bieżnia.lnk"

[Icons]
Name: "{autoprograms}\Bieżnia Gamingowa"; Filename: "{app}\Bieznia.exe"
; skrót na pulpicie przy pierwszej instalacji albo w miejsce starego - aktualizacja nie odtworzy
; skrótu, który użytkownik sam usunął
Name: "{autodesktop}\Bieżnia Gamingowa"; Filename: "{app}\Bieznia.exe"; Check: WantDesktopIcon

[Run]
; po instalacji (także cichej, z przycisku Aktualizuj) od razu uruchom aplikację
Filename: "{app}\Bieznia.exe"; Flags: nowait

[Code]
var
  DesktopIcon: Boolean;

function InitializeSetup(): Boolean;
begin
  { pierwsza instalacja albo był skrót z poprzedniej wersji (stara lub nowa nazwa) }
  DesktopIcon := (not RegKeyExists(HKEY_CURRENT_USER, '{#UninstallKey}'))
    or FileExists(ExpandConstant('{autodesktop}\Bieżnia.lnk'))
    or FileExists(ExpandConstant('{autodesktop}\Bieżnia Gamingowa.lnk'));
  Result := True;
end;

function WantDesktopIcon(): Boolean;
begin
  Result := DesktopIcon;
end;
