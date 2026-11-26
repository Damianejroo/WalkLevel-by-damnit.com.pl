// „Nic nie wystaje” (docs/wyglad.md 11.2): elementy wychodzące poza prawą krawędź albo
// ucięte przez overflow bez wielokropka. Wołane przez test (tests-ui/sprawdz.mjs) i dostępne
// w konsoli przeglądarki jako window.wystaje() w atrapie.

const opis = (el: Element) => {
  const klasa = el.getAttribute('class')?.split(' ').find((k) => !k.startsWith('svelte-')) ?? '';
  return `${el.tagName.toLowerCase()}${klasa ? '.' + klasa : ''} „${(el.textContent ?? '').trim().slice(0, 30)}”`;
};

/** Lista problemów (pusta = w porządku). `granica` - prawa krawędź w px (domyślnie szerokość okna). */
export function wystaje(korzen: Element = document.body, granica = window.innerWidth + 0.5): string[] {
  const wyniki: string[] = [];
  for (const el of korzen.querySelectorAll('*')) {
    // niewidoczne (nieaktywne ekrany, display: none) i tylko dla czytników ekranu - pomijamy
    if (!el.checkVisibility({ visibilityProperty: true }) || el.closest('.niewidoczne')) continue;
    const r = el.getBoundingClientRect();
    if (r.width === 0 && r.height === 0) continue;
    if (r.right > granica) wyniki.push(`poza oknem: ${opis(el)} (${Math.round(r.right - granica)} px)`);
    if (!(el instanceof HTMLElement)) continue;
    const s = getComputedStyle(el);
    const ucina = s.overflowX !== 'visible' && s.textOverflow !== 'ellipsis';
    if (ucina && el.scrollWidth > el.clientWidth + 1) wyniki.push(`ucięte: ${opis(el)} (${el.scrollWidth - el.clientWidth} px)`);
  }
  return wyniki;
}
