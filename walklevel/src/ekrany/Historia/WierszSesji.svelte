<script lang="ts">
  // Wiersz sesji w Historii (docs/wyglad.md 9.6): ikona profilu 32 | „Dziś 18:20 · Profil” w jednej
  // linii + „5 432 kroki · 42 min · 3,1 km/h” zawijane | oko „ukryj na koncie online” (gdy osoba
  // sesji ma konto). Klik w wiersz otwiera szczegóły; oko to osobny przycisk obok.
  import type { SessionRow } from '../../lib/typy';
  import { czas, t, tl } from '../../lib/teksty';
  import { liczba } from '../../lib/format';
  import Ikona from '../../lib/ui/Ikona.svelte';
  import IkonaProfilu from './IkonaProfilu.svelte';
  import { dzien, godzina } from './daty';

  interface Props {
    sesja: SessionRow;
    /** Historia wszystkich osób - w tytule także nazwa osoby. */
    wszyscy: boolean;
    teraz: Date;
    onotworz: () => void;
    onoko: (ukryta: boolean) => void;
  }
  let { sesja, wszyscy, teraz, onotworz, onoko }: Props = $props();

  const ZNAK_UKRYTA = ''; // Segoe Fluent „Hide” - oko przekreślone

  const kiedy = $derived({ dzien: dzien(sesja.start, teraz), godzina: godzina(sesja.start) });
  const teksty = $derived(
    tl(wszyscy ? 'historia.sesje.wiersz' : 'historia.sesje.wiersz_moja', {
      ...kiedy,
      osoba: sesja.personName,
      profil: sesja.profileName,
      kroki: sesja.steps,
      czas: czas(sesja.activeS),
      kmh: liczba(sesja.avgKmh, 1),
    }),
  );
  const ukryta = $derived(sesja.online?.hidden ?? false);
  const opisOka = $derived(t(ukryta ? 'sesja.online.pokaz' : 'sesja.online.ukryj'));
</script>

<li class="wiersz wiersz-sesji">
  <button type="button" class="otworz" aria-label={t('historia.sesje.dostepnosc', kiedy)} onclick={onotworz}>
    <IkonaProfilu ikona={sesja.icon} rozmiar={32} />
    <span class="tekst">
      <span class="tytul jedna-linia dane" title={teksty[0]}>{teksty[0]}</span>
      <span class="opis">{teksty[1]}</span>
    </span>
  </button>
  {#if sesja.online}
    <button
      type="button"
      class="oko"
      class:ukryta
      aria-label={opisOka}
      title={opisOka}
      onclick={() => onoko(!ukryta)}
    >
      {#if ukryta}<Ikona znak={ZNAK_UKRYTA} />{:else}<Ikona nazwa="oko" />{/if}
    </button>
  {/if}
</li>

<style>
  .wiersz {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: var(--sp-1);
    min-height: var(--row-h);
  }
  :global(.wiersz-sesji + .wiersz-sesji) { border-top: 1px solid var(--line); }
  .otworz {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    min-width: 0;
    min-height: var(--row-h);
    padding: 6px 8px;
    margin-inline: -8px 0;
    border-radius: var(--r-md);
    text-align: start;
    transition: background-color var(--t-color) linear;
  }
  .otworz:hover { background: var(--surface-2); }
  .tekst { display: flex; flex-direction: column; min-width: 0; flex: 1; }
  .tytul { font-size: var(--fs-base); font-weight: var(--fw-semibold); line-height: var(--lh-tight); }
  .opis { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-body); }
  .oko {
    display: grid;
    place-items: center;
    width: var(--icon-btn);
    height: var(--icon-btn);
    border: 1px solid transparent;
    border-radius: var(--r-md);
    color: var(--text-2);
    transition:
      background-color var(--t-color) linear,
      border-color var(--t-color) linear,
      color var(--t-color) linear;
  }
  .oko:hover { background: var(--surface-2); border-color: var(--control); color: var(--text); }
  .oko.ukryta { color: var(--text); }
</style>
