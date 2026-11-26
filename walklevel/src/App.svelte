<script lang="ts">
  // Okno główne: jedno okno, ekrany montowane raz (przy pierwszym wejściu) i potem tylko chowane
  // (docs/wyglad.md 6.3). Przejście: wejście opacity + translateX 8 px (wstecz -8 px) 220 ms,
  // wyjście opacity 180 ms. Stan rdzenia - src/lib/stan.svelte.ts.
  import { onMount, untrack } from 'svelte';
  import { SvelteSet } from 'svelte/reactivity';
  import Toast from './lib/ui/Toast.svelte';
  import { EKRANY } from './ekrany/rejestr';
  import { stan, ui, uruchom, schowajKomunikat, type Trasa } from './lib/stan.svelte';
  import { ustawAkcent, ustawDuzyTekst, ustawMotyw } from './lib/wyglad';
  import { t } from './lib/teksty';

  let bladStartu = $state(false);
  const odwiedzone = new SvelteSet<Trasa>(['main']);
  let biezaca = $state<Trasa>('main');
  let wychodzi = $state<Trasa | null>(null);
  let zegarWyjscia: ReturnType<typeof setTimeout> | undefined;

  onMount(() => {
    const ciemny = matchMedia('(prefers-color-scheme: dark)');
    const zmiana = () => (ui.ciemnySystem = ciemny.matches);
    zmiana();
    ciemny.addEventListener('change', zmiana);
    uruchom().catch((e) => {
      console.error('Start interfejsu nie powiódł się', e);
      bladStartu = true;
    });
    return () => ciemny.removeEventListener('change', zmiana);
  });

  // Wygląd z ustawień osoby i profilu: tylko podmiana zmiennych CSS
  $effect(() => {
    if (!ui.gotowy) return;
    const motyw = stan.settings.theme;
    ustawMotyw(motyw === 'system' ? (ui.ciemnySystem ? 'dark' : 'light') : motyw);
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
      zegarWyjscia = setTimeout(() => (wychodzi = null), 180);
    });
  });

  const parametry = (trasa: Trasa) => ui.stos.findLast((w) => w.trasa === trasa)?.parametry ?? {};
</script>

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

<Toast toast={ui.toast} onzamknij={() => schowajKomunikat()} />

<style>
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
  .ekran[data-stan='aktywny'] { animation: wl-wejscie var(--t-in) var(--ease-enter); }
  .ekran[data-stan='wychodzi'] {
    animation: wl-wyjscie var(--t-out) var(--ease-exit) forwards;
    pointer-events: none;
  }
  .ekran[data-stan='ukryty'] { visibility: hidden; }
  .blad-startu {
    align-self: center;
    justify-self: center;
    max-width: 420px;
    padding: var(--sp-4);
    color: var(--text-2);
    text-align: center;
  }
</style>
