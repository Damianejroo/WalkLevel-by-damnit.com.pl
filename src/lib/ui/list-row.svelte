<script lang="ts">
  // Wiersz listy (design.md 8.14 „Wiersz wyboru”, 9.6 wiersz sesji): [początek] tytuł w jednej
  // linii + opis zawijany | [koniec]. Z `onclick` cały wiersz jest przyciskiem (strzałka albo
  // własny koniec). `wybrany` - neutralne zaznaczenie (ramka --ink).
  import type { Snippet } from 'svelte';
  import Ikona, { type NazwaIkony } from './icon.svelte';

  interface Props {
    tytul: string;
    opis?: string;
    ikona?: NazwaIkony;
    /** Znak fontu ikon (np. ikona profilu). */
    znak?: string;
    /** Element na początku (awatar, odznaka) zamiast ikony. */
    poczatek?: Snippet;
    /** Element na końcu (przełącznik, przycisk, wartość) - wtedy wiersz nie jest przyciskiem. */
    koniec?: Snippet;
    wybrany?: boolean;
    etykieta?: string;
    onclick?: () => void;
  }
  let { tytul, opis, ikona, znak, poczatek, koniec, wybrany = false, etykieta, onclick }: Props = $props();
  const klikalny = $derived(!!onclick && !koniec);
</script>

{#snippet wnetrze()}
  {#if poczatek}
    <span class="poczatek">{@render poczatek()}</span>
  {:else if ikona || znak}
    <span class="poczatek ikona"><Ikona nazwa={ikona} {znak} rozmiar={20} /></span>
  {/if}
  <span class="tekst">
    <span class="tytul jedna-linia dane" title={tytul}>{tytul}</span>
    {#if opis}<span class="opis dane">{opis}</span>{/if}
  </span>
  {#if koniec}
    <span class="koniec">{@render koniec()}</span>
  {:else if klikalny}
    <span class="koniec strzalka"><Ikona nazwa="w_prawo" rozmiar={12} /></span>
  {/if}
{/snippet}

{#if klikalny}
  <button type="button" class="wiersz klikalny" class:wybrany aria-label={etykieta} aria-current={wybrany || undefined} {onclick}>
    {@render wnetrze()}
  </button>
{:else}
  <div class="wiersz" class:wybrany>{@render wnetrze()}</div>
{/if}

<style>
  .wiersz {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    width: 100%;
    min-width: 0;
    min-height: var(--row-h);
    padding: 8px 12px;
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: var(--surface);
    text-align: start;
  }
  .klikalny {
    transition:
      background-color var(--t-color) linear,
      border-color var(--t-color) linear;
  }
  .klikalny:hover { background: var(--surface-2); border-color: var(--control); }
  .wybrany { border: 2px solid var(--ink); padding: 7px 11px; }
  .poczatek { display: grid; place-items: center; flex: none; }
  .ikona {
    width: 38px;
    height: 38px;
    border-radius: var(--r-md);
    background: var(--surface-2);
    color: var(--text-2);
  }
  .tekst { display: flex; flex-direction: column; flex: 1; min-width: 0; }
  .tytul { font-size: var(--fs-base); font-weight: var(--fw-semibold); line-height: var(--lh-tight); }
  .opis { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-body); }
  .koniec { display: flex; align-items: center; flex: none; min-width: 0; max-width: 50%; }
  .strzalka { color: var(--text-2); }
</style>
