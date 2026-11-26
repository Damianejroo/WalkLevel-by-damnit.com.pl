// Lista ekranów i scenariuszy atrapy do testu „nic nie wystaje” i zrzutów (npm run test:ui).
// Nowy ekran: dopisz { ekran: '<trasa>', scenariusze: [...] }. Trasy: src/lib/stan.svelte.ts,
// scenariusze: SCENARIUSZE w src/lib/atrapa.ts.

export const SZEROKOSCI = [420, 740, 1400];
export const MOTYWY = ['jasny', 'ciemny'];
export const WYSOKOSC = 900;

export const EKRANY = [
  { ekran: 'main', scenariusze: ['domyslny', 'trening', 'brak-biezni', 'wylogowany', 'dlugie'] },
  { ekran: 'settings', scenariusze: ['domyslny'] },
  { ekran: 'history', scenariusze: ['domyslny'] },
  { ekran: 'achievements', scenariusze: ['domyslny'] },
  { ekran: 'trail', scenariusze: ['domyslny'] },
  { ekran: 'online', scenariusze: ['domyslny'] },
  { ekran: 'onlinePerson', scenariusze: ['domyslny'] },
  { ekran: 'report', scenariusze: ['domyslny'] },
  { ekran: 'log', scenariusze: ['domyslny'] },
  { ekran: 'export', scenariusze: ['domyslny', 'dlugie'] },
  { ekran: 'import', scenariusze: ['domyslny', 'dlugie'] },
  { ekran: 'models', scenariusze: ['domyslny'] },
  { ekran: 'wizard', scenariusze: ['pierwsze'] },
  { ekran: 'devices', scenariusze: ['domyslny'] },
  { ekran: 'profile', scenariusze: ['domyslny'] },
  { ekran: 'person', scenariusze: ['domyslny'] },
  { ekran: 'session', scenariusze: ['domyslny'] },
  { ekran: 'account', scenariusze: ['wylogowany'] },
  { ekran: 'wzornik', scenariusze: ['domyslny', 'dlugie'] },
];
