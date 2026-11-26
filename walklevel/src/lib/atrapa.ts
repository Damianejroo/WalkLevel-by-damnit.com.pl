// Atrapa rdzenia do pracy nad interfejsem w zwykłej przeglądarce (npm run dev, test:ui).
// Ładowana tylko poza Tauri (src/lib/api.ts). Udaje rdzeń: trzyma wycinki stanu, obsługuje
// komendy z docs/kontrakt.md rozdz. 6, wysyła zdarzenia wl:* i telemetrię 10 razy na sekundę.
//
// Scenariusze z adresu: ?stan=<nazwa> (SCENARIUSZE niżej) oraz pojedyncze nadpisania:
//   tryb=keyboard|gamepad|treadmill|training   bieznia=<StanBiezni>   konto=0|1   historia=0|1
//   wlaczone=0|1   motyw=jasny|ciemny   tekst=duzy
// Przykład: /?stan=trening&ekran=main&szer=420&motyw=jasny
import type * as K from './typy';
import type { Silnik } from './api';
import { jestKlucz, t } from './teksty';
import { liczba } from './format';
import definicjeJson from '../../docs/definicje-v2.json';

// ============================================================================
// Scenariusze
// ============================================================================

type StanBiezni =
  | 'strumien' | 'czekam' | 'brak' | 'rozlaczona' | 'skanowanie' | 'szukanie' | 'laczenie'
  | 'ponawiam' | 'bt' | 'nieobslugiwane' | 'symulator';

interface Scenariusz {
  tryb: K.Direction;
  bieznia: StanBiezni;
  wlaczone: boolean;
  wyjscie: K.OutputKind;
  konto: boolean;
  historia: boolean;
  pierwsze: boolean;
  dlugie: boolean;
}

const BAZA: Scenariusz = {
  tryb: 'keyboard', bieznia: 'strumien', wlaczone: false, wyjscie: 'keys',
  konto: true, historia: true, pierwsze: false, dlugie: false,
};

export const SCENARIUSZE: Record<string, Partial<Scenariusz>> = {
  domyslny: {},
  klawisze: { tryb: 'keyboard', wlaczone: true },
  pad: { tryb: 'gamepad', wlaczone: true },
  chodzenie: { tryb: 'treadmill', wlaczone: true },
  proba: { tryb: 'treadmill', wyjscie: 'dry', wlaczone: true },
  trening: { tryb: 'training', wlaczone: true },
  'brak-biezni': { bieznia: 'brak' },
  rozlaczona: { bieznia: 'rozlaczona' },
  szukanie: { bieznia: 'szukanie' },
  ponawiam: { bieznia: 'ponawiam' },
  'bt-wylaczony': { bieznia: 'bt' },
  symulator: { bieznia: 'symulator', wlaczone: true },
  wylogowany: { konto: false },
  pusto: { konto: false, historia: false, bieznia: 'brak' },
  pierwsze: { pierwsze: true, konto: false, historia: false, bieznia: 'brak' },
  dlugie: { dlugie: true, tryb: 'training', wlaczone: true },
  'dlugie-pad': { dlugie: true, tryb: 'gamepad', wlaczone: true },
  // Ustawienia ze wszystkimi kategoriami otwartymi (src/ekrany/Ustawienia.svelte: stan=ustawienia*)
  ustawienia: {},
  'ustawienia-chodzenie': { tryb: 'treadmill' },
  'ustawienia-trening': { tryb: 'training' },
  'ustawienia-wylogowany': { konto: false, bieznia: 'brak' },
  'ustawienia-dlugie': { dlugie: true, tryb: 'gamepad' },
};

function czytajScenariusz(): Scenariusz {
  const p = new URLSearchParams(location.search);
  const s: Scenariusz = { ...BAZA, ...(SCENARIUSZE[p.get('stan') ?? 'domyslny'] ?? {}) };
  const tryb = p.get('tryb');
  if (tryb === 'keyboard' || tryb === 'gamepad' || tryb === 'treadmill' || tryb === 'training') s.tryb = tryb;
  const bieznia = p.get('bieznia');
  if (bieznia) s.bieznia = bieznia as StanBiezni;
  for (const pole of ['konto', 'historia', 'wlaczone'] as const) {
    const v = p.get(pole);
    if (v !== null) s[pole] = v === '1';
  }
  return s;
}

// ============================================================================
// Dane stałe (to, co rdzeń wysyła w statics)
// ============================================================================

const PALETA: K.PaletteColor[] = (
  [
    ['Czerwień', 0, '#fa3838', '#e70606'], ['Koral', 10, '#f34523', '#dc2e0c'],
    ['Pomarańcz', 25, '#f96b06', '#bf5305'], ['Morela', 32, '#df8620', '#a56318'],
    ['Banan', 52, '#f9d806', '#847303'], ['Limonka', 80, '#a4ec13', '#587f0a'],
    ['Zieleń', 118, '#26df20', '#178613'], ['Szmaragd', 145, '#19e66f', '#0f8540'],
    ['Mięta', 158, '#8fc2b0', '#2f5d50'], ['Morski', 174, '#13ecd6', '#0b8276'],
    ['Niebo', 196, '#0db5f2', '#097ba5'], ['Błękit', 212, '#2384f3', '#0c6fe1'],
    ['Chaber', 226, '#5c7ff2', '#1346ec'], ['Indygo', 242, '#7c78f0', '#2019e6'],
    ['Fiolet', 258, '#956df3', '#5413ec'], ['Lawenda', 270, '#a16dd5', '#7f39c6'],
    ['Orchidea', 292, '#cf46e5', '#c01fd8'], ['Fuksja', 312, '#f312c6', '#d30bab'],
    ['Róż', 330, '#f42f91', '#df0c75'], ['Malina', 345, '#f0426e', '#e21246'],
  ] as const
).map(([name, hue, dark, light]) => ({ name, hue, dark, light }));

const GLIFY = [
  ['Pad', '\ue7fc'], ['Tarcza', '\uea18'], ['Ogień', '\uecad'], ['Gwiazda', '\ue734'],
  ['Auto', '\ue804'], ['Samolot', '\ue709'], ['Świat', '\ue774'], ['Flaga', '\ue7c1'],
  ['Robot', '\ue99a'], ['Klocek', '\uea86'], ['Iskry', '\uf4a5'], ['Serce', '\ueb51'],
].map(([name, code]) => ({ name, code }));
const GLIF_POSTAC = '\ue805';

interface DefinicjeJson {
  wersja: number;
  xp_za_km: number;
  xp_za_rzadkosc: Record<K.Rarity, number>;
  rzadkosci: K.Rarity[];
  poziomy_xp: number[];
  rangi: Array<{ od_poziomu: number; nazwa: string; opis: string }>;
  zasady: { seria_wolne_dni_na_7: number; min_km_dnia_serii: number };
  osiagniecia: Array<Record<string, string | number>>;
}

function definicje(): K.LevelDefinitions {
  const d = definicjeJson as unknown as DefinicjeJson;
  return {
    version: d.wersja,
    xpPerKm: d.xp_za_km,
    rarityXp: d.xp_za_rzadkosc,
    rarities: d.rzadkosci,
    levelsXp: d.poziomy_xp,
    ranks: d.rangi.map((r) => ({ level: r.od_poziomu, name: r.nazwa, about: r.opis })),
    achievements: d.osiagniecia.map((o) => {
      const { id, typ, rzadkosc, znak, nazwa, klimat, warunek, ...params } = o;
      return {
        id: String(id), kind: typ as K.AchievementKind, rarity: rzadkosc as K.Rarity, mark: String(znak),
        name: String(nazwa), flavor: String(klimat), condition: String(warunek),
        km: typ === 'dystans' ? Number(o.km) : null, params,
      };
    }),
    rules: { restDaysPer7: d.zasady.seria_wolne_dni_na_7, minKmPerStreakDay: d.zasady.min_km_dnia_serii },
  };
}

// ============================================================================
// Pomocnicze
// ============================================================================

/** Powtarzalny generator liczb (te same dane przy każdym uruchomieniu - stabilne zrzuty). */
function losowosc(ziarno: number) {
  let a = ziarno >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let x = Math.imul(a ^ (a >>> 15), 1 | a);
    x = (x + Math.imul(x ^ (x >>> 7), 61 | x)) ^ x;
    return ((x ^ (x >>> 14)) >>> 0) / 4294967296;
  };
}

const kopia = <T>(v: T): T => (v === undefined ? v : structuredClone(v));
const kmh = (x: number) => liczba(x, 1);
const teraz = () => Date.now() / 1000;
const id10 = (r: () => number) => Array.from({ length: 10 }, () => Math.floor(r() * 16).toString(16)).join('');

function klawisz(name: string, scan: number, vk: number, mods: KeyBindingMods = []): K.KeyBinding {
  const nazwy = { ctrl: 'Ctrl', alt: 'Alt', shift: 'Shift', win: 'Win' };
  return { scan, extended: false, name, vk, mods, label: [...mods.map((m) => nazwy[m]), name].join('+') };
}
type KeyBindingMods = K.KeyBinding['mods'];

function blad(code: K.ErrorCode, message: string, fields?: Record<string, string>): K.CmdError {
  return fields ? { code, message, fields } : { code, message };
}

// ============================================================================
// Atrapa
// ============================================================================

export function utworzAtrape(): Silnik {
  const sc = czytajScenariusz();
  const p = new URLSearchParams(location.search);
  const r = losowosc(20261126);
  const dlugie = sc.dlugie;

  // ---------------- osoby i profile ----------------
  const profil = (name: string, color: K.ColorName, glyph: string, direction: K.Direction): K.GameProfile => ({
    id: id10(r), name, color, iconColor: null, icon: { color, glyph, url: null },
    exe: name === 'Domyślny' ? null : `C:\\Gry\\${name}\\${name.replace(/\W+/g, '')}.exe`,
    online: true,
    summary: { direction, walkSpeed: 1.5, runSpeed: 2.5, padStepCount: dlugie ? 5 : 4, padMaxKmh: 4, trainingMin: 2, trainingMax: 5, standBelow: 1.2 },
  });
  const nazwaGry = dlugie ? 'Gra o bardzo długiej nazwie, czterdzieści' : 'Elden Ring';
  const profileDamiana = [
    profil(nazwaGry, 'Morela', GLIFY[0].code, sc.tryb),
    profil(dlugie ? 'The Elder Scrolls V: Skyrim Special' : 'Skyrim', 'Niebo', GLIFY[1].code, 'treadmill'),
    profil('Domyślny', 'Mięta', GLIF_POSTAC, 'keyboard'),
    ...(dlugie
      ? [profil('Minecraft', 'Zieleń', GLIFY[9].code, 'gamepad'), profil('Cyberpunk 2077', 'Fuksja', GLIFY[10].code, 'training'), profil('Factorio', 'Banan', GLIFY[8].code, 'keyboard')]
      : []),
  ];
  const osoba = (name: string, color: K.ColorName, profiles: K.GameProfile[], konto: string | null): K.Person => ({
    id: id10(r), name, color, activeProfileId: profiles[0].id, profiles, todaySteps: 0,
    hasAccount: konto !== null, accountName: konto,
  });
  const ludzie: K.Person[] = [
    osoba(dlugie ? 'Damianejro_bardzo_dlugi1' : 'Damian', 'Chaber', profileDamiana, sc.konto ? (dlugie ? 'Damianejro_bardzo_dlugi1' : 'Damianejro') : null),
    osoba('Ola', 'Róż', [profil('Minecraft', 'Zieleń', GLIFY[9].code, 'keyboard'), profil('Domyślny', 'Mięta', GLIF_POSTAC, 'keyboard')], null),
    ...(dlugie ? [osoba('Kuba', 'Limonka', [profil('Domyślny', 'Mięta', GLIF_POSTAC, 'keyboard')], null), osoba('Babcia Zosia', 'Lawenda', [profil('Domyślny', 'Mięta', GLIF_POSTAC, 'keyboard')], null)] : []),
  ];

  const people: K.PeopleView = {
    ready: !sc.pierwsze,
    activePersonId: sc.pierwsze ? '' : ludzie[0].id,
    activeProfileId: sc.pierwsze ? '' : ludzie[0].profiles[0].id,
    askAtStart: false,
    people: sc.pierwsze ? [] : ludzie,
  };
  const aktywnaOsoba = () => people.people.find((o) => o.id === people.activePersonId) ?? null;
  const aktywnyProfil = () => aktywnaOsoba()?.profiles.find((x) => x.id === people.activeProfileId) ?? null;

  // ---------------- ustawienia ----------------
  const motyw = p.get('motyw');
  const settings: K.Settings = {
    deviceAddress: 'C8:47:8C:12:34:56',
    deviceName: dlugie ? 'KS-ST-X21C' : 'KS-NACH-MXG',
    beltControl: true, quickStop: true,
    toggleKey: klawisz('F8', 0x42, 0x77), togglePad: '',
    simulator: sc.bieznia === 'symulator', record: false, detailedLogs: false,
    theme: motyw === 'jasny' ? 'light' : motyw === 'ciemny' ? 'dark' : 'system',
    largeText: p.get('tekst') === 'duzy',
    alwaysOnTop: false, floatingButton: false, showPersonSwitch: true, showProfileSwitch: true,
    achievementPopups: true, achievementSound: true,
    direction: sc.tryb, output: sc.wyjscie,
    walkKey: klawisz('W', 0x11, 0x57), runKey: klawisz('Shift', 0x2a, 0x10),
    standBelow: 1.2, runOn: 3.4, runOff: 2.8,
    idleSpeed: 1.0, walkSpeed: 1.5, runSpeed: 2.5,
    padSteps: dlugie ? [[0, 1], [25, 1.5], [50, 2.5], [75, 3.5], [100, 4.5]] : [[0, 1], [25, 1.5], [50, 2.5], [100, 4]],
    padIdle: 'stop',
    trainingLevels: dlugie ? [6.5, 8.5, 10.5, 12.5] : [2, 3, 4, 5],
    trainingLevel: 1,
    profileColor: aktywnyProfil()?.color ?? 'Mięta',
    personColor: aktywnaOsoba()?.color ?? 'Mięta',
  };

  // ---------------- bieżnia ----------------
  const model = dlugie ? 'WalkingPad X21 (KS-ST-X21C)' : 'KingSmith X21';
  const caps: K.Capabilities = {
    speedMin: 0.5, speedMax: 6, speedStep: 0.1, startSpeed: 1, startDelayS: 3,
    acksCommands: true, pause: false, steps: true, incline: null,
    telemetryS: 0.5, minCommandGapS: 0.3, control: true,
  };
  let stanBiezni: StanBiezni = sc.bieznia;
  const device: K.DeviceView = {
    link: 'idle', linkText: '', status: null as unknown as K.LinkStatus, device: null, caps,
    remembered: true, label: '', scanning: false, scanText: '', devices: [], bluetoothOff: false,
    diagnostics: { running: false, text: '' },
  };

  function przeliczBieznie() {
    const s = stanBiezni;
    const link: Record<StanBiezni, K.Link> = {
      strumien: 'streaming', czekam: 'streaming', brak: 'idle', rozlaczona: 'idle', skanowanie: 'searching',
      szukanie: 'searching', laczenie: 'connecting', ponawiam: 'retrying', bt: 'bluetoothOff',
      nieobslugiwane: 'unsupported', symulator: 'streaming',
    };
    device.link = link[s];
    device.remembered = s !== 'brak' && s !== 'skanowanie' && s !== 'symulator';
    device.bluetoothOff = s === 'bt';
    device.scanning = s === 'skanowanie';
    device.device = s === 'strumien' || s === 'czekam'
      ? { model, driver: 'kingsmith', profile: 'kingsmith-x21', name: settings.deviceName, firmware: '1.2.7', simulated: false }
      : s === 'symulator' ? { model: t('ustawienia.bieznia.symulator'), driver: 'symulator', profile: '', name: '', firmware: '', simulated: true } : null;
    device.linkText = s === 'ponawiam' ? t('ble.powod.timeout') : s === 'strumien' ? t('glowny.status.polaczono.podtytul', { min_kmh: kmh(caps.speedMin), max_kmh: kmh(caps.speedMax) }) : '';
    const tasma = { min_kmh: kmh(caps.speedMin), max_kmh: kmh(caps.speedMax) };
    const tekst = (k: string, zm: Record<string, string>) => (jestKlucz(k) ? t(k, zm) : '');
    const st = (tone: K.Tone, busy: boolean, klucz: string, action: K.StatusAction, primary: boolean, zm: Record<string, string> = {}): K.LinkStatus => ({
      tone, busy, action, primary,
      title: tekst(`glowny.status.${klucz}.tytul`, zm),
      sub: tekst(`glowny.status.${klucz}.podtytul`, zm),
      actionLabel: tekst(`glowny.status.${klucz}.akcja`, zm),
    });
    const zModelem = { model, ...tasma };
    const statusy: Record<StanBiezni, () => K.LinkStatus> = {
      strumien: () => st('accent', false, 'polaczono', 'disconnect', false, zModelem),
      czekam: () => ({ ...st('run', true, 'czekam', 'disconnect', false, zModelem), actionLabel: t('glowny.status.polaczono.akcja') }),
      brak: () => st('muted', false, 'brak', 'scan', true),
      rozlaczona: () => st('muted', false, 'rozlaczona', 'retry', true, zModelem),
      skanowanie: () => st('accent', true, 'skanowanie', 'cancel', false),
      szukanie: () => st('accent', true, 'szukanie', 'cancel', false, zModelem),
      laczenie: () => st('accent', true, 'laczenie', 'cancel', false, zModelem),
      ponawiam: () => ({ ...st('run', true, 'ponawiam', 'disconnect', false), sub: t('ble.powod.timeout'), actionLabel: t('glowny.status.ponawiam.akcja') }),
      bt: () => st('danger', false, 'bt_wylaczony', 'retry', true),
      nieobslugiwane: () => st('danger', false, 'nieobslugiwane', 'scan', true),
      symulator: () => st('accent', false, 'symulator', 'simulatorOff', false, { klawisz_szybciej: 'F7', klawisz_wolniej: 'F6' }),
    };
    device.status = statusy[s]();
    device.label = s === 'symulator'
      ? t('ustawienia.bieznia.symulator_opis', tasma)
      : device.remembered
        ? t('ustawienia.bieznia.opis', { ...zModelem, sekundy: caps.startDelayS, wersja_bt: '1.2.7' })
        : t('ustawienia.bieznia.brak');
    settings.simulator = s === 'symulator';
  }
  przeliczBieznie();

  // ---------------- sterowanie ----------------
  const control: K.ControlView = {
    enabled: sc.wlaczone && !sc.pierwsze, direction: settings.direction, setsSpeed: false, hint: '', subtitle: '',
    toggleLabel: '', walkName: '', runName: '', padButtonLabel: '', padCapturing: false, keyCapturing: null,
    beltCommands: true,
  };

  function przeliczSterowanie() {
    const d = settings.direction;
    const zm = {
      klawisz_chodu: settings.walkKey.name, klawisz_biegu: settings.runKey.name,
      klawisz_szybciej: 'F7', klawisz_wolniej: 'F6',
    };
    control.direction = d;
    control.setsSpeed = d !== 'treadmill';
    control.toggleLabel = settings.toggleKey.label;
    control.walkName = settings.walkKey.name;
    control.runName = settings.runKey.name;
    control.beltCommands = stanBiezni === 'strumien' || stanBiezni === 'symulator';
    control.hint =
      d === 'treadmill'
        ? t(settings.output === 'dry' ? 'glowny.podpowiedz.chodzenie_proba' : 'glowny.podpowiedz.chodzenie', zm)
        : d === 'keyboard' ? t('glowny.podpowiedz.klawisze', zm)
          : d === 'gamepad' ? t('glowny.podpowiedz.pad', zm) : t('glowny.podpowiedz.trening', zm);
    let opis = t(settings.beltControl ? 'glowny.sterowanie.opis.tasma' : 'glowny.sterowanie.opis.bez_tasmy');
    if (control.padButtonLabel) opis = t('glowny.sterowanie.opis.z_padem', { opis, przycisk_pada: control.padButtonLabel });
    const klucz = { treadmill: 'chodzenie', keyboard: 'klawisze', gamepad: 'pad', training: 'trening' } as const;
    control.subtitle = t(`glowny.sterowanie.opis.${klucz[d]}`, { opis, ...zm });
    const pr = aktywnyProfil();
    if (pr) pr.summary.direction = d;
  }
  przeliczSterowanie();

  // ---------------- historia ----------------
  const dzis0 = new Date();
  dzis0.setHours(0, 0, 0, 0);
  const sesje: K.SessionDetail[] = [];
  if (sc.historia) {
    for (const o of ludzie) {
      const ileDni = o === ludzie[0] ? 18 : 9;
      for (let dzien = ileDni; dzien >= 0; dzien--) {
        const ile = r() < 0.25 ? 0 : 1 + Math.floor(r() * 2);
        for (let i = 0; i < ile; i++) {
          const godzina = 7 + Math.floor(r() * 14);
          const start = dzis0.getTime() / 1000 - dzien * 86400 + godzina * 3600 + Math.floor(r() * 50) * 60;
          if (start > teraz() - 600) continue;
          const activeS = (15 + Math.floor(r() * 55)) * 60;
          const v = 2.4 + r() * 2.2;
          const pr = o.profiles[Math.floor(r() * o.profiles.length)];
          const symulator = r() < 0.08;
          const kadencja = 85 + r() * 30;
          sesje.push({
            id: id10(r), start, end: start + activeS + 120, personId: o.id, personName: o.name,
            profileId: pr.id, profileName: pr.name, steps: Math.round((activeS / 60) * kadencja),
            distanceKm: (v * activeS) / 3600, activeS, avgKmh: v,
            source: symulator ? 'symulator' : 'bieznia', mode: pr.summary.direction === 'training' ? 'trening' : 'gra',
            icon: kopia(pr.icon),
            online: o.hasAccount && !symulator ? { hidden: r() < 0.1, sent: r() < 0.85, rejected: null } : null,
            cadence: kadencja, wholeS: activeS + 120, deviceModel: symulator ? t('ustawienia.bieznia.symulator') : model,
            driver: symulator ? 'symulator' : 'kingsmith', firmware: symulator ? '' : '1.2.7', appVersion: '0.6.0',
            tzOffsetMin: -dzis0.getTimezoneOffset(), linked: o.hasAccount,
          });
        }
      }
    }
    sesje.sort((a, b) => b.start - a.start);
    const odrzucona = sesje.find((s) => s.online);
    if (odrzucona?.online) odrzucona.online = { hidden: false, sent: false, rejected: 'Sesja nachodzi na inną sesję z tego samego czasu.' };
  }
  let wersjaHistorii = 1;
  const history: K.HistoryStamp = { available: true, stamp: '' };
  function przeliczDzis() {
    for (const o of people.people) {
      o.todaySteps = sesje.filter((s) => s.personId === o.id && s.start >= dzis0.getTime() / 1000).reduce((a, s) => a + s.steps, 0);
    }
    history.stamp = `${wersjaHistorii}-${dzis0.toISOString().slice(0, 10)}-${people.activePersonId}`;
  }
  przeliczDzis();

  function sumy(lista: K.SessionRow[]): K.Totals {
    const steps = lista.reduce((a, s) => a + s.steps, 0);
    const distanceKm = lista.reduce((a, s) => a + s.distanceKm, 0);
    const activeS = lista.reduce((a, s) => a + s.activeS, 0);
    return { steps, distanceKm, activeS, avgKmh: activeS > 0 ? distanceKm / (activeS / 3600) : 0 };
  }

  function zapytanieHistorii(q: K.HistoryQuery): K.HistoryView {
    const many = people.people.length > 1;
    const everyone = many && q.scope === 'all';
    const od0 = dzis0.getTime() / 1000;
    const od = { day: od0, week: od0 - 6 * 86400, month: od0 - 29 * 86400, all: 0 }[q.range];
    const lista = sesje.filter((s) => s.start >= od && (everyone || s.personId === people.activePersonId));
    const groupKey = everyone ? 'person' : 'profile';
    const nazwa = (s: K.SessionRow) => (everyone ? s.personName : s.profileName);
    const kolorNazwy = new Map<string, string>();
    for (const o of people.people) {
      if (everyone) kolorNazwy.set(o.name, PALETA.find((k) => k.name === o.color)!.dark);
      else for (const pr of o.profiles) kolorNazwy.set(pr.name, PALETA.find((k) => k.name === pr.color)!.dark);
    }
    const hourly = q.range === 'day';
    const ileSlupkow = hourly ? 24 : q.range === 'week' ? 7 : 30;
    const bars: K.HistoryChart['bars'] = [];
    for (let i = 0; i < ileSlupkow; i++) {
      const pocz = hourly ? od0 + i * 3600 : od0 - (ileSlupkow - 1 - i) * 86400;
      const kon = pocz + (hourly ? 3600 : 86400);
      const czesci = new Map<string, number>();
      for (const s of lista) if (s.start >= pocz && s.start < kon) czesci.set(nazwa(s), (czesci.get(nazwa(s)) ?? 0) + s.steps);
      const data = new Date(pocz * 1000);
      bars.push({
        label: hourly ? String(i).padStart(2, '0') : data.toISOString().slice(0, 10),
        weekday: hourly ? null : (data.getDay() + 6) % 7,
        parts: [...czesci].map(([name, steps]) => ({ name, steps })).sort((a, b) => b.steps - a.steps),
      });
    }
    const razem = sumy(lista);
    const grupy = new Map<string, K.SessionRow[]>();
    for (const s of lista) grupy.set(nazwa(s), [...(grupy.get(nazwa(s)) ?? []), s]);
    const groups: K.HistoryGroup[] = [...grupy].map(([name, l]) => {
      const tot = sumy(l);
      const o = people.people.find((x) => x.name === name);
      const pr = people.people.flatMap((x) => x.profiles).find((x) => x.name === name);
      return {
        name, totals: tot, share: razem.steps ? tot.steps / razem.steps : 0, color: kolorNazwy.get(name) ?? '#8fc2b0',
        icon: !everyone && pr ? kopia(pr.icon) : null, avatarColor: everyone && o ? o.color : null,
      };
    }).sort((a, b) => b.totals.steps - a.totals.steps);
    const legend = groups.slice(0, 6).map((g) => g.name).sort();
    const recent = lista.slice(0, 8).map(({ cadence: _c, wholeS: _w, deviceModel: _d, driver: _dr, firmware: _f, appVersion: _a, tzOffsetMin: _t, linked: _l, ...w }) => w);
    return {
      query: q, stamp: history.stamp, many, everyone, totals: razem,
      chart: { hourly, bars, colors: Object.fromEntries(kolorNazwy), legend, empty: lista.length === 0 },
      groupKey, groups, recent, online: people.people.some((o) => o.hasAccount),
    };
  }

  // ---------------- poziomy ----------------
  const defs = definicje();
  const rarytas = (id: string) => defs.achievements.find((a) => a.id === id)!;
  function przeliczPoziomy(): K.LevelsView | null {
    const o = aktywnaOsoba();
    if (!o) return null;
    const mojeSesje = sesje.filter((s) => s.personId === o.id && s.source === 'bieznia');
    const km = (sc.historia && o === people.people[0] ? (dlugie ? 1234.5 : 212) : 0) + mojeSesje.reduce((a, s) => a + s.distanceKm, 0);
    const rr = losowosc(7);
    const progress: K.AchievementProgress[] = defs.achievements.map((a, i) => {
      let target = Number(a.params.km ?? a.params.kroki ?? a.params.dni ?? a.params.gier ?? a.params.ile ?? 1);
      if (a.kind === 'dystans') target = a.km ?? 1;
      const current = a.kind === 'dystans' ? Math.min(km, target) : km > 0 ? Math.min(target, Math.round(target * rr() * 1.3 * 10) / 10) : 0;
      const zdobyte = current >= target && km > 0;
      return {
        id: a.id, current, target, fraction: zdobyte ? 1 : target ? current / target : 0,
        earnedAt: zdobyte ? teraz() - (defs.achievements.length - i) * 3 * 86400 * rr() : null,
      };
    });
    const xp = Math.round(km * defs.xpPerKm) + progress.filter((x) => x.earnedAt !== null).reduce((a, x) => a + defs.rarityXp[rarytas(x.id).rarity], 0);
    let level = 1;
    while (level < defs.levelsXp.length && defs.levelsXp[level] <= xp) level++;
    const floor = defs.levelsXp[level - 1];
    const ceiling = defs.levelsXp[level] ?? floor;
    const rank = [...defs.ranks].reverse().find((x) => x.level <= level) ?? defs.ranks[0];
    const nextRank = defs.ranks.find((x) => x.level > level) ?? null;
    return {
      personId: o.id, km, xp, level, maxLevel: defs.levelsXp.length,
      levelFloorXp: floor, levelCeilingXp: ceiling, levelFraction: ceiling > floor ? (xp - floor) / (ceiling - floor) : 1,
      kmToNextLevel: Math.max(0, (ceiling - xp) / defs.xpPerKm), rank, nextRank,
      earnedCount: progress.filter((x) => x.earnedAt !== null).length, total: progress.length, progress,
    };
  }
  let levels = przeliczPoziomy();

  // ---------------- konto i dane online ----------------
  const pseudonim = ludzie[0].accountName ?? '';
  const account: K.AccountView = {
    enabled: true, personId: people.activePersonId, loggedIn: sc.konto && !sc.pierwsze, userId: sc.konto ? 'u-damianejro' : '',
    name: sc.konto ? pseudonim : '', email: sc.konto ? 'damianejro@przyklad.pl' : '', verified: true, upload: true,
    visibility: 'znajomi', live: true, profileVisibility: 'znajomi', gamesVisibility: 'znajomi',
    terms: '2026-11-01', termsCurrent: dlugie ? '2026-11-20' : '2026-11-01', termsOutdated: dlugie,
    gamesAllowed: true, remember: true, termsUrl: 'https://damnit.com.pl/bieznia/regulamin',
    busy: [], message: '', verifyPolling: false, pendingUploads: 2,
  };
  const tydzien = (k: number): K.PeriodTotals => ({ steps: k, distanceKm: k / 1350, activeS: k * 0.55, sessions: Math.max(1, Math.round(k / 6000)) });
  const znajomi: K.Friend[] = [
    { id: 'u-ola', name: 'OlaBiega', level: 14, live: { kmh: 3.4, steps: 2140 }, week: tydzien(31240) },
    { id: 'u-kuba', name: 'KubaWędrowiec', level: 9, live: null, week: tydzien(18420) },
    { id: 'u-marta', name: 'Marta_z_Gór', level: 23, live: null, week: tydzien(52310) },
    { id: 'u-tomek', name: dlugie ? 'Tomasz_o_dlugim_nicku24' : 'Tomek', level: 3, live: null, week: tydzien(4120) },
  ];
  const ranking = (scope: K.RankingScope): { scope: K.RankingScope; rows: K.RankingRow[] } => {
    const mnoznik = { tydzien: 1, miesiac: 4.2, razem: 26, poziom: 1 }[scope];
    const wiersze = [
      ...znajomi.map((f) => ({ id: f.id, name: f.name, me: false, level: f.level, steps: Math.round(f.week.steps * mnoznik), distanceKm: f.week.distanceKm * mnoznik })),
      { id: account.userId, name: pseudonim, me: true, level: levels?.level ?? 1, steps: Math.round(41230 * mnoznik), distanceKm: 30.5 * mnoznik },
      { id: 'u-ania', name: 'AniaNaBieżni', me: false, level: 31, steps: Math.round(68400 * mnoznik), distanceKm: 50.7 * mnoznik },
      { id: 'u-rekord', name: 'Rekordzista', me: false, level: dlugie ? 100 : 41, steps: dlugie ? 12345678 : Math.round(90210 * mnoznik), distanceKm: dlugie ? 1234.5 : 66.8 * mnoznik },
    ];
    const klucz = scope === 'poziom' ? (w: (typeof wiersze)[number]) => (w.level ?? 0) * 1e9 + w.distanceKm : (w: (typeof wiersze)[number]) => w.steps;
    return { scope, rows: wiersze.sort((a, b) => klucz(b) - klucz(a)).map((w, i) => ({ place: i + 1, ...w })) };
  };
  const pustyOnline = (): K.OnlineView => ({
    personId: people.activePersonId, loading: [], stats: null, friends: null, invitations: null, ranking: null,
    people: null, peopleQuery: { search: '', page: 1 }, person: null, regCode: null,
  });
  const online: K.OnlineView = pustyOnline();
  if (account.loggedIn) {
    online.stats = {
      ...tydzien(41230),
      games: [
        { game: nazwaGry, gameId: 'g1', steps: 28100, distanceKm: 20.8, activeS: 14400 },
        { game: 'Skyrim', gameId: 'g2', steps: 9800, distanceKm: 7.3, activeS: 5400 },
        { game: '', gameId: null, steps: 3330, distanceKm: 2.4, activeS: 1900 },
      ],
    };
    online.friends = znajomi;
    online.invitations = {
      incoming: [
        { id: 'z1', person: { id: 'u-ewa', name: 'EwaKroczy' }, createdAt: teraz() - 7200 },
        ...(dlugie ? [{ id: 'z2', person: { id: 'u-x', name: 'Zaproszenie_dlugie_nick' }, createdAt: teraz() - 86400 }, { id: 'z3', person: { id: 'u-y', name: 'Trzeci' }, createdAt: null }] : []),
      ],
      sent: [{ id: 'z9', person: { id: 'u-piotr', name: 'Piotr' }, createdAt: teraz() - 3 * 86400 }],
    };
    online.ranking = ranking('tydzien');
  }

  // ---------------- pozostałe wycinki ----------------
  const app: K.AppInfo = {
    version: '0.6.0', title: 'WalkLevel 0.6.0 by damnit.com.pl', installed: false, dataDir: 'C:\\Users\\Gracz\\AppData\\Local\\Bieznia',
    firstRun: sc.pierwsze, legacy02: false, updatedFrom: null, latestBackup: null,
    accountsEnabled: true, reportEndpoint: true, windowFocused: true,
  };
  // najdłuższy wariant Ustawień > Dane i kopia: z przyciskiem „Przywróć dane sprzed…”
  if (dlugie) app.latestBackup = 'przed-importem-20261126-120000';
  const update: K.UpdateView = { available: dlugie ? '0.6.1' : null, busy: false, status: '', installing: false };
  const report: K.ReportView = { busy: false, status: '', lastFile: null, sentNumber: null };
  const model1 = (id: string, name: string, driver: K.DriverId, match: string[], extra: Partial<K.ModelItem> = {}): K.ModelItem => ({
    id, name, driver, driverName: driver === 'ftms' ? t('modele.rodzaj.ftms') : t('modele.rodzaj.kingsmith'), match, caps: kopia(caps),
    generic: false, mine: false, source: 'wbudowany', removable: false, ...extra,
  });
  const models: K.ModelsView = {
    items: [
      model1('kingsmith-x21', 'KingSmith X21', 'kingsmith', ['KS-NACH-MXG', 'KS-ST-X21C'], { mine: true }),
      model1('walkingpad', 'WalkingPad', 'kingsmith', ['KS-'], { generic: true }),
      model1('urevo-5l', 'Urevo SpaceWalk 5L', 'ftms', ['URTM051']),
      model1('moja', 'Moja bieżnia z pliku', 'ftms', ['MOJA-'], { source: 'dodatkowy', removable: true }),
    ],
    busy: false, status: '',
  };
  const bubble: K.BubbleView = { visible: false, enabled: control.enabled, theme: settings.theme, accent: settings.profileColor };
  const logi: string[] = [];
  const loguj = (tekst: string) => {
    const d = new Date();
    const linia = `${d.toTimeString().slice(0, 8)}  ${tekst}`;
    logi.push(linia);
    if (logi.length > 500) logi.shift();
    emituj('wl:log', linia);
  };
  for (let i = 0; i < 24; i++) logi.push(`0${8 + Math.floor(i / 6)}:${String(10 + i * 2).padStart(2, '0')}:0${i % 10}  bieznia.ble: odczyt prędkości ${liczba(2 + (i % 5) * 0.3, 1)} km/h, kroki ${1200 + i * 37}`);

  const wycinki = (): K.SliceMap => ({
    app, settings, people, device, control, levels, history, account, online, update, report, models, bubble,
  });

  // ---------------- zdarzenia ----------------
  const sluchacze = new Map<string, Set<(tresc: unknown) => void>>();
  let rev = 1;
  function emituj(zdarzenie: string, tresc: unknown) {
    for (const f of sluchacze.get(zdarzenie) ?? []) f(kopia(tresc));
  }
  function wyslij(...nazwy: K.SliceName[]) {
    for (const slice of nazwy) {
      rev++;
      emituj('wl:stan', { rev, slice, value: wycinki()[slice] });
    }
  }
  const toast = (text: string, kind: K.Toast['kind'] = 'info') => {
    emituj('wl:komunikat', { text, kind });
    loguj(text);
  };
  const pozniej = (ms: number, f: () => void) => setTimeout(f, ms);

  function zmienOsobe() {
    const o = aktywnaOsoba();
    settings.personColor = o?.color ?? 'Mięta';
    settings.profileColor = aktywnyProfil()?.color ?? 'Mięta';
    bubble.accent = settings.profileColor;
    account.personId = people.activePersonId;
    account.loggedIn = !!o?.hasAccount;
    account.name = o?.accountName ?? '';
    Object.assign(online, pustyOnline());
    levels = przeliczPoziomy();
    przeliczDzis();
    przeliczSterowanie();
    wyslij('people', 'settings', 'control', 'levels', 'history', 'account', 'online', 'bubble');
  }

  // ---------------- telemetria ----------------
  let kroki = 0;
  let dystans = 0;
  let czasRuchu = 0;
  let predkosc = 0;
  let tMs = 0;
  setInterval(() => {
    tMs += 100;
    const live = stanBiezni === 'strumien' || stanBiezni === 'symulator';
    const s = settings;
    const faza = Math.floor(tMs / 4000) % 3;
    let cel = 0;
    let mode: K.GaugeMode = 'idle';
    let active = -1;
    let caption = t('glowny.wskaznik.wylaczone', { klawisz_wlaczania: s.toggleKey.label });
    const tasma = (v: number) => t('glowny.wskaznik.tasma_kmh', { kmh: kmh(v) });
    if (control.enabled) {
      if (s.direction === 'keyboard') {
        cel = [s.idleSpeed, s.walkSpeed, s.runSpeed][faza];
        active = faza;
        mode = (['idle', 'walk', 'run'] as const)[faza];
        caption = t((['glowny.wskaznik.klawisze.bez', 'glowny.wskaznik.klawisze.chod', 'glowny.wskaznik.klawisze.bieg'] as const)[faza], { kmh: kmh(cel), klawisz_chodu: s.walkKey.name, klawisz_biegu: s.runKey.name });
      } else if (s.direction === 'training') {
        cel = s.trainingLevels[s.trainingLevel];
        active = s.trainingLevel;
        mode = s.trainingLevel < 2 ? 'walk' : 'run';
        caption = t('glowny.wskaznik.trening', { poziom: s.trainingLevel + 1, kmh: kmh(cel) });
      } else if (s.direction === 'gamepad') {
        const krok = faza === 0 ? 0 : faza === 1 ? 2 : s.padSteps.length - 1;
        cel = s.padSteps[krok][1];
        active = krok;
        mode = krok === 0 ? 'idle' : krok === s.padSteps.length - 1 ? 'run' : 'walk';
        caption = t('glowny.wskaznik.pad.stan', { procent: s.padSteps[krok][0], stopien: krok, tasma: krok === 0 && s.padIdle === 'stop' ? t('glowny.wskaznik.tasma_stoi') : tasma(cel) });
      } else {
        cel = 2.2 + Math.sin(tMs / 3000) * 1.6;
        active = cel < s.standBelow ? 0 : cel < s.runOn ? 1 : 2;
        mode = (['idle', 'walk', 'run'] as const)[active];
        const proba = s.output === 'dry';
        caption = active === 0 ? t('glowny.wskaznik.chodzenie.stoisz')
          : proba ? t('glowny.wskaznik.chodzenie.proba')
            : t(active === 1 ? 'glowny.wskaznik.chodzenie.chod' : 'glowny.wskaznik.chodzenie.bieg', { klawisz_chodu: s.walkKey.name, klawisz_biegu: s.runKey.name });
      }
    }
    if (!live) {
      cel = 0;
      mode = 'offline';
      if (s.direction === 'treadmill' && control.enabled) caption = t('glowny.wskaznik.brak_danych');
    }
    predkosc += (cel - predkosc) * 0.25;
    if (predkosc < 0.05) predkosc = cel === 0 ? 0 : predkosc;
    const dt = 0.1;
    if (predkosc > 0.3) {
      czasRuchu += dt;
      dystans += (predkosc * dt) / 3600;
      kroki += (predkosc * 28 * dt) / 60;
    }
    const tel: K.Telemetry = {
      tMs, speedKmh: predkosc, live, enabled: control.enabled, mode, active, caption,
      steps: live ? Math.floor(kroki) : null, cadence: live && predkosc > 0.3 ? Math.round(predkosc * 28) : null,
      distanceKm: dystans, activeS: czasRuchu, beltTargetKmh: control.enabled && control.setsSpeed ? cel : null,
      pad: s.direction === 'gamepad'
        ? { name: 'DualSense (Bluetooth)', deflection: s.padSteps[Math.max(0, active)][0] / 100, step: Math.max(0, active), statusText: `DualSense: wychylenie ${s.padSteps[Math.max(0, active)][0]}%, stopień ${Math.max(0, active)}.` }
        : null,
    };
    emituj('wl:telemetria', tel);
  }, 100);

  // ---------------- komendy ----------------
  type A = Record<string, unknown>;
  const komendy: Record<string, (a: A) => unknown> = {
    app_snapshot: () => ({ rev, statics: statics(), logTail: logi, ...wycinki() }) satisfies K.AppSnapshot,
    ui_ready: (a) => {
      if (a.window === 'main' && sc.pierwsze) pozniej(50, () => emituj('wl:nawigacja', { to: 'wizard', reason: 'firstRun' }));
    },
    app_quit: () => loguj('Zamykanie (atrapa: okno zostaje otwarte)'),
    window_show_main: () => undefined,
    open_folder: (a) => loguj(`Otwórz folder: ${String(a.kind)} (atrapa)`),
    open_link: (a) => loguj(`Otwórz adres: ${String(a.kind)} (atrapa)`),
    log_tail: () => logi,
    diagnostics_run: () => new Promise((ok) => {
      device.diagnostics = { running: true, text: t('dziennik.raport_trwa') };
      wyslij('device');
      pozniej(1500, () => {
        device.diagnostics = { running: false, text: t('dziennik.raport_gotowy') };
        wyslij('device');
        ok('logs\\diagnostyka-20261126-120000.txt');
      });
    }),

    settings_set: (a) => {
      const key = a.key as K.SettingKey;
      (settings as unknown as A)[key] = a.value;
      if (key === 'direction') przeliczSterowanie();
      if (key === 'output') przeliczSterowanie();
      if (key === 'simulator') {
        stanBiezni = a.value ? 'symulator' : 'strumien';
        przeliczBieznie();
        przeliczSterowanie();
        wyslij('device');
      }
      if (key === 'theme') bubble.theme = settings.theme;
      if (key === 'floatingButton') bubble.visible = settings.floatingButton;
      wyslij('settings', 'control', 'people', 'bubble');
    },
    thresholds_set: (a) => {
      const v = Math.round(Number(a.kmh) * 10) / 10;
      if (a.which === 'walk') {
        settings.standBelow = v;
        settings.runOn = Math.max(settings.runOn, Math.min(12, v + 0.8));
      } else {
        settings.runOn = v;
        settings.standBelow = Math.min(settings.standBelow, Math.max(0.5, v - 0.8));
      }
      settings.runOff = Math.round((settings.runOn - 0.6) * 10) / 10;
      wyslij('settings');
    },
    pad_steps_count: (a) => {
      settings.padSteps = kopia(statics().padPresets[a.count as 2 | 3 | 4 | 5]);
      wyslij('settings', 'people');
    },
    pad_step_set: (a) => {
      const i = Number(a.index);
      const krok = settings.padSteps[i];
      if (!krok) throw blad('invalid', t('wspolne.blad_nieznany'));
      if (a.at !== null && a.at !== undefined) krok[0] = Number(a.at);
      if (a.kmh !== null && a.kmh !== undefined) krok[1] = Number(a.kmh);
      wyslij('settings');
    },
    training_speed_set: (a) => {
      settings.trainingLevels[Number(a.index)] = Math.round(Number(a.kmh) * 10) / 10;
      wyslij('settings', 'people');
    },
    key_capture: (a) => {
      control.keyCapturing = (a.role as K.KeyRole | null) ?? null;
      wyslij('control');
    },
    key_set: (a) => {
      const role = a.role as K.KeyRole;
      const k = a.key as K.KeyCapture;
      control.keyCapturing = null;
      if (k.code === 'Escape') {
        wyslij('control');
        throw blad('key', t('klawisz.blad.ogolny', { powod: t('klawisz.powod.esc') }));
      }
      const nazwa = k.code.startsWith('Key') ? k.code.slice(3) : k.code.startsWith('Digit') ? k.code.slice(5) : k.key.length === 1 ? k.key.toUpperCase() : k.code;
      const mods: KeyBindingMods = role === 'toggle' ? [...(k.ctrl ? ['ctrl' as const] : []), ...(k.alt ? ['alt' as const] : []), ...(k.shift ? ['shift' as const] : [])] : [];
      const kb = klawisz(nazwa, 0, 0, mods);
      if (role === 'walk') settings.walkKey = kb;
      else if (role === 'run') settings.runKey = kb;
      else settings.toggleKey = kb;
      przeliczSterowanie();
      wyslij('settings', 'control');
      return kb;
    },
    pad_capture_start: () => {
      control.padCapturing = true;
      wyslij('control');
      pozniej(1500, () => {
        if (!control.padCapturing) return;
        control.padCapturing = false;
        settings.togglePad = 'touchpad';
        control.padButtonLabel = t('pad.ps.panel_dotykowy');
        przeliczSterowanie();
        wyslij('settings', 'control');
      });
    },
    pad_capture_cancel: () => {
      control.padCapturing = false;
      wyslij('control');
    },

    control_set: (a) => {
      control.enabled = Boolean(a.on);
      bubble.enabled = control.enabled;
      wyslij('control', 'bubble');
    },
    control_toggle: () => komendy.control_set({ on: !control.enabled }),
    training_level_set: (a) => {
      settings.trainingLevel = Number(a.level) as 0 | 1 | 2 | 3;
      if (!control.enabled) komendy.control_set({ on: true });
      wyslij('settings');
    },
    training_level_step: (a) => {
      settings.trainingLevel = Math.max(0, Math.min(3, settings.trainingLevel + Number(a.step))) as 0 | 1 | 2 | 3;
      wyslij('settings');
    },
    training_stop: () => komendy.control_set({ on: false }),

    device_scan: () => {
      stanBiezni = 'skanowanie';
      device.devices = [];
      device.scanText = t('bieznia.lista.szukam');
      przeliczBieznie();
      wyslij('device');
      pozniej(1500, () => {
        device.devices = [
          { address: 'C8:47:8C:12:34:56', name: settings.deviceName, rssi: -58, treadmill: true, model },
          { address: 'D1:22:33:44:55:66', name: 'URTM051', rssi: -71, treadmill: true, model: 'Urevo SpaceWalk 5L' },
          { address: 'A0:11:22:33:44:55', name: 'Mi Band 7', rssi: -80, treadmill: false, model: 'Mi Band 7' },
          { address: 'A0:11:22:33:44:56', name: '', rssi: -88, treadmill: false, model: '' },
        ];
        device.scanText = t('bieznia.lista.znalezione', { liczba: 2, razem: 2 });
        stanBiezni = device.remembered ? 'rozlaczona' : 'brak';
        przeliczBieznie();
        device.scanning = false;
        wyslij('device');
        emituj('wl:nawigacja', { to: 'devices', reason: 'scan' });
      });
    },
    device_connect: (a) => {
      settings.deviceAddress = String(a.address);
      settings.deviceName = String(a.name);
      stanBiezni = 'laczenie';
      przeliczBieznie();
      wyslij('device', 'settings');
      pozniej(1200, () => {
        stanBiezni = 'strumien';
        przeliczBieznie();
        przeliczSterowanie();
        wyslij('device', 'control');
      });
    },
    device_disconnect: () => {
      stanBiezni = 'rozlaczona';
      przeliczBieznie();
      przeliczSterowanie();
      wyslij('device', 'control');
    },
    device_forget: () => {
      settings.deviceAddress = '';
      settings.deviceName = '';
      stanBiezni = 'brak';
      przeliczBieznie();
      wyslij('device', 'settings');
    },
    device_status_action: () => {
      const akcja = device.status.action;
      if (akcja === 'scan') return komendy.device_scan({});
      if (akcja === 'disconnect' || akcja === 'cancel') return komendy.device_disconnect({});
      if (akcja === 'simulatorOff') return komendy.settings_set({ key: 'simulator', value: false });
      return komendy.device_connect({ address: settings.deviceAddress, name: settings.deviceName });
    },
    models_refresh: () => wyslij('models'),
    models_fetch: () => new Promise((ok) => {
      models.busy = true;
      models.status = t('modele.sprawdzam');
      wyslij('models');
      pozniej(1000, () => {
        models.busy = false;
        models.status = t('modele.aktualne');
        wyslij('models');
        ok(models.status);
      });
    }),
    models_add_file: () => null,
    models_remove: (a) => {
      models.items = models.items.filter((m) => m.id !== a.id || !m.removable);
      models.status = t('modele.usunieto');
      wyslij('models');
    },

    wizard_seed: () => ({ name: 'Szybki Wędrowiec', suggestions: ['Cicha Biegaczka', 'Dzielny Piechur', 'Wytrwała Tropicielka'], color: 'Szmaragd' }),
    name_random: () => ['Spokojny Tropiciel', 'Zwinna Kozica', 'Nocny Marek'][Math.floor(Math.random() * 3)],
    wizard_finish: (a) => {
      const w = a.result as K.WizardResult;
      const profiles = [profil('Domyślny', 'Mięta', GLIF_POSTAC, w.direction)];
      if (w.game) profiles.unshift(profil(w.game.name, w.game.color, GLIFY[0].code, w.direction));
      const nowa = osoba(w.name, w.color, profiles, null);
      people.people.push(nowa);
      people.ready = true;
      people.activePersonId = nowa.id;
      people.activeProfileId = nowa.activeProfileId;
      app.firstRun = false;
      settings.direction = w.direction;
      wyslij('app');
      zmienOsobe();
    },
    person_switch: (a) => {
      const o = people.people.find((x) => x.id === a.personId);
      if (!o) throw blad('notFound', t('wspolne.blad_nieznany'));
      people.activePersonId = o.id;
      people.activeProfileId = o.activeProfileId;
      zmienOsobe();
    },
    person_update: (a) => {
      const o = people.people.find((x) => x.id === a.personId);
      const patch = a.patch as K.PersonPatch;
      if (o && patch.name?.trim()) o.name = patch.name.trim();
      if (o && patch.color) o.color = patch.color;
      if (o?.id === people.activePersonId && patch.color) settings.personColor = patch.color;
      wyslij('people', 'settings');
    },
    person_remove: (a) => {
      if (people.people.length < 2) throw blad('invalid', t('wspolne.blad_nieznany'));
      people.people = people.people.filter((x) => x.id !== a.personId);
      if (people.activePersonId === a.personId) {
        people.activePersonId = people.people[0].id;
        people.activeProfileId = people.people[0].activeProfileId;
      }
      zmienOsobe();
    },
    profile_switch: (a) => {
      const o = aktywnaOsoba();
      if (!o?.profiles.some((x) => x.id === a.profileId)) throw blad('notFound', t('wspolne.blad_nieznany'));
      people.activeProfileId = String(a.profileId);
      o.activeProfileId = people.activeProfileId;
      settings.profileColor = aktywnyProfil()!.color;
      settings.direction = aktywnyProfil()!.summary.direction;
      bubble.accent = settings.profileColor;
      przeliczSterowanie();
      wyslij('people', 'settings', 'control', 'bubble');
    },
    game_pick: () => ({
      pickId: 'wybor-1', name: 'Baldur’s Gate 3', exe: 'C:\\Gry\\Baldurs Gate 3\\bin\\bg3.exe', exeName: 'bg3.exe',
      folderName: 'bin', icon: { color: 'Fiolet', glyph: GLIFY[3].code, url: null }, iconColor: 'Fiolet',
    }) satisfies K.GamePick,
    game_pick_discard: () => undefined,
    profile_create: (a) => {
      const d = a.draft as K.ProfileDraft;
      const pr = profil(d.name, d.color, d.glyph ?? GLIF_POSTAC, settings.direction);
      aktywnaOsoba()!.profiles.push(pr);
      komendy.profile_switch({ profileId: pr.id });
      return pr.id;
    },
    profile_update: (a) => {
      const pr = people.people.flatMap((o) => o.profiles).find((x) => x.id === a.profileId);
      if (!pr) throw blad('notFound', t('wspolne.blad_nieznany'));
      const patch = a.patch as K.ProfilePatch;
      if (patch.name?.trim()) pr.name = patch.name.trim();
      if (patch.color) pr.color = pr.icon.color = patch.color;
      if (patch.glyph) pr.icon = { color: pr.color, glyph: patch.glyph, url: null };
      if (patch.online !== undefined) pr.online = patch.online;
      if (pr.id === people.activeProfileId) settings.profileColor = bubble.accent = pr.color;
      wyslij('people', 'settings', 'bubble');
    },
    profile_duplicate: (a) => {
      const o = aktywnaOsoba()!;
      const zrodlo = o.profiles.find((x) => x.id === a.profileId);
      if (!zrodlo) throw blad('notFound', t('wspolne.blad_nieznany'));
      const nowy = { ...kopia(zrodlo), id: id10(r), name: t('profil.kopia_nazwa', { profil: zrodlo.name }) };
      o.profiles.push(nowy);
      wyslij('people');
      return nowy.id;
    },
    profile_remove: (a) => {
      const o = aktywnaOsoba()!;
      if (o.profiles.length < 2) throw blad('invalid', t('wspolne.blad_nieznany'));
      o.profiles = o.profiles.filter((x) => x.id !== a.profileId);
      if (people.activeProfileId === a.profileId) komendy.profile_switch({ profileId: o.profiles[0].id });
      wyslij('people');
    },

    history_query: (a) => zapytanieHistorii(a.query as K.HistoryQuery),
    history_session: (a) => sesje.find((s) => s.id === a.sessionId) ?? null,
    history_session_delete: (a) => {
      const i = sesje.findIndex((s) => s.id === a.sessionId);
      if (i >= 0) sesje.splice(i, 1);
      wersjaHistorii++;
      przeliczDzis();
      levels = przeliczPoziomy();
      wyslij('history', 'people', 'levels');
      toast(t('sesja.usunieta'));
    },
    history_session_hide_online: (a) => {
      const s = sesje.find((x) => x.id === a.sessionId);
      if (s?.online) s.online.hidden = Boolean(a.hidden);
      wersjaHistorii++;
      przeliczDzis();
      wyslij('history');
    },

    levels_refresh: () => wyslij('levels'),
    levels_sample_popup: () => emituj('wl:popupy', { items: [] }),

    account_login: (a) => new Promise((ok, nie) => {
      if (!String(a.email).includes('@')) return nie(blad('invalid', t('konto.blad.email'), { email: t('konto.blad.email') }));
      if (!a.password) return nie(blad('invalid', t('konto.blad.logowanie')));
      account.busy = ['login'];
      wyslij('account');
      pozniej(700, () => {
        const o = aktywnaOsoba()!;
        o.hasAccount = true;
        o.accountName = 'Damianejro';
        Object.assign(account, { busy: [], loggedIn: true, name: 'Damianejro', email: String(a.email), userId: 'u-damianejro' });
        wyslij('account', 'people');
        toast(t('konto.zalogowano', { pseudonim: account.name }));
        emituj('wl:nawigacja', { to: 'online', reason: 'signedIn' });
        ok(account);
      });
    }),
    account_register: (a) => {
      const fields: Record<string, string> = {};
      if (String(a.code).length < 4) fields.code = t('konto.blad.kod');
      if (!String(a.email).includes('@')) fields.email = t('konto.blad.email');
      if (String(a.password).length < 10) fields.password = t('konto.blad.haslo_krotkie', { liczba: 10 });
      if (String(a.name).length < 3) fields.name = t('konto.blad.pseudonim_dlugosc', { min: 3, max: 24 });
      if (Object.keys(fields).length) throw blad('invalid', Object.values(fields)[0], fields);
      Object.assign(account, { loggedIn: true, verified: false, name: String(a.name), email: String(a.email), message: t('konto.rejestracja.gotowe') });
      wyslij('account');
      return account;
    },
    account_remind_password: () => {
      account.message = t('konto.reset.wyslano');
      wyslij('account');
    },
    account_logout: () => {
      const o = aktywnaOsoba();
      if (o) Object.assign(o, { hasAccount: false, accountName: null });
      Object.assign(account, { loggedIn: false, name: '', email: '', userId: '' });
      Object.assign(online, pustyOnline());
      wyslij('account', 'online', 'people');
      toast(t('konto.wylogowano'));
      emituj('wl:nawigacja', { to: 'settings', reason: 'settings' });
    },
    account_delete: (a) => {
      if (!a.password) throw blad('invalid', t('konto.blad.logowanie'));
      komendy.account_logout({});
    },
    account_change_password: (a) => {
      if (String(a.newPassword).length < 10) throw blad('invalid', t('konto.blad.haslo_krotkie', { liczba: 10 }), { newPassword: t('konto.blad.haslo_krotkie', { liczba: 10 }) });
    },
    account_send_verification: () => {
      account.verifyPolling = true;
      wyslij('account');
      toast(t('konto.email.wyslano_toast', { email: account.email }));
      return 'wyslano';
    },
    account_check_verified: () => account.verified,
    account_set_remember: (a) => { account.remember = Boolean(a.on); wyslij('account'); },
    account_set_upload: (a) => { account.upload = Boolean(a.on); wyslij('account'); },
    account_set_visibility: (a) => { account.visibility = a.value as K.SessionVisibility; wyslij('account'); },
    account_set_live: (a) => { account.live = Boolean(a.on); wyslij('account'); },
    account_set_profile_visibility: (a) => { account.profileVisibility = a.value as K.ProfileVisibility; wyslij('account'); },
    account_set_games_visibility: (a) => { account.gamesVisibility = a.value as K.GamesVisibility; wyslij('account'); },
    account_rename: (a) => {
      const n = String(a.name);
      if (n.length < 3 || n.length > 24) throw blad('invalid', t('konto.blad.pseudonim_dlugosc', { min: 3, max: 24 }));
      account.name = n;
      wyslij('account');
    },
    account_accept_terms: () => {
      account.terms = account.termsCurrent;
      account.termsOutdated = false;
      wyslij('account');
      toast(t('konto.regulamin.dziekujemy'));
    },
    account_upload_now: () => {
      const n = account.pendingUploads;
      account.pendingUploads = 0;
      wyslij('account');
      return n;
    },
    account_export_my_data: () => 'dane\\moje-dane-konto-20261126-120000.json',

    online_load: (a) => {
      if (!account.loggedIn) throw blad('loggedOut', t('konto.blad.niezalogowany'));
      const tab = a.tab as K.OnlineTab;
      const pierwszy = tab === 'friends' ? online.friends === null : tab === 'ranking' ? online.ranking === null : online.people === null;
      if (pierwszy || a.force) {
        online.loading = tab === 'friends' ? ['stats', 'friends', 'invitations'] : tab === 'ranking' ? ['ranking'] : ['people'];
        wyslij('online');
      }
      pozniej(400, () => {
        if (tab === 'ranking') online.ranking = ranking(online.ranking?.scope ?? 'tydzien');
        if (tab === 'people') komendy.online_people_search(online.peopleQuery);
        online.loading = [];
        wyslij('online');
      });
    },
    online_ranking_scope: (a) => {
      online.ranking = ranking(a.scope as K.RankingScope);
      wyslij('online');
    },
    online_people_search: (a) => {
      const search = String(a.search ?? '').toLowerCase();
      const wszyscy = [...znajomi.map((f) => ({ id: f.id, name: f.name, level: f.level, weekSteps: f.week.steps, place: null, friend: true })),
        { id: 'u-ania', name: 'AniaNaBieżni', level: 31, weekSteps: 68400, place: 2, friend: false },
        { id: 'u-rekord', name: 'Rekordzista', level: 41, weekSteps: 90210, place: 1, friend: false }];
      online.peopleQuery = { search: String(a.search ?? ''), page: Number(a.page ?? 1) };
      online.people = { page: online.peopleQuery.page, hasNext: false, people: wszyscy.filter((x) => x.name.toLowerCase().includes(search)) };
      wyslij('online');
    },
    online_person_open: (a) => {
      const f = znajomi.find((x) => x.id === a.userId);
      const ja = a.userId === account.userId;
      const person: K.OnlinePerson = {
        id: String(a.userId), name: ja ? account.name : f?.name ?? 'AniaNaBieżni', me: ja,
        level: ja ? levels?.level ?? 1 : f?.level ?? 31, xp: 25300, achievementsCount: 17,
        achievements: (levels?.progress ?? []).filter((x) => x.earnedAt !== null).slice(0, 17).map((x) => ({ id: x.id, earnedAt: x.earnedAt! })),
        since: teraz() - 200 * 86400, treadmillModel: model, friend: !!f,
        week: f?.week ?? tydzien(41230), total: { steps: 812400, distanceKm: 601.8, activeS: 1_300_000, sessions: 214 },
        games: f ? null : online.stats?.games ?? null, gamesHidden: !!f,
      };
      online.person = person;
      wyslij('online');
      emituj('wl:nawigacja', { to: 'onlinePerson', reason: 'personOpened' });
      return person;
    },
    online_invite_name: (a) => {
      toast(t('znajomi.zaproszono', { pseudonim: String(a.name) }));
      return 'oczekuje';
    },
    online_invite_user: () => 'oczekuje',
    online_invitation_answer: (a) => {
      if (online.invitations) online.invitations.incoming = online.invitations.incoming.filter((x) => x.id !== a.invitationId);
      wyslij('online');
      toast(t('znajomi.odpowiedziano'));
    },
    online_invitation_withdraw: (a) => {
      if (online.invitations) online.invitations.sent = online.invitations.sent.filter((x) => x.id !== a.invitationId);
      wyslij('online');
    },
    online_friend_remove: (a) => {
      online.friends = (online.friends ?? []).filter((x) => x.id !== a.friendId);
      wyslij('online');
      toast(t('znajomi.usunieto'));
    },
    online_registration_code: () => {
      online.regCode = { code: 'WL-7K3M-Q9', validUntil: teraz() + 14 * 86400 };
      wyslij('online');
      return online.regCode;
    },

    backup_export_summary: (a) => {
      const c = a.choice as K.ExportChoice;
      const osoby = Object.values(c.people).filter((l) => l.length > 0);
      return { people: osoby.length, profiles: osoby.flat().length, sessions: c.results ? sesje.length : 0, bytes: 48_000 + sesje.length * 900 };
    },
    backup_export: (a) => {
      const s = komendy.backup_export_summary(a) as K.ExportSummary;
      toast(t('kopia.eksport.gotowe', { liczba: s.people, profile: s.profiles, sesje: s.sessions }));
      return s;
    },
    backup_import_open: () => ({
      file: 'kopia-walklevel-2026-11-20.bieznia', appVersion: '0.5.2', hasSettings: true, hasResults: true,
      description: 'WalkLevel 0.5.2 · 2 osoby · 4 profile gier · 61 sesji',
      people: [
        { id: ludzie[0].id, name: ludzie[0].name, color: ludzie[0].color, profiles: 3, sessions: 48, exists: true },
        { id: 'nowa', name: 'Kuba', color: 'Limonka', profiles: 1, sessions: 13, exists: false },
      ],
    }) satisfies K.ImportPreview,
    backup_import_apply: () => {
      app.latestBackup = 'przed-importem-20261126-120000';
      wyslij('app');
      return { peopleAdded: 1, peopleMerged: 1, profilesAdded: 1, profilesUpdated: 2, sessionsAdded: 13, backup: app.latestBackup } satisfies K.ImportReport;
    },
    backup_restore_latest: () => {
      app.latestBackup = null;
      wyslij('app');
      toast(t('kopia.import.przywrocono'));
    },

    report_send: (a) => new Promise((ok, nie) => {
      const opis = String(a.description).trim();
      if (opis.length === 0) return nie(blad('invalid', t('zgloszenie.blad.pusty')));
      if (opis.length < 5) return nie(blad('invalid', t('zgloszenie.blad.krotki', { liczba: 5 - opis.length })));
      report.busy = true;
      report.status = t('zgloszenie.przygotowuje');
      wyslij('report');
      pozniej(1000, () => {
        Object.assign(report, { busy: false, sentNumber: 'WL-1042', status: t('zgloszenie.wyslano', { numer: 'WL-1042' }) });
        wyslij('report');
        ok(undefined);
      });
    }),
    report_save: () => {
      report.lastFile = 'zgloszenie-20261126-120000.zip';
      report.status = t('zgloszenie.zapisano', { plik: report.lastFile, email_autora: 'kontakt@damnit.com.pl' });
      wyslij('report');
      return report.lastFile;
    },
    update_check: (a) => {
      if (a.install) throw blad('unavailable', t('glowny.pasek.aktualizuj_niedostepne'));
      update.busy = true;
      update.status = t('aktualizacja.sprawdzam');
      wyslij('update');
      pozniej(900, () => {
        update.busy = false;
        update.status = update.available ? t('aktualizacja.dostepna_krotko', { nowa_wersja: update.available }) : t('aktualizacja.najnowsza', { wersja: app.version });
        wyslij('update');
      });
    },

    bubble_click: () => komendy.control_toggle({}),
    bubble_open: () => undefined,
    bubble_drag_start: () => undefined,
    bubble_drag_end: () => undefined,
    popup_ready: () => ({ items: [] }) satisfies K.PopupsView,
  };

  function statics(): K.StaticData {
    return {
      appName: 'WalkLevel', author: 'damnit.com.pl Damian Skrzypiec', homepage: 'https://damnit.com.pl',
      contact: 'kontakt@damnit.com.pl', palette: PALETA, defaultColor: 'Mięta', runAltColor: 'Malina',
      glyphs: GLIFY, defaultGlyph: GLIF_POSTAC,
      modeNames: { keyboard: t('tryb.klawisze.nazwa'), gamepad: t('tryb.pad.nazwa'), treadmill: t('tryb.chodzenie.nazwa'), training: t('tryb.trening.nazwa') },
      maxSpeed: 12, hysteresis: 0.6, thresholdGap: 0.8, hotkeys: { simSlower: 'F6', simFaster: 'F7' },
      padPresets: {
        2: [[0, 1], [100, 2.5]], 3: [[0, 1], [50, 2], [100, 3]],
        4: [[0, 1], [25, 1.5], [50, 2.5], [100, 4]], 5: [[0, 1], [25, 1.5], [50, 2.5], [75, 3.5], [100, 4.5]],
      },
      padMinGap: 5, trainingRange: [0.5, 20], padSpeedRange: [0.5, 12],
      reportPeriods: [
        { seconds: 3600, label: t('zgloszenie.okres.1h') }, { seconds: 21600, label: t('zgloszenie.okres.6h') },
        { seconds: 86400, label: t('zgloszenie.okres.24h') }, { seconds: 604800, label: t('zgloszenie.okres.7d') },
      ],
      reportDefaultS: 21600, reportMinChars: 5,
      account: {
        nameMin: 3, nameMax: 24, nameRe: '^[A-Za-z0-9 _.\\-ĄĆĘŁŃÓŚŹŻąćęłńóśźż]+$',
        reservedNames: ['admin', 'administrator', 'moderator', 'damnit', 'bieznia', 'bieżnia'],
        emailRe: '^[^@\\s]{1,64}@[^@\\s]{1,190}\\.[^@\\s]{2,24}$', passwordMin: 10, codeMin: 4, gameNameMax: 40,
      },
      definitions: defs,
      driverNames: { kingsmith: t('modele.rodzaj.kingsmith'), ftms: t('modele.rodzaj.ftms') },
    };
  }

  // Narzędzie z wyglad.md 11.2 dostępne w konsoli przeglądarki i dla testu: window.wystaje()
  void import('./dev/wystaje').then((m) => Object.assign(window, { wystaje: m.wystaje }));

  return {
    async wywolaj(komenda, argumenty = {}) {
      const f = komendy[komenda];
      if (!f) throw blad('unavailable', `Atrapa nie zna komendy ${komenda}`);
      return kopia(await f(argumenty));
    },
    async sluchaj(zdarzenie, obsluga) {
      const zbior = sluchacze.get(zdarzenie) ?? new Set();
      sluchacze.set(zdarzenie, zbior);
      zbior.add(obsluga);
      return () => zbior.delete(obsluga);
    },
  };
}
