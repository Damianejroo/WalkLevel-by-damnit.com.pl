// Daty i godziny sesji (czas lokalny komputera): „Dziś”/„Wczoraj”/„24.11”, „18:20”, „26.11.2026”.
import { t } from '../../lib/texts';
import { data as formatDaty } from '../../lib/format';

/** Początek dnia lokalnego (ms). */
const poczatekDnia = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();

/** „Dziś”, „Wczoraj” albo „24.11” dla chwili `utcS` (sekundy UTC). */
export function dzien(utcS: number, teraz = new Date()): string {
  const d = new Date(utcS * 1000);
  const roznica = Math.round((poczatekDnia(teraz) - poczatekDnia(d)) / 86_400_000);
  if (roznica === 0) return t('historia.sesje.dzis');
  if (roznica === 1) return t('historia.sesje.wczoraj');
  return formatDaty(d, 'dzien');
}

/** „18:20”. */
export function godzina(utcS: number): string {
  return formatDaty(new Date(utcS * 1000), 'godzina');
}

/** „26.11.2026”. */
export function data(utcS: number): string {
  return formatDaty(new Date(utcS * 1000));
}
