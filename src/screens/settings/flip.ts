// FLIP dla zmian wysokości w Ustawieniach (docs/design.md 6.2): zmiana DOM dzieje się od razu,
// a elementy pod nią zjeżdżają/podjeżdżają przez transform. Element, który się zmienia, i wszystko
// nad nim stoi w miejscu - kliknięta kontrolka nie ucieka spod kursora.
import { tick } from 'svelte';

/** Rodzeństwo `el` i jego przodków leżące za nim, aż do kontenera przewijania strony. */
function ponizej(el: HTMLElement): HTMLElement[] {
  const wynik: HTMLElement[] = [];
  let w: HTMLElement | null = el;
  while (w?.parentElement && !w.parentElement.classList.contains('przewijanie')) {
    for (let s = w.nextElementSibling; s; s = s.nextElementSibling) if (s instanceof HTMLElement) wynik.push(s);
    w = w.parentElement;
  }
  return wynik;
}

/** Wykonuje `zmiana` (np. inny tryb na karcie sterowania) i płynnie przesuwa to, co pod `el`. */
export async function flip(el: HTMLElement | undefined, zmiana: () => unknown) {
  const lista = el ? ponizej(el) : [];
  const przed = lista.map((e) => e.getBoundingClientRect().top);
  zmiana();
  await tick();
  if (!lista.length || matchMedia('(prefers-reduced-motion: reduce)').matches) return;
  const styl = getComputedStyle(document.documentElement);
  const czas = parseFloat(styl.getPropertyValue('--t-morph')) || 0;
  const krzywa = styl.getPropertyValue('--ease-smooth').trim() || 'ease';
  lista.forEach((e, i) => {
    const roznica = przed[i] - e.getBoundingClientRect().top;
    if (roznica) e.animate([{ transform: `translateY(${roznica}px)` }, { transform: 'none' }], { duration: czas, easing: krzywa });
  });
}
