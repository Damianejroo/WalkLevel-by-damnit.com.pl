// Liczby po polsku (docs/texts.md 0.2, docs/design.md 4.2): przecinek dziesiętny, tysiące
// rozdzielone niełamliwą wąską spacją U+202F (także dla 4 cyfr), jednostka po U+00A0.

const NBSP = ' ';
const WASKA = ' ';

/** `liczba(12345)` -> „12 345”, `liczba(2.5, 1)` -> „2,5”, `liczba(6, 1)` -> „6,0”. */
export function liczba(x: number, miejsca = 0): string {
  if (!Number.isFinite(x)) return '–';
  const znak = x < 0 ? '-' : '';
  const [cale, ulamek] = Math.abs(x).toFixed(miejsca).split('.');
  const grupy = cale.replace(/\B(?=(\d{3})+(?!\d))/g, WASKA);
  return znak + grupy + (ulamek ? ',' + ulamek : '');
}

/** Liczba z jednostką po twardej spacji: `zJednostka(2.5, 'km/h', 1)` -> „2,5 km/h”. */
export function zJednostka(x: number, jednostka: string, miejsca = 0): string {
  return liczba(x, miejsca) + NBSP + jednostka;
}
