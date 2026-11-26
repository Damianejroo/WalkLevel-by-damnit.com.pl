<script lang="ts">
  // Okno główne: jedno okno, ekrany montowane raz (przy pierwszym wejściu) i potem tylko chowane
  // (docs/wyglad.md 6.3). Przejście: wejście opacity + translateX 8 px (wstecz -8 px) 220 ms,
  // wyjście opacity 180 ms. Stan rdzenia - src/lib/stan.svelte.ts.
  import { onMount, untrack } from 'svelte';
  import { SvelteSet } from 'svelte/reactivity';
  import Toast from './lib/ui/Toast.svelte';
  import PanelZnajomych from './ekrany/PanelZnajomych.svelte';
  import { uruchomZnajomych } from './lib/znajomi.svelte';
  import { EKRANY } from './ekrany/rejestr';
  import { stan, ui, uruchom, schowajKomunikat, type Trasa } from './lib/stan.svelte';
  import { rozwinMotyw, ustawAkcent, ustawDuzyTekst, ustawMotyw } from './lib/wyglad';
  import { t } from './lib/teksty';

  let bladStartu = $state(false);
  const odwiedzone = new SvelteSet<Trasa>(['main']);
  let biezaca = $state<Trasa>('main');
  let wychodzi = $state<Trasa | null>(null);
  let zegarWyjscia: ReturnType<typeof setTimeout> | undefined;

  // Panel znajomych (docs/wyglad.md 8.16): pełny 288 px albo pasek 64 px obok kolumny ekranów.
  // Okno pilnuje minimalnej szerokości (okna.rs); gdy mimo to jest za wąsko (podgląd, ekran
  // mniejszy niż okno) - pasek zamiast pełnego, a przy 420 px bez panelu.
  const PANEL_PELNY = 288;
  const PANEL_WASKI = 64;
  const KOLUMNA_MIN = 420;
  let szerOkna = $state(typeof window === 'undefined' ? 1200 : window.innerWidth);
  const trybPanelu = $derived.by((): 'pelny' | 'waski' | 'wylaczony' => {
    if (!ui.gotowy || !stan.settings.friendsPanel || !stan.account.enabled || ui.trasa === 'wizard') return 'wylaczony';
    const chce = stan.settings.friendsPanelCollapsed ? 'waski' : 'pelny';
    if (chce === 'pelny' && szerOkna >= KOLUMNA_MIN + PANEL_PELNY) return 'pelny';
    return szerOkna >= KOLUMNA_MIN + PANEL_WASKI ? 'waski' : 'wylaczony';
  });
  const stronaPanelu = $derived(ui.gotowy && stan.settings.friendsPanelSide === 'left' ? 'left' : 'right');
  // komunikat (Toast) i listy rozwijane liczą się w kolumnie ekranów, nie nad panelem
  $effect(() => {
    const w = trybPanelu === 'pelny' ? PANEL_PELNY : trybPanelu === 'waski' ? PANEL_WASKI : 0;
    const s = document.documentElement.style;
    s.setProperty('--panel-lewo', `${stronaPanelu === 'left' ? w : 0}px`);
    s.setProperty('--panel-prawo', `${stronaPanelu === 'right' ? w : 0}px`);
  });

  onMount(() => {
    const ciemny = matchMedia('(prefers-color-scheme: dark)');
    const zmiana = () => (ui.ciemnySystem = ciemny.matches);
    zmiana();
    ciemny.addEventListener('change', zmiana);
    const rozmiar = () => (szerOkna = window.innerWidth);
    window.addEventListener('resize', rozmiar);
    const stopZnajomych = uruchomZnajomych();
    uruchom().catch((e) => {
      console.error('Start interfejsu nie powiódł się', e);
      bladStartu = true;
    });
    return () => {
      ciemny.removeEventListener('change', zmiana);
      window.removeEventListener('resize', rozmiar);
      stopZnajomych();
    };
  });

  // Wygląd z ustawień osoby i profilu: tylko podmiana zmiennych CSS
  $effect(() => {
    if (!ui.gotowy) return;
    ustawMotyw(rozwinMotyw(stan.settings.theme, ui.ciemnySystem));
    ustawAkcent(stan.settings.profileColor, stan.statics.palette);
    ustawDuzyTekst(stan.settings.largeText);
    document.documentElement.dataset.gotowe = '1';
  });

  // Zmiana ekranu: poprzedni wygasa, nowy wjeżdża; odwiedzone zostają zamontowane
  $effect(() => {
    const nowa = ui.trasa;
    untrack(() => {
      if (nowa === biezaca) return;
      odwiedzone.add(nowa);
      wychodzi = biezaca;
      biezaca = nowa;
      clearTimeout(zegarWyjscia);
      zegarWyjscia = setTimeout(() => (wychodzi = null), 120); // po --t-out (100 ms)
    });
  });

  const parametry = (trasa: Trasa) => ui.stos.findLast((w) => w.trasa === trasa)?.parametry ?? {};
</script>

<div class="uklad" data-panel={trybPanelu} data-strona-panelu={stronaPanelu}>
<main class="okno">
  {#if ui.gotowy}
    {#each odwiedzone as trasa (trasa)}
      {@const Ekran = EKRANY[trasa].komponent}
      <div
        class="ekran"
        data-ekran={trasa}
        data-stan={trasa === biezaca ? 'aktywny' : trasa === wychodzi ? 'wychodzi' : 'ukryty'}
        style:--wl-przesun="{ui.kierunek * 8}px"
        inert={trasa !== biezaca}
      >
        <Ekran parametry={parametry(trasa)} aktywny={trasa === biezaca} />
      </div>
    {/each}
  {:else if bladStartu}
    <p class="blad-startu">{t('wspolne.blad_nieznany')}</p>
  {/if}
</main>
{#if trybPanelu !== 'wylaczony'}
  <PanelZnajomych waski={trybPanelu === 'waski'} />
{/if}
</div>

<Toast toast={ui.toast} onzamknij={() => schowajKomunikat()} />

<style>
  .uklad {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: 100%;
    height: 100%;
    min-width: 0;
  }
  .uklad[data-panel='pelny'] { --panel-w: 288px; }
  .uklad[data-panel='waski'] { --panel-w: 64px; }
  .uklad:not([data-panel='wylaczony'])[data-strona-panelu='right'] { grid-template-columns: minmax(0, 1fr) var(--panel-w); }
  .uklad:not([data-panel='wylaczony'])[data-strona-panelu='left'] { grid-template-columns: var(--panel-w) minmax(0, 1fr); }
  /* miejsca wprost (element z samym grid-row trafiałby do pierwszej wolnej kolumny) */
  .uklad > .okno { grid-area: 1 / 1; }
  .uklad > :global(.panel) { grid-area: 1 / 2; }
  .uklad[data-strona-panelu='left']:not([data-panel='wylaczony']) > .okno { grid-area: 1 / 2; }
  .uklad[data-strona-panelu='left'] > :global(.panel) { grid-area: 1 / 1; }
  .okno {
    display: grid;
    height: 100%;
    min-width: 0;
    overflow: hidden;
  }
  .ekran {
    grid-area: 1 / 1;
    min-width: 0;
    min-height: 0;
  }
  /* 26.11: przenikanie dwóch ekranów (stary wygasa, nowy wjeżdża od zera) wyglądało jak lag -
     stary znika od razu, nowy startuje od połowy jasności i dojeżdża w 100 ms */
  .ekran[data-stan='aktywny'] { animation: wejscie-ekranu 100ms var(--ease-enter); }
  .ekran[data-stan='wychodzi'],
  .ekran[data-stan='ukryty'] { visibility: hidden; }
  @keyframes wejscie-ekranu {
    from { opacity: 0.5; transform: translateX(calc(var(--wl-przesun, 8px) / 2)); }
    to { opacity: 1; transform: none; }
  }
  .blad-startu {
    align-self: center;
    justify-self: center;
    max-width: 420px;
    padding: var(--sp-4);
    color: var(--text-2);
    text-align: center;
  }
</style>
