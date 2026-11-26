; Instalator WalkLevel (Inno Setup 6/7). Budowanie: uv run python packaging/zbuduj.py
; Jedno kliknięcie: bez pytań i bez umowy, pasek postępu, potem aplikacja sama się otwiera.
; Instalacja na koncie użytkownika (bez administratora). Dane w %LOCALAPPDATA%\Bieznia
; zostają przy aktualizacji i odinstalowaniu. Techniczne nazwy (Bieznia.exe, folder, AppId,
; plik Bieznia-Setup-*.exe) zostają po zmianie nazwy, żeby aktualizacja przeszła bez zmian.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#define UninstallKey "Software\Microsoft\Windows\CurrentVersion\Uninstall\{6F4C2D8E-3B1A-4E5F-9C7D-2A8B1E0F4D63}_is1"

[Setup]
AppId={{6F4C2D8E-3B1A-4E5F-9C7D-2A8B1E0F4D63}
AppName=WalkLevel
AppVersion={#AppVersion}
AppVerName=WalkLevel {#AppVersion} by damnit.com.pl
AppPublisher=damnit.com.pl Damian Skrzypiec
AppPublisherURL=https://damnit.com.pl
AppSupportURL=https://github.com/Damianejroo/WalkingPad-X21-PC-Gaming-by-damnit.com.pl/issues
AppUpdatesURL=https://github.com/Damianejroo/WalkingPad-X21-PC-Gaming-by-damnit.com.pl/releases
AppCopyright=© 2026 damnit.com.pl Damian Skrzypiec
VersionInfoVersion={#AppVersion}
DefaultDirName={localappdata}\Programs\Bieznia
DefaultGroupName=WalkLevel
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\dist
OutputBaseFilename=Bieznia-Setup-{#AppVersion}
SetupIconFile=bieznia.ico
UninstallDisplayIcon={app}\walklevel.ico
UninstallDisplayName=WalkLevel
WizardStyle=modern
WizardSmallImageFile=instalator-55.bmp,instalator-64.bmp,instalator-83.bmp,instalator-110.bmp
; szybka kompresja: ok. 3 s zamiast 14 s przy +4 MB - liczy się czas szybkich poprawek
Compression=lzma2/fast
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
WizardReady=Instalacja WalkLevel
ReadyLabel1=WalkLevel {#AppVersion} by damnit.com.pl zostanie zainstalowany na tym komputerze.
ReadyLabel2b=Kliknij Instaluj. Nie są potrzebne uprawnienia administratora, a po instalacji aplikacja otworzy się sama.

[Files]
Source: "..\build\dist\Bieznia\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
; ikona skrótów jako osobny plik o nowej nazwie: Windows trzyma ikonę w pamięci podręcznej po ścieżce
; pliku, więc skrót wskazujący dalej Bieznia.exe po aktualizacji pokazywał stare logo
Source: "bieznia.ico"; DestDir: "{app}"; DestName: "walklevel.ico"; Flags: ignoreversion

[InstallDelete]
; przy aktualizacji usuń stare biblioteki, żeby nie mieszały się z nowymi
Type: filesandordirs; Name: "{app}\_internal"
; skróty ze starszych wersji (nazwy "Bieżnia" i "Bieżnia Gamingowa")
Type: files; Name: "{autoprograms}\Bieżnia.lnk"
Type: files; Name: "{autodesktop}\Bieżnia.lnk"
Type: files; Name: "{autoprograms}\Bieżnia Gamingowa.lnk"
Type: files; Name: "{autodesktop}\Bieżnia Gamingowa.lnk"

[Icons]
Name: "{autoprograms}\WalkLevel"; Filename: "{app}\Bieznia.exe"; IconFilename: "{app}\walklevel.ico"
; skrót na pulpicie przy pierwszej instalacji albo w miejsce starego - aktualizacja nie odtworzy
; skrótu, który użytkownik sam usunął
Name: "{autodesktop}\WalkLevel"; Filename: "{app}\Bieznia.exe"; IconFilename: "{app}\walklevel.ico"; Check: WantDesktopIcon

[Run]
; nowa ikona i nazwa: odśwież pamięć ikon Windows (skróty, pasek zadań, menu Start)
Filename: "{sys}\ie4uinit.exe"; Parameters: "-show"; Flags: runhidden nowait skipifdoesntexist
; po instalacji (także cichej, z przycisku Aktualizuj) od razu uruchom aplikację
Filename: "{app}\Bieznia.exe"; Flags: nowait

[Code]
var
  DesktopIcon: Boolean;

function InitializeSetup(): Boolean;
begin
  { pierwsza instalacja albo był skrót z poprzedniej wersji (którakolwiek nazwa) }
  DesktopIcon := (not RegKeyExists(HKEY_CURRENT_USER, '{#UninstallKey}'))
    or FileExists(ExpandConstant('{autodesktop}\Bieżnia.lnk'))
    or FileExists(ExpandConstant('{autodesktop}\Bieżnia Gamingowa.lnk'))
    or FileExists(ExpandConstant('{autodesktop}\WalkLevel.lnk'));
  Result := True;
end;

function WantDesktopIcon(): Boolean;
begin
  Result := DesktopIcon;
end;
