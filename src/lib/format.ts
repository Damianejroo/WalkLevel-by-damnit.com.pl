// Liczby i daty w interfejsie (docs/i18n.md, docs/design.md 4.2). Liczby według języka
// (Intl.NumberFormat: PL „2,5” i „12 345”, EN „2.5” i „12,345”), jednostka po U+00A0.
// Daty DD.MM.RRRR we wszystkich językach (decyzja właściciela) - tylko przez `data`.
import { formatNumber } from './i18n.svelte';

const NBSP = ' ';
const dwa = (n: number) => String(n).padStart(2, '0');

/** `liczba(12345)` -> „12 345”, `liczba(2.5, 1)` -> „2,5”, `liczba(6, 1)` -> „6,0” (po polsku). */
export function liczba(x: number, miejsca = 0): string {
  return formatNumber(x, miejsca);
}

/** Liczba z jednostką po twardej spacji: `zJednostka(2.5, 'km/h', 1)` -> „2,5 km/h”. */
export function zJednostka(x: number, jednostka: string, miejsca = 0): string {
  return liczba(x, miejsca) + NBSP + jednostka;
}

/** Jedyne miejsce formatu daty: „26.11.2026” (`pelna`), „26.11” (`dzien`), „18:20” (`godzina`). */
export function data(d: Date, rodzaj: 'pelna' | 'dzien' | 'godzina' = 'pelna'): string {
  if (rodzaj === 'godzina') return `${dwa(d.getHours())}:${dwa(d.getMinutes())}`;
  const dzien = `${dwa(d.getDate())}.${dwa(d.getMonth() + 1)}`;
  return rodzaj === 'dzien' ? dzien : `${dzien}.${d.getFullYear()}`;
}
