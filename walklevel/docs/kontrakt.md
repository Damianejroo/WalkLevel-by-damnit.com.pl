# WalkLevel 0.6.0 - kontrakt rdzeń (Rust) ↔ interfejs (Svelte 5)

Ten dokument jest jedynym źródłem prawdy o tym, **co rdzeń wysyła do interfejsu, jakie komendy
interfejs wywołuje i jakie pliki rdzeń czyta i pisze**. Kilka osób ma równolegle pisać rdzeń
(`crates/core`), warstwę Tauri (`src-tauri`) i ekrany (`src/`) - każda zmiana tego kontraktu
idzie najpierw tutaj.

Źródło: wersja 0.5.2 w Pythonie (`bieznia/gui/app.py`, `bieznia/gui/ekrany/*.py`, rdzeń
`bieznia/*.py`, `bieznia/konto`, `bieznia/poziomy`, `bieznia/ble`, `bieznia/sterowniki`). Przy
każdej decyzji podaję, skąd pochodzi zachowanie, żeby dało się je sprawdzić w starym kodzie.

Spis treści:

1. Zasady
2. Architektura: wątki, okna, przepływ danych
3. Zdarzenia (rdzeń → UI)
4. Typy TypeScript (komplet)
5. Struktury Rust (serde)
6. Komendy Tauri (UI → rdzeń)
7. Okna: główne, popup osiągnięć, pływający przycisk
8. Pliki danych - zgodność z 0.5.1/0.5.2
9. Teksty liczone przez rdzeń (podpisy, statusy)
10. Uwagi i znalezione problemy

---

## 1. Zasady

1. **Rdzeń jest właścicielem stanu.** Wszystko, co trwałe albo wspólne (ustawienia, osoby,
   profile, bieżnia, sterowanie, historia, poziomy, konto, dane online, aktualizacje), żyje
   w rdzeniu. UI trzyma lokalnie tylko stan prezentacji: bieżący ekran, rozwinięta kategoria
   Ustawień, zakładka konta online, treść wpisywana w pola, krok konfiguratora (do „Gotowe”).
2. **UI nigdy nie przebudowuje całych ekranów.** Każdy wycinek stanu (`slice`) ląduje w jednym
   obiekcie reaktywnym (`$state` w `src/lib/stan.svelte.ts`). Komponenty czytają pola
   (`stan.control.enabled`), Svelte 5 przelicza tylko to, co zależy od zmienionego pola.
   Odpowiednik Pythonowych `replace_page()`/`rebuild_settings()` nie istnieje - zmiana nazwy
   profilu zmienia jeden napis, nie ekran Ustawień.
3. **Pełny stan na starcie, potem przyrosty.** UI najpierw zapisuje się na zdarzenia, potem
   wywołuje `app_snapshot`, a zdarzenia z `rev` ≤ `snapshot.rev` odrzuca (patrz 3.2).
4. **Telemetria osobno.** Prędkość, kroki, podpis wskaźnika itd. idą zdarzeniem `wl:telemetria`
   10-20 razy na sekundę i nigdy nie przechodzą przez wycinki stanu (żeby wycinki zmieniały
   się rzadko i nie powodowały pracy w całym UI).
5. **Nawigacja w jednym oknie, bez przeładowań.** Jedna strona (SPA), ekrany przełączane przez
   `stan.ui.route` (lokalnie w UI). Rdzeń może tylko *poprosić* o przejście (`wl:nawigacja`),
   np. po wynikach skanowania - UI decyduje (np. nie przerywa konfiguratora).
6. **Komenda = jedna akcja użytkownika.** Komendy nie zwracają „nowego ekranu”, tylko wynik
   akcji (albo nic). Skutki widać przez zdarzenia stanu. Wynik zwracany jest dla wygody
   (`await`, komunikat przy polu), ale UI renderuje ze stanu.
7. **Nazwy:** pola w TS i w JSON-ie do UI w camelCase (`#[serde(rename_all = "camelCase")]`).
   Wartości wyliczeniowe zostają takie jak w 0.5 i na serwerze (`"keyboard"`, `"gamepad"`,
   `"prywatna"`, `"gra"`, `"tydzien"`) - nie tłumaczymy ich, bo są w plikach i w API.
8. **Komunikaty dla użytkownika po polsku.** Błędy (`CmdError.message`) i teksty zależne od
   logiki rdzenia (rozdział 9) układa rdzeń - kopiujemy je 1:1 z 0.5.2. Stałe etykiety
   (tytuły sekcji, nazwy przycisków) są w UI.
9. **Bez ścieżek z UI.** Okna wyboru plików (folder gry, plik kopii, profil bieżni) otwiera
   rdzeń (`tauri-plugin-dialog` po stronie Rusta). UI nie dostaje dowolnego dostępu do dysku
   ani do otwierania adresów - tylko komendy z wyliczonymi celami (`open_folder("logi")`).
10. **Rdzeń nie zależy od Tauri.** `walklevel_core` emituje przez cechę `UiSink` (5.1);
    `src-tauri` ją implementuje (`AppHandle::emit`). Dzięki temu rdzeń ma testy bez okna.

---

## 2. Architektura

```
┌──────────────────────── proces Bieznia.exe (Tauri 2) ────────────────────────┐
│                                                                              │
│  wątek "petla" (50 ms, jak TICK_MS)       wątek "bluetooth" (tokio)          │
│  ├─ Bridge.tick (mapowanie, klawisze)  ◄── odczyty prędkości/kroków          │
│  ├─ skrót F8 / F6 / F7 (GetAsyncKeyState)  polecenia taśmy (start/stop/km/h) │
│  ├─ pad (XInput/HID) co klatkę gdy trzeba                                    │
│  ├─ cele prędkości (klawisze/pad/trening)  wątki sieciowe (konto, aktualizacje,│
│  ├─ SessionTracker co 5. klatkę             zgłoszenia, profile modeli)      │
│  ├─ poziomy co 40. klatkę (~2 s)                                             │
│  ├─ konto: wysyłka co 120 s, na żywo co 5 s, odświeżenie co dobę             │
│  └─ flush: brudne wycinki → UiSink, telemetria → UiSink                      │
│                                                                              │
│  src-tauri: komendy (#[tauri::command]) → metody Core; UiSink → emit         │
│                                                                              │
│  okna WebView2:  "main" (index.html)  "popup" (popup.html)  "przycisk" (przycisk.html)
└──────────────────────────────────────────────────────────────────────────────┘
```

- `Core` (w `walklevel_core`) to uchwyt `Arc` do stanu za `Mutex`/`RwLock`; komendy Tauri
  dostają go przez `tauri::State<'_, Core>`. Długie akcje (sieć, dialogi, eksport) są
  `async` i nie trzymają blokady w trakcie I/O.
- Pętla 50 ms działa zawsze (także gdy okno jest zminimalizowane) - jak `QTimer` w 0.5.
- Każda zmiana stanu oznacza wycinek jako „brudny”; na końcu klatki pętla (albo komenda, gdy
  zmiana przyszła z komendy) wysyła każdy brudny wycinek **raz**, w całości. Wycinki są małe
  (pojedyncze kB), więc łatanie pól (JSON Patch) nie jest potrzebne.

---

## 3. Zdarzenia (rdzeń → UI)

### 3.1 Lista zdarzeń

| Zdarzenie | Do okien | Treść | Kiedy |
|---|---|---|---|
| `wl:stan` | wszystkie | `SliceEvent` (`{rev, slice, value}`) | zmiana wycinka, najwyżej raz na klatkę na wycinek |
| `wl:telemetria` | `main`, `przycisk` | `Telemetry` | co klatkę (20 Hz), gdy okno główne widoczne; 4 Hz, gdy ukryte/zminimalizowane (przycisk i tak pokazuje prędkość) |
| `wl:komunikat` | `main` | `Toast` | toast na dole okna (4 s, klik chowa) - odpowiednik `App.toast()` |
| `wl:log` | `main` | `string` (linia `HH:MM:SS  treść`) | każda linia logu INFO+ (Dziennik), UI trzyma ostatnie 500 |
| `wl:nawigacja` | `main` | `NavRequest` | rdzeń prosi o ekran (lista bieżni po skanowaniu, konfigurator przy pierwszym starcie, konto online po zalogowaniu) |
| `wl:popupy` | `popup` | `PopupsView` | lista widocznych popupów osiągnięć/awansów |

Okna `popup` i `przycisk` subskrybują tylko to, czego potrzebują (`wl:popupy`; `wl:telemetria`
+ `wl:stan` z wycinkiem `bubble`). Rdzeń wysyła `wl:popupy` przez `emit_to("popup", …)`,
a `wl:telemetria` przez `emit_to` do `main` i `przycisk`.

### 3.2 Pełny stan i kolejność

```ts
// src/lib/api.ts (szkic) - jedyne miejsce z invoke/listen; wynik trafia do stan.svelte.ts
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
const pending: SliceEvent[] = [];
let ready = false;
await listen<SliceEvent>('wl:stan', (e) => (ready ? apply(e.payload) : pending.push(e.payload)));
const snap = await invoke<AppSnapshot>('app_snapshot');
Object.assign(stan, snap);            // wszystkie wycinki naraz
lastRev = snap.rev;
for (const ev of pending) apply(ev);  // apply() pomija ev.rev <= lastRev
ready = true;
await invoke('ui_ready', { window: 'main' }); // rdzeń pokazuje okno (bez białego mignięcia)
```

- `rev` to jeden licznik rdzenia (`u64`, rośnie przy każdej emisji wycinka). `apply()` podmienia
  `stan[ev.slice] = ev.value` tylko gdy `ev.rev > lastRev[ev.slice]` (licznik per wycinek po
  stronie UI, wartość startowa = `snap.rev`).
- Podmiana całego obiektu wycinka w `$state` nie przebudowuje ekranu: Svelte 5 porównuje
  wartości pól w miejscach użycia. Dla list z kluczem (`{#each people as p (p.id)}`) zawsze
  podawać klucz.

### 3.3 Częstotliwości (zgodne z 0.5)

| Co | 0.5.2 | 0.6.0 |
|---|---|---|
| pętla | 50 ms | 50 ms |
| wskaźnik prędkości, przycisk | co klatkę | `wl:telemetria` co klatkę |
| kafelki kroki/kadencja/dystans/czas | co 5. klatkę | te same pola w telemetrii; UI może je pokazywać rzadziej |
| zapis sesji | co 60 s (`SAVE_EVERY_S`) | bez zmian |
| poziomy i osiągnięcia | co 40 klatek (~2 s), gdy zmieni się `history.version()` | bez zmian |
| wysyłka sesji | co 120 s, pierwsza 20 s po starcie | bez zmian |
| stan na żywo | co 5 s | bez zmian |
| odświeżenie tokenu | start + co 24 h | bez zmian |
| sprawdzanie potwierdzenia e-mail | co 15 s przez 10 min po wysłaniu linku; przy powrocie okna najwyżej raz na 10 s | bez zmian (powrót okna = `WindowEvent::Focused(true)` okna `main`) |

---

## 4. Typy TypeScript (komplet)

Plik: `src/lib/kontrakt.ts`. Typy liczbowe: `number`; czasy uniksowe UTC w **sekundach**
(`number`, ułamkowe), chyba że pole kończy się na `Ms`.

```ts
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
  actionLabel: string;         // glowny.status.<stan>.akcja (rozdz. 9.1)
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
  scanText: string;            // bieznia.lista.szukam | bieznia.lista.znalezione | błąd skanowania (ble.powod.*)
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
  loading: Array<'stats' | 'friends' | 'invitations' | 'ranking' | 'people' | 'person' | 'achievementStats'>;
  stats: OnlineStats | null;               // „Moje wyniki online · ten tydzień”
  friends: Friend[] | null;
  invitations: Invitations | null;
  ranking: { scope: RankingScope; rows: RankingRow[] } | null;
  people: PeoplePage | null;
  peopleQuery: { search: string; page: number };
  person: OnlinePerson | null;             // ostatnio otwarty profil
  regCode: RegistrationCode | null;
  achievementStats: AchievementStats | null; // ekran Osiągnięcia: online_load('achievements'), najwyżej raz na dobę
}
/** GET osiagniecia/statystyki: poniżej progu kont serwer nie podaje procentów (accounts null, percent {}). */
export interface AchievementStats { accounts: number | null; percent: Record<string, number> }
/** Profil osoby: gry poza listą (inne_gry) - wiersz „Inne gry (N)” bez nazw. */
export interface OtherGames { steps: number; distanceKm: number; activeS: number; sessions: number | null; games: number }
export interface PeriodTotals { steps: number; distanceKm: number; activeS: number; sessions: number | null }
export interface GameTotals { game: string; gameId: string | null; steps: number; distanceKm: number; activeS: number } // game '' = „bez gry”
export interface OnlineStats extends PeriodTotals { games: GameTotals[] | null }
/** 0.6.2: distanceKm/mode/game null bez zgody tej osoby na regulamin z grami; game null przy ukrytej grze („W grze”). */
export interface LiveState {
  kmh: number; steps: number; activeS: number;
  since: number | null;        // od_utc_s
  distanceKm: number | null;   // dystans BIEŻĄCEJ sesji (live XP = xp + floor(distanceKm * 100))
  mode: SessionMode | null; game: string | null;
}
export interface Friend {
  id: string; name: string;
  level: number | null;        // null - serwer nie podaje poziomu bez zgody tej osoby: UI pokazuje neutralny awatar „Poziom ukryty”
  xp: number | null;           // 0.6.2; null jak level
  description: string | null;  // 0.6.2: opis dla znajomych (pusty = null)
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
  games: GameTotals[] | null;  // profil: gry[].nazwa, statystyki: gry[].gra
  otherGames: OtherGames | null; // inne_gry; null - brak
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
```

---

## 5. Struktury Rust (serde)

Moduł `walklevel_core::kontrakt` (plik `crates/core/src/kontrakt.rs`). Każda struktura ma
`#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]` i
`#[serde(rename_all = "camelCase")]`, chyba że zaznaczono inaczej. Nazwy struktur = nazwy
interfejsów TS. Poniżej komplet; komentarze są w części TS.

### 5.1 Granica rdzeń ↔ Tauri

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Implementuje src-tauri (AppHandle). Testy rdzenia - wersja zbierająca zdarzenia do Vec.
pub trait UiSink: Send + Sync + 'static {
    fn slice(&self, ev: &SliceEvent);                 // "wl:stan" do wszystkich okien
    fn telemetry(&self, t: &Telemetry);               // "wl:telemetria" do main + przycisk
    fn toast(&self, t: &Toast);                       // "wl:komunikat"
    fn log_line(&self, line: &str);                   // "wl:log"
    fn navigate(&self, n: &NavRequest);               // "wl:nawigacja"
    fn popups(&self, p: &PopupsView);                 // "wl:popupy" do okna popup
    fn window(&self, r: &WindowRequest);              // sterowanie oknami (rozdz. 7)
    fn dialog(&self, d: &DialogRequest) -> Option<std::path::PathBuf>; // blokujące okno wyboru pliku (z wątku zadania)
}

#[derive(Debug, Clone, Serialize)]
pub struct SliceEvent { pub rev: u64, pub slice: SliceName, pub value: Value }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SliceName { App, Settings, People, Device, Control, Levels, History, Account, Online, Update, Report, Models, Bubble }

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    pub rev: u64,
    pub statics: StaticData,
    pub log_tail: Vec<String>,
    pub app: AppInfo, pub settings: Settings, pub people: PeopleView, pub device: DeviceView,
    pub control: ControlView, pub levels: Option<LevelsView>, pub history: HistoryStamp,
    pub account: AccountView, pub online: OnlineView, pub update: UpdateView,
    pub report: ReportView, pub models: ModelsView, pub bubble: BubbleView,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CmdError {
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub fields: std::collections::BTreeMap<String, String>,
}
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ErrorCode { Invalid, NotFound, Busy, Offline, LoggedOut, UpdateRequired, Server, Io, Device, Key, Cancelled, Unavailable }
pub type CmdResult<T> = Result<T, CmdError>;
// impl From<account::KontoError>, From<std::io::Error>, From<rusqlite::Error> for CmdError

#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct Toast { pub text: String, pub kind: ToastKind }
#[derive(Debug, Clone, Copy, Serialize)] #[serde(rename_all = "camelCase")]
pub enum ToastKind { Info, Error }

#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct NavRequest { pub to: Route, pub reason: NavReason, #[serde(skip_serializing_if = "Option::is_none")] pub account_mode: Option<AccountFormMode> }
#[derive(Debug, Clone, Copy, Serialize, Deserialize)] #[serde(rename_all = "camelCase")]
pub enum Route { Main, Devices, Settings, Log, Report, Profile, ProfileNew, Person, Wizard, History, Session, Export, Import, Models, Account, Online, OnlinePerson, Achievements, Trail }
#[derive(Debug, Clone, Copy, Serialize)] #[serde(rename_all = "camelCase")]
pub enum NavReason { Scan, FirstRun, SignedIn, PersonOpened, Settings }

/// Prośby rdzenia o zmianę okien (wykonuje src-tauri/okna.rs).
#[derive(Debug, Clone)]
pub enum WindowRequest {
    ShowMain,                              // przycisk prawym / „otwórz okno”: showNormal + raise + focus
    MainAlwaysOnTop(bool),
    MainPanel(PanelLayout, PanelLayout), // 0.6.2: panel znajomych przed/po - min. szerokość okna 720 (pełny) / 484 (pasek) / 420 (bez);
                                         // zwinięcie -224 px, rozwinięcie +224 (z lewej okno przesuwa się o tyle samo w drugą stronę);
                                         // zmaksymalizowane albo przyciągnięte (Aero Snap) - bez zmiany rozmiaru
    BubbleVisible(bool),
    BubbleSize(bool),                      // true - tryb Trening (88 × 84 z +, − i Stop), false - 56 × 56
    BubbleMove { x: i32, y: i32 },         // piksele logiczne (jak floating_pos w 0.5)
    PopupPlace { x: i32, y: i32, w: u32, h: u32 }, // piksele fizyczne ekranu z grą
    PopupVisible(bool),                    // pokazanie BEZ aktywacji (SW_SHOWNOACTIVATE)
    Quit,                                  // po zakończeniu zamykania rdzenia
}
#[derive(Debug, Clone)]
pub enum DialogRequest {
    PickFolder { title: String },
    OpenFile { title: String, filter_name: String, extensions: Vec<String> },
    SaveFile { title: String, default_name: String, filter_name: String, extensions: Vec<String> },
}

#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct Telemetry {
    pub t_ms: u64, pub speed_kmh: f64, pub live: bool, pub enabled: bool, pub mode: GaugeMode,
    pub active: i32, pub caption: String, pub steps: Option<i64>, pub cadence: Option<f64>,
    pub distance_km: f64, pub active_s: f64, pub belt_target_kmh: Option<f64>,
    pub pad: Option<PadTelemetry>,
}
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct PadTelemetry { pub name: Option<String>, pub deflection: f64, pub step: u32, pub status_text: String }
#[derive(Debug, Clone, Copy, PartialEq, Serialize)] #[serde(rename_all = "camelCase")]
pub enum GaugeMode { Idle, Walk, Run, Offline }
```

### 5.2 Wyliczenia z wartościami z plików

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction { Keyboard, Gamepad, Treadmill, Training }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputKind { Dry, Keys }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeChoice { System, Light, Dark }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PadIdle { Stop, Slow, Zero }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyRole { Walk, Run, Toggle, Faster, Slower }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Link { Idle, Searching, Connecting, Connected, Streaming, Retrying, BluetoothOff, Unsupported }
// Wartości z polskimi nazwami (sesje, konto, serwer) trzymamy jako String z walidacją
// (stałe VISIBILITY, PROFILE_VISIBILITY, GAMES_VISIBILITY, RANKING_SCOPES w account.rs),
// bo ten sam tekst idzie do bazy i na serwer bez mapowania. ColorName - String sprawdzany
// przez paleta::kolor() (nieznany -> 'Mięta'), jak w 0.5.
```

### 5.3 Wycinki

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)] #[serde(rename_all = "camelCase")]
pub struct KeyBinding { pub scan: u16, pub extended: bool, pub name: String, pub vk: u16, pub mods: Vec<String>, #[serde(default)] pub label: String }
#[derive(Debug, Clone, Deserialize)] #[serde(rename_all = "camelCase")]
pub struct KeyCapture { pub code: String, pub key: String, pub ctrl: bool, pub alt: bool, pub shift: bool, pub meta: bool, pub location: u8 }
pub type PadStep = (u8, f64); // serializuje się jako [at, kmh] - tak samo jak w osoby.json
#[derive(Debug, Clone, PartialEq, Serialize)] #[serde(rename_all = "camelCase")]
pub struct IconRef { pub color: String, pub glyph: Option<String>, pub url: Option<String> }
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)] #[serde(rename_all = "camelCase")]
pub struct Totals { pub steps: i64, pub distance_km: f64, pub active_s: f64, pub avg_kmh: f64 }
// (model::Totals ma avg_kmh() jako metodę; do UI idzie ta wersja z polem)

#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String, pub title: String, pub installed: bool, pub data_dir: String,
    pub first_run: bool, pub legacy02: bool, pub updated_from: Option<String>,
    pub latest_backup: Option<String>, pub accounts_enabled: bool, pub report_endpoint: bool,
    pub window_focused: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)] #[serde(rename_all = "camelCase")]
pub struct Settings {
    pub device_address: String, pub device_name: String, pub belt_control: bool, pub quick_stop: bool,
    pub toggle_key: KeyBinding, pub faster_key: KeyBinding, pub slower_key: KeyBinding,
    pub toggle_pad: String, pub simulator: bool, pub record: bool,
    pub detailed_logs: bool,
    pub theme: ThemeChoice, pub large_text: bool, pub always_on_top: bool, pub floating_button: bool,
    pub show_person_switch: bool, pub show_profile_switch: bool,
    pub achievement_popups: bool, pub achievement_sound: bool,
    pub direction: Direction, pub output: OutputKind, pub walk_key: KeyBinding, pub run_key: KeyBinding,
    pub stand_below: f64, pub run_on: f64, pub run_off: f64,
    pub idle_speed: f64, pub walk_speed: f64, pub run_speed: f64,
    pub pad_steps: Vec<PadStep>, pub pad_idle: PadIdle,
    pub training_levels: [f64; 4], pub training_level: u8,
    pub profile_color: String, pub person_color: String,
}

#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct PeopleView { pub ready: bool, pub active_person_id: String, pub active_profile_id: String, pub ask_at_start: bool, pub people: Vec<PersonView> }
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct PersonView { pub id: String, pub name: String, pub color: String, pub active_profile_id: String, pub profiles: Vec<GameProfileView>, pub today_steps: i64, pub has_account: bool, pub account_name: Option<String> }
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct GameProfileView { pub id: String, pub name: String, pub color: String, pub icon_color: Option<String>, pub icon: IconRef, pub exe: Option<String>, pub online: bool, pub summary: ProfileSummary }
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct ProfileSummary { pub direction: Direction, pub walk_speed: f64, pub run_speed: f64, pub pad_step_count: u8, pub pad_max_kmh: f64, pub training_min: f64, pub training_max: f64, pub stand_below: f64 }
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct GamePick { pub pick_id: String, pub name: String, pub exe: String, pub exe_name: String, pub folder_name: String, pub icon: IconRef, pub icon_color: String }
// W TS: Person = PersonView, GameProfile = GameProfileView (nazwy w Ruście z sufiksem, żeby
// nie myliły się z store::Person / store::Profile - modelem pliku osoby.json).

#[derive(Debug, Clone, PartialEq, Serialize)] #[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub speed_min: f64, pub speed_max: f64, pub speed_step: f64, pub start_speed: f64,
    pub start_delay_s: f64, pub acks_commands: bool, pub pause: bool, pub steps: bool,
    pub incline: Option<(f64, f64, f64)>, pub telemetry_s: f64, pub min_command_gap_s: f64,
    pub control: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize)] #[serde(rename_all = "camelCase")]
pub struct DeviceInfo { pub model: String, pub driver: String, pub profile: String, pub name: String, pub firmware: String, pub simulated: bool }
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct FoundDevice { pub address: String, pub name: String, pub rssi: i16, pub treadmill: bool, pub model: String }
#[derive(Debug, Clone, Copy, PartialEq, Serialize)] #[serde(rename_all = "camelCase")]
pub enum StatusAction { Scan, Cancel, Disconnect, Retry, SimulatorOff }
#[derive(Debug, Clone, Copy, PartialEq, Serialize)] #[serde(rename_all = "camelCase")]
pub enum Tone { Accent, Run, Danger, Muted }
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct LinkStatus { pub tone: Tone, pub busy: bool, pub title: String, pub sub: String, pub action: StatusAction, pub action_label: String, pub primary: bool }
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct Diagnostics { pub running: bool, pub text: String }
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct DeviceView {
    pub link: Link, pub link_text: String, pub status: LinkStatus, pub device: Option<DeviceInfo>,
    pub caps: Capabilities, pub remembered: bool, pub label: String, pub scanning: bool,
    pub scan_text: String, pub devices: Vec<FoundDevice>, pub bluetooth_off: bool,
    pub diagnostics: Diagnostics,
}

#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct ControlView {
    pub enabled: bool, pub direction: Direction, pub sets_speed: bool, pub hint: String,
    pub subtitle: String, pub toggle_label: String, pub walk_name: String, pub run_name: String,
    pub pad_button_label: String, pub pad_capturing: bool, pub key_capturing: Option<KeyRole>,
    pub belt_commands: bool,
}

#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct RankView { pub level: u32, pub name: String, pub about: String }
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct AchievementProgress { pub id: String, pub earned_at: Option<f64>, pub current: f64, pub target: f64, pub fraction: f64 }
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct LevelsView {
    pub person_id: String, pub km: f64, pub xp: i64, pub level: u32, pub max_level: u32,
    pub level_floor_xp: i64, pub level_ceiling_xp: i64, pub level_fraction: f64,
    pub km_to_next_level: f64, pub rank: RankView, pub next_rank: Option<RankView>,
    pub earned_count: u32, pub total: u32, pub progress: Vec<AchievementProgress>,
}

#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct HistoryStamp { pub available: bool, pub stamp: String }
#[derive(Debug, Clone, Deserialize, Serialize)] #[serde(rename_all = "camelCase")]
pub struct HistoryQuery { pub scope: String /* "me" | "all" */, pub range: String /* "day" | "week" | "month" | "all" */ }
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct ChartPart { pub name: String, pub steps: i64 }
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct ChartBar { pub label: String, pub weekday: Option<u8>, pub parts: Vec<ChartPart> }
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct HistoryChart { pub hourly: bool, pub bars: Vec<ChartBar>, pub colors: std::collections::BTreeMap<String, String>, pub legend: Vec<String>, pub empty: bool }
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct HistoryGroup { pub name: String, pub totals: Totals, pub share: f64, pub color: String, pub icon: Option<IconRef>, pub avatar_color: Option<String> }
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct OnlineMark { pub hidden: bool, pub sent: bool, pub rejected: Option<String> }
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct SessionRow {
    pub id: String, pub start: f64, pub end: f64, pub person_id: String, pub person_name: String,
    pub profile_id: String, pub profile_name: String, pub steps: i64, pub distance_km: f64,
    pub active_s: f64, pub avg_kmh: f64, pub source: String, pub mode: String, pub icon: IconRef,
    pub online: Option<OnlineMark>,
}
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct SessionDetail {
    #[serde(flatten)] pub row: SessionRow,
    pub cadence: Option<f64>, pub whole_s: f64, pub device_model: String, pub driver: String,
    pub firmware: String, pub app_version: String, pub tz_offset_min: i64, pub linked: bool,
}
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct HistoryView {
    pub query: HistoryQuery, pub stamp: String, pub many: bool, pub everyone: bool,
    pub totals: Totals, pub chart: HistoryChart, pub group_key: String, pub groups: Vec<HistoryGroup>,
    pub recent: Vec<SessionRow>, pub online: bool,
}

#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct AccountView {
    pub enabled: bool, pub person_id: String, pub logged_in: bool, pub user_id: String,
    pub name: String, pub email: String, pub verified: bool, pub upload: bool,
    pub visibility: String, pub live: bool,
    pub live_status: String,     // 0.6.2: wszystko / chodze / niewidoczny (live = status != niewidoczny)
    pub description: String,     // 0.6.2: opis dla znajomych
    pub profile_visibility: String,
    pub games_visibility: String, pub terms: String, pub terms_current: String,
    pub terms_outdated: bool, pub games_allowed: bool, pub remember: bool, pub terms_url: String,
    pub busy: Vec<String>, pub message: String, pub verify_polling: bool, pub pending_uploads: u32,
}

// Dane online (PeriodTotals ... RegistrationCode) i ich odczyt z odpowiedzi serwera:
// walklevel_core::account::online (rdzeń), src-tauri tylko je przekazuje.
#[derive(Debug, Clone, Default, Serialize)] #[serde(rename_all = "camelCase")]
pub struct PeriodTotals { pub steps: i64, pub distance_km: f64, pub active_s: f64, pub sessions: Option<i64> }
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct GameTotals { pub game: String, pub game_id: Option<String>, pub steps: i64, pub distance_km: f64, pub active_s: f64 }
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct OnlineStats { #[serde(flatten)] pub totals: PeriodTotals, pub games: Option<Vec<GameTotals>> }
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct LiveState { pub kmh: f64, pub steps: i64 }
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct Friend { pub id: String, pub name: String, pub level: Option<u32>, pub live: Option<LiveState>, pub week: PeriodTotals }
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct InvitationPerson { pub id: String, pub name: String }
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct Invitation { pub id: String, pub person: InvitationPerson, pub created_at: Option<f64> }
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct Invitations { pub incoming: Vec<Invitation>, pub sent: Vec<Invitation> }
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct RankingRow { pub place: u32, pub id: String, pub name: String, pub me: bool, pub level: Option<u32>, pub steps: i64, pub distance_km: f64 }
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct Ranking { pub scope: String, pub rows: Vec<RankingRow> }
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct PeopleItem { pub id: String, pub name: String, pub level: Option<u32>, pub week_steps: i64, pub place: Option<u32>, pub friend: bool }
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct PeoplePage { pub page: u32, pub has_next: bool, pub people: Vec<PeopleItem> }
#[derive(Debug, Clone, Serialize, Deserialize)] #[serde(rename_all = "camelCase")]
pub struct PeopleQuery { pub search: String, pub page: u32 }
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct EarnedOnline { pub id: String, pub earned_at: Option<f64> } // cudzy profil: bez zdobyte_utc_s (prywatność) -> None, UI bez „zdobyte …”, kolejność z serwera
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct OnlinePerson {
    pub id: String, pub name: String, pub me: bool, pub level: Option<u32>, pub xp: Option<i64>,
    pub achievements_count: Option<u32>, pub achievements: Option<Vec<EarnedOnline>>,
    pub since: Option<f64>, pub treadmill_model: Option<String>, pub friend: bool,
    pub week: Option<PeriodTotals>, pub total: Option<PeriodTotals>,
    pub games: Option<Vec<GameTotals>>, pub games_hidden: bool,
    pub description: Option<String>, // 0.6.2: `opis` - tylko znajomy i właściciel
}
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct RegistrationCode { pub code: String, pub valid_until: Option<f64> }
#[derive(Debug, Clone, Default, Serialize)] #[serde(rename_all = "camelCase")]
pub struct OnlineView {
    pub person_id: String, pub loading: Vec<String>, pub stats: Option<OnlineStats>,
    pub friends: Option<Vec<Friend>>, pub invitations: Option<Invitations>,
    pub ranking: Option<Ranking>, pub people: Option<PeoplePage>, pub people_query: PeopleQuery,
    pub person: Option<OnlinePerson>, pub reg_code: Option<RegistrationCode>,
}

#[derive(Debug, Clone, Default, Serialize)] #[serde(rename_all = "camelCase")]
pub struct UpdateView { pub available: Option<String>, pub busy: bool, pub status: String, pub installing: bool }
#[derive(Debug, Clone, Default, Serialize)] #[serde(rename_all = "camelCase")]
pub struct ReportView { pub busy: bool, pub status: String, pub last_file: Option<String>, pub sent_number: Option<String> }
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct ModelItem { pub id: String, pub name: String, pub driver: String, pub driver_name: String, pub r#match: Vec<String>, pub caps: Capabilities, pub generic: bool, pub mine: bool, pub source: String, pub removable: bool }
// uwaga: pole `match` w Ruście jako r#match - serde nazwie je "match"
#[derive(Debug, Clone, Default, Serialize)] #[serde(rename_all = "camelCase")]
pub struct ModelsView { pub items: Vec<ModelItem>, pub busy: bool, pub status: String }
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct BubbleView {
    pub visible: bool, pub enabled: bool, pub theme: ThemeChoice, pub accent: String,
    pub training: bool, pub training_level: u32, pub faster_label: String, pub slower_label: String,
}

#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct PopupItem { pub id: u64, pub kicker: String, pub title: String, pub line: String, pub color: String, pub rarity: String, pub mark: String, pub badge_text: Option<String>, pub progress: Option<f64>, pub leaving: bool }
#[derive(Debug, Clone, Default, Serialize)] #[serde(rename_all = "camelCase")]
pub struct PopupsView { pub items: Vec<PopupItem> }

// --- argumenty komend ---
#[derive(Debug, Clone, Deserialize)] #[serde(rename_all = "camelCase")]
pub struct WizardGame { pub pick_id: String, pub name: String, pub color: String }
#[derive(Debug, Clone, Deserialize)] #[serde(rename_all = "camelCase")]
pub struct WizardResult { pub mode: String /* "first" | "add" */, pub name: String, pub color: String, pub copy_profiles: bool, pub game: Option<WizardGame>, pub direction: Direction }
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct WizardSeed { pub name: String, pub suggestions: Vec<String>, pub color: String }
#[derive(Debug, Clone, Deserialize)] #[serde(rename_all = "camelCase")]
pub struct ProfileDraft { pub name: String, pub color: String, pub icon_color: Option<String>, pub glyph: Option<String>, pub pick_id: Option<String>, pub copy_from_active: bool }
#[derive(Debug, Clone, Default, Deserialize)] #[serde(rename_all = "camelCase")]
pub struct ProfilePatch { pub name: Option<String>, pub color: Option<String>, pub glyph: Option<String>, pub pick_id: Option<String>, pub online: Option<bool> }
#[derive(Debug, Clone, Default, Deserialize)] #[serde(rename_all = "camelCase")]
pub struct PersonPatch { pub name: Option<String>, pub color: Option<String> }
#[derive(Debug, Clone, Deserialize)] #[serde(rename_all = "camelCase")]
pub struct ExportChoice { pub people: std::collections::BTreeMap<String, Vec<String>>, pub settings: bool, pub results: bool, pub logs: bool, pub range: String /* all | year | month */ }
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct ExportSummary { pub people: u32, pub profiles: u32, pub sessions: u32, pub bytes: u64 }
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct ImportPerson { pub id: String, pub name: String, pub color: String, pub profiles: u32, pub sessions: u32, pub exists: bool }
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct ImportPreview { pub file: String, pub app_version: String, pub has_settings: bool, pub has_results: bool, pub description: String, pub people: Vec<ImportPerson> }
#[derive(Debug, Clone, Deserialize)] #[serde(rename_all = "camelCase")]
pub struct ImportChoice { pub people: Vec<String>, pub settings: bool, pub results: bool, pub new_person: Vec<String>, pub keep_my_settings: bool }
#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]
pub struct ImportReport { pub people_added: u32, pub people_merged: u32, pub profiles_added: u32, pub profiles_updated: u32, pub sessions_added: u32, pub backup: Option<String> }
```

`StaticData`, `PaletteColor`, `AccountRules`, `LevelDefinitions`, `Rank`, `Achievement` -
pola 1:1 z TS (jw. camelCase; `Achievement.params: BTreeMap<String, serde_json::Value>`).

**Test zgodności typów** (obowiązkowy): test w `crates/core` serializuje przykładowe
`AppSnapshot`, `Telemetry`, `PopupsView` i każdy wynik komendy do
`src/lib/__fixtures__/*.json`; `npm run check` (svelte-check) kompiluje
`src/lib/__fixtures__/sprawdz.ts`, który importuje te pliki z `satisfies AppSnapshot` itd.
Rozjazd nazw pól wychodzi przy kompilacji, a nie w aplikacji.

---

## 6. Komendy Tauri (UI → rdzeń)

Zasady:

- Każda komenda: `#[tauri::command] pub async fn nazwa(core: State<'_, Core>, …) -> CmdResult<T>`
  w `src-tauri/src/komendy/<obszar>.rs`; ciało to jedno wywołanie metody `Core` (logika
  tylko w rdzeniu). Rejestracja w `tauri::generate_handler![…]`.
- Argumenty w Ruście w snake_case, z JS przekazywane w **camelCase** (domyślne zachowanie
  Tauri 2): `fn person_switch(person_id: String)` ← `invoke('person_switch', { personId })`.
- Opakowania w `src/lib/api.ts` (jedyne miejsce z `invoke`/`listen`, zasada z CLAUDE.md;
  typy z `src/lib/kontrakt.ts`): `export const personSwitch = (personId: string) =>
  invoke<void>('person_switch', { personId });` - ekrany nie wołają `invoke` bezpośrednio.
- Komendy, które zmieniają stan, **nie muszą** zwracać nowego stanu - przyjdzie `wl:stan`.
  Zwracają wynik tylko tam, gdzie UI potrzebuje odpowiedzi w miejscu wywołania.
- Komendy sieciowe są niezależne od pętli: trwają do odpowiedzi serwera (timeout 15 s), a w
  `account.busy` jest ich nazwa. Druga taka sama komenda tej samej osoby w trakcie → `busy`.
- „(okno)” = rdzeń otwiera systemowe okno wyboru pliku; zamknięcie bez wyboru → `Ok(None)`.

Poniżej `T` to typ wyniku; `()` = `void`.

### 6.1 Start, okna, ogólne

| Komenda (Rust) | Argumenty | Wynik | Skutek / uwagi |
|---|---|---|---|
| `app_snapshot` | - | `AppSnapshot` | pełny stan + `statics` + `logTail` |
| `ui_ready` | `window: String` (`"main"`/`"popup"`/`"przycisk"`) | `()` | `main`: pokazuje okno główne w zapamiętanym miejscu (`window_geometry`), potem rusza połączenie z bieżnią/skan; przy `firstRun` wysyła `wl:nawigacja {to:'wizard', reason:'firstRun'}` |
| `app_quit` | - | `()` | zamykanie jak `App.quit()`: chowa okno, puszcza klawisze, **zatrzymuje taśmę** (`stop_belt_now`, do 8 s), zapisuje geometrię, zamyka historię, `WindowRequest::Quit` |
| `window_show_main` | - | `()` | z przycisku (prawy klik) albo z popupu - `showNormal` + raise + focus |
| `open_folder` | `kind: FolderKind` | `()` | `dane` / `logi` / `zgloszenia` / `nagrania` w folderze danych (tworzy, jeśli brak) |
| `open_link` | `kind: LinkKind` | `()` | strona autora / polityka prywatności / kod / regulamin konta (adresy z rdzenia) |
| `log_tail` | - | `Vec<String>` | ostatnie 500 linii Dziennika (po ponownym otwarciu okna) |
| `diagnostics_run` | - | `String` (ścieżka raportu) | raport Bluetooth `logs/diagnostyka-YYYYmmdd-HHMMSS.txt` (~30 s); w trakcie `device.diagnostics.running` |

### 6.2 Ustawienia

| Komenda | Argumenty | Wynik | Skutek / uwagi |
|---|---|---|---|
| `settings_set` | `key: SettingKey (String)`, `value: Value` | `()` | zapis do właściwej części osoby.json (8.1). Walidacja jak `normalize()`; zły typ/wartość → `invalid`. Skutki uboczne: `alwaysOnTop` → `WindowRequest::MainAlwaysOnTop`; `floatingButton` → `BubbleVisible`; `simulator` → `on_simulator` (połączenie z symulatorem albo z zapamiętaną bieżnią); `record` → nagrywanie; `detailedLogs` → poziom logu `bieznia.ble`; `direction` → `apply_direction` (wyjście `dry` dla trybów sterujących taśmą), `belt_target = None`; `output` → `Bridge.set_output`; `idle/walk/runSpeed` → zaokrąglenie do 0,1, `belt_target = None`; `keyHoldS` → 0-3, zaokrąglenie do pełnych sekund, `belt_target = None`; `keyIdle` (`slow`/`stop`) i `padIdle` → `belt_target = None`; `achievementSound = true` → od razu odtwarza dźwięk osiągnięcia; `friendsPanel`/`friendsPanelCollapsed`/`friendsPanelSide` → `WindowRequest::MainPanel(przed, po)` (także przy `ui_ready`: minimalna szerokość); `togglePad: ''` → usuwa przycisk pada |
| `thresholds_set` | `which: String` (`"walk"`/`"run"`), `kmh: f64` | `()` | `on_threshold`: zmieniony próg wygrywa, drugi przesuwa się o 0,8 (≤ `maxSpeed`), `runOff = runOn - 0.6`, zaokr. 0,1 |
| `pad_steps_count` | `count: u8` (2-5) | `()` | gotowe skoki `PRESETS[count]` |
| `pad_step_set` | `index: u8`, `at: Option<u8>`, `kmh: Option<f64>` | `()` | `on_pad_at` / `on_pad_speed`; walidacja `parse_steps` (pierwszy 0, odstęp ≥ 5, ≤ 100, 0,5-12 km/h) → `invalid` |
| `training_speed_set` | `index: u8` (0-3), `kmh: f64` | `()` | `on_training_speed` (0,5-20), nowa prędkość od razu na taśmę |
| `key_capture` | `role: Option<KeyRole>` | `()` | wejście/wyjście z przypisywania klawisza: skróty globalne (włączanie, szybciej, wolniej) wstrzymane, `control.keyCapturing` |
| `key_set` | `role: KeyRole`, `key: KeyCapture` | `KeyBinding` | zamiana `code` → scan/extended (tabela kodów), `vk` przez `MapVirtualKeyW(MAPVK_VSC_TO_VK_EX)`, nazwa przez `GetKeyNameTextW` (zapas: `key`); walidacja `keys::problem_with()` (walk/run/toggle; klawisze szybciej/wolniej zajęte), a dla `faster`/`slower` `keys::speed_problem()` (różne od siebie i od włączania, chodu i biegu; kombinacje dozwolone, skróty Windows nie) → błąd `key` z komunikatem (toast i podpowiedź robi UI z `message`); zmiana toggle/faster/slower przebudowuje skróty od razu; kończy przypisywanie |
| `pad_capture_start` | - | `()` | czeka do 10 s na przycisk pada (`control.padCapturing`); wybrany przycisk → `togglePad`; „Krzyżyk”/„Kółko” (`south`/`east`) → toast z ostrzeżeniem |
| `pad_capture_cancel` | - | `()` | przerywa czekanie |

Zmiana koloru profilu to `profile_update(id, { color })`; wygląd przelicza UI (akcent z palety).

### 6.3 Sterowanie i trening

| Komenda | Argumenty | Wynik | Skutek |
|---|---|---|---|
| `control_set` | `on: bool`, `keyboard: Option<bool>` | `()` | `set_control`: jedno miejsce dla przełącznika, skrótu, pada i przycisku; zapis `dane/stan.json`; start/stop taśmy wg trybu (rozdz. 9.3). Od 0.6.2 każda zmiana stanu idzie do dziennika ze źródłem (`ControlSource`): „Sterowanie WYŁĄCZONE (klawisz ])”, „(okno: przełącznik)”, „(okno: przełącznik, z klawiatury)” - `keyboard` = Enter/spacja, „(przycisk na ekranie)”, „(przycisk na ekranie: Stop)”, „(Trening: Stop)”, „(Trening: poziom N)”, „(pad: przycisk X)”, „(polecenie)” |
| `control_toggle` | - | `()` | jak przycisk na ekranie / F8 |
| `training_level_set` | `level: u8` | `()` | ustawia poziom; gdy sterowanie wyłączone - włącza je (taśma rusza) |
| `training_level_step` | `step: i8` (±1) | `()` | klawisze szybciej/wolniej (domyślnie F7/F6) w trybie Trening robią to samo w pętli; wołane też z okna przycisku (+/−) |
| `training_stop` | - | `()` | wyłącza sterowanie (taśma staje), a gdy już wyłączone - sam stop taśmy; wołane też z okna przycisku (Stop) |
| `device_belt_stop` | - | `()` | Stop z okna przycisku poza Treningiem (`bubble.stop`): włączone sterowanie się wyłącza, potem zawsze `BeltCommand::Stop` (także w Klawiszach z „Wolne tempo” i bez „start i stop z aplikacji”) |

### 6.4 Bieżnia

| Komenda | Argumenty | Wynik | Skutek |
|---|---|---|---|
| `device_scan` | - | `()` | czyści listę, `device.scanning = true`, skan 6 s; wynik w `device.devices`; jedna bieżnia przy pierwszym starcie bez zapamiętanej → łączy sama; wynik skanu NIGDY sam nie otwiera listy - `wl:nawigacja {to:'devices', reason:'scan'}` wysyła tylko `device_status_action` (przycisk Szukaj bieżni) |
| `device_connect` | `address: String`, `name: String` | `()` | zapamiętuje bieżnię (wyłącza symulator), łączy, możliwości z profilu modelu od razu |
| `device_disconnect` | - | `()` | rozłącza (nie zapomina) |
| `device_forget` | - | `()` | rozłącza i czyści `deviceAddress`/`deviceName` |
| `device_status_action` | - | `()` | przycisk na karcie statusu - logika `on_status_action` (wyłącz symulator / rozłącz / ponów / szukaj) |
| `models_refresh` | - | `()` | przelicza `models` (lista profili modeli, „Twoja bieżnia”) - przy wejściu na ekran |
| `models_fetch` | - | `String` (status) | pobranie profili z GitHuba (`pobieranie.download_profiles`); tekst też w `models.status` |
| `models_add_file` | - (okno) | `Option<String>` | `install_from_file`; `invalid` z powodem odrzucenia |
| `models_remove` | `id: String` | `()` | tylko profil `dodatkowy` |

Symulator: `settings_set('simulator', true|false)`. Klawisze wolniej/szybciej (pilot symulatora, domyślnie F6/F7, `settings.slowerKey/fasterKey`) obsługuje pętla.

### 6.5 Osoby i profile gier

| Komenda | Argumenty | Wynik | Skutek |
|---|---|---|---|
| `wizard_seed` | `mode: String` (`"first"`/`"add"`) | `WizardSeed` | losowa nazwa (`nazwy.random_name`), 3 propozycje, wolny kolor |
| `name_random` | - | `String` | kostka przy nazwie osoby (z wykluczeniem zajętych) |
| `wizard_finish` | `result: WizardResult` | `()` | `first`: kopia ustawień 0.2 (`dane/kopie/ustawienia-0.2.json`), `Store.create` z ustawieniami 0.2 i zmianami z tej sesji; `add`: `add_person` (+kopiowanie profili). Gra → nowy profil (kopia ustawień Domyślnego, ikona `nowa-*.png` → `<id>.png`). `direction` do aktywnego profilu. Potem `apply_profile` |
| `person_switch` | `person_id: String` | `()` | `choose_person`: kończy bieżącą sesję, czyści `online`, nowe ustawienia/wygląd |
| `person_update` | `person_id: String`, `patch: PersonPatch` | `()` | nazwa (pusta → bez zmian) i kolor; UI zapisuje 500 ms po ostatniej literze i przy wyjściu z pola |
| `person_remove` | `person_id: String` | `()` | ostatniej się nie da (`invalid`); wylogowuje konto osoby lokalnie (`Accounts.forget_person`), sesje → „Usunięte osoby” |
| `profile_switch` | `profile_id: String`, `from: Option<String>` (`main`/`settings`), `keyboard: Option<bool>` | `()` | `choose_profile` (kończy sesję poprzedniego profilu); dziennik: „Profil: X (okno: lista profili na ekranie głównym, z klawiatury)”. Sterowania nie wyłącza |
| `game_pick` | `kind: String` (`"folder"`/`"file"`) (okno) | `Option<GamePick>` | `ikony.resolve` + ikona 128 px + kolor z ikony; nic nie znaleziono → `notFound` „Nie znalazłem pliku gry w tym miejscu.” |
| `game_pick_discard` | `pick_id: String` | `()` | usuwa tymczasową ikonę (wyjście bez zapisu) |
| `profile_create` | `draft: ProfileDraft` | `String` (id) | `add_profile` + aktywacja + `apply_profile` |
| `profile_update` | `profile_id: String`, `patch: ProfilePatch` | `()` | `update_profile`; `online` zmieniony → `history.mark_for_resend(osoba, profil, now)` (sesje tej gry pójdą jeszcze raz, nowsze - z nowym `gra_publiczna`); kolor aktywnego profilu zmienia akcent |
| `profile_duplicate` | `profile_id: String` | `String` (id kopii) | „<nazwa> (kopia)” z kopią ikony |
| `profile_remove` | `profile_id: String` | `()` | ostatniego się nie da (`invalid`) |

Wszystkie zmiany osób i profili emitują `people` (i `settings`, gdy dotyczą aktywnego profilu).

### 6.6 Historia

| Komenda | Argumenty | Wynik | Skutek |
|---|---|---|---|
| `history_query` | `query: HistoryQuery` | `HistoryView` | najpierw `tracker.flush()` (bieżąca sesja też widać). Zakres: `day` = od dziś 00:00 (wykres godzinowy), `week` 7 dni, `month` 30 dni, `all` od 0 (wykres z 30 dni). Wykres dzienny: `week` → 7 słupków, `month`/`all` → 30. UI woła ponownie, gdy zmieni się `history.stamp` albo zapytanie |
| `history_session` | `session_id: String` | `Option<SessionDetail>` | szczegóły sesji |
| `history_session_delete` | `session_id: String` | `()` | trwająca sesja - kończona bez zapisu; usuwa z bazy (z `wyslane`, `ukryte_online`); osoba z kontem → także z serwera (w tle); toast „Sesja usunięta.” |
| `history_session_hide_online` | `session_id: String`, `hidden: bool` | `()` | `set_hidden_online` (sesja z `zmieniono_utc_s` = teraz) + wysyłka od razu (`account_upload_now`) |

### 6.7 Poziomy i osiągnięcia

| Komenda | Argumenty | Wynik | Skutek |
|---|---|---|---|
| `levels_refresh` | - | `()` | przeliczenie bez popupów (wejście na ekran Osiągnięcia - `refresh_levels(popups=False)`) |
| `levels_sample_popup` | - | `()` | przykładowy popup („km483”), po 1,4 s przykład awansu; dźwięk, jeśli włączony |

Definicje (rangi, osiągnięcia, progi) są w `statics.definitions`; postęp w wycinku `levels`.
Szlak i listy liczy UI z tych dwóch (bez komend).

### 6.8 Konto online

Wszystkie dotyczą **wybranej osoby**, chyba że podano `new_person`. Błąd `loggedOut` znaczy, że
rdzeń już wylogował osobę i wysłał toast „Logowanie konta osoby X wygasło…”.

| Komenda | Argumenty | Wynik | Skutek |
|---|---|---|---|
| `account_login` | `email: String`, `password: String`, `new_person: bool` | `AccountView` | `new_person`: konto tymczasowe, po sukcesie nowa osoba z pseudonimem i kolejnym kolorem palety. Konto już przypięte do innej osoby → `invalid` „To konto jest już przypięte do osoby X - wybierz ją.” Sukces: toast „Zalogowano jako …”, wysyłka od razu, `wl:nawigacja {to:'online', reason:'signedIn'}` |
| `account_register` | `code`, `email`, `password`, `name: String`, `new_person: bool` | `AccountView` | walidacja (`check_code`, `check_email`, `check_password`, `check_name`) → `invalid` z `fields`; wersja regulaminu z serwera; po sukcesie od razu wysyła link potwierdzający; UI pokazuje tryb `registered` |
| `account_remind_password` | `email: String` | `()` | zawsze ten sam komunikat w `account.message` („Jeśli konto z tym adresem istnieje…”) |
| `account_logout` | - | `()` | czyści stan na żywo na serwerze, token z sejfu; toast „Wylogowano.”, nawigacja do Ustawień |
| `account_delete` | `password: String` | `()` | puste hasło → `invalid`; toast „Konto usunięte.” |
| `account_change_password` | `old_password: String`, `new_password: String` | `()` | nowy token z odpowiedzi; komunikat „Hasło zmienione - inne urządzenia zostały wylogowane.” |
| `account_send_verification` | - | `String` (`"wyslano"`/`"potwierdzony"`) | `wyslano` → toast z adresem, sprawdzanie co 15 s przez 10 min (`verifyPolling`) |
| `account_check_verified` | `force: bool` | `bool` | ręcznie / po powrocie okna (najwyżej co 10 s bez `force`); potwierdzenie → toast |
| `account_set_remember` | `on: bool` | `()` | token do Menedżera poświadczeń albo tylko w pamięci |
| `account_set_upload` | `on: bool` | `()` | lokalnie |
| `account_set_visibility` | `value: String` (`prywatna`/`znajomi`/`publiczna`) | `()` | `history.mark_all_for_resend(osoba, teraz)` - wszystkie sesje osoby pójdą z nową widocznością i nowszym `zmieniono_utc_s` (serwer nadpisuje tylko nowszą wersją) |
| `account_set_live_status` | `value: String` (`wszystko`/`chodze`/`niewidoczny`) | `()` | 0.6.2, zastępuje `account_set_live`: status dla znajomych (stopka panelu znajomych i Moje konto - jedno ustawienie). `niewidoczny` → `DELETE /api/bieznia/stan`, potem żadnego `PUT stan`. konto.json: `live_status` (bez pola: `live` true → wszystko, false → niewidoczny), `live` dalej zapisywane dla 0.5.x |
| `account_set_description` | `text: String` | `()` | 0.6.2: `PATCH /api/bieznia/profil {opis}` (spacje sklejone, do 40 znaków; dłuższy → `invalid` z polem `description` „Opis może mieć najwyżej 40 znaków.” - ten sam tekst daje 400 serwera; "" czyści). Wartość wraca z `GET profil` przy odświeżeniu |
| `account_set_profile_visibility` | `value: String` (`ukryty`/`znajomi`/`publiczny`) | `()` | PATCH profilu; „Zapisano.” |
| `account_set_games_visibility` | `value: String` (`ja`/`znajomi`/`wszyscy`) | `()` | PATCH profilu |
| `account_rename` | `name: String` | `()` | `check_name` → `invalid`; PATCH profilu |
| `account_accept_terms` | - | `()` | akceptuje `termsCurrent`, wysyłka od razu (pierwsza wysyłka po akceptacji nowej wersji oznacza historię do ponownej wysyłki - docs/synchro.md „Gry na koncie”); toast „Dziękujemy - regulamin zaakceptowany.” |
| `account_upload_now` | - | `u32` (wysłane) | „Wyślij teraz” |
| `account_export_my_data` | - | `String` (ścieżka) | `dane/moje-dane-konto-YYYYmmdd-HHMMSS.json` |

### 6.9 Dane online (znajomi, ranking, osoby, profil)

| Komenda | Argumenty | Wynik | Skutek |
|---|---|---|---|
| `online_load` | `tab: String` (`friends`/`ranking`/`people`/`me`/`achievements`), `force: bool` | `()` | `friends`: statystyki tygodnia + znajomi + zaproszenia; `ranking`: ranking z bieżącym zakresem; `people`: lista z `peopleQuery`; `achievements`: `GET osiagniecia/statystyki` do `online.achievementStats` (po cichu, najwyżej raz na dobę - ekran Osiągnięcia: „Ma to X% graczy”). Bez `force` dane już w pamięci odświeżane po cichu (bez „Ładuję…”). Wyniki w `online`. UI na otwartej zakładce Znajomi (ekran aktywny, okno widoczne) woła `friends` bez `force` co 10 s - najwyżej jedno zapytanie naraz, błąd cichego odświeżenia zostawia ostatnią listę |
| `online_ranking_scope` | `scope: String` (`tydzien`/`miesiac`/`razem`/`poziom`) | `()` | zmienia zakres i ładuje |
| `online_people_search` | `search: String` (≤ 24 zn.), `page: u32` | `()` | `peopleQuery` + ładowanie |
| `online_person_open` | `user_id: String` | `OnlinePerson` | `online.person` + `wl:nawigacja {to:'onlinePerson', reason:'personOpened'}` |
| `online_invite_name` | `name: String` | `String` (stan: `oczekuje`/`przyjeta`) | toast „X jest teraz Twoim znajomym.” / „Wysłano zaproszenie do X.”; odświeża znajomych |
| `online_invite_user` | `user_id: String` | `String` | jw. |
| `online_invitation_answer` | `invitation_id: String`, `accept: bool` | `()` | toast „Gotowe.” |
| `online_invitation_withdraw` | `invitation_id: String` | `()` | jw. |
| `online_friend_remove` | `friend_id: String` | `()` | toast „Usunięto ze znajomych.” |
| `online_registration_code` | - | `RegistrationCode` | kod „Zaproś do WalkLevel” (jednorazowy, 14 dni) |

„Pokaż podział na gry” to stan lokalny UI (bez komendy).

### 6.10 Kopia danych

| Komenda | Argumenty | Wynik | Skutek |
|---|---|---|---|
| `backup_export_summary` | `choice: ExportChoice` | `ExportSummary` | liczby na żywo przy zaznaczaniu (`kopia.summarize`) |
| `backup_export` | `choice: ExportChoice` (okno zapisu, domyślna nazwa `kopia.default_name()`) | `Option<ExportSummary>` | `.bieznia` (zip, format 2); toast „Zapisano: N os., M profili, K sesji.” |
| `backup_import_open` | - (okno) | `Option<ImportPreview>` | `kopia.read` - podgląd trzymany w rdzeniu do `backup_import_apply`; zły plik → `invalid` z komunikatem `CopyError` |
| `backup_import_apply` | `choice: ImportChoice` | `ImportReport` | kopia sprzed importu w `dane/kopie/przed-importem-*`, potem `apply_import`; przeładowanie osób i historii (`reload_store`) |
| `backup_restore_latest` | - | `()` | `kopia.restore(latest_backup)` + przeładowanie; toast „Przywrócono dane sprzed importu.” |

### 6.11 Zgłoszenia problemu

| Komenda | Argumenty | Wynik | Skutek |
|---|---|---|---|
| `report_send` | `description: String`, `contact: String`, `period_s: f64` | `()` | opis < 5 znaków (po `trim`) → `invalid` z komunikatem jak `show_report_error` („Opisz, co się stało - wystarczy jedno zdanie.” / „Opis jest za krótki - dopisz jeszcze N znak/znaki/znaków, np. co poszło nie tak.”) - UI pokazuje go **pod polem**. Z adresem serwera: wysyłka; bez: paczka + `mailto:` + folder z paczką. Wynik w `report.status` |
| `report_save` | `description: String`, `contact: String`, `period_s: f64` | `String` (nazwa paczki) | zapis paczki w `dane/zgloszenia` i pokazanie w Eksploratorze; pusty opis → „(bez opisu)” |

### 6.12 Aktualizacje

| Komenda | Argumenty | Wynik | Skutek |
|---|---|---|---|
| `update_check` | `install: bool` | `()` | `false`: tylko sprawdza (start - cicho, bez komunikatu „najnowsza”); `true`: sprawdza, pobiera (postęp w `update.status`), weryfikuje SHA-256, **zatrzymuje taśmę i zapisuje stan**, uruchamia instalator `/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /CLOSEAPPLICATIONS` i po 400 ms zamyka aplikację. W wersji z kodu → `unavailable` |

### 6.13 Okna pomocnicze

| Komenda | Okno | Argumenty | Wynik | Skutek |
|---|---|---|---|---|
| `bubble_click` | przycisk | - | `()` | = `control_toggle` |
| `bubble_open` | przycisk | - | `()` | = `window_show_main` (prawy klik) |
| `bubble_drag_start` | przycisk | - | `()` | rdzeń zaczyna śledzić `WindowEvent::Moved`; po puszczeniu (`bubble_drag_end`) zapisuje `floatingPos` |
| `bubble_drag_end` | przycisk | - | `()` | zapis pozycji (piksele logiczne), sprawdzenie, czy na ekranie |
| `training_level_step`, `training_stop` | przycisk | jak w 6.4 | `()` | przyciski +, − i Stop przy kółku w trybie Trening (`bubble.training`) |
| `device_belt_stop` | przycisk | jak w 6.4 | `()` | Stop pod kółkiem poza Treningiem, gdy taśma jedzie (`bubble.stop`) |
| `popup_ready` | popup | - | `PopupsView` | stan przy starcie okna popupu |

Przesuwanie przycisku wykonuje JS przez `getCurrentWindow().startDragging()` po przesunięciu
wskaźnika o > 4 px (jak `FloatingToggle`); klik bez przesunięcia → `bubble_click`.

### 6.14 Sygnatury Rust (komplet, do skopiowania)

```rust
// src-tauri/src/komendy/*.rs - wszystkie: #[tauri::command] pub async fn …(core: State<'_, Core>, …)
app_snapshot() -> CmdResult<AppSnapshot>
ui_ready(window: String) -> CmdResult<()>
app_quit() -> CmdResult<()>
window_show_main() -> CmdResult<()>
open_folder(kind: String) -> CmdResult<()>
open_link(kind: String) -> CmdResult<()>
log_tail() -> CmdResult<Vec<String>>
diagnostics_run() -> CmdResult<String>

settings_set(key: String, value: serde_json::Value) -> CmdResult<()>
thresholds_set(which: String, kmh: f64) -> CmdResult<()>
pad_steps_count(count: u8) -> CmdResult<()>
pad_step_set(index: u8, at: Option<u8>, kmh: Option<f64>) -> CmdResult<()>
training_speed_set(index: u8, kmh: f64) -> CmdResult<()>
key_capture(role: Option<KeyRole>) -> CmdResult<()>
key_set(role: KeyRole, key: KeyCapture) -> CmdResult<KeyBinding>
pad_capture_start() -> CmdResult<()>
pad_capture_cancel() -> CmdResult<()>

control_set(on: bool, keyboard: Option<bool>) -> CmdResult<()>
control_toggle() -> CmdResult<()>
training_level_set(level: u8) -> CmdResult<()>
training_level_step(step: i8) -> CmdResult<()>
training_stop() -> CmdResult<()>
device_belt_stop() -> CmdResult<()>

device_scan() -> CmdResult<()>
device_connect(address: String, name: String) -> CmdResult<()>
device_disconnect() -> CmdResult<()>
device_forget() -> CmdResult<()>
device_status_action() -> CmdResult<()>
models_refresh() -> CmdResult<()>
models_fetch() -> CmdResult<String>
models_add_file() -> CmdResult<Option<String>>
models_remove(id: String) -> CmdResult<()>

wizard_seed(mode: String) -> CmdResult<WizardSeed>
name_random() -> CmdResult<String>
wizard_finish(result: WizardResult) -> CmdResult<()>
person_switch(person_id: String) -> CmdResult<()>
person_update(person_id: String, patch: PersonPatch) -> CmdResult<()>
person_remove(person_id: String) -> CmdResult<()>
profile_switch(profile_id: String, from: Option<String>, keyboard: Option<bool>) -> CmdResult<()>
game_pick(kind: String) -> CmdResult<Option<GamePick>>
game_pick_discard(pick_id: String) -> CmdResult<()>
profile_create(draft: ProfileDraft) -> CmdResult<String>
profile_update(profile_id: String, patch: ProfilePatch) -> CmdResult<()>
profile_duplicate(profile_id: String) -> CmdResult<String>
profile_remove(profile_id: String) -> CmdResult<()>

history_query(query: HistoryQuery) -> CmdResult<HistoryView>
history_session(session_id: String) -> CmdResult<Option<SessionDetail>>
history_session_delete(session_id: String) -> CmdResult<()>
history_session_hide_online(session_id: String, hidden: bool) -> CmdResult<()>

levels_refresh() -> CmdResult<()>
levels_sample_popup() -> CmdResult<()>

account_login(email: String, password: String, new_person: bool) -> CmdResult<AccountView>
account_register(code: String, email: String, password: String, name: String, new_person: bool) -> CmdResult<AccountView>
account_remind_password(email: String) -> CmdResult<()>
account_logout() -> CmdResult<()>
account_delete(password: String) -> CmdResult<()>
account_change_password(old_password: String, new_password: String) -> CmdResult<()>
account_send_verification() -> CmdResult<String>
account_check_verified(force: bool) -> CmdResult<bool>
account_set_remember(on: bool) -> CmdResult<()>
account_set_upload(on: bool) -> CmdResult<()>
account_set_visibility(value: String) -> CmdResult<()>
account_set_live_status(value: String) -> CmdResult<()>
account_set_description(text: String) -> CmdResult<()>
account_set_profile_visibility(value: String) -> CmdResult<()>
account_set_games_visibility(value: String) -> CmdResult<()>
account_rename(name: String) -> CmdResult<()>
account_accept_terms() -> CmdResult<()>
account_upload_now() -> CmdResult<u32>
account_export_my_data() -> CmdResult<String>

online_load(tab: String, force: bool) -> CmdResult<()>
online_ranking_scope(scope: String) -> CmdResult<()>
online_people_search(search: String, page: u32) -> CmdResult<()>
online_person_open(user_id: String) -> CmdResult<OnlinePerson>
online_invite_name(name: String) -> CmdResult<String>
online_invite_user(user_id: String) -> CmdResult<String>
online_invitation_answer(invitation_id: String, accept: bool) -> CmdResult<()>
online_invitation_withdraw(invitation_id: String) -> CmdResult<()>
online_friend_remove(friend_id: String) -> CmdResult<()>
online_registration_code() -> CmdResult<RegistrationCode>

backup_export_summary(choice: ExportChoice) -> CmdResult<ExportSummary>
backup_export(choice: ExportChoice) -> CmdResult<Option<ExportSummary>>
backup_import_open() -> CmdResult<Option<ImportPreview>>
backup_import_apply(choice: ImportChoice) -> CmdResult<ImportReport>
backup_restore_latest() -> CmdResult<()>

report_send(description: String, contact: String, period_s: f64) -> CmdResult<()>
report_save(description: String, contact: String, period_s: f64) -> CmdResult<String>

update_check(install: bool) -> CmdResult<()>

bubble_click() -> CmdResult<()>
bubble_open() -> CmdResult<()>
bubble_drag_start() -> CmdResult<()>
bubble_drag_end() -> CmdResult<()>
popup_ready() -> CmdResult<PopupsView>
```

Argumenty `account_change_password` w JS: `{ oldPassword, newPassword }`.

---

## 7. Okna

Trzy wejścia Vite (`build.rollupOptions.input`): `index.html` (okno główne), `popup.html`,
`przycisk.html`. Okna `popup` i `przycisk` mają własne małe komponenty i nie ładują ekranów.

### 7.1 Okno główne (`main`)

- Tworzone w `tauri.conf.json` z `visible: false`; rdzeń pokazuje je po `ui_ready('main')`
  w miejscu z `window_geometry` (jeśli całe mieści się na którymś ekranie), inaczej na środku
  ekranu głównego w rozmiarze 740 × 890 (nie większe niż ekran - 40/60 px). Minimalny rozmiar
  jak w 0.5 (okno.py `Window`).
- Zwykła ramka Windows; tło: Mica (`windowEffects: { effects: ["mica"] }`, okno przezroczyste)
  z kolorem tła motywu pod spodem, gdy efektu brak (Windows 10).
- `alwaysOnTop` z ustawienia osoby (zmiana na żywo: `WindowRequest::MainAlwaysOnTop`).
- Zamknięcie (krzyżyk): `CloseRequested` → `prevent_close()` → `app_quit` w rdzeniu (stop
  taśmy!) → `app.exit(0)`. To samo przy `WM_QUERYENDSESSION`/zamknięciu przez instalator.
- `geometry` zapamiętywana przy zamknięciu (nie gdy zminimalizowane/zmaksymalizowane albo poza
  ekranem), w pikselach **logicznych** (tak zapisywało Qt) - `outer_position()/inner_size()`
  podzielone przez `scale_factor()`.
- Klawisze przeglądarki WebView2 wyłączone w wersji wydanej (F5, Ctrl+R, Ctrl+P, Ctrl+F,
  F12, Ctrl+Shift+I, menu kontekstowe): `ICoreWebView2Settings3::put_AreBrowserAcceleratorKeysEnabled(false)`
  przez `with_webview`, plus `preventDefault` na `contextmenu`. Bez tego F5 w trakcie
  przypisywania klawisza przeładowałby stronę.
- Przypisywanie klawisza: UI woła `key_capture(role)`, łapie `keydown` (z `preventDefault`,
  także Alt/F10/Tab), wysyła `key_set(role, capture)`, Escape = `key_capture(null)` bez zmiany.

### 7.2 Popup osiągnięć (`popup`)

Odpowiednik `bieznia/gui/popupy.py`. Jedno okno na stos do 3 popupów.

- Tworzone przez rdzeń dopiero przy pierwszym powiadomieniu (każde okno to osobny proces
  WebView2; potem zostaje ukryte do następnego) - strona pobiera stan przez `popup_ready`, więc
  zdarzenia sprzed jej załadowania nie giną (`WebviewWindowBuilder::new(app, "popup", WebviewUrl::App("popup.html".into()))`):
  `transparent(true)`, `decorations(false)`, `shadow(false)`, `always_on_top(true)`,
  `skip_taskbar(true)`, `resizable(false)`, `focused(false)`, `visible(false)`.
- Po utworzeniu: `set_ignore_cursor_events(true)` (klik przechodzi do gry) i przez `hwnd()`
  styl `WS_EX_TRANSPARENT | WS_EX_LAYERED | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW` + `HWND_TOPMOST`
  (jak `winapi.make_overlay`). Pokazanie **wyłącznie** `ShowWindow(hwnd, SW_SHOWNOACTIVATE)`
  (Tauri `show()` może aktywować okno i zabrać fokus grze).
- Rozmiar stały: 380 × (3 × 96 + 2 × 10) = 380 × 308 px logicznych. Miejsce: prawy dolny róg
  obszaru roboczego (bez paska zadań) **monitora, na którym jest okno na pierwszym planie**
  (`GetForegroundWindow` → `MonitorFromWindow` → `GetMonitorInfoW.rcWork`), margines 16 px;
  liczone przy każdym nowym popupie (gra mogła się przenieść). Rdzeń wysyła
  `WindowRequest::PopupPlace` w pikselach fizycznych (rozmiar × skala monitora).
- Kolejka i czasy w rdzeniu: najwyżej 3 widoczne, nowsze na dole, reszta czeka; każdy
  widoczny 6000 ms, potem `leaving: true` na 220 ms (UI animuje zanikanie), potem wypada.
  Pusta lista → `PopupVisible(false)`.
- Treść (`PopupItem`) układa rdzeń jak `achievement_popup` / `level_popup` /
  `show_history_summary` / zbiorczy „+N kolejnych” (> 4 nowe naraz). Tło popupu zawsze ciemne
  `#15171d` (czytelne nad każdą grą), niezależnie od motywu.
- Dźwięki (`osiagniecie.wav`, `awans.wav` z zasobów) odtwarza **rdzeń** (`PlaySoundW`
  z `SND_ASYNC | SND_FILENAME`), nie okno - działa też, gdy okno popupu jest jeszcze ukryte.
- Popupy włączone/wyłączone: `settings.achievementPopups`; dźwięk: `achievementSound`.

### 7.3 Pływający przycisk (`przycisk`)

Odpowiednik `FloatingToggle`.

- 56 × 56 px logicznych (tryb Trening: 88 × 84, `WindowRequest::BubbleSize(training)` przy każdej
  zmianie trybu i po utworzeniu okna; kółko zostaje w lewym górnym rogu, okno rośnie w prawo
  i w dół, a gdy wyszłoby poza obszar roboczy - przesuwa się do środka bez zapisu pozycji), `transparent`, bez ramki i cienia, `always_on_top`, `skip_taskbar`,
  `focused(false)` + `WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW` przez `hwnd()` (klik nie zabiera
  fokusu grze), **bez** `ignore_cursor_events` (ma być klikalny).
- Okno istnieje tylko, gdy `settings.floatingButton` (`WindowRequest::BubbleVisible`): włączenie
  je tworzy (stan startowy z `app_snapshot`), wyłączenie zamyka. Pokazywany w miejscu
  `floating_pos` (jeśli punkt + 28 px jest na którymś ekranie), inaczej przy prawej krawędzi
  ekranu głównego, w połowie wysokości, 24 px od brzegu.
- Rysowanie (SVG w `przycisk.svelte`): koło; włączone - wypełnienie akcentem (bieg: kolor biegu),
  ikona postaci; gdy taśma jedzie (`live && speedKmh > 0,05`, także przy wyłączonym
  sterowaniu, 0.6.1) - pod ikoną prędkość („2,5”); wyłączone - tło
  powierzchni, ramka kontrolki. Dane: `wl:telemetria` (`enabled`, `mode`, `speedKmh`, `live`)
  + wycinek `bubble` (motyw, akcent, widoczność, tryb Trening i poziom).
- Tryb Trening: obok kółka kolumna 28 px (+ / numer poziomu / −, jedna ramka `--control`), pod
  spodem Stop 24 px (wariant „stop”: ramka i tekst `--danger`). + → `training_level_step(1)`,
  − → `training_level_step(-1)` (wyłączone na poziomie 4 / 1), Stop → `training_stop`. Klik
  nie aktywuje okna (ten sam `WS_EX_NOACTIVATE`).
- Poza Treningiem (0.6.1): gdy taśma jedzie (`bubble.stop`: dane na żywo i > 0,05 km/h przez
  1 s - histereza w pętli, okno nie miga) i bieżnia przyjmuje stop (`caps.control`), pod kółkiem
  ten sam Stop, okno 56 × 84 (`okna.rs` `BubbleLayout::Stop`, `BUBBLE_STOP_H`); Stop →
  `device_belt_stop` (włączone sterowanie się wyłącza, taśma staje). Rozmiar okna zmienia się
  tylko przy zmianie układu (`sync_bubble_size`).
- Lewy klik → `bubble_click`; prawy → `bubble_open`; przeciąganie → `startDragging()`
  i `bubble_drag_start/end`. Podpowiedź (title): „Sterowanie: włączone/wyłączone · Kliknij -
  wyłącz/włącz · Prawy przycisk - okno · Przeciągnij - przesuń”.

### 7.4 Ikony profili (protokół `ikona`)

`src-tauri` rejestruje `register_uri_scheme_protocol("ikona", …)`, który serwuje **tylko**
pliki `*.png` z `dane/ikony` (nazwa bez `/`, `\`, `..`). Na Windows adres ma postać
`http://ikona.localhost/<plik>`; rdzeń dokleja `?v=<mtime>`, żeby po zmianie ikony nie było
starego obrazka z pamięci WebView2. `IconRef.url` zawsze przychodzi gotowy z rdzenia.

### 7.5 Uprawnienia (capabilities)

- `capabilities/main.json` (okno `main`): `core:default`, `core:window:allow-start-dragging`
  nie jest potrzebne; komendy aplikacji - wszystkie.
- `capabilities/przycisk.json` (okno `przycisk`): `core:event:default`,
  `core:window:allow-start-dragging`; komendy `bubble_*`.
- `capabilities/popup.json` (okno `popup`): `core:event:default`; komenda `popup_ready`.
- Ograniczenie komend do okien: `tauri_build::Attributes::app_manifest(AppManifest::new().commands(&[…]))`
  w `build.rs` i uprawnienia `allow-<komenda>` w plikach capability.

---

## 8. Pliki danych - zgodność z 0.5.1/0.5.2

Folder danych wersji zainstalowanej: **`%LOCALAPPDATA%\Bieznia`** (`paths.data_dir()`); w dev
i testach - folder ze zmiennej `WALKLEVEL_DANE` (CLAUDE.md), żeby nie ruszać prawdziwych danych. 0.6.0 czyta i pisze **te same pliki w tych samych
formatach**, a nieznane klucze zachowuje przy zapisie (żeby późniejsza wersja nic nie
gubiła). WebView2 trzyma swoje dane osobno (`%LOCALAPPDATA%\Bieznia\webview` - ustawić
`data_directory` okien), żeby nie mieszały się z `dane/` i nie trafiały do kopii.

### 8.1 `dane/osoby.json` (format `"wersja": 1`)

Zapis: JSON z `indent=1`, `ensure_ascii=False` (UTF-8 bez BOM), **atomowo**: `osoby.tmp` →
`rename` nad `osoby.json`. Uszkodzony plik → kopia `dane/kopie/uszkodzony-<unix>.json`
i konfigurator od nowa (jak 0.5). Brak pliku → pierwsze uruchomienie (konfigurator `first`),
ewentualnie z ustawieniami 0.2 z `dane/ustawienia.json` (tylko czytane, nigdy nie zmieniane).

```json
{
 "wersja": 1,
 "aktywna": "a1b2c3d4e5",
 "pytaj_przy_starcie": true,
 "wspolne": { "device_address": "…", "device_name": "URTM054", "belt_control": true, "…": "…" },
 "osoby": [
  {
   "id": "a1b2c3d4e5", "nazwa": "Olek", "kolor": "Mięta",
   "wyglad": { "theme": "system", "large_text": false, "…": "…" },
   "aktywny_profil": "f6e5d4c3b2",
   "profile": [
    {
     "id": "f6e5d4c3b2", "nazwa": "Domyślny", "kolor": "Mięta", "kolor_z_ikony": null,
     "glif": "", "ikona": null, "exe": null, "gra_online": true,
     "ustawienia": { "direction": "keyboard", "walk_key": {"scan": 17, "extended": false, "name": "W", "vk": 87}, "…": "…" }
    }
   ]
  }
 ]
}
```

- Id osoby i profilu: 10 znaków hex (`uuid4().hex[:10]`). Nieznany `aktywna`/`aktywny_profil`
  → pierwszy z listy. Pusta lista profili → profil „Domyślny” z glifem ``.
- `kolor` to nazwa z palety; nieznana → „Mięta”. `glif` - znak Segoe Fluent albo `null`;
  `ikona` - nazwa pliku w `dane/ikony` (zwykle `<id profilu>.png`) albo `null` (gdy jest
  `ikona`, `glif` = `null`).
- Profil gry ma ustawienia tylko z listy profilu (pusty słownik = same domyślne).
- **Kolejność ważności przy odczycie** (`Store.flat`): domyślne z `config.toml` <
  `{show_person_switch: true, show_profile_switch: true}` < `wspolne` < `wyglad` osoby <
  `ustawienia` profilu. Zapis klucza idzie tylko tam, gdzie klucz należy (tabela poniżej).

| Klucz w pliku | Pole TS | Część | Domyślnie (config.toml / kod) | Walidacja (`normalize`) |
|---|---|---|---|---|
| `device_address` | `deviceAddress` | wspolne | `""` | - |
| `device_name` | `deviceName` | wspolne | `""` | - |
| `belt_control` | `beltControl` | wspolne | `true` | bool |
| `quick_stop` | `quickStop` | wspolne | `true` | bool |
| `toggle_key` | `toggleKey` | wspolne | F8 `{scan: 0x42, extended: false, name: "F8", vk: 0x77}` | `Key.from_dict`; `vk == 0` (zapis sprzed kodów wirtualnych) → F8 |
| `faster_key` | `fasterKey` | wspolne | brak klucza (0.5.x i nowe dane) → `[hotkeys] sim_faster` z config.toml (F1-F12), potem F7 | `Key::speed_from_value`: błędny zapis albo `vk == 0` → jak brak klucza. Nowe w 0.6.0 (0.5.x pomija nieznany klucz) |
| `slower_key` | `slowerKey` | wspolne | jw., `sim_slower`, potem F6 | jw. |
| `toggle_pad` | `togglePad` | wspolne | `""` | string, inaczej `""` |
| `simulator` | `simulator` | wspolne | `false` | bool |
| `record` | `record` | wspolne | `false` | bool |
| `detailed_logs` | `detailedLogs` | wspolne | `true` | bool |
| `last_version` | - (rdzeń) | wspolne | `""` | zapisywana przy starcie; inna niż bieżąca → log „Zaktualizowano z X do Y”, `app.updatedFrom` |
| `window_geometry` | - (rdzeń) | wspolne | `null` | `[x, y, w, h]` liczby całkowite, piksele logiczne |
| `theme` | `theme` | wyglad | `"system"` | `system`/`light`/`dark` |
| `large_text` | `largeText` | wyglad | `false` | bool |
| `always_on_top` | `alwaysOnTop` | wyglad | `false` | bool |
| `floating_button` | `floatingButton` | wyglad | `false` | bool |
| `floating_pos` | - (rdzeń) | wyglad | `null` | `[x, y]`, piksele logiczne |
| `show_person_switch` | `showPersonSwitch` | wyglad | `true` | bool |
| `show_profile_switch` | `showProfileSwitch` | wyglad | `true` | bool |
| `achievement_popups` | `achievementPopups` | wyglad (**nowe**) | `true` | bool |
| `achievement_sound` | `achievementSound` | wyglad (**nowe**) | `false` | bool |
| `friends_notify` | `friendsNotify` | wyglad (0.6.2) | `false` | bool - komunikat w oknie, gdy znajomy zacznie chodzić |
| `friends_panel` | `friendsPanel` | wspolne (0.6.2) | `true` | bool - panel znajomych z boku okna |
| `friends_panel_side` | `friendsPanelSide` | wspolne (0.6.2) | `"right"` | `right`/`left` |
| `friends_panel_collapsed` | `friendsPanelCollapsed` | wspolne (0.6.2) | `false` | bool - pasek 64 px |
| `direction` | `direction` | profil | `"keyboard"` | 4 wartości, inaczej `keyboard` |
| `output` | `output` | profil | `"dry"` | `dry`/`keys`, inaczej `dry` |
| `walk_key` | `walkKey` | profil | W `{scan: 0x11, extended: false, name: "W", vk: 0x57}` | `Key.from_dict` (scan 1-255) |
| `run_key` | `runKey` | profil | Shift `{scan: 0x2A, extended: false, name: "Shift", vk: 0xA0}` | jw. |
| `stand_below` | `standBelow` | profil | `0.8` | float |
| `run_on` | `runOn` | profil | `6.3` | float |
| `run_off` | `runOff` | profil | `5.7` | float (zapisywane `run_on - 0.6`) |
| `idle_speed` | `idleSpeed` | profil | `1.0` | float |
| `walk_speed` | `walkSpeed` | profil | `1.5` | float |
| `run_speed` | `runSpeed` | profil | `2.5` | float |
| `key_hold_s` | `keyHoldS` | profil | `0` | 0, 1, 2 albo 3 s (Klawisze: trzymanie przed przyspieszeniem, chód i osobno bieg od swojego wciśnięcia; puszczenie od razu; klawisze w grze bez opóźnienia) |
| `key_idle` | `keyIdle` | profil | `"slow"` | `slow` / `stop` (Klawisze, bez klawisza: tempo `idle_speed` albo stop po 0,4 s; W budzi taśmę `SetSpeed{start_if_stopped}`; włączenie sterowania nie uruchamia taśmy, wyłączenie ją zatrzymuje) |
| `pad_steps` | `padSteps` | profil | `[[0,1.0],[25,1.5],[50,2.5],[100,4.0]]` | 2-5 par, pierwszy `at = 0`, rosnąco co ≥ 5, ≤ 100, prędkość 0,5-12; inaczej 4 domyślne |
| `pad_idle` | `padIdle` | profil | `"stop"` | `stop`/`slow`/`zero`, inaczej `stop` |
| `training_levels` | `trainingLevels` | profil | `[1.5, 2.5, 3.5, 4.5]` | dokładnie 4, każdy 0,5-20, zaokr. 0,1; inaczej domyślne |
| `training_level` | `trainingLevel` | profil | `0` | int 0-3, inaczej 0 |

- Klucz `toggle_key`/`faster_key`/`slower_key`/`walk_key`/`run_key`: `{"scan": int, "extended": bool, "name": str,
  "vk": int}` + `"mods": ["ctrl","alt","shift"]` tylko gdy niepuste (kolejność ctrl, alt,
  shift, win). `label` NIE jest zapisywany.
- Progi `MappingConfig` muszą spełniać `0 ≤ stand_below < run_off ≤ run_on ≤ max_speed`;
  przy błędzie 0.5 zostawia poprzednie mapowanie (log „Progi: …”) - tak samo w 0.6.
- `dane/ikony/<plik>.png` - ikony gier 128 px; tymczasowe `nowa-<8 hex>.png` (porzucone można
  usuwać przy starcie, jeśli starsze niż doba).

### 8.2 `dane/historia.db` (SQLite, `PRAGMA user_version = 5`, WAL)

Otwierana raz, `journal_mode=WAL` (pliki `historia.db-wal`, `-shm`). Brak dostępu → aplikacja
działa bez historii (`history.available = false`), jak 0.5.

```sql
CREATE TABLE IF NOT EXISTS sesje (
    id TEXT PRIMARY KEY,                       -- UUIDv7, 32 znaki hex bez kresek
    osoba TEXT NOT NULL, osoba_nazwa TEXT NOT NULL,
    profil TEXT NOT NULL, profil_nazwa TEXT NOT NULL,
    start_utc_s REAL NOT NULL, koniec_utc_s REAL NOT NULL,
    kroki INTEGER NOT NULL DEFAULT 0,
    dystans_km REAL NOT NULL DEFAULT 0,
    ruch_s REAL NOT NULL DEFAULT 0,
    bieznia_model TEXT NOT NULL DEFAULT '',
    sterownik TEXT NOT NULL DEFAULT '',
    bieznia_profil TEXT NOT NULL DEFAULT '',
    firmware TEXT NOT NULL DEFAULT '',
    wersja_aplikacji TEXT NOT NULL DEFAULT '',
    zrodlo TEXT NOT NULL DEFAULT 'bieznia',     -- bieznia | symulator
    widocznosc TEXT NOT NULL DEFAULT 'prywatna',
    strefa_min INTEGER NOT NULL DEFAULT 0,      -- przesunięcie strefy przy starcie (min od UTC)
    zmieniono_utc_s REAL NOT NULL DEFAULT 0,
    tryb TEXT NOT NULL DEFAULT 'gra'            -- gra | trening
);
CREATE INDEX IF NOT EXISTS sesje_osoba_start ON sesje (osoba, start_utc_s);
CREATE TABLE IF NOT EXISTS wyslane (id TEXT PRIMARY KEY, zmieniono_utc_s REAL NOT NULL);
CREATE TABLE IF NOT EXISTS odrzucone (id TEXT PRIMARY KEY, zmieniono_utc_s REAL NOT NULL, powod TEXT NOT NULL, ponow_utc_s REAL NOT NULL);
CREATE TABLE IF NOT EXISTS ukryte_online (id TEXT PRIMARY KEY);
CREATE TABLE IF NOT EXISTS osiagniecia (osoba TEXT NOT NULL, id TEXT NOT NULL, zdobyte_utc_s REAL NOT NULL, wersja INTEGER NOT NULL, PRIMARY KEY (osoba, id));
CREATE TABLE IF NOT EXISTS poziom_osoby (osoba TEXT PRIMARY KEY, poziom INTEGER NOT NULL);
```

- **Kolejność kolumn jest kontraktem** (INSERT pozycyjny w 0.5 i w kopii `historia.jsonl`).
  `model::Session` ma pola w tej kolejności: `id, person, person_name, profile, profile_name,
  start, end, steps, distance_km, active_s, device_model, driver, device_profile, firmware,
  app_version, source, visibility, tz_offset_min (strefa_min), updated (zmieniono_utc_s), mode`.
- Migracja (`historia.migrate`) dla `user_version < 5`: jeśli tabela `sesje` istnieje -
  `start`/`koniec` → `ALTER TABLE … RENAME COLUMN` na `start_utc_s`/`koniec_utc_s`, brakujące
  kolumny `ALTER TABLE ADD COLUMN` z definicją jak wyżej, dla wersji < 2
  `UPDATE sesje SET zmieniono_utc_s = koniec_utc_s WHERE zmieniono_utc_s = 0`; potem cały
  skrypt `CREATE … IF NOT EXISTS` i `PRAGMA user_version = 5`, w jednej transakcji.
  `user_version > 5` (nowsza wersja) - nie dotykamy schematu.
- Zapis sesji: `INSERT OR REPLACE` z `zmieniono_utc_s = now`; sesja trafia do bazy, gdy
  trwa ≥ 10 s albo ma kroki; zapis co 60 s i przy zakończeniu; koniec sesji po 120 s bez
  ruchu, przy zmianie osoby/profilu/trybu (gra/trening)/bieżni/symulatora i przy zamknięciu.
- `unsent(osoba)`: sesje osoby z `zrodlo = 'bieznia'`, których nie ma w `wyslane` z
  `zmieniono_utc_s >=` sesji, i nie są w `odrzucone` z terminem `ponow_utc_s > now`
  (ponowienie po 24 h). `forget_sent()` czyści `wyslane` i `odrzucone`.
  `mark_for_resend(osoba, profil?, now)`: `zmieniono_utc_s = now` dla sesji osoby z bieżni
  z niepustym profilem (opcjonalnie jednego profilu) - wracają do `unsent` w nowszej wersji,
  którą serwer nadpisze (docs/synchro.md „Gry na koncie”).
- Osiągnięć raz zapisanych w `osiagniecia` nie usuwamy (poza „wyczyść historię osoby”).
  `poziom_osoby` - ostatni pokazany poziom (brak = pierwsze liczenie: jeden popup
  podsumowujący zamiast stosu).
- Usunięta osoba: `UPDATE sesje SET osoba_nazwa = 'Usunięte osoby' WHERE osoba = ?`.

### 8.3 `dane/konto.json` (format `"wersja": 2`)

```json
{
 "wersja": 2,
 "konta": [
  {
   "user_id": "…", "name": "Olek", "email": "olek@example.com", "verified": true,
   "person": "a1b2c3d4e5", "upload": true, "visibility": "znajomi", "live": false,
   "profile_visibility": "znajomi", "terms": "2026-11-25", "remember": true,
   "terms_current": "2026-11-25", "games_visibility": "znajomi",
   "sync_cursor": "…", "terms_games": "2026-11-26", "resent_terms": "2026-11-26"
  }
 ]
}
```

- Zapis `indent=1`, UTF-8, atomowo (`konto.tmp` → `konto.json`). Tylko zalogowane konta
  z przypiętą osobą. **Bez tokenu i hasła.**
- Format 1 (starsza wersja z jednym kontem): plik to sam obiekt z polami jak wyżej → jedno
  konto. Nieznane pola pomijamy przy odczycie; złe `visibility` → `znajomi`, złe
  `profile_visibility` → `znajomi`.
- `sync_cursor` (0.6.0), `terms_games` i `resent_terms` (0.6.1) zapisywane tylko niepuste;
  0.5.x je pomija. `terms_games` - `poziomy` z `GET regulamin` (od tej wersji serwer zapisuje pola
  gier; pusty - jeszcze nie); `resent_terms` - wersja, po której akceptacji historia poszła
  jeszcze raz z grami. Zmiana konta (inny `user_id`) czyści kursor i `resent_terms`.
- Konto z `remember = true`, a bez tokenu w sejfie (inny użytkownik Windows, wyczyszczony
  sejf) → traktowane jak wylogowane (`_forget`), osoba zostaje.

### 8.4 Menedżer poświadczeń Windows (token konta)

| Co | Wartość |
|---|---|
| Typ | `CRED_TYPE_GENERIC` (1) |
| Cel (TargetName) dla osoby | **`Bieznia/konto/<id osoby>`** |
| Cel starej wersji z jednym kontem | **`Bieznia/konto`** - przy wczytaniu konta z `remember`, gdy cel osoby jest pusty, token jest przenoszony do `Bieznia/konto/<id osoby>`, a stary cel usuwany |
| Persist | `CRED_PERSIST_LOCAL_MACHINE` (2) |
| UserName | `bieznia` |
| Comment | `WalkLevel - logowanie do konta online` |
| CredentialBlob | token jako **UTF-8** (nie UTF-16), najwyżej 2560 bajtów |
| API | `CredWriteW` / `CredReadW` / `CredDeleteW` / `CredFree` (feature `Win32_Security_Credentials`) |

Token nigdy nie trafia do plików, kopii danych ani logów; w żądaniach tylko w nagłówku
`Authorization`. `remember = false` → token tylko w pamięci, cel w sejfie usunięty.

### 8.5 Pozostałe pliki

| Plik | Format / zasada |
|---|---|
| `config.toml` (zasób aplikacji) + `config.local.toml` (folder danych) | TOML, scalanie słowników (lokalny wygrywa). Sekcje: `[mapping]` (progi, `max_speed`, `stale_after_s`, hamowanie), `[hotkeys]` (`toggle`, `sim_slower`, `sim_faster` - domyślne klawisze wolniej/szybciej, gdy osoby.json nie ma `slower_key`/`faster_key`), `[ble]`, `[ui]`, `[zgloszenia] adres`, `[konto] adres, regulamin`. 0.6.0 osadza ten sam `config.toml` |
| `dane/ustawienia.json` | ustawienia 0.2 - tylko odczyt (migracja w konfiguratorze); kopia do `dane/kopie/ustawienia-0.2.json` |
| `dane/stan.json` | `{"enabled": bool}` po każdej zmianie sterowania; przy starcie używany tylko, gdy zmienna środowiskowa `BIEZNIA_RESUME=1` (zwykły start zawsze wyłączony) |
| `dane/profile/*.json` | profile modeli bieżni dodane/pobrane (`"format": 1`, `id`, `name`, `driver` = `kingsmith`/`ftms`, `match[]`, `capabilities{}`, `options{}`); wbudowane są w zasobach (`sterowniki/profile/*.json` + `indeks.json`) |
| `dane/kopie/przed-importem-YYYYmmdd-HHMMSS/` | `osoby.json`, `historia.db` (kopia przez SQLite backup API), `ikony/` |
| `dane/zgloszenia/zgloszenie-YYYYmmdd-HHMMSS.zip` | `zgloszenie.json` (`opis`, `kontakt`, `wersja`, `windows`, `python` → w 0.6 `rdzen`, `czas`, `okres_logow_h`), `podsumowanie.txt`, `logs/*.log` (końcówki z okresu, adresy MAC zamaskowane poza adresem bieżni), ≤ 2 MB |
| `dane/moje-dane-konto-*.json` | eksport danych z serwera |
| `*.bieznia` (kopia) | zip: `manifest.json` (`format: "bieznia-kopia"`, `wersja: 2`, `aplikacja`, `utworzono`, `zawartosc{ustawienia, wyniki, logi}`), `osoby.json` (`{"osoby": [...]}` jak w 8.1), `historia.jsonl` (wiersz = obiekt z kolumnami `sesje`), `ikony/*.png`, `logi/*.log`; odczyt także wersji 1 (`start`/`koniec`) |
| `logs/bieznia.log` | rotacja 2 MB × 3; linia: `YYYY-mm-dd HH:MM:SS LEVEL   nazwa: treść` (format `"%(asctime)s %(levelname)-7s %(name)s: %(message)s"`). **Data na początku linii jest wymagana** - zgłoszenie wybiera okres po regexie `^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}` |
| `logs/bluetooth.log` | szczegółowe logi BLE (DEBUG), `YYYY-mm-dd HH:MM:SS.mmm treść`, rotacja jw.; wyłączone szczegóły = tylko INFO |
| `logs/diagnostyka-*.txt` | raport diagnostyczny Bluetooth |
| `nagrania/YYYYmmdd-HHMMSS.jsonl` | `{"t": s od początku, "speed": km/h, "steps": int|null}` na linię |

### 8.6 Aktualizacja z 0.5.1 do 0.6.0

Żeby przycisk „Aktualizuj” w 0.5.1 zainstalował 0.6.0 i nic nie zginęło:

- Wydanie na GitHubie z plikiem **`Bieznia-Setup-0.6.0.exe`** (regex 0.5:
  `^Bieznia-Setup-(\d+\.\d+\.\d+)\.exe$`) i sumą `sha256:` w polu `digest` zasobu.
- Instalator przyjmuje `/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /CLOSEAPPLICATIONS` - czyli
  **dalej Inno Setup** z tym samym `AppId={{6F4C2D8E-3B1A-4E5F-9C7D-2A8B1E0F4D63}`,
  `DefaultDirName={localappdata}\Programs\Bieznia`, plikiem **`Bieznia.exe`** (exe Tauri
  przemianowany w `productName`/`mainBinaryName`), skrótami „WalkLevel”, usuwaniem
  `{app}\_internal` (pliki PyInstallera 0.5) i uruchomieniem aplikacji po instalacji.
  Instalator NSIS/MSI z Tauri nie przyjmie tych przełączników ani AppId.
- 0.6.0 wymaga WebView2 Runtime (Windows 11 ma; Windows 10 zwykle ma) - instalator
  powinien zawierać bootstrapper WebView2 albo sprawdzać jego obecność.
- Dane w `%LOCALAPPDATA%\Bieznia` nie są ruszane przez instalator; 0.6.0 czyta je jak wyżej.
- 0.5.1 przed uruchomieniem instalatora sam zatrzymuje taśmę i się zamyka - 0.6.0 robi to
  samo przy swojej aktualizacji (rozdz. 6.12).

---

## 9. Teksty liczone przez rdzeń

Brzmienie: wyłącznie `docs/teksty.md` (kolumna „Nowy tekst”). Rdzeń bierze teksty po kluczu z
tego samego słownika co interfejs (`walklevel_core::teksty::t(klucz, zmienne)` czyta
`src/lib/teksty-slownik.ts`, generowany przez `npm run teksty`); test
`teksty::tests::every_used_key_exists` pilnuje, że każdy użyty w Rust klucz istnieje. Tu jest
tylko logika: który klucz kiedy. Liczby: `{kmh}` z jednym miejscem po przecinku („2,5”, „6,0”),
tysiące z wąską twardą spacją. UI tylko wyświetla gotowe teksty.

### 9.1 Karta statusu bieżni (`device.status`)

Tytuł, podtytuł i napis przycisku: `glowny.status.<stan>.{tytul,podtytul,akcja}`.

| Stan | tone | busy | `<stan>` | action | primary |
|---|---|---|---|---|---|
| symulator włączony | accent | nie | `symulator` | simulatorOff | nie |
| streaming, dane świeże | accent | nie | `polaczono` | disconnect | nie |
| streaming, brak świeżych danych | run | tak | `czekam` (przycisk jak `polaczono`) | disconnect | nie |
| searching, bieżnia zapamiętana | accent | tak | `szukanie` | cancel | nie |
| searching, bez zapamiętanej (skan) | accent | tak | `skanowanie` | cancel (wynik skanu nie otwiera listy) | nie |
| searching w trakcie raportu diagnostycznego | accent | tak | tytuł `dziennik.raport_trwa` | cancel | nie |
| connecting / connected | accent | tak | `laczenie` | cancel | nie |
| retrying | run | tak | `ponawiam`, podtytuł: powód `ble.powod.*` | disconnect | nie |
| bluetoothOff | danger | nie | `bt_wylaczony` | retry | tak |
| unsupported | danger | nie | `nieobslugiwane` | scan | tak |
| idle, bieżnia zapamiętana | muted | nie | `rozlaczona` | retry (łączy bez skanu) | tak |
| idle, bez zapamiętanej | muted | nie | `brak` | scan | tak |

`{model}`: `device.model` (prawdziwa bieżnia) albo `model_name(deviceName)` albo „bieżnia”.

### 9.2 Podpis wskaźnika (`Telemetry.caption`, `mode`, `active`)

Wyłączone sterowanie: `glowny.wskaznik.wylaczone`.

| Tryb | Warunek | mode | active | caption |
|---|---|---|---|---|
| treadmill | brak świeżych danych | offline | -1 | `glowny.wskaznik.brak_danych` (link ≠ streaming) / `glowny.wskaznik.czekam` |
| gamepad | włączone | idle/walk/run (stopień 0 / ostatni / pozostałe) | indeks stopnia | `glowny.wskaznik.pad.brak` albo `glowny.wskaznik.pad.stan`; `{tasma}` = `glowny.wskaznik.tasma_stoi` / `glowny.wskaznik.tasma_kmh` |
| training | włączone | walk (poziom 1-2) / run (3-4) | poziom | `glowny.wskaznik.trening` |
| keyboard | włączone | idle/walk/run (trzymane klawisze) | 0/1/2 | `glowny.wskaznik.klawisze.{bez,chod,bieg}` |
| treadmill | włączone | z mapowania | 0/1/2 | `glowny.wskaznik.chodzenie.{stoisz,chod,bieg}`; przy `output = dry` `glowny.wskaznik.chodzenie.proba` / `proba_bieg` |
| dowolny | wyłączone | idle | -1 | `glowny.wskaznik.wylaczone` |

Na końcu: brak świeżych danych → `mode = offline` niezależnie od trybu (liczba na wskaźniku szara).
`PadTelemetry.statusText`: `ustawienia.pad.brak` albo `ustawienia.pad.stan`.

### 9.3 Podpowiedź pod trybami (`control.hint`) i zdarzenia taśmy

Domyślnie wg trybu: gamepad `glowny.podpowiedz.pad`, keyboard `glowny.podpowiedz.klawisze`,
training `glowny.podpowiedz.trening`, treadmill + keys `glowny.podpowiedz.chodzenie`,
treadmill + dry `glowny.podpowiedz.chodzenie_proba`.

Zastępowane (do następnej zmiany trybu/klawiszy) przez: błąd klawisza (`klawisz.blad.*`),
`tasma.uruchamiam_odliczanie` (gdy `startDelayS ≥ 2`) albo `tasma.uruchamiam` przy włączaniu,
oraz wynik polecenia taśmy: started `tasma.rusza`, stopped `tasma.zatrzymana`, refused
`tasma.odmowa`, unknown `tasma.bez_odpowiedzi`, unsupported `tasma.nieobslugiwane`,
not_connected `tasma.brak_polaczenia`.

Logika `set_control(on)` (do przeniesienia 1:1): przy wyłączeniu najpierw puszczenie
klawiszy, potem: `beltCommands || padStop || training` → stop taśmy, w innym razie, gdy tryb
ustawia prędkość → prędkość bezczynna (`idle_target`). Przy włączeniu:
`(beltCommands || training) && !padStop` → start taśmy. `padStop` = tryb gamepad i
`padIdle = stop` (taśmę budzi ruch gałki).

### 9.4 Podtytuł karty sterowania (`control.subtitle`)

`{opis}` = `glowny.sterowanie.opis.tasma` (beltControl) albo `glowny.sterowanie.opis.bez_tasmy`;
z przyciskiem pada: `glowny.sterowanie.opis.z_padem`. Dalej wg trybu:
`glowny.sterowanie.opis.{trening,klawisze,pad,chodzenie}`.

### 9.5 Etykieta zapamiętanej bieżni (`device.label`)

Symulator: `ustawienia.bieznia.symulator_opis`. Zapamiętana: `ustawienia.bieznia.opis` (linia 1
model, linia 2 części „start ok.” i „oprogramowanie” tylko gdy są). Brak:
`ustawienia.bieznia.brak`. Nazwa Bluetooth nie jest w treści (UI może ją pokazać w podpowiedzi
z `device.device.name` albo `settings.deviceName`).

---

## 10. Uwagi i znalezione problemy

1. **Ustawienia popupów nie przetrwają restartu w 0.5.x.** `achievement_popups`
   i `achievement_sound` są w ustawieniach płaskich, ale nie ma ich w `SHARED_KEYS`,
   `LOOK_KEYS` ani `PROFILE_KEYS` (`bieznia/dane.py`), więc `Store.update_flat` ich nie
   zapisuje - po ponownym uruchomieniu wracają do domyślnych. W 0.6.0 trafiają do `wyglad`
   osoby (tabela 8.1). Starsza wersja te klucze przy odczycie pominie - nie szkodzi.
2. **Nazwa biblioteki rdzenia `core` koliduje z wbudowanym crate `core`.**
   `crates/core/Cargo.toml` ma `[lib] name = "core"`; w `src-tauri` ścieżka `core::…` będzie
   niejednoznaczna z biblioteką standardową `core`. Proponuję `[lib] name = "walklevel_core"`
   (tak nazywa ją ten dokument).
3. **`tauri.conf.json` z szablonu**: `identifier: "com.tauri.dev"`, `version: "0.1.0"`, okno
   800 × 600 widoczne od razu. Do zmiany: `identifier` np. `pl.damnit.walklevel`, wersja
   z `Cargo.toml` (0.6.0), okno `main` 740 × 890 `visible: false`, `mainBinaryName: "Bieznia"`
   i instalator Inno Setup zamiast domyślnego bundla (8.6).
4. **`pytaj_przy_starcie`** jest w pliku, ale 0.5 go nie używa - przenosimy bez zmian
   (`people.askAtStart`), żadnego ekranu.
5. **`BIEZNIA_RESUME`** czyta tylko `settings.load_resumed_enabled`; w repozytorium nic tej
   zmiennej nie ustawia, więc wznowienie sterowania po aktualizacji w praktyce nie działa.
   0.6.0 zachowuje odczyt (zgodność), a decyzja o ustawianiu zmiennej w instalatorze należy
   do osoby od pakowania.
6. Historia prędkości z ostatniej minuty (`Snapshot.history`) nie jest używana przez żaden
   ekran - nie przenosimy jej do telemetrii.
7. Teksty w podpisach używają nazw klawiszy z `KeyBinding.name` (np. „W”, „Shift”) - nie
   `label`. Skrót włączania zawsze `label` (z modyfikatorami).
