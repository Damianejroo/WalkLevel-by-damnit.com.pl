<script lang="ts" module>
  import type { NazwaIkony } from '../../lib/ui/icon.svelte';

  export interface Pozycja {
    id: string;
    tytul: string;
    opis?: string;
    aktywna: boolean;
  }
  export interface Link {
    tytul: string;
    ikona: NazwaIkony;
    onclick: () => void;
  }
</script>

<script lang="ts">
  // Lista rozwijana profilu / osoby (docs/design.md 8.14): pod wyzwalaczem, szerokość
  // clamp(280, kolumna, 360), przycięta do kolumny z marginesem 16. Wejście opacity + translateY
  // 160 ms. Esc, klik obok, przewinięcie i zmiana rozmiaru okna zamykają; fokus wraca na wyzwalacz.
  import type { Snippet } from 'svelte';
  import Ikona from '../../lib/ui/icon.svelte';

  interface Props {
    otwarta: boolean;
    wyzwalacz: HTMLElement | undefined;
    /** Do której krawędzi wyzwalacza wyrównać listę. */
    strona?: 'lewo' | 'prawo';
    etykieta: string;
    pozycje: Pozycja[];
    linki: Link[];
    poczatek: Snippet<[string]>;
    /** Po zamknięciu fokus wraca na wyzwalacz (lista otwarta z klawiatury); domyślnie tak. */
    fokusPoZamknieciu?: boolean;
    /** `zKlawiatury` - wybór Enterem albo spacją. */
    onwybierz: (id: string, zKlawiatury: boolean) => void;
  }
  let {
    otwarta = $bindable(false),
    wyzwalacz,
    strona = 'lewo',
    etykieta,
    pozycje,
    linki,
    poczatek,
    fokusPoZamknieciu = true,
    onwybierz,
  }: Props = $props();

  let lista = $state<HTMLElement>();
  let polozenie = $state<{ left: number; top?: number; bottom?: number; width: number; maxHeight: number }>({
    left: 0, top: 0, width: 300, maxHeight: 400,
  });

  function zmierz() {
    if (!wyzwalacz) return;
    const r = wyzwalacz.getBoundingClientRect();
    const k = (wyzwalacz.closest('.strona') ?? document.body).getBoundingClientRect();
    // position: fixed liczy się od <body> (w podglądzie z ?szer body ma transform)
    const b = document.body.getBoundingClientRect();
    const min = k.left + 16;
    const max = k.right - 16 - 8;
    const width = Math.min(360, max - min);
    const left = Math.max(min, Math.min(max - width, strona === 'prawo' ? r.right - width : r.left)) - b.left;
    const pod = k.bottom - r.bottom - 20;
    const nad = r.top - k.top - 20;
    // za mało miejsca pod wyzwalaczem (karta sterowania na dole okna) - lista otwiera się w górę
    polozenie = pod >= 320 || pod >= nad
      ? { left, top: r.bottom + 4 - b.top, width, maxHeight: Math.max(160, pod) }
      : { left, bottom: b.bottom - r.top + 4, width, maxHeight: nad };
  }

  function zamknij(fokus = fokusPoZamknieciu) {
    otwarta = false;
    if (fokus) wyzwalacz?.focus({ focusVisible: false } as FocusOptions);
  }

  function wybierz(id: string, e: MouseEvent) {
    const zKlawiatury = e.detail === 0;
    zamknij(fokusPoZamknieciu || zKlawiatury);
    onwybierz(id, zKlawiatury);
  }

  function przejdz(link: Link) {
    zamknij(false);
    link.onclick();
  }

  // Tab poza listę zamyka ją (fokus zostaje tam, dokąd przeszedł)
  function wyjscie(e: FocusEvent) {
    const dokad = e.relatedTarget as Node | null;
    // klik w wyzwalacz zamyka listę sam (przełącza), tu by ją od razu otworzył ponownie
    if (dokad && !lista?.contains(dokad) && !wyzwalacz?.contains(dokad)) zamknij(false);
  }

  function klawisz(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      zamknij(true);
      return;
    }
    if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return;
    e.preventDefault();
    const przyciski = [...(lista?.querySelectorAll<HTMLButtonElement>('button') ?? [])];
    const i = przyciski.indexOf(document.activeElement as HTMLButtonElement);
    const n = przyciski.length;
    przyciski[(i + (e.key === 'ArrowDown' ? 1 : -1) + n) % n]?.focus();
  }

  $effect(() => {
    if (!otwarta) return;
    zmierz();
    queueMicrotask(() => (lista?.querySelector<HTMLButtonElement>('[aria-checked="true"]') ?? lista?.querySelector('button'))?.focus());
    const obok = (e: PointerEvent) => {
      const cel = e.target as Node;
      if (!lista?.contains(cel) && !wyzwalacz?.contains(cel)) zamknij(false);
    };
    const przewiniecie = (e: Event) => {
      if (!lista?.contains(e.target as Node)) zamknij(false);
    };
    const rozmiar = () => zamknij(false);
    window.addEventListener('pointerdown', obok, true);
    window.addEventListener('scroll', przewiniecie, true);
    window.addEventListener('resize', rozmiar);
    return () => {
      window.removeEventListener('pointerdown', obok, true);
      window.removeEventListener('scroll', przewiniecie, true);
      window.removeEventListener('resize', rozmiar);
    };
  });
</script>

{#if otwarta}
  <div
    class="lista"
    bind:this={lista}
    role="dialog"
    aria-label={etykieta}
    tabindex="-1"
    style:left="{polozenie.left}px"
    style:top={polozenie.top === undefined ? undefined : `${polozenie.top}px`}
    style:bottom={polozenie.bottom === undefined ? undefined : `${polozenie.bottom}px`}
    style:width="{polozenie.width}px"
    style:max-height="{polozenie.maxHeight}px"
    onkeydown={klawisz}
    onfocusout={wyjscie}
  >
    <div role="radiogroup" aria-label={etykieta}>
      {#each pozycje as p (p.id)}
        <button
          type="button"
          class="pozycja"
          class:z-opisem={!!p.opis}
          role="radio"
          aria-checked={p.aktywna}
          onclick={(e) => wybierz(p.id, e)}
        >
          {@render poczatek(p.id)}
          <span class="tekst">
            <span class="tytul jedna-linia" title={p.tytul}>{p.tytul}</span>
            {#if p.opis}<span class="opis jedna-linia">{p.opis}</span>{/if}
          </span>
          <span class="znacznik">{#if p.aktywna}<Ikona nazwa="zaznacz" />{/if}</span>
        </button>
      {/each}
    </div>
    {#if linki.length}
      <div class="linki">
        {#each linki as l (l.tytul)}
          <button type="button" class="link" onclick={() => przejdz(l)}>
            <Ikona nazwa={l.ikona} />
            <span class="jedna-linia">{l.tytul}</span>
          </button>
        {/each}
      </div>
    {/if}
  </div>
{/if}

<style>
  .lista {
    position: fixed;
    z-index: 20;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 6px;
    overflow-y: auto;
    border: 1px solid var(--control);
    border-radius: var(--r-lg);
    background: var(--surface-2);
    box-shadow: var(--shadow-pop);
    animation: wl-rozwin var(--t-in) var(--ease-enter);
  }
  .pozycja,
  .link {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    min-width: 0;
    height: 42px;
    padding: 0 10px 0 8px;
    border-radius: var(--r-md);
    color: var(--text);
    transition: background-color var(--t-color) linear;
  }
  .z-opisem { height: 50px; }
  .pozycja:hover,
  .link:hover { background: color-mix(in srgb, var(--text) 10%, transparent); }
  .tekst { display: flex; flex-direction: column; flex: 1; min-width: 0; }
  .tytul { font-size: var(--fs-base); line-height: var(--lh-tight); }
  [aria-checked='true'] .tytul { font-weight: var(--fw-bold); }
  .opis { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-tight); }
  .znacznik { display: grid; place-items: center; width: 16px; flex: none; }
  .linki { margin-top: 4px; padding-top: 4px; border-top: 1px solid var(--line); }
  .link { padding-inline-start: 14px; font-size: var(--fs-md); }
</style>
