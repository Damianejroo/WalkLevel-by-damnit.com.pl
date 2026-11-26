// Motyw, akcent profilu i wielkość tekstu (docs/wyglad.md 2.3-2.4, 6.3). Zmiana to tylko
// podmiana zmiennych CSS na <html> - bez przebudowy widoków.
import type { ColorName, PaletteColor } from './typy';

const BIEG_MALINA = { d: '#f0426e', l: '#e21246' };

/** Motyw „light”/„dark” na <html data-theme>; przy zmianie przenikanie 160 ms (View Transitions). */
export function ustawMotyw(motyw: 'light' | 'dark') {
  const root = document.documentElement;
  if (root.dataset.theme === motyw) return;
  const zmien = () => {
    root.dataset.theme = motyw;
  };
  const spokojnie = matchMedia('(prefers-reduced-motion: reduce)').matches;
  if (root.dataset.theme && !spokojnie && typeof document.startViewTransition === 'function') {
    document.startViewTransition(zmien);
  } else {
    zmien();
  }
}

/** Akcent = kolor aktywnego profilu. Profile w odcieniach 10-70° dostają bieg w kolorze Malina. */
export function ustawAkcent(nazwa: ColorName, paleta: PaletteColor[]) {
  const k = paleta.find((x) => x.name === nazwa) ?? paleta.find((x) => x.name === 'Mięta');
  const s = document.documentElement.style;
  if (!k || k.name === 'Mięta') {
    for (const v of ['--accent-d', '--accent-l', '--run-d', '--run-l']) s.removeProperty(v);
    return;
  }
  s.setProperty('--accent-d', k.dark);
  s.setProperty('--accent-l', k.light);
  if (k.hue >= 10 && k.hue <= 70) {
    s.setProperty('--run-d', BIEG_MALINA.d);
    s.setProperty('--run-l', BIEG_MALINA.l);
  } else {
    s.removeProperty('--run-d');
    s.removeProperty('--run-l');
  }
}

/** „Duży tekst”: wszystkie napisy ×1,15 (odstępy bez zmian). */
export function ustawDuzyTekst(duzy: boolean) {
  document.documentElement.style.setProperty('--text-scale', duzy ? '1.15' : '1');
}

/** Kolor próbki (awatar, wykres, paleta) - odcień „d” z palety. */
export function probka(nazwa: ColorName, paleta: PaletteColor[]): string {
  return (paleta.find((x) => x.name === nazwa) ?? paleta.find((x) => x.name === 'Mięta'))?.dark ?? '#8fc2b0';
}
