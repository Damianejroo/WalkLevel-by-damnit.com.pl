<script lang="ts">
  // Stopka panelu znajomych (docs/wyglad.md 8.16): moja odznaka, pseudonim i status dla
  // znajomych; klik otwiera menu (lista rozwijana jak 8.14): „Pokazuj grę i tempo” / „Pokazuj
  // tylko, że chodzę” / „Niewidoczny” i pole „Opis dla znajomych” (Enter zapisuje, błąd pod
  // polem). To samo ustawienie co w Konto online > Moje konto.
  import OdznakaRzadkosci, { rzadkoscPoziomu } from '../../lib/ui/OdznakaRzadkosci.svelte';
  import Ikona from '../../lib/ui/Ikona.svelte';
  import Pole from '../../lib/ui/Pole.svelte';
  import * as api from '../../lib/api';
  import { stan, wykonaj } from '../../lib/stan.svelte';
  import { t } from '../../lib/teksty';
  import { rangaPoziomu, sprobuj } from '../KontoOnline/pomocnicze';
  import type { LiveStatus } from '../../lib/typy';

  interface Props {
    waski: boolean;
    otwarte: boolean;
    onprzelacz: () => void;
    onzamknij: () => void;
  }
  let { waski, otwarte, onprzelacz, onzamknij }: Props = $props();

  const OPIS_MAX = 40;
  const STATUSY: LiveStatus[] = ['wszystko', 'chodze', 'niewidoczny'];
  const konto = $derived(stan.account);
  const poziom = $derived(stan.levels?.level ?? 1);
  const ranga = $derived(rangaPoziomu(stan.statics.definitions, poziom));
  const ja = $derived(t('panel.osoba', { pseudonim: konto.name, poziom, ranga: ranga?.name ?? '' }));

  let opis = $state('');
  let bladOpisu = $state('');
  $effect(() => {
    if (otwarte) {
      opis = konto.description;
      bladOpisu = '';
    }
  });

  async function wybierz(s: LiveStatus) {
    onzamknij();
    if (s !== konto.liveStatus) await wykonaj(api.accountSetLiveStatus(s));
  }
  async function zapiszOpis() {
    const w = await sprobuj(api.accountSetDescription(opis));
    if (w.ok) onzamknij();
    else bladOpisu = w.pola.description || w.blad || Object.values(w.pola)[0] || '';
  }
</script>

<footer class="stopka" class:waski>
  <span class="moja" title={ja}>
    <OdznakaRzadkosci rzadkosc={rzadkoscPoziomu(poziom)} liczba={poziom} rozmiar={34} etykieta={ja} />
  </span>
  {#if !waski}
    <button
      type="button"
      class="status"
      aria-haspopup="menu"
      aria-expanded={otwarte}
      aria-label={t('panel.status')}
      onclick={onprzelacz}
    >
      <span class="tresc">
        <span class="nick jedna-linia dane" title={konto.name}>{konto.name}</span>
        <span class="linia">
          <Ikona nazwa={konto.liveStatus === 'niewidoczny' ? 'oko' : 'chod'} rozmiar={12} />
          {#if konto.description}
            <span class="jedna-linia dane opis" title={konto.description}>{konto.description}</span>
          {:else}
            <span class="jedna-linia">{t(`panel.status.${konto.liveStatus}`)}</span>
          {/if}
        </span>
      </span>
      <span class="strzalka"><Ikona nazwa="w_gore" rozmiar={12} /></span>
    </button>
  {/if}

  {#if otwarte && !waski}
    <div class="menu" role="menu" aria-label={t('panel.status')}>
      {#each STATUSY as s (s)}
        <button
          type="button"
          class="pozycja"
          role="menuitemradio"
          aria-checked={konto.liveStatus === s}
          disabled={konto.busy.includes('live')}
          onclick={() => wybierz(s)}
        >
          <span class="znak"><Ikona nazwa="zaznacz" rozmiar={14} /></span>
          <span class="opis-pozycji">
            <span class="t">{t(`panel.status.${s}`)}</span>
            <span class="o">{t(`panel.status.${s}_opis`)}</span>
          </span>
        </button>
      {/each}
      <hr />
      <div class="pole-opisu">
        <Pole
          bind:wartosc={opis}
          etykieta={t('panel.opis', { liczba: OPIS_MAX })}
          podpowiedz={t('panel.opis_podpowiedz')}
          maxlength={OPIS_MAX}
          blad={bladOpisu}
          oninput={() => (bladOpisu = '')}
          onenter={zapiszOpis}
        />
      </div>
    </div>
  {/if}
</footer>

<style>
  .stopka {
    position: relative;
    display: grid;
    grid-template-columns: 34px minmax(0, 1fr);
    gap: 10px;
    align-items: center;
    min-height: 64px;
    padding: 8px 10px 8px 16px;
    border-top: 1px solid var(--line);
    background: var(--surface);
  }
  .waski { grid-template-columns: 34px; justify-content: center; padding: 14px 0; }
  .moja { display: grid; place-items: center; width: 34px; height: 34px; }
  .status {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 6px;
    align-items: center;
    min-width: 0;
    margin: -6px 0 -6px -8px;
    padding: 6px 8px;
    border-radius: 8px;
    text-align: start;
    color: var(--text);
    transition: background-color var(--t-color) linear;
  }
  .status:hover,
  .status[aria-expanded='true'] { background: var(--surface-2); }
  .tresc { display: grid; gap: 2px; min-width: 0; }
  .nick { font-size: 0.9375rem; font-weight: var(--fw-semibold); line-height: var(--lh-tight); }
  .linia {
    display: flex;
    align-items: center;
    gap: 5px;
    min-width: 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
    line-height: var(--lh-tight);
  }
  .linia :global(.ikona) { color: var(--text); }
  .opis { font-style: italic; }
  .strzalka { color: var(--text-2); }

  /* lista rozwijana (wyglad.md 8.14): --surface-2, ramka --control, promień 12, cień */
  .menu {
    position: absolute;
    left: 8px;
    right: 8px;
    bottom: calc(100% + 6px);
    z-index: 30;
    display: grid;
    gap: 2px;
    max-height: calc(100vh - 140px);
    overflow-y: auto;
    padding: 6px;
    border: 1px solid var(--control);
    border-radius: var(--r-lg);
    background: var(--surface-2);
    box-shadow: var(--shadow-pop);
    animation: wl-rozwin var(--t-in) var(--ease-enter);
  }
  .pozycja {
    display: grid;
    grid-template-columns: 16px minmax(0, 1fr);
    gap: 10px;
    align-items: start;
    min-height: 42px;
    padding: 8px 10px;
    border-radius: 8px;
    text-align: start;
    color: var(--text);
    transition: background-color var(--t-color) linear;
  }
  .pozycja:hover { background: color-mix(in srgb, var(--text) 10%, transparent); }
  .znak { padding-top: 1px; visibility: hidden; }
  .pozycja[aria-checked='true'] .znak { visibility: visible; }
  .opis-pozycji { display: grid; gap: 2px; min-width: 0; }
  .t { font-size: var(--fs-md); line-height: var(--lh-tight); }
  .pozycja[aria-checked='true'] .t { font-weight: var(--fw-bold); }
  .o { font-size: var(--fs-xs); color: var(--text-2); line-height: var(--lh-tight); }
  hr { margin: 4px 6px; border: 0; border-top: 1px solid var(--control); opacity: 0.6; }
  .pole-opisu { padding: 4px 6px; min-width: 0; }
</style>
