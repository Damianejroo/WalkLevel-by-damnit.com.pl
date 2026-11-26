// Daty i godziny sesji (czas lokalny komputera): „Dziś”/„Wczoraj”/„24.11”, „18:20”, „26.11.2026”.
import { t } from '../../lib/teksty';

const dwa = (n: number) => String(n).padStart(2, '0');

/** Początek dnia lokalnego (ms). */
const poczatekDnia = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();

/** „Dziś”, „Wczoraj” albo „24.11” dla chwili `utcS` (sekundy UTC). */
export function dzien(utcS: number, teraz = new Date()): string {
  const d = new Date(utcS * 1000);
  const roznica = Math.round((poczatekDnia(teraz) - poczatekDnia(d)) / 86_400_000);
  if (roznica === 0) return t('historia.sesje.dzis');
  if (roznica === 1) return t('historia.sesje.wczoraj');
  return `${dwa(d.getDate())}.${dwa(d.getMonth() + 1)}`;
}

/** „18:20”. */
export function godzina(utcS: number): string {
  const d = new Date(utcS * 1000);
  return `${dwa(d.getHours())}:${dwa(d.getMinutes())}`;
}

/** „26.11.2026”. */
export function data(utcS: number): string {
  const d = new Date(utcS * 1000);
  return `${dwa(d.getDate())}.${dwa(d.getMonth() + 1)}.${d.getFullYear()}`;
}
