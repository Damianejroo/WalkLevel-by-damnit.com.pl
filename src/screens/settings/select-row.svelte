<script lang="ts">
  // Wiersz wyboru osoby / profilu gry (docs/design.md 8.14): pierścień | obrazek | nazwa + opis |
  // ołówek. Wybrany - neutralnie (ramka --ink, pełny pierścień), bez akcentu.
  import type { Snippet } from 'svelte';
  import Ikona from '../../lib/ui/icon.svelte';

  interface Props {
    tytul: string;
    opis: string;
    wybrany: boolean;
    etykietaEdycji: string;
    poczatek: Snippet;
    onwybierz: () => void;
    onedytuj: () => void;
  }
  let { tytul, opis, wybrany, etykietaEdycji, poczatek, onwybierz, onedytuj }: Props = $props();
</script>

<div class="wiersz" class:wybrany>
  <button type="button" class="wybor" aria-pressed={wybrany} onclick={onwybierz}>
    <span class="pierscien" aria-hidden="true"></span>
    {@render poczatek()}
    <span class="tekst">
      <span class="tytul jedna-linia" title={tytul}>{tytul}</span>
      <span class="opis dane">{opis}</span>
    </span>
  </button>
  <button type="button" class="edytuj" aria-label={etykietaEdycji} title={etykietaEdycji} onclick={onedytuj}>
    <Ikona nazwa="edytuj" />
  </button>
</div>

<style>
  .wiersz {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 44px;
    align-items: center;
    min-height: 62px;
    padding-inline-end: 6px;
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: var(--surface);
    transition:
      background-color var(--t-color) linear,
      border-color var(--t-color) linear;
  }
  .wiersz:hover { background: var(--surface-2); border-color: var(--control); }
  /* 2 px ramki bez zmiany rozmiaru: druga linia jako obrys do środka */
  .wybrany,
  .wybrany:hover { border-color: var(--ink); box-shadow: inset 0 0 0 1px var(--ink); }
  .wybor {
    display: grid;
    grid-template-columns: 20px 38px minmax(0, 1fr);
    gap: var(--sp-3);
    align-items: center;
    min-width: 0;
    min-height: 60px;
    padding: 8px 4px 8px 12px;
    border-radius: var(--r-md);
    text-align: start;
  }
  .pierscien {
    width: 20px;
    height: 20px;
    border: 2px solid var(--control);
    border-radius: var(--r-round);
  }
  .wybrany .pierscien { border: 6px solid var(--ink); }
  .tekst { display: flex; flex-direction: column; min-width: 0; }
  .tytul { font-size: var(--fs-base); font-weight: var(--fw-semibold); line-height: var(--lh-tight); }
  .opis { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-body); }
  .edytuj {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border: 1px solid transparent;
    border-radius: var(--r-md);
    color: var(--text-2);
    transition:
      background-color var(--t-color) linear,
      border-color var(--t-color) linear,
      color var(--t-color) linear;
  }
  .edytuj:hover { color: var(--text); border-color: var(--control); background: var(--surface); }
</style>
