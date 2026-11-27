<script lang="ts">
  // Ekran: Dziennik zdarzeń (trasa 'log'), docs/texts.md 18.2. Wpisy na żywo z ui.dziennik
  // (rdzeń: zdarzenie wl:log, najwyżej 500 ostatnich linii). Ograniczenie zamiast wirtualnego
  // przewijania: lista z kluczami (nowy wpis = jeden nowy wiersz, najstarszy znika), panel z
  // `contain`, więc nowy wpis nie przelicza układu reszty ekranu. Lista trzyma się dołu, dopóki
  // ktoś nie przewinie wyżej. Panel wypełnia resztę wysokości okna (pomiar przy zmianie rozmiaru).
  import { onMount, tick } from 'svelte';
  import Strona from '../lib/ui/page.svelte';
  import Przycisk from '../lib/ui/button.svelte';
  import * as api from '../lib/api';
  import { t } from '../lib/texts';
  import { komunikat, stan, ui, wykonaj, type PropsEkranu } from '../lib/state.svelte';

  let { aktywny }: PropsEkranu = $props();

  interface Wpis {
    id: number;
    czas: string;
    tresc: string;
  }

  // --- wpisy z kluczami: dopisane linie dostają nowe id, reszta zostaje ---
  let nastepneId = 0;
  let poprzednie: readonly string[] = [];
  let poprzednieWpisy: Wpis[] = [];

  function rozbij(linia: string): Wpis {
    const m = /^(\d\d:\d\d:\d\d) {1,2}(.*)$/s.exec(linia);
    return { id: nastepneId++, czas: m?.[1] ?? '', tresc: m ? m[2] : linia };
  }

  /** Ile linii dopisano na końcu (najstarsze ucięte z początku); null - lista podmieniona. */
  function dopisane(stare: readonly string[], nowe: readonly string[]): number | null {
    for (let d = 0; d <= nowe.length; d++) {
      const zostaje = nowe.length - d;
      if (zostaje > stare.length) continue;
      const od = stare.length - zostaje;
      let zgodne = true;
      for (let i = zostaje - 1; i >= 0 && zgodne; i--) zgodne = nowe[i] === stare[od + i];
      if (zgodne) return d;
    }
    return null;
  }

  const wpisy = $derived.by(() => {
    const nowe = ui.dziennik;
    const d = dopisane(poprzednie, nowe);
    const wynik =
      d === null
        ? nowe.map(rozbij)
        : [...poprzednieWpisy.slice(poprzednieWpisy.length - (nowe.length - d)), ...nowe.slice(nowe.length - d).map(rozbij)];
    poprzednie = nowe;
    poprzednieWpisy = wynik;
    return wynik;
  });

  // --- trzymanie się dołu: dopóki lista jest przewinięta do końca, nowe wpisy ją przesuwają ---
  let panel: HTMLDivElement;
  let naDole = true;

  function przewinNaDol() {
    if (panel && naDole) panel.scrollTop = panel.scrollHeight;
  }
  function przewinieto() {
    naDole = panel.scrollHeight - panel.scrollTop - panel.clientHeight < 24;
  }

  $effect(() => {
    void wpisy;
    przewinNaDol();
  });

  $effect(() => {
    if (aktywny) tick().then(przewinNaDol);
  });

  // --- wysokość panelu: to, co zostaje z okna po opisie, przyciskach i stanie raportu ---
  let blok: HTMLDivElement;
  let wysokosc = $state(320);

  onMount(() => {
    // .przewijanie > .tresc (Strona) > .dziennik (blok) > [.gora, .panel, .dol]
    const przewijanie = panel.closest<HTMLElement>('.przewijanie');
    const kolumna = blok.parentElement;
    if (!przewijanie || !kolumna) return;
    const dopasuj = () => {
      const s = getComputedStyle(kolumna);
      const reszta = blok.offsetHeight - panel.offsetHeight + parseFloat(s.paddingTop) + parseFloat(s.paddingBottom);
      wysokosc = Math.max(240, Math.floor(przewijanie.clientHeight - reszta));
      tick().then(przewinNaDol);
    };
    const ro = new ResizeObserver(() => requestAnimationFrame(dopasuj)); // bez pętli obserwatora
    ro.observe(przewijanie);
    for (const el of blok.children) if (el !== panel) ro.observe(el);
    return () => ro.disconnect();
  });

  // --- akcje ---
  const diagnostyka = $derived(stan.device.diagnostics);

  async function kopiuj() {
    try {
      await navigator.clipboard.writeText(ui.dziennik.join('\n'));
      komunikat(t('dziennik.skopiowano'));
    } catch {
      komunikat(t('dziennik.kopiuj_blad'), 'error');
    }
  }
</script>

<Strona tytul={t('dziennik.tytul')} podpis={wpisy.length ? t('dziennik.podpis', { liczba: wpisy.length }) : ''}>
  <div class="dziennik" bind:this={blok}>
    <div class="gora">
      <p class="opis">{t('dziennik.opis')}</p>
      <div class="akcje">
        <Przycisk napis={t('dziennik.kopiuj')} disabled={!wpisy.length} onclick={kopiuj} />
        <Przycisk napis={t('dziennik.folder')} onclick={() => wykonaj(api.openFolder('logi'))} />
      </div>
    </div>

    <!-- przewijany panel musi dać się przewinąć z klawiatury (strzałki, PageUp/PageDown) -->
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div
      class="panel"
      bind:this={panel}
      style:height="{wysokosc}px"
      onscroll={przewinieto}
      tabindex="0"
      role="log"
      aria-live="off"
      aria-label={t('dziennik.dostepnosc')}
    >
      {#each wpisy as w (w.id)}
        <div class="wiersz">
          {#if w.czas}<span class="czas">{w.czas}</span>{/if}
          <span class="tresc" class:bez-czasu={!w.czas}>{w.tresc}</span>
        </div>
      {:else}
        <p class="pusty">{t('dziennik.pusty')}</p>
      {/each}
    </div>

    <div class="dol">
      <Przycisk
        napis={t('dziennik.raport')}
        disabled={diagnostyka.running}
        onclick={() => wykonaj(api.diagnosticsRun())}
      />
      <p class="stan-raportu" role="status" aria-live="polite">{diagnostyka.text}</p>
    </div>
  </div>
</Strona>

<style>
  .dziennik {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    min-width: 0;
  }
  .gora { display: flex; flex-direction: column; gap: var(--sp-3); min-width: 0; }
  .opis { color: var(--text-2); font-size: var(--fs-md); }
  .akcje {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2);
    min-width: 0;
  }
  /* szerokość z napisu: za wąsko na rząd - przycisk przechodzi niżej zamiast skracać napis */
  .akcje > :global(*) { flex: 1 1 auto; }
  .dol {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2) var(--sp-3);
    min-width: 0;
  }
  /* stała wysokość dwóch linii: zmiana stanu raportu nie zmienia wysokości panelu */
  .stan-raportu {
    flex: 1 1 220px;
    min-width: 0;
    height: calc(2 * var(--lh-body) * var(--fs-sm));
    overflow: hidden;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }

  .panel {
    min-width: 0;
    overflow-y: auto;
    overflow-x: hidden;
    overscroll-behavior: contain;
    contain: strict;
    padding: 6px 0;
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: var(--bg);
    font-family: var(--font-mono);
    font-size: var(--fs-sm);
    line-height: 1.5;
  }
  .wiersz {
    display: grid;
    grid-template-columns: 8ch minmax(0, 1fr);
    gap: 1.5ch;
    padding: 1px 12px;
  }
  .czas { color: var(--text-2); font-variant-numeric: tabular-nums; }
  .tresc { min-width: 0; overflow-wrap: anywhere; white-space: pre-wrap; }
  .bez-czasu { grid-column: 1 / -1; }
  .pusty {
    padding: var(--sp-4);
    font-family: var(--font-ui);
    color: var(--text-2);
    text-align: center;
  }
</style>
