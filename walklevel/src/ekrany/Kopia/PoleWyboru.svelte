<script lang="ts" module>
  export type Zaznaczenie = 'tak' | 'nie' | 'czesciowo';
</script>

<script lang="ts">
  // Wiersz z polem wyboru (kopia danych): [pole] [awatar/ikona] tytuł w jednej linii + opis
  // zawijany. Cały wiersz jest przyciskiem (role="checkbox"), zaznaczenie neutralne (--ink),
  // „częściowo” dla osoby z częścią profili. Wcięcie: pole profilu pod awatarem osoby.
  import type { Snippet } from 'svelte';
  import Ikona from '../../lib/ui/Ikona.svelte';

  interface Props {
    zaznaczenie: Zaznaczenie;
    tytul: string;
    opis?: string;
    poczatek?: Snippet;
    wciecie?: boolean;
    disabled?: boolean;
    onzmiana: (zaznaczony: boolean) => void;
  }
  let { zaznaczenie, tytul, opis, poczatek, wciecie = false, disabled = false, onzmiana }: Props = $props();
  const aria = $derived(zaznaczenie === 'czesciowo' ? 'mixed' : zaznaczenie === 'tak');
</script>

<button
  type="button"
  role="checkbox"
  class="wiersz"
  class:wciecie
  aria-checked={aria}
  {disabled}
  onclick={() => onzmiana(zaznaczenie !== 'tak')}
>
  <span class="pole" class:pelne={zaznaczenie !== 'nie'} aria-hidden="true">
    {#if zaznaczenie === 'tak'}
      <Ikona nazwa="zaznacz" rozmiar={12} />
    {:else if zaznaczenie === 'czesciowo'}
      <span class="kreska"></span>
    {/if}
  </span>
  {#if poczatek}<span class="poczatek">{@render poczatek()}</span>{/if}
  <span class="tekst">
    <span class="tytul jedna-linia dane" title={tytul}>{tytul}</span>
    {#if opis}<span class="opis">{opis}</span>{/if}
  </span>
</button>

<style>
  .wiersz {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    width: 100%;
    min-width: 0;
    min-height: var(--control-h);
    padding: 6px var(--sp-2);
    border-radius: var(--r-md);
    text-align: start;
    transition: background-color var(--t-color) linear;
  }
  .wiersz:hover:not(:disabled) { background: var(--surface-2); }
  .wiersz:disabled { color: var(--text-2); }
  /* pole profilu pod awatarem osoby: szerokość pola (20) + odstęp (12) */
  .wciecie { padding-inline-start: calc(var(--sp-2) + 20px + var(--sp-3)); }

  .pole {
    display: grid;
    place-items: center;
    flex: none;
    width: 20px;
    height: 20px;
    border: 2px solid var(--control);
    border-radius: 4px;
    color: var(--on-ink);
    transition:
      background-color var(--t-color) linear,
      border-color var(--t-color) linear;
  }
  .pelne { background: var(--ink); border-color: var(--ink); }
  .wiersz:disabled .pole { border-color: var(--line); }
  .wiersz:disabled .pelne { background: var(--line); }
  .kreska { width: 10px; height: 2px; background: currentColor; }

  .poczatek { display: grid; place-items: center; flex: none; }
  .tekst { display: flex; flex-direction: column; flex: 1; min-width: 0; }
  .tytul { font-size: var(--fs-base); font-weight: var(--fw-semibold); line-height: var(--lh-tight); }
  .wciecie .tytul { font-weight: var(--fw-regular); }
  .opis { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-body); }
</style>
