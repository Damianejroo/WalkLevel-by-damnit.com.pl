<script lang="ts">
  // Okno główne: jedno okno, ekrany montowane raz (przy pierwszym wejściu) i potem tylko chowane
  // (docs/design.md 6.3). Przejście: nowy ekran opacity 0,5 -> 1 w 100 ms, bez przesuwu (przesuw
  // o 4 px wyglądał jak drgnięcie okna). Stan rdzenia - src/lib/state.svelte.ts.
  import { onMount, untrack } from 'svelte';
  import { SvelteSet } from 'svelte/reactivity';
  import Toast from './lib/ui/toast.svelte';
  import PanelZnajomych from './screens/friends-panel.svelte';
  import { uruchomZnajomych } from './lib/friends.svelte';
  import { oknoZmienione, panel, sluchajOknaPanelu, szerokoscPanelu } from './lib/friends-panel.svelte';
  import * as api from './lib/api';
  import { EKRANY } from './screens/registry';
  import { stan, ui, uruchom, schowajKomunikat, type Trasa } from './lib/state.svelte';
  import { rozwinMotyw, ustawAkcent, ustawDuzyTekst, ustawMotyw } from './lib/theme';
  import { t } from './lib/texts';

  let bladStartu = $state(false);
  const odwiedzone = new SvelteSet<Trasa>(['main']);
  let biezaca = $state<Trasa>('main');
  let wychodzi = $state<Trasa | null>(null);
  let zegarWyjscia: ReturnType<typeof setTimeout> | undefined;

  // Panel znajomych (docs/design.md 8.16): pełny 288 px albo pasek 64 px obok kolumny ekranów,
  // w miejscu panelu (slot w kolorze panelu). Układ i przejście przy zwijaniu - lib/friends-panel.svelte.ts:
  // w przejściu główna kolumna ma zamrożoną szerokość, panel bierze resztę okna.
  const trybPanelu = $derived(panel.widoczny);
  const stronaPanelu = $derived(panel.strona);
  // slot panelu jest w siatce, także gdy w przejściu panel jest wyłączany/włączany (puste tło)
  const zSlotem = $derived(panel.kolumna !== null || panel.slot > 0);
  const kolumny = $derived.by(() => {
    if (!zSlotem) return 'minmax(0, 1fr)';
    const glowna = panel.kolumna === null ? 'minmax(0, 1fr)' : `${panel.kolumna}px`;
    const boczna = panel.kolumna === null ? `${panel.slot}px` : 'minmax(0, 1fr)';
    return stronaPanelu === 'left' ? `${boczna} ${glowna}` : `${glowna} ${boczna}`;
  });
  // komunikat (Toast) i listy rozwijane liczą się w kolumnie ekranów, nie nad panelem
  $effect(() => {
    const w = panel.slot;
    const s = document.documentElement.style;
    s.setProperty('--panel-lewo', `${stronaPanelu === 'left' ? w : 0}px`);
    s.setProperty('--panel-prawo', `${stronaPanelu === 'right' ? w : 0}px`);
  });

  onMount(() => {
    const ciemny = matchMedia('(prefers-color-scheme: dark)');
    const zmiana = () => (ui.ciemnySystem = ciemny.matches);
    zmiana();
    ciemny.addEventListener('change', zmiana);
    const rozmiar = () => oknoZmienione(window.innerWidth);
    window.addEventListener('resize', rozmiar);
    const stopZnajomych = uruchomZnajomych();
    let stopOkna: (() => void) | undefined;
    void sluchajOknaPanelu().then((stop) => (stopOkna = stop));
    uruchom().catch((e) => {
      console.error('Start interfejsu nie powiódł się', e);
      bladStartu = true;
    });
    return () => {
      ciemny.removeEventListener('change', zmiana);
      window.removeEventListener('resize', rozmiar);
      stopZnajomych();
      stopOkna?.();
    };
  });

  // Wygląd z ustawień osoby i profilu: tylko podmiana zmiennych CSS
  $effect(() => {
    if (!ui.gotowy) return;
    const motyw = rozwinMotyw(stan.settings.theme, ui.ciemnySystem);
    ustawMotyw(motyw);
    // tło okna i WebView2 w kolorze motywu: odsłonięty przy zmianie rozmiaru fragment okna nie
    // jest przezroczysty (zwijanie panelu)
    void api.windowTheme(motyw === 'dark').catch(() => {});
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

<div class="uklad" data-panel={trybPanelu} data-strona-panelu={stronaPanelu} style:grid-template-columns={kolumny}>
<main class="okno">
  {#if ui.gotowy}
    {#each odwiedzone as trasa (trasa)}
      {@const Ekran = EKRANY[trasa].komponent}
      <div
        class="ekran"
        data-ekran={trasa}
        data-stan={trasa === biezaca ? 'aktywny' : trasa === wychodzi ? 'wychodzi' : 'ukryty'}
        inert={trasa !== biezaca}
      >
        <Ekran parametry={parametry(trasa)} aktywny={trasa === biezaca} />
      </div>
    {/each}
  {:else if bladStartu}
    <p class="blad-startu">{t('wspolne.blad_nieznany')}</p>
  {/if}
</main>
{#if zSlotem}
  {#if panel.staly && trybPanelu !== 'wylaczony'}
    <!-- w aplikacji: pełny panel zawsze pod spodem, pasek nałożony na brzeg od strony kolumny -->
    <div class="slot-panelu z-nakladka" style:--panel-w="288px">
      <div class="pod-spodem" inert={panel.nakladka}><PanelZnajomych waski={false} /></div>
      {#if panel.nakladka}<div class="nakladka"><PanelZnajomych waski /></div>{/if}
    </div>
  {:else}
    <div class="slot-panelu" style:--panel-w="{szerokoscPanelu(trybPanelu)}px">
      {#if trybPanelu !== 'wylaczony'}<PanelZnajomych waski={trybPanelu === 'waski'} />{/if}
    </div>
  {/if}
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
  /* miejsca wprost (element z samym grid-row trafiałby do pierwszej wolnej kolumny); kolumny
     ustawia styl z friends-panel.svelte.ts */
  .uklad > .okno { grid-area: 1 / 1; }
  .slot-panelu {
    grid-area: 1 / 2;
    display: grid;
    grid-template-columns: var(--panel-w);
    justify-content: start;
    min-width: 0;
    overflow: hidden;
    background: var(--surface);
  }
  .uklad[data-strona-panelu='left'] > .okno { grid-area: 1 / 2; }
  .z-nakladka { position: relative; }
  .pod-spodem { display: grid; min-width: 0; min-height: 0; }
  .nakladka { position: absolute; inset: 0 auto 0 0; width: 64px; display: grid; }
  .uklad[data-strona-panelu='left'] .nakladka { inset: 0 0 0 auto; }
  .uklad[data-strona-panelu='left'] > .slot-panelu { grid-area: 1 / 1; justify-content: end; }
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
  /* bez przesuwu: przesunięcie treści o 4 px przy każdej zmianie ekranu wyglądało jak drgnięcie
     okna w bok (zgłoszenie 26.11) */
  .ekran[data-stan='wychodzi'],
  .ekran[data-stan='ukryty'] { visibility: hidden; }
  @keyframes wejscie-ekranu {
    from { opacity: 0.5; }
    to { opacity: 1; }
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
