// Teksty interfejsu w bieżącym języku (docs/i18n.md). Źródło kluczy: docs/texts.md
// (`npm run texts`), komunikaty w ICU MessageFormat, silnik: src/lib/i18n.svelte.ts.
// Użycie: t('glowny.status.polaczono.tytul', { model: 'KingSmith X21' })
//         t('glowny.osoby.dzis', { kroki: 1234 })  ->  „dziś 1 234 kroki”
import { isTextKey, translate, type TextKey, type Values } from './i18n.svelte';

export type Klucz = TextKey;
export type Zmienne = Values;

/** Czy napis jest kluczem słownika (dla kluczy składanych w locie). */
export function jestKlucz(k: string): k is Klucz {
  return isTextKey(k);
}

/**
 * Tekst ze słownika z podstawionymi zmiennymi: `{nazwa}` - wartość (liczby całkowite
 * z separatorem tysięcy języka, ułamki z jednym miejscem; inną dokładność podaj jako gotowy
 * napis: `liczba(km, 2)`), `{nazwa, plural, ...}` - forma według reguł CLDR języka.
 * Brak zmiennej zostawia `{nazwa}` w tekście (widać to od razu na ekranie).
 */
export function t(klucz: Klucz, zmienne: Zmienne = {}): string {
  return translate(klucz, zmienne);
}

/** Tekst dzielony na części „ / ” (np. tytuł / opis kafelka, warianty „jak Windows / jasny / ciemny”). */
export function tl(klucz: Klucz, zmienne: Zmienne = {}): string[] {
  return t(klucz, zmienne).split(' / ');
}

/** Czas ruchu: „12 min”, „1 h 05 min”. */
export function czas(sekundy: number): string {
  const minuty = Math.floor(Math.max(0, sekundy) / 60);
  const godziny = Math.floor(minuty / 60);
  return godziny > 0
    ? t('historia.czas.godziny', { godziny, minuty: String(minuty % 60).padStart(2, '0') })
    : t('historia.czas.minuty', { minuty });
}

/** „Konto online · Damianejro” -> tytuł „Konto online” i podpis „Damianejro” (design.md 9.1). */
export function tytulIPodpis(tekst: string): { tytul: string; podpis: string } {
  const i = tekst.indexOf(' · ');
  return i < 0 ? { tytul: tekst, podpis: '' } : { tytul: tekst.slice(0, i), podpis: tekst.slice(i + 3) };
}
