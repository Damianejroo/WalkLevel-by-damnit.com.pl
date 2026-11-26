; Instalator WalkLevel 0.6.0+ (Tauri 2 + Rust). Budowanie: walklevel/packaging/zbuduj.ps1
; Jedno kliknięcie: bez pytań i bez umowy, pasek postępu, potem aplikacja sama się otwiera.
; Instalacja na koncie użytkownika (bez administratora). Dane w %LOCALAPPDATA%\Bieznia
; zostają przy aktualizacji i odinstalowaniu. Techniczne nazwy (Bieznia.exe, folder, AppId,
; plik Bieznia-Setup-*.exe) są te same co w 0.5.x - przycisk Aktualizuj w 0.5.x podmienia
; aplikację tym instalatorem bez zmian po stronie użytkownika.

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
OutputDir=..\dist-instalator
OutputBaseFilename=Bieznia-Setup-{#AppVersion}
SetupIconFile=..\..\packaging\bieznia.ico
UninstallDisplayIcon={app}\walklevel.ico
UninstallDisplayName=WalkLevel
WizardStyle=modern
WizardSmallImageFile=..\..\packaging\instalator-55.bmp,..\..\packaging\instalator-64.bmp,..\..\packaging\instalator-83.bmp,..\..\packaging\instalator-110.bmp
Compression=lzma2/max
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
; jeden plik exe (interfejs wbudowany); nazwa Bieznia.exe jak w 0.5.x - skróty i aktualizacja
; z 0.5.x wskazują ten plik
Source: "..\target-release\release\walklevel.exe"; DestDir: "{app}"; DestName: "Bieznia.exe"; Flags: ignoreversion
; konfiguracja obok exe (adresy serwera i zgłoszeń, jak w 0.5.x)
Source: "..\..\config.toml"; DestDir: "{app}"; Flags: ignoreversion
; ikona skrótów jako osobny plik o nowej nazwie: Windows trzyma ikonę w pamięci podręcznej po
; ścieżce pliku, więc skrót wskazujący Bieznia.exe po aktualizacji mógł pokazywać stare logo
Source: "..\..\packaging\bieznia.ico"; DestDir: "{app}"; DestName: "walklevel.ico"; Flags: ignoreversion

[InstallDelete]
; przy aktualizacji z 0.5.x usuń wersję w Pythonie (biblioteki PyInstallera i pliki obok exe)
Type: filesandordirs; Name: "{app}\_internal"
Type: files; Name: "{app}\wersja.txt"
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

{ WebView2 (silnik okna): jest w Windows 11 i aktualnym Windows 10; bez niego okno się nie otworzy }
function HasWebView2(): Boolean;
var
  Version: String;
begin
  Result :=
    (RegQueryStringValue(HKLM, 'SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}', 'pv', Version)
      and (Version <> '') and (Version <> '0.0.0.0'))
    or (RegQueryStringValue(HKCU, 'Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}', 'pv', Version)
      and (Version <> '') and (Version <> '0.0.0.0'));
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if (CurStep = ssPostInstall) and (not HasWebView2()) then
    SuppressibleMsgBox('Brakuje składnika Microsoft Edge WebView2, z którego korzysta okno WalkLevel. ' +
      'Zainstaluj go ze strony https://developer.microsoft.com/microsoft-edge/webview2/ ' +
      'i uruchom WalkLevel ponownie.', mbError, MB_OK, IDOK);
end;

function WantDesktopIcon(): Boolean;
begin
  Result := DesktopIcon;
end;
