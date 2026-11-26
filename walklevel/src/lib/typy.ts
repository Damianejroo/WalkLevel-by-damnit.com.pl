// Typy kontraktu rdzeń (Rust) <-> interfejs - 1:1 z docs/kontrakt.md, rozdział 4.
// Zmiana kontraktu idzie najpierw do docs/kontrakt.md, potem tutaj (i do crates/core/src/kontrakt.rs).

// ============================================================================
// Podstawowe wyliczenia (wartości jak w 0.5 i w plikach)
// ============================================================================

export type Direction = 'keyboard' | 'gamepad' | 'treadmill' | 'training'; // Klawisze, Pad, Chodzenie, Trening
export type OutputKind = 'dry' | 'keys';           // podgląd / naciska klawisze w grze
export type ThemeChoice = 'system' | 'light' | 'dark';
export type PadIdle = 'stop' | 'slow' | 'zero';    // gałka puszczona: stop / prędkość skoku 1 / 0 km/h (test)
export type KeyIdle = 'slow' | 'stop';             // Klawisze, bez klawisza: tempo „bez klawisza” / taśma stoi
export type KeyRole = 'walk' | 'run' | 'toggle' | 'faster' | 'slower'; // faster/slower: symulator i poziom Treningu
export type ColorName =                            // nazwy z bieznia/paleta.py (zapisywane w plikach)
  | 'Czerwień' | 'Koral' | 'Pomarańcz' | 'Morela' | 'Banan' | 'Limonka' | 'Zieleń' | 'Szmaragd'
  | 'Mięta' | 'Morski' | 'Niebo' | 'Błękit' | 'Chaber' | 'Indygo' | 'Fiolet' | 'Lawenda'
  | 'Orchidea' | 'Fuksja' | 'Róż' | 'Malina';
export type PadButton =                            // nazwy wspólne dla DualSense/DualShock/Xbox (pad.py)
  | 'south' | 'east' | 'west' | 'north' | 'l1' | 'r1' | 'l2' | 'r2' | 'select' | 'start'
  | 'l3' | 'r3' | 'home' | 'touchpad' | 'up' | 'down' | 'left' | 'right' | `b${number}`;
export type Link =                                 // bieznia/ble/worker.py Link
  | 'idle' | 'searching' | 'connecting' | 'connected' | 'streaming' | 'retrying'
  | 'bluetoothOff' | 'unsupported';
export type DriverId = 'kingsmith' | 'ftms' | 'symulator' | '';
export type SessionSourceKind = 'bieznia' | 'symulator';
export type SessionMode = 'gra' | 'trening';
export type SessionVisibility = 'prywatna' | 'znajomi' | 'publiczna';
export type ProfileVisibility = 'ukryty' | 'znajomi' | 'publiczny';
export type GamesVisibility = 'ja' | 'znajomi' | 'wszyscy';
export type RankingScope = 'tydzien' | 'miesiac' | 'razem' | 'poziom';
export type StatsScope = 'tydzien' | 'miesiac' | 'razem';
export type Rarity = 'pospolite' | 'rzadkie' | 'epickie' | 'heroiczne' | 'legendarne';
export type AchievementKind =
  | 'dystans' | 'sesja_km' | 'dzien_kroki' | 'seria' | 'gra_pierwsza' | 'trening'
  | 'gry_km' | 'gra_km' | 'start_miedzy' | 'znajomi' | 'polecenia';
export type GaugeMode = 'idle' | 'walk' | 'run' | 'offline';
export type Tone = 'accent' | 'run' | 'danger' | 'muted'; // kolor kropki statusu (t.accent / t.run / t.danger / t.text_2)
export type FolderKind = 'dane' | 'logi' | 'zgloszenia' | 'nagrania';
export type LinkKind = 'strona' | 'prywatnosc' | 'kod' | 'regulamin';
export type HistoryScope = 'me' | 'all';
export type HistoryRange = 'day' | 'week' | 'month' | 'all';
export type ExportRange = 'all' | 'year' | 'month';

// ============================================================================
// Wspólne kawałki
// ============================================================================

/** Klawisz jak w osoby.json (klawisze.py Key). `label` z modyfikatorami: „Ctrl+Shift+F8”. */
export interface KeyBinding {
  scan: number;        // kod skanowania (1..255)
  extended: boolean;
  name: string;        // „W”, „Shift”, „F8”
  vk: number;          // kod wirtualny (do skrótu globalnego)
  mods: Array<'ctrl' | 'alt' | 'shift' | 'win'>;
  label: string;       // tylko do odczytu (rdzeń liczy), nie zapisywane
}

/** To, co UI przekazuje po wciśnięciu klawisza w trybie przypisywania (KeyboardEvent). */
export interface KeyCapture {
  code: string;        // KeyboardEvent.code, np. 'KeyW', 'ShiftLeft', 'F8', 'Numpad1'
  key: string;         // KeyboardEvent.key - zapasowa nazwa
  ctrl: boolean; alt: boolean; shift: boolean; meta: boolean;
  location: number;    // KeyboardEvent.location (rozróżnia lewy/prawy Shift, numpad)
}

/** Skok pada: [wychylenie w %, prędkość km/h]. Pierwszy zawsze od 0 (gałka puszczona). */
export type PadStep = [at: number, kmh: number];

/** Ikona profilu do narysowania (profile_pixmap): obrazek z gry albo znak Segoe Fluent na kolorze. */
export interface IconRef {
  color: ColorName;
  glyph: string | null;   // pojedynczy znak, np. ''; rysować fontem 'Segoe Fluent Icons', 'Segoe MDL2 Assets'
  url: string | null;     // http://ikona.localhost/<plik>?v=<mtime> (protokół rdzenia, rozdz. 7.4)
}

export interface Totals {
  steps: number;
  distanceKm: number;
  activeS: number;
  avgKmh: number;         // liczone przez rdzeń (0 przy activeS == 0)
}

/** Błąd komendy (odrzucona obietnica invoke). */
export interface CmdError {
  code: ErrorCode;
  message: string;                    // po polsku, gotowe do pokazania
  fields?: Record<string, string>;    // błędy pól z serwera (np. {email: '…'}) - podpowiedź przy polu
}
export type ErrorCode =
  | 'invalid'        // złe dane (walidacja) - message przy polu
  | 'notFound'
  | 'busy'           // to samo zadanie już trwa (account_job, aktualizacja, zgłoszenie)
  | 'offline'        // brak połączenia
  | 'loggedOut'      // 401 - rdzeń już wylogował osobę i wysłał toast
  | 'updateRequired' // 426 - serwer wymaga nowszej aplikacji
  | 'server'         // inny błąd serwera (message z serwera)
  | 'io'             // dysk
  | 'device'         // bieżnia / Bluetooth
  | 'key'            // klawisz nie do użycia (klawisze.problem)
  | 'cancelled'      // użytkownik zamknął okno wyboru pliku
  | 'unavailable';   // funkcja niedostępna (np. aktualizacje w wersji z kodu)

// ============================================================================
// Zdarzenia
// ============================================================================

export interface SliceMap {
  app: AppInfo;
  settings: Settings;
  people: PeopleView;
  device: DeviceView;
  control: ControlView;
  levels: LevelsView | null;   // null: brak osoby albo historii
  history: HistoryStamp;
  account: AccountView;        // konto WYBRANEJ osoby
  online: OnlineView;          // dane z serwera dla wybranej osoby (pamięć podręczna)
  update: UpdateView;
  report: ReportView;
  models: ModelsView;
  bubble: BubbleView;          // dla okna "przycisk"
}
export type SliceName = keyof SliceMap;
export interface SliceEvent<K extends SliceName = SliceName> {
  rev: number;
  slice: K;
  value: SliceMap[K];
}

export interface AppSnapshot extends SliceMap {
  rev: number;
  statics: StaticData;         // tylko w snapshot - nie zmienia się w trakcie działania
  logTail: string[];           // ostatnie 500 linii Dziennika
}

export interface Toast { text: string; kind: 'info' | 'error' }

export type Route =
  | 'main' | 'devices' | 'settings' | 'log' | 'report' | 'profile' | 'profileNew' | 'person'
  | 'wizard' | 'history' | 'session' | 'export' | 'import' | 'models' | 'account' | 'online'
  | 'onlinePerson' | 'achievements' | 'trail';
export interface NavRequest {
  to: Route;
  /** 'scan' - wyniki skanowania; 'firstRun' - brak osoby; 'signedIn' - po zalogowaniu;
   *  'personOpened' - otwarto profil online; 'settings' - po usunięciu/wylogowaniu konta */
  reason: 'scan' | 'firstRun' | 'signedIn' | 'personOpened' | 'settings';
  /** dla 'account': tryb formularza */
  accountMode?: AccountFormMode;
}

/** Telemetria (wl:telemetria) - wszystko, co zmienia się co klatkę. */
export interface Telemetry {
  tMs: number;                 // monotoniczny czas rdzenia w ms
  speedKmh: number;            // wygładzona prędkość taśmy (Control.speed)
  live: boolean;               // świeże dane z bieżni
  enabled: boolean;            // sterowanie włączone
  mode: GaugeMode;             // kolor wskaźnika / przycisku
  active: number;              // podświetlony żeton: strefa (0-2), skok pada, poziom treningu; -1 brak
  caption: string;             // podpis pod wskaźnikiem (rozdz. 9.2)
  steps: number | null;        // licznik kroków bieżni
  cadence: number | null;      // kroki/min
  distanceKm: number;          // narastająco od startu aplikacji
  activeS: number;             // czas ruchu narastająco
  beltTargetKmh: number | null;// ostatnia prędkość wysłana na taśmę (null: nie sterujemy)
  pad: PadTelemetry | null;    // null, gdy pada nie czytamy
}
export interface PadTelemetry {
  name: string | null;         // null: nie widać pada
  deflection: number;          // 0..1
  step: number;                // indeks skoku (0 = puszczona)
  statusText: string;          // pad_status_text() (rozdz. 9.2)
}

// ============================================================================
// Dane stałe (tylko w snapshot)
// ============================================================================

export interface StaticData {
  appName: 'WalkLevel';
  author: string;              // 'damnit.com.pl Damian Skrzypiec'
  homepage: string;            // 'https://damnit.com.pl'
  contact: string;             // 'kontakt@damnit.com.pl'
  palette: PaletteColor[];     // 20 kolorów w kolejności z paleta.py
  defaultColor: ColorName;     // 'Mięta'
  runAltColor: ColorName;      // 'Malina' (kolor biegu, gdy profil ma kolor zbliżony do biegu)
  glyphs: Array<{ name: string; code: string }>; // ICON_GLYPHS (komponenty.py), 12 pozycji
  defaultGlyph: string;        // '' (postać idąca, profil Domyślny)
  modeNames: Record<Direction, string>; // Klawisze, Pad, Chodzenie, Trening
  maxSpeed: number;            // mapping.max_speed z config.toml (12.0) - górna krawędź wskaźnika w trybie Chodzenie
  hysteresis: number;          // 0.6 - run_off = run_on - 0.6
  thresholdGap: number;        // 0.8 - najmniejszy odstęp progów chodu i biegu (HYSTERESIS + 0.2)
  padPresets: Record<2 | 3 | 4 | 5, PadStep[]>;
  padMinGap: number;           // 5 (punkty procentowe między skokami)
  trainingRange: [number, number];   // [0.5, 20]
  padSpeedRange: [number, number];   // [0.5, 12]
  reportPeriods: Array<{ seconds: number; label: string }>; // 1 h, 6 h (domyślny), 24 h, 7 dni
  reportDefaultS: number;      // 21600
  reportMinChars: number;      // 5
  account: AccountRules;
  definitions: LevelDefinitions;
  driverNames: Record<string, string>;  // {kingsmith: 'KingSmith', ftms: 'Bluetooth FTMS'}
}
export interface PaletteColor {
  name: ColorName;
  hue: number;
  dark: string;     // odcień tekstu/akcentu w ciemnym motywie (i „swatch” - kółka, awatary, wykresy)
  light: string;    // odcień tekstu/akcentu w jasnym motywie
}
export interface AccountRules {
  nameMin: 3; nameMax: 24;
  nameRe: string;              // '^[A-Za-z0-9 _.\\-ĄĆĘŁŃÓŚŹŻąćęłńóśźż]+$'
  reservedNames: string[];     // admin, administrator, moderator, damnit, bieznia, bieżnia
  emailRe: string;             // '^[^@\\s]{1,64}@[^@\\s]{1,190}\\.[^@\\s]{2,24}$'
  passwordMin: 10;
  codeMin: 4;                  // kod zaproszenia
  gameNameMax: 40;
}

// --- poziomy (bieznia/poziomy/definicje.json, wersja 1) ---
export interface LevelDefinitions {
  version: number;
  xpPerKm: number;                         // 100
  rarityXp: Record<Rarity, number>;        // 20 / 100 / 500 / 1200 / 2500
  rarities: Rarity[];                      // od najczęstszej
  levelsXp: number[];                      // 100 pozycji: XP potrzebne do poziomu n (indeks n-1)
  ranks: Rank[];                           // 39 rang
  achievements: Achievement[];             // 61 osiągnięć, kolejność z pliku
  rules: { restDaysPer7: number; minKmPerStreakDay: number };
}
export interface Rank { level: number; name: string; about: string }
export interface Achievement {
  id: string;
  kind: AchievementKind;
  rarity: Rarity;
  mark: string;          // znak na odznace (rysowany w UI - odznaki.py)
  name: string;
  flavor: string;        // „klimat”
  condition: string;     // „warunek”
  km: number | null;     // próg szlaku (tylko kind === 'dystans')
  params: Record<string, number | string>; // pozostałe pola z pliku (km, dni, kroki, gier, od, do, ile, km_dziennie)
}

// ============================================================================
// Wycinek: app
// ============================================================================

export interface AppInfo {
  version: string;             // '0.6.0' (albo 'dev' z kodu)
  title: string;               // 'WalkLevel 0.6.0 by damnit.com.pl'
  installed: boolean;          // wersja zainstalowana (FROZEN) - aktualizacje działają
  dataDir: string;             // %LOCALAPPDATA%\Bieznia
  firstRun: boolean;           // brak dane/osoby.json -> konfigurator 'first'
  legacy02: boolean;           // znaleziono dane/ustawienia.json z 0.2 (konfigurator je przeniesie)
  updatedFrom: string | null;  // poprzednia wersja, gdy to pierwszy start po aktualizacji
  latestBackup: string | null; // nazwa folderu dane/kopie/przed-importem-* (przycisk „Przywróć…”)
  accountsEnabled: boolean;    // config [konto] adres niepusty i poprawny
  reportEndpoint: boolean;     // config [zgloszenia] adres niepusty -> „Wyślij zgłoszenie”, inaczej „Przygotuj e-mail z logami”
  windowFocused: boolean;      // okno główne ma fokus (do ponownego sprawdzenia e-mail - informacyjnie)
}

// ============================================================================
// Wycinek: settings - płaskie ustawienia AKTYWNEJ osoby i jej AKTYWNEGO profilu
// (dane.Store.flat: domyślne < wspólne < wygląd osoby < profil)
// ============================================================================

export interface Settings {
  // --- wspólne (osoby.json "wspolne") ---
  deviceAddress: string;
  deviceName: string;
  beltControl: boolean;        // start i stop taśmy razem ze sterowaniem
  quickStop: boolean;          // szybkie zatrzymanie (hamowanie = postać staje)
  toggleKey: KeyBinding;       // domyślnie F8
  fasterKey: KeyBinding;       // szybciej (symulator, poziom Treningu); bez zapisu config.toml [hotkeys] sim_faster, potem F7
  slowerKey: KeyBinding;       // wolniej; bez zapisu config.toml [hotkeys] sim_slower, potem F6
  togglePad: PadButton | '';   // przycisk pada działający jak F8
  simulator: boolean;
  record: boolean;             // nagrywanie przejść do nagrania/
  detailedLogs: boolean;       // szczegółowe logi Bluetooth
  // --- wygląd osoby (osoby.json osoba.wyglad) ---
  theme: ThemeChoice;
  largeText: boolean;          // +15% wszystkie napisy
  alwaysOnTop: boolean;
  floatingButton: boolean;
  showPersonSwitch: boolean;
  showProfileSwitch: boolean;
  achievementPopups: boolean;  // NOWE w zapisie (0.5 trzymało tylko w pamięci - rozdz. 10)
  achievementSound: boolean;   // NOWE w zapisie
  // --- profil gry (osoby.json profil.ustawienia) ---
  direction: Direction;
  output: OutputKind;
  walkKey: KeyBinding;         // domyślnie W
  runKey: KeyBinding;          // domyślnie Shift
  standBelow: number;          // km/h - poniżej stoisz (Chodzenie)
  runOn: number;               // km/h - od tej bieg
  runOff: number;              // = runOn - 0.6 (tylko do odczytu w UI)
  idleSpeed: number;           // Klawisze: bez klawisza
  walkSpeed: number;           // Klawisze: chód
  runSpeed: number;            // Klawisze: chód + bieg
  keyHoldS: number;            // Klawisze: sekundy trzymania, zanim taśma przyspieszy (0, 1, 2 albo 3; 0 - od razu; puszczenie od razu)
  keyIdle: KeyIdle;            // Klawisze: bez klawisza wolne tempo albo stop taśmy (domyślnie slow)
  padSteps: PadStep[];         // 2..5 skoków
  padIdle: PadIdle;
  trainingLevels: [number, number, number, number];
  trainingLevel: 0 | 1 | 2 | 3;
  // --- pochodne, tylko do odczytu ---
  profileColor: ColorName;     // kolor aktywnego profilu = akcent motywu
  personColor: ColorName;
}
/** Klucze, które UI może zmienić przez settings_set (pozostałe mają własne komendy). */
export type SettingKey =
  | 'beltControl' | 'quickStop' | 'togglePad' | 'simulator' | 'record' | 'detailedLogs'
  | 'theme' | 'largeText' | 'alwaysOnTop' | 'floatingButton' | 'showPersonSwitch'
  | 'showProfileSwitch' | 'achievementPopups' | 'achievementSound'
  | 'direction' | 'output' | 'idleSpeed' | 'walkSpeed' | 'runSpeed' | 'keyHoldS' | 'keyIdle' | 'padIdle';

// ============================================================================
// Wycinek: people - osoby i profile gier
// ============================================================================

export interface PeopleView {
  ready: boolean;              // false = nie ma jeszcze osoby (konfigurator 'first')
  activePersonId: string;
  activeProfileId: string;
  askAtStart: boolean;         // osoby.json "pytaj_przy_starcie" (0.5 zapisuje, nie używa - przenosimy bez zmian)
  people: Person[];
}
export interface Person {
  id: string;                  // 10 znaków hex
  name: string;
  color: ColorName;
  activeProfileId: string;
  profiles: GameProfile[];
  todaySteps: number;          // „dziś N kroków” w liście osób (history_today)
  hasAccount: boolean;         // osoba ma zalogowane konto online
  accountName: string | null;  // pseudonim konta (Ustawienia > Konto i osoby)
}
export interface GameProfile {
  id: string;                  // 10 znaków hex
  name: string;
  color: ColorName;
  iconColor: ColorName | null; // kolor dobrany z ikony gry (podpowiedź w palecie)
  icon: IconRef;
  exe: string | null;          // pełna ścieżka exe gry (tylko do pokazania nazwy pliku/folderu)
  online: boolean;             // „Pokazuj tę grę na koncie online”
  summary: ProfileSummary;     // do wiersza profilu w Ustawieniach (profile_summary)
}
/** Dane do „Klawisze · 1,5 / 2,5 km/h” itd. - ustawienia profilu scalone z domyślnymi. */
export interface ProfileSummary {
  direction: Direction;
  walkSpeed: number; runSpeed: number;     // keyboard
  padStepCount: number; padMaxKmh: number; // gamepad
  trainingMin: number; trainingMax: number;// training
  standBelow: number;                      // treadmill
}

/** Gra wskazana w oknie wyboru (profil.py GamePick). Ikona leży tymczasowo w dane/ikony/nowa-*.png. */
export interface GamePick {
  pickId: string;              // identyfikator wyboru - podawany przy tworzeniu/zmianie profilu
  name: string;                // nazwa gry (ikony.game_name)
  exe: string;                 // pełna ścieżka
  exeName: string;             // 'Game.exe'
  folderName: string;          // nazwa folderu nad exe
  icon: IconRef;               // url do tymczasowej ikony albo glyph null + url null
  iconColor: ColorName;        // kolor z ikony (albo domyślny)
}

// ============================================================================
// Wycinek: device - bieżnia i Bluetooth
// ============================================================================

export interface Capabilities {
  speedMin: number; speedMax: number; speedStep: number;
  startSpeed: number; startDelayS: number;
  acksCommands: boolean; pause: boolean; steps: boolean;
  incline: [number, number, number] | null;
  telemetryS: number; minCommandGapS: number;
  control: boolean;            // aplikacja może uruchamiać/zatrzymywać/zmieniać prędkość
}
export interface DeviceInfo {
  model: string;               // 'Urevo SpaceWalk 5L'
  driver: DriverId;
  profile: string;             // id profilu modelu
  name: string;                // nazwa Bluetooth
  firmware: string;
  simulated: boolean;
}
export interface FoundDevice {
  address: string;
  name: string;
  rssi: number;
  treadmill: boolean;
  model: string;               // model_name(name) - gdy równe name, UI pisze „Bieżnia”
}
export type StatusAction = 'scan' | 'cancel' | 'disconnect' | 'retry' | 'simulatorOff';
export interface LinkStatus {    // karta statusu na ekranie głównym (render_link)
  tone: Tone;
  busy: boolean;               // kropka pulsuje
  title: string;
  sub: string;
  action: StatusAction;        // co zrobi device_status_action
  actionLabel: string;         // 'Szukaj bieżni' | 'Anuluj' | 'Rozłącz' | 'Spróbuj ponownie' | 'Wyłącz symulator'
  primary: boolean;            // przycisk wyróżniony
}
export interface DeviceView {
  link: Link;
  linkText: string;
  status: LinkStatus;
  device: DeviceInfo | null;   // co powiedziała bieżnia (albo null przed połączeniem)
  caps: Capabilities;          // znane z profilu modelu przed połączeniem, potem od bieżni
  remembered: boolean;         // jest zapamiętana bieżnia (przycisk „Zapomnij” aktywny)
  label: string;               // render_device_label (2 linie, '\n')
  scanning: boolean;
  scanText: string;            // 'Szukam urządzeń Bluetooth (ok. 6 s)…' | 'Znalezione bieżnie: 1, inne urządzenia: 3' | błąd skanowania
  devices: FoundDevice[];      // bieżnie pierwsze; inne najwyżej 12
  bluetoothOff: boolean;       // pusta lista: instrukcja włączenia Bluetooth zamiast „Nie widzę bieżni”
  diagnostics: { running: boolean; text: string }; // 'Zbieram raport (ok. 30 s)…' / 'Raport: <ścieżka>'
}

// ============================================================================
// Wycinek: control - sterowanie
// ============================================================================

export interface ControlView {
  enabled: boolean;
  direction: Direction;
  setsSpeed: boolean;          // keyboard/gamepad/training: aplikacja steruje taśmą, nic nie naciska
  hint: string;                // render_output_hint (podpowiedź pod kafelkami trybów) - rozdz. 9.3
  subtitle: string;            // render_control_subtitle (obok klawisza) - rozdz. 9.4
  toggleLabel: string;         // 'F8' / 'Ctrl+Shift+F8'
  walkName: string; runName: string;
  padButtonLabel: string;      // '' gdy brak; „Panel dotykowy”, „A”…
  padCapturing: boolean;       // czekamy na przycisk pada (do 10 s)
  keyCapturing: KeyRole | null;// UI w trybie przypisywania klawisza (skrót globalny wstrzymany)
  beltCommands: boolean;       // belt_commands_possible()
}

// ============================================================================
// Wycinek: levels - poziom, ranga, osiągnięcia aktywnej osoby
// ============================================================================

export interface LevelsView {
  personId: string;
  km: number;                  // przebyte łącznie (wiarygodne sesje z bieżni)
  xp: number;
  level: number;
  maxLevel: number;            // 100
  levelFloorXp: number;
  levelCeilingXp: number;      // == floor na najwyższym poziomie
  levelFraction: number;       // 0..1
  kmToNextLevel: number;
  rank: Rank;
  nextRank: Rank | null;
  earnedCount: number;
  total: number;
  /** Wszystkie osiągnięcia w kolejności definicji. UI liczy: zdobyte (od najnowszego),
   *  najbliższe (niezdobyte z current > 0, największy fraction, 3 szt.), szlak (kind 'dystans'). */
  progress: AchievementProgress[];
}
export interface AchievementProgress {
  id: string;
  earnedAt: number | null;     // koniec sesji, w której padł próg (UTC s)
  current: number;
  target: number;
  fraction: number;            // 1 gdy zdobyte
}

// ============================================================================
// Wycinek: history - tylko znacznik zmian; dane historii przez history_query
// ============================================================================

export interface HistoryStamp {
  available: boolean;          // baza otwarta (false: zablokowana/uszkodzona - app działa bez historii)
  stamp: string;               // zmienia się, gdy zmieni się zawartość (history.version() + dzień + osoby/profile)
}

export interface HistoryQuery { scope: HistoryScope; range: HistoryRange }
export interface HistoryView {
  query: HistoryQuery;
  stamp: string;               // ten sam co HistoryStamp.stamp w chwili liczenia
  many: boolean;               // więcej niż jedna osoba (pokaż przełącznik Moja/Wszyscy)
  everyone: boolean;           // many && scope === 'all'
  totals: Totals;
  chart: HistoryChart;
  groupKey: 'profile' | 'person';
  groups: HistoryGroup[];      // od największej liczby kroków
  recent: SessionRow[];        // ostatnie 8
  online: boolean;             // jakakolwiek osoba ma konto (pokaż oko przy sesjach)
}
export interface HistoryChart {
  hourly: boolean;             // range 'day': 24 słupki godzinowe
  /** label: 'YYYY-MM-DD' (dziennie) albo 'HH' (godzinowo). parts: {nazwa profilu/osoby: kroki} */
  bars: Array<{ label: string; weekday: number | null; parts: Array<{ name: string; steps: number }> }>;
  colors: Record<string, string>; // nazwa -> kolor swatch (hex)
  legend: string[];            // najwyżej 6 nazw, posortowane
  empty: boolean;
}
export interface HistoryGroup {
  name: string;
  totals: Totals;
  share: number;               // 0..1 udziału w krokach
  color: string;               // swatch hex
  icon: IconRef | null;        // profil (groupKey 'profile')
  avatarColor: ColorName | null; // osoba (groupKey 'person')
}
export interface SessionRow {
  id: string;
  start: number; end: number;
  personId: string; personName: string;
  profileId: string; profileName: string;
  steps: number; distanceKm: number; activeS: number; avgKmh: number;
  source: SessionSourceKind;
  mode: SessionMode;
  icon: IconRef;
  online: OnlineMark | null;   // null: osoba bez konta albo sesja z symulatora
}
export interface OnlineMark {
  hidden: boolean;             // ukryta online (tylko u Ciebie)
  sent: boolean;               // serwer ma aktualną wersję
  rejected: string | null;     // powód odrzucenia przez serwer (czeka na ponowienie)
}
export interface SessionDetail extends SessionRow {
  cadence: number | null;      // kroki/min (gdy activeS >= 30)
  wholeS: number;              // end - start
  deviceModel: string;
  driver: string;
  firmware: string;
  appVersion: string;
  tzOffsetMin: number;
  linked: boolean;             // osoba sesji ma konto (usunięcie usuwa też z konta)
}

// ============================================================================
// Wycinek: account - konto online WYBRANEJ osoby
// ============================================================================

export type AccountFormMode = 'login' | 'register' | 'reset' | 'registered';
export type AccountAction =
  | 'login' | 'register' | 'reset' | 'logout' | 'delete' | 'password' | 'verify' | 'verified'
  | 'refresh' | 'upload' | 'profile' | 'terms' | 'export' | 'live' | 'invite' | 'answer'
  | 'removeFriend' | 'regCode' | 'person' | 'loadFriends' | 'loadStats' | 'loadInvitations'
  | 'loadRanking' | 'loadPeople' | 'deleteSession';
export interface AccountView {
  enabled: boolean;            // konto online w ogóle dostępne (adres serwera)
  personId: string;
  loggedIn: boolean;
  userId: string;
  name: string;                // pseudonim
  email: string;
  verified: boolean;
  upload: boolean;             // „Wysyłaj moje sesje”
  visibility: SessionVisibility;
  live: boolean;               // „Na żywo dla znajomych”
  profileVisibility: ProfileVisibility;
  gamesVisibility: GamesVisibility;
  terms: string;               // zaakceptowana wersja regulaminu
  termsCurrent: string;        // obowiązująca (z serwera)
  termsOutdated: boolean;      // karta „zaakceptuj nowy regulamin”
  gamesAllowed: boolean;       // nazwy gier mogą wychodzić
  remember: boolean;           // „Zapamiętaj logowanie”
  termsUrl: string;            // config [konto] regulamin
  busy: AccountAction[];       // trwające zadania tej osoby
  message: string;             // account_message - ostatni komunikat formularza/ustawień konta
  verifyPolling: boolean;      // po wysłaniu linku: rdzeń sam sprawdza, czy kliknięty
  pendingUploads: number;      // sesje czekające na wysyłkę (informacyjnie)
}

// ============================================================================
// Wycinek: online - dane z serwera (pamięć podręczna, czyszczona przy zmianie osoby)
// ============================================================================

export interface OnlineView {
  personId: string;
  loading: Array<'stats' | 'friends' | 'invitations' | 'ranking' | 'people' | 'person'>;
  stats: OnlineStats | null;               // „Moje wyniki online · ten tydzień”
  friends: Friend[] | null;
  invitations: Invitations | null;
  ranking: { scope: RankingScope; rows: RankingRow[] } | null;
  people: PeoplePage | null;
  peopleQuery: { search: string; page: number };
  person: OnlinePerson | null;             // ostatnio otwarty profil
  regCode: RegistrationCode | null;
}
export interface PeriodTotals { steps: number; distanceKm: number; activeS: number; sessions: number | null }
export interface GameTotals { game: string; gameId: string | null; steps: number; distanceKm: number; activeS: number } // game '' = „bez gry”
export interface OnlineStats extends PeriodTotals { games: GameTotals[] | null }
export interface LiveState { kmh: number; steps: number }
export interface Friend {
  id: string; name: string;
  level: number | null;
  live: LiveState | null;      // idzie teraz
  week: PeriodTotals;
}
export interface InvitationPerson { id: string; name: string }
export interface Invitation { id: string; person: InvitationPerson; createdAt: number | null }
export interface Invitations { incoming: Invitation[]; sent: Invitation[] }
export interface RankingRow {
  place: number; id: string; name: string; me: boolean;
  level: number | null; steps: number; distanceKm: number;
}
export interface PeoplePage {
  page: number; hasNext: boolean;
  people: Array<{ id: string; name: string; level: number | null; weekSteps: number; place: number | null; friend: boolean }>;
}
export interface OnlinePerson {
  id: string; name: string; me: boolean;
  level: number | null; xp: number | null; achievementsCount: number | null;
  achievements: Array<{ id: string; earnedAt: number }> | null; // osiagniecia_lista; null - serwer nie podał
  since: number | null;        // od_utc_s
  treadmillModel: string | null;
  friend: boolean;
  week: PeriodTotals | null;
  total: PeriodTotals | null;
  games: GameTotals[] | null;
  gamesHidden: boolean;        // serwer podał "gry": null - osoba nie pokazuje podziału
}
export interface RegistrationCode { code: string; validUntil: number | null }

// ============================================================================
// Wycinek: update, report, models, bubble
// ============================================================================

export interface UpdateView {
  available: string | null;    // nowsza wersja (ciche sprawdzenie przy starcie)
  busy: boolean;
  status: string;              // 'Sprawdzam aktualizacje…' / 'Pobieram 0.6.1: 45%' / 'Masz najnowszą wersję' / błąd
  installing: boolean;         // instalator uruchomiony - aplikacja zaraz się zamknie
}
export interface ReportView {
  busy: boolean;
  status: string;              // tekst pod przyciskami (wysłano / zapisano / błąd)
  lastFile: string | null;     // nazwa paczki .zip
  sentNumber: string | null;   // numer zgłoszenia z serwera
}
export interface ModelItem {
  id: string; name: string; driver: DriverId; driverName: string;
  match: string[];
  caps: Capabilities;
  generic: boolean;            // profil marki (krótkie początki nazw)
  mine: boolean;               // pasuje do zapamiętanej bieżni
  source: 'wbudowany' | 'dodatkowy';
  removable: boolean;          // source !== 'wbudowany'
}
export interface ModelsView { items: ModelItem[]; busy: boolean; status: string }
export interface BubbleView {   // okno "przycisk"
  visible: boolean;
  enabled: boolean;
  theme: ThemeChoice;
  accent: ColorName;            // kolor profilu
  training: boolean;            // tryb Trening: przy kółku +, − i Stop (okno 88 × 84 zamiast 56 × 56)
  stop: boolean;                // poza Treningiem: taśma jedzie (histereza 1 s), bieżnia przyjmuje stop - pod kółkiem Stop (okno 56 × 84)
  trainingLevel: 0 | 1 | 2 | 3;
  fasterLabel: string;          // nazwy klawiszy szybciej/wolniej do podpowiedzi + i −
  slowerLabel: string;
}

// ============================================================================
// Popupy osiągnięć (okno "popup")
// ============================================================================

export interface PopupItem {
  id: number;                  // rosnący numer (klucz w {#each})
  kicker: string;              // 'OSIĄGNIĘCIE · EPICKIE' / 'AWANS · POZIOM 12' / 'Z TWOJEJ HISTORII'
  title: string;
  line: string;
  color: string;               // hex (kolor rzadkości albo akcent)
  rarity: Rarity;
  mark: string;                // znak odznaki ('' gdy badgeText)
  badgeText: string | null;    // numer poziomu zamiast znaku
  progress: number | null;     // pasek do następnego poziomu
  leaving: boolean;            // znika (animacja 220 ms), potem wypada z listy
}
export interface PopupsView {
  items: PopupItem[];          // najwyżej 3, najnowszy OSTATNI (rysowany na dole)
}

// ============================================================================
// Argumenty komend złożonych
// ============================================================================

export interface WizardResult {
  mode: 'first' | 'add';
  name: string;
  color: ColorName;
  copyProfiles: boolean;       // tylko 'add': skopiuj profile i wygląd bieżącej osoby
  game: { pickId: string; name: string; color: ColorName } | null; // profil z gry (oprócz Domyślnego)
  direction: Direction;        // ustawiany w aktywnym profilu nowej osoby
}
export interface WizardSeed {   // propozycje na start konfiguratora
  name: string;                // nazwy.random_name(taken)
  suggestions: string[];       // 3 inne
  color: ColorName;            // losowy wolny kolor
}
export interface ProfileDraft {
  name: string;
  color: ColorName;
  iconColor: ColorName | null;
  glyph: string | null;
  pickId: string | null;       // gra z game_pick (ikona i exe)
  copyFromActive: boolean;     // kopia ustawień sterowania aktywnego profilu
}
export interface ProfilePatch {
  name?: string;
  color?: ColorName;
  glyph?: string;              // ustawia znak i usuwa ikonę z gry
  pickId?: string;             // nowa gra: ikona, exe, iconColor
  online?: boolean;
}
export interface PersonPatch { name?: string; color?: ColorName }

export interface ExportChoice {
  people: Record<string, string[]>;    // osoba -> wybrane id profili (pusta lista = osoba pominięta)
  settings: boolean;
  results: boolean;
  logs: boolean;
  range: ExportRange;
}
export interface ExportSummary { people: number; profiles: number; sessions: number; bytes: number }
export interface ImportPreview {
  file: string;                // nazwa pliku
  appVersion: string;
  hasSettings: boolean;
  hasResults: boolean;
  description: string;         // kopia.describe()
  people: Array<{ id: string; name: string; color: ColorName; profiles: number; sessions: number; exists: boolean }>;
}
export interface ImportChoice {
  people: string[];
  settings: boolean;
  results: boolean;
  newPerson: string[];         // istniejące osoby dodać jako nowe zamiast łączyć
  keepMySettings: boolean;
}
export interface ImportReport {
  peopleAdded: number; peopleMerged: number;
  profilesAdded: number; profilesUpdated: number;
  sessionsAdded: number;
  backup: string | null;       // folder kopii sprzed importu
}

// ============================================================================
// Uzupełnienia UI: nazwane unie argumentów i wyników komend z rozdziału 6
// (w kontrakcie opisane w tabelach jako String z listą wartości)
// ============================================================================

export type WindowLabel = 'main' | 'popup' | 'przycisk';
export type ThresholdWhich = 'walk' | 'run';
export type WizardMode = WizardResult['mode'];
export type GamePickKind = 'folder' | 'file';
export type OnlineTab = 'friends' | 'ranking' | 'people' | 'me';
export type InviteState = 'oczekuje' | 'przyjeta';
export type VerificationResult = 'wyslano' | 'potwierdzony';
export type NavReason = NavRequest['reason'];

/** Zdarzenia rdzenia (rozdz. 3.1): nazwa -> treść. */
export interface Zdarzenia {
  'wl:stan': SliceEvent;
  'wl:telemetria': Telemetry;
  'wl:komunikat': Toast;
  'wl:log': string;
  'wl:nawigacja': NavRequest;
  'wl:popupy': PopupsView;
}
