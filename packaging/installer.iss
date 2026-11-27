; WalkLevel installer, 0.6.0 and newer (Tauri 2 + Rust). Build: packaging/build.ps1
; File encoding: UTF-8 without a BOM (Inno Setup 6.3 and newer read it as UTF-8).
;
; Interactive: the installer's own language (English by default, Polish), then one page with the
; app languages to install and the Install button; afterwards the app opens by itself. Silent
; (/VERYSILENT from the app's Update button): the previous installer language and language
; selection, no pages, then the app opens.
; Per-user installation (no administrator). Data in %LOCALAPPDATA%\Bieznia stays on update and
; uninstall. The install folder and AppId are the same as in 0.5.x, so the Update button of an
; older version replaces the app with this installer. Since 0.6.4 the program is WalkLevel.exe
; (Bieznia.exe before) and the installer WalkLevel-Setup-X.Y.Z.exe.
; App languages (docs/i18n.md): English is built into the exe; every other pack is a component
; from target-installer\languages.iss, which build.ps1 generates from locales\languages.json.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#define UninstallKey "Software\Microsoft\Windows\CurrentVersion\Uninstall\{6F4C2D8E-3B1A-4E5F-9C7D-2A8B1E0F4D63}_is1"
#define LanguagesInclude AddBackslash(SourcePath) + "..\target-installer\languages.iss"

[Setup]
AppId={{6F4C2D8E-3B1A-4E5F-9C7D-2A8B1E0F4D63}
AppName=WalkLevel
AppVersion={#AppVersion}
AppVerName=WalkLevel {#AppVersion} by damnit.com.pl
AppPublisher=damnit.com.pl Damian Skrzypiec
AppPublisherURL=https://damnit.com.pl
AppSupportURL=https://github.com/damnit-damian/WalkLevel-by-damnit.com.pl/issues
AppUpdatesURL=https://github.com/damnit-damian/WalkLevel-by-damnit.com.pl/releases
AppCopyright=© 2026 damnit.com.pl Damian Skrzypiec
VersionInfoVersion={#AppVersion}
DefaultDirName={localappdata}\Programs\Bieznia
DefaultGroupName=WalkLevel
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\dist-installer
OutputBaseFilename=WalkLevel-Setup-{#AppVersion}
SetupIconFile=icon.ico
UninstallDisplayIcon={app}\walklevel.ico
UninstallDisplayName=WalkLevel
WizardStyle=modern
WizardSmallImageFile=installer-55.bmp,installer-64.bmp,installer-83.bmp,installer-110.bmp
Compression=lzma2/max
SolidCompression=yes
CloseApplications=yes
RestartApplications=no
; language dialog first, English preselected on a new installation; on an update Inno preselects
; the language of the previous installation (0.6.3 and older: Polish), and a silent update uses it
ShowLanguageDialog=yes
LanguageDetectionMethod=none
UsePreviousLanguage=yes
; the language selection (components) of the previous installation; a silent update keeps it
UsePreviousSetupType=yes
; the only page: the app languages with the Install button
DisableWelcomePage=yes
DisableDirPage=yes
DisableProgramGroupPage=yes
DisableReadyPage=yes
DisableFinishedPage=yes
ShowComponentSizes=no

[Languages]
; English first: the default. The names are stored with the installation ("polish" in 0.6.3)
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "polish"; MessagesFile: "compiler:Languages\Polish.isl"

[Messages]
english.WizardSelectComponents=WalkLevel Setup
english.SelectComponentsDesc=WalkLevel {#AppVersion} will be installed on this computer.
english.SelectComponentsLabel2=Choose the languages of WalkLevel. English is built in. You can change the language later in Settings > Language.%n%nClick Install. Administrator rights are not needed, and WalkLevel opens by itself after installation.
polish.WizardSelectComponents=Instalacja WalkLevel
polish.SelectComponentsDesc=WalkLevel {#AppVersion} zostanie zainstalowany na tym komputerze.
polish.SelectComponentsLabel2=Wybierz języki WalkLevel. Angielski jest wbudowany. Język zmienisz później w Ustawienia > Język.%n%nKliknij Instaluj. Uprawnienia administratora nie są potrzebne, a po instalacji WalkLevel otworzy się sam.

[CustomMessages]
; BCP 47 tag of the app language that matches the installer language (docs/i18n.md)
english.AppLanguage=en
polish.AppLanguage=pl
english.LanguageEnglish=English (built in)
polish.LanguageEnglish=English (wbudowany)
english.LanguagesType=Selected languages
polish.LanguagesType=Wybrane języki
english.WebView2Missing=WalkLevel needs Microsoft Edge WebView2 for its window, and it is missing. Install it from https://developer.microsoft.com/microsoft-edge/webview2/ and start WalkLevel again.
polish.WebView2Missing=Brakuje składnika Microsoft Edge WebView2, z którego korzysta okno WalkLevel. Zainstaluj go ze strony https://developer.microsoft.com/microsoft-edge/webview2/ i uruchom WalkLevel ponownie.

[Types]
; one type: no type list on the page, only the languages
Name: "languages"; Description: "{cm:LanguagesType}"; Flags: iscustom

[Components]
; built into the exe: always installed, no files
Name: "en"; Description: "{cm:LanguageEnglish}"; Types: languages; Flags: fixed

#if FileExists(LanguagesInclude)
  #include LanguagesInclude
#else
  #error Language packs missing: build with packaging\build.ps1 (it writes target-installer\languages.iss)
#endif

[Files]
; one exe (interface built in); afterwards the start language for an interactive installation
Source: "..\target-release\release\walklevel.exe"; DestDir: "{app}"; DestName: "WalkLevel.exe"; Flags: ignoreversion; AfterInstall: WriteStartLanguage
; configuration next to the exe (server and report addresses, as in 0.5.x)
Source: "..\config.toml"; DestDir: "{app}"; Flags: ignoreversion
; shortcut icon as a separate file with a new name: Windows caches icons by file path, so a
; shortcut to the old exe could show the old logo after an update
Source: "icon.ico"; DestDir: "{app}"; DestName: "walklevel.ico"; Flags: ignoreversion

[InstallDelete]
; update from 0.5.x: remove the Python version (PyInstaller libraries and files next to the exe)
Type: filesandordirs; Name: "{app}\_internal"
Type: files; Name: "{app}\wersja.txt"
; program up to 0.6.3 (WalkLevel.exe since 0.6.4); CloseApplications closes it before removal
Type: files; Name: "{app}\Bieznia.exe"
; shortcuts of older versions (names "Bieżnia" and "Bieżnia Gamingowa")
Type: files; Name: "{autoprograms}\Bieżnia.lnk"
Type: files; Name: "{autodesktop}\Bieżnia.lnk"
Type: files; Name: "{autoprograms}\Bieżnia Gamingowa.lnk"
Type: files; Name: "{autodesktop}\Bieżnia Gamingowa.lnk"

[UninstallDelete]
; in case the old exe was still locked during an update from 0.6.3 and could not be removed
Type: files; Name: "{app}\Bieznia.exe"
; the start language request written by the installer; this section runs last, so the folders
; it leaves empty go here as well
Type: files; Name: "{app}\locales\installer.json"
Type: dirifempty; Name: "{app}\locales"
Type: dirifempty; Name: "{app}"

[Icons]
Name: "{autoprograms}\WalkLevel"; Filename: "{app}\WalkLevel.exe"; IconFilename: "{app}\walklevel.ico"
; desktop shortcut on the first installation or in place of an old one: an update does not bring
; back a shortcut the user removed
Name: "{autodesktop}\WalkLevel"; Filename: "{app}\WalkLevel.exe"; IconFilename: "{app}\walklevel.ico"; Check: WantDesktopIcon
; a taskbar pin from 0.6.3 points to Bieznia.exe: repin it only if it is there
Name: "{userappdata}\Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar\WalkLevel"; Filename: "{app}\WalkLevel.exe"; IconFilename: "{app}\walklevel.ico"; Check: HasTaskbarPin

[Run]
; new icon and name: refresh the Windows icon cache (shortcuts, taskbar, Start menu)
Filename: "{sys}\ie4uinit.exe"; Parameters: "-show"; Flags: runhidden nowait skipifdoesntexist
; after installation (also silent, from the Update button) start the app right away
Filename: "{app}\WalkLevel.exe"; Flags: nowait

[Code]
var
  DesktopIcon: Boolean;

function InitializeSetup(): Boolean;
begin
  { first installation, or there was a shortcut of a previous version (any name) }
  DesktopIcon := (not RegKeyExists(HKEY_CURRENT_USER, '{#UninstallKey}'))
    or FileExists(ExpandConstant('{autodesktop}\Bieżnia.lnk'))
    or FileExists(ExpandConstant('{autodesktop}\Bieżnia Gamingowa.lnk'))
    or FileExists(ExpandConstant('{autodesktop}\WalkLevel.lnk'));
  Result := True;
end;

{ Inno component of an app language (pt-BR -> pt_BR), '' when this installer has no such pack }
function PackComponent(const Code: String): String;
begin
  Result := Code;
  StringChangeEx(Result, '-', '_', True);
  if Pos(',' + Lowercase(Result) + ',', Lowercase('{#LanguagePacks}')) = 0 then
    Result := '';
end;

{ App language of the installer language when its pack is selected, otherwise English }
function StartLanguage(): String;
var
  Component: String;
begin
  Result := ExpandConstant('{cm:AppLanguage}');
  Component := PackComponent(Result);
  if (Component = '') or not WizardIsComponentSelected(Component) then
    Result := 'en';
end;

{ Preselected packs. Inno restores the selection of an earlier installation itself
  (UsePreviousSetupType), and a silent update keeps it as it is. }
procedure InitializeWizard();
var
  Previous, Component: String;
  HasPrevious: Boolean;
begin
  HasPrevious := RegQueryStringValue(HKEY_CURRENT_USER, '{#UninstallKey}',
    'Inno Setup: Selected Components', Previous) and (Previous <> '');
  { update from 0.6.3 or older (no selection stored): those versions were Polish only }
  if (not HasPrevious) and RegKeyExists(HKEY_CURRENT_USER, '{#UninstallKey}')
    and (PackComponent('pl') <> '') then
    WizardSelectComponents(PackComponent('pl'));
  { the pack of the installer language, unless a silent update keeps the previous selection }
  Component := PackComponent(ExpandConstant('{cm:AppLanguage}'));
  if (Component <> '') and ((not WizardSilent()) or (not HasPrevious)) then
    WizardSelectComponents(Component);
end;

procedure CurPageChanged(CurPageID: Integer);
begin
  { the Ready page is off, so the language page starts the installation }
  if CurPageID = wpSelectComponents then
    WizardForm.NextButton.Caption := SetupMessage(msgButtonInstall);
end;

{ Start language for the app (docs/i18n.md): only after an interactive installation, so a silent
  update from the Update button keeps the language chosen in Settings }
procedure WriteStartLanguage();
var
  Dir: String;
begin
  if WizardSilent() then
    Exit;
  Dir := ExpandConstant('{app}\locales');
  ForceDirectories(Dir);
  if not SaveStringToFile(Dir + '\installer.json',
    '{"language":"' + StartLanguage() + '","id":"' + GetDateTimeString('yyyymmddhhnnsszzz', #0, #0) + '"}',
    False) then
    Log('Could not write ' + Dir + '\installer.json');
end;

{ WebView2 (the window engine): part of Windows 11 and current Windows 10; without it the window
  does not open }
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
    SuppressibleMsgBox(ExpandConstant('{cm:WebView2Missing}'), mbError, MB_OK, IDOK);
end;

function WantDesktopIcon(): Boolean;
begin
  Result := DesktopIcon;
end;

function HasTaskbarPin(): Boolean;
begin
  Result := FileExists(ExpandConstant('{userappdata}\Microsoft\Internet Explorer\Quick Launch\User Pinned\TaskBar\WalkLevel.lnk'));
end;
