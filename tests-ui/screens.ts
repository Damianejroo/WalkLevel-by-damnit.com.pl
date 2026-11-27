// Lista ekranów i scenariuszy atrapy do testu „nic nie wystaje” i zrzutów (npm run test:ui).
// Nowy ekran: dopisz { ekran: '<trasa>', scenariusze: [...] }. Trasy: src/lib/state.svelte.ts,
// scenariusze: SCENARIUSZE w src/dev/mock.ts.

// Wpis może mieć własne `szerokosci` (panel znajomych: 720 = minimum okna z pełnym panelem,
// 484 = z paskiem); bez nich - DOMYSLNE.
export const DOMYSLNE = [420, 740, 1400];
export const SZEROKOSCI = [420, 484, 720, 740, 1400];
export const MOTYWY = ['jasny', 'ciemny'];
export const WYSOKOSC = 900;
// Other languages (docs/i18n.md): English and the pseudo-language en-XA (longer, accented texts),
// every screen at the narrowest width in the light theme.
export const JEZYKI = ['en', 'en-XA'];

// Each screen x scenario x width once (the test stops on a repeat).
export const EKRANY: { ekran: string; scenariusze: string[]; szerokosci?: number[] }[] = [
  { ekran: 'main', scenariusze: ['domyslny', 'trening', 'brak-biezni', 'wylogowany', 'dlugie', 'klawisze', 'pad', 'chodzenie', 'proba', 'symulator', 'szukanie', 'ponawiam', 'bt-wylaczony', 'rozlaczona', 'dlugie-pad', 'klawisze-stop'] },
  {
    ekran: 'settings',
    scenariusze: ['domyslny', 'ustawienia', 'ustawienia-chodzenie', 'ustawienia-trening', 'ustawienia-wylogowany', 'ustawienia-dlugie', 'ustawienia-klawisze-stop', 'kategoria-jezyk'],
  },
  { ekran: 'history', scenariusze: ['domyslny', 'pusto', 'dlugie'] },
  { ekran: 'achievements', scenariusze: ['domyslny', 'dlugie', 'pusto'] },
  { ekran: 'trail', scenariusze: ['domyslny', 'dlugie', 'pusto'] },
  { ekran: 'online', scenariusze: ['domyslny', 'dlugie', 'wylogowany'] },
  { ekran: 'onlinePerson', scenariusze: ['domyslny', 'dlugie', 'obcy'] },
  { ekran: 'report', scenariusze: ['domyslny', 'zgloszenie-wysylanie', 'zgloszenie-wyslano', 'zgloszenie-email'] },
  { ekran: 'log', scenariusze: ['domyslny', 'dlugie'] },
  { ekran: 'export', scenariusze: ['domyslny', 'dlugie'] },
  { ekran: 'import', scenariusze: ['domyslny', 'dlugie', 'kopia-wybor'] },
  { ekran: 'models', scenariusze: ['domyslny', 'dlugie'] },
  { ekran: 'wizard', scenariusze: ['pierwsze', 'domyslny'] },
  { ekran: 'devices', scenariusze: ['lista-biezni', 'domyslny', 'bt-wylaczony', 'skanowanie'] },
  { ekran: 'profile', scenariusze: ['domyslny', 'dlugie'] },
  { ekran: 'profileNew', scenariusze: ['domyslny', 'dlugie'] },
  { ekran: 'person', scenariusze: ['domyslny', 'wylogowany', 'dlugie'] },
  { ekran: 'session', scenariusze: ['domyslny', 'dlugie'] },
  { ekran: 'account', scenariusze: ['wylogowany', 'domyslny'] },
  { ekran: 'wzornik', scenariusze: ['domyslny', 'dlugie'] },
  // panel znajomych (0.6.2): pełny / pasek / wyłączony, lewa / prawa, bez konta, duży tekst;
  // 0.6.3: pasek z kropką „idzie teraz”, licznikiem i inicjałem (obie strony), nikt nie idzie
  { ekran: 'main', szerokosci: [484, 720], scenariusze: ['domyslny'] },
  {
    ekran: 'main',
    szerokosci: [484, 720, 740, 1400],
    scenariusze: [
      'panel-waski', 'panel-lewa', 'panel-lewa-waski', 'panel-wylaczony', 'panel-duzy', 'panel-bez-konta', 'panel-dlugie',
      'panel-waski-dlugie', 'panel-lewa-waski-dlugie', 'panel-waski-nikt',
    ],
  },
  { ekran: 'online', szerokosci: [720], scenariusze: ['domyslny', 'panel-duzy'] },
  // karta błędu (serwer kont nie odpowiada) w zakładce i w panelu
  { ekran: 'online', szerokosci: [420, 720, 1400], scenariusze: ['online-blad'] },
  { ekran: 'settings', szerokosci: [720], scenariusze: ['ustawienia', 'panel-duzy'] },
];

// Osobne okna (docs/contract.md 7.2-7.3): strona <strona>?demo=1&<zapytanie>, okno stałego rozmiaru
// na podglądzie z marginesem 16 px (--screen=popup / --screen=przycisk). Zrzuty w skali 2:
// docs/screenshots/<ekran>-<scenariusz>-<motyw>.png. Nic nie może wyjść poza prostokąt okna.
export const OKNA = [
  {
    ekran: 'popup', strona: 'popup.html', okno: { szer: 380, wys: 308 },
    scenariusze: [{ nazwa: 'domyslny', zapytanie: 'stan=domyslny' }, { nazwa: 'dlugie', zapytanie: 'stan=dlugie' }],
  },
  {
    ekran: 'przycisk', strona: 'bubble.html', okno: { szer: 56, wys: 56 },
    scenariusze: [
      { nazwa: 'wylaczony', zapytanie: 'stan=domyslny' },
      // Trening: okno 88 × 84 (kółko + kolumna +/poziom/− i Stop, window.rs BUBBLE_TRAINING_*)
      { nazwa: 'chod', zapytanie: 'stan=trening&poziom=1', okno: { szer: 88, wys: 84 } },
      { nazwa: 'bieg', zapytanie: 'stan=trening&poziom=4', okno: { szer: 88, wys: 84 } },
      // poza Treningiem, gdy taśma jedzie: Stop pod kółkiem, okno 56 × 84 (window.rs BUBBLE_STOP_H)
      { nazwa: 'klawisze', zapytanie: 'stan=klawisze', okno: { szer: 56, wys: 84 } },
      { nazwa: 'jedzie-wylaczony', zapytanie: 'stan=jedzie', okno: { szer: 56, wys: 84 } },
      { nazwa: 'pad-jedzie', zapytanie: 'stan=pad', okno: { szer: 56, wys: 84 } },
      { nazwa: 'bez-danych', zapytanie: 'stan=klawisze&bieznia=rozlaczona' },
    ],
  },
];
