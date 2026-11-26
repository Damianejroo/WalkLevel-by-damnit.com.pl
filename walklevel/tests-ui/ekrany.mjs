// Lista ekranów i scenariuszy atrapy do testu „nic nie wystaje” i zrzutów (npm run test:ui).
// Nowy ekran: dopisz { ekran: '<trasa>', scenariusze: [...] }. Trasy: src/lib/stan.svelte.ts,
// scenariusze: SCENARIUSZE w src/lib/atrapa.ts.

export const SZEROKOSCI = [420, 740, 1400];
export const MOTYWY = ['jasny', 'ciemny'];
export const WYSOKOSC = 900;

export const EKRANY = [
  { ekran: 'main', scenariusze: ['domyslny', 'trening', 'brak-biezni', 'wylogowany', 'dlugie', 'klawisze', 'pad', 'chodzenie', 'proba', 'symulator', 'szukanie', 'ponawiam', 'bt-wylaczony', 'rozlaczona', 'dlugie-pad'] },
  { ekran: 'settings', scenariusze: ['domyslny'] },
  { ekran: 'main', scenariusze: ['domyslny', 'trening', 'brak-biezni', 'wylogowany', 'dlugie'] },
  {
    ekran: 'settings',
    scenariusze: ['domyslny', 'ustawienia', 'ustawienia-chodzenie', 'ustawienia-trening', 'ustawienia-wylogowany', 'ustawienia-dlugie'],
  },
  { ekran: 'history', scenariusze: ['domyslny'] },
  { ekran: 'history', scenariusze: ['domyslny', 'pusto', 'dlugie'] },
  { ekran: 'achievements', scenariusze: ['domyslny'] },
  { ekran: 'trail', scenariusze: ['domyslny'] },
  { ekran: 'achievements', scenariusze: ['domyslny', 'dlugie', 'pusto'] },
  { ekran: 'trail', scenariusze: ['domyslny', 'dlugie', 'pusto'] },
  { ekran: 'online', scenariusze: ['domyslny'] },
  { ekran: 'onlinePerson', scenariusze: ['domyslny'] },
  { ekran: 'online', scenariusze: ['domyslny', 'dlugie', 'wylogowany'] },
  { ekran: 'onlinePerson', scenariusze: ['domyslny', 'dlugie'] },
  { ekran: 'report', scenariusze: ['domyslny'] },
  { ekran: 'log', scenariusze: ['domyslny'] },
  { ekran: 'report', scenariusze: ['domyslny', 'zgloszenie-wysylanie', 'zgloszenie-wyslano', 'zgloszenie-email'] },
  { ekran: 'log', scenariusze: ['domyslny', 'dlugie'] },
  { ekran: 'export', scenariusze: ['domyslny'] },
  { ekran: 'import', scenariusze: ['domyslny'] },
  { ekran: 'export', scenariusze: ['domyslny', 'dlugie'] },
  { ekran: 'import', scenariusze: ['domyslny', 'dlugie', 'kopia-wybor'] },
  { ekran: 'models', scenariusze: ['domyslny'] },
  { ekran: 'wizard', scenariusze: ['pierwsze'] },
  { ekran: 'devices', scenariusze: ['domyslny'] },
  { ekran: 'models', scenariusze: ['domyslny', 'dlugie'] },
  { ekran: 'wizard', scenariusze: ['pierwsze', 'domyslny'] },
  { ekran: 'devices', scenariusze: ['lista-biezni', 'domyslny', 'bt-wylaczony', 'skanowanie'] },
  { ekran: 'profile', scenariusze: ['domyslny'] },
  { ekran: 'person', scenariusze: ['domyslny'] },
  { ekran: 'session', scenariusze: ['domyslny', 'dlugie'] },
  { ekran: 'profile', scenariusze: ['domyslny', 'dlugie'] },
  { ekran: 'profileNew', scenariusze: ['domyslny', 'dlugie'] },
  { ekran: 'person', scenariusze: ['domyslny', 'wylogowany', 'dlugie'] },
  { ekran: 'session', scenariusze: ['domyslny'] },
  { ekran: 'account', scenariusze: ['wylogowany'] },
  { ekran: 'session', scenariusze: ['domyslny'] },
  { ekran: 'account', scenariusze: ['wylogowany', 'domyslny'] },
  { ekran: 'wzornik', scenariusze: ['domyslny', 'dlugie'] },
];

// Osobne okna (docs/kontrakt.md 7.2-7.3): strona <strona>?demo=1&<zapytanie>, okno stałego rozmiaru
// na podglądzie z marginesem 16 px (--ekran=popup / --ekran=przycisk). Zrzuty w skali 2:
// docs/zrzuty/<ekran>-<scenariusz>-<motyw>.png. Nic nie może wyjść poza prostokąt okna.
export const OKNA = [
  {
    ekran: 'popup', strona: 'popup.html', okno: { szer: 380, wys: 308 },
    scenariusze: [{ nazwa: 'domyslny', zapytanie: 'stan=domyslny' }, { nazwa: 'dlugie', zapytanie: 'stan=dlugie' }],
  },
  {
    ekran: 'przycisk', strona: 'przycisk.html', okno: { szer: 56, wys: 56 },
    scenariusze: [
      { nazwa: 'wylaczony', zapytanie: 'stan=domyslny' },
      { nazwa: 'chod', zapytanie: 'stan=trening&poziom=1' },
      { nazwa: 'bieg', zapytanie: 'stan=trening&poziom=4' },
      { nazwa: 'bez-danych', zapytanie: 'stan=klawisze&bieznia=rozlaczona' },
    ],
  },
];
