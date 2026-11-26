<script lang="ts">
  // Karta (docs/wyglad.md 8.4): płaska, ramka --line, bez paska z lewej i bez gradientu.
  // Z `onclick` cała karta jest przyciskiem ze strzałką z prawej.
  import type { Snippet } from 'svelte';
  import Ikona from './Ikona.svelte';

  interface Props {
    tytul?: string;
    /** Dynamiczna część tytułu (np. nazwa osoby) - druga linia albo z prawej. */
    podpis?: string;
    podpisZPrawej?: boolean;
    /** Nazwa dostępna karty klikalnej. */
    etykieta?: string;
    onclick?: () => void;
    children?: Snippet;
  }
  let { tytul, podpis, podpisZPrawej = false, etykieta, onclick, children }: Props = $props();
</script>

{#snippet tresc()}
  {#if tytul}
    <svelte:element this={onclick ? 'span' : 'div'} class="naglowek" class:w-rzedzie={podpisZPrawej}>
      <svelte:element this={onclick ? 'span' : 'h2'} class="tytul jedna-linia" title={tytul}>{tytul}</svelte:element>
      {#if podpis}<span class="podpis jedna-linia" title={podpis}>{podpis}</span>{/if}
    </svelte:element>
  {/if}
  {@render children?.()}
{/snippet}

{#if onclick}
  <button type="button" class="karta klikalna" aria-label={etykieta} {onclick}>
    <span class="srodek">{@render tresc()}</span>
    <span class="strzalka"><Ikona nazwa="w_prawo" rozmiar={12} /></span>
  </button>
{:else}
  <section class="karta">{@render tresc()}</section>
{/if}

<style>
  .karta {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    min-width: 0;
    padding: var(--card-pad);
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: var(--surface);
    contain: layout paint;
  }
  .klikalna {
    flex-direction: row;
    align-items: center;
    width: 100%;
    text-align: start;
    transition:
      background-color var(--t-color) linear,
      border-color var(--t-color) linear;
  }
  .klikalna:hover { background: var(--surface-2); border-color: var(--control); }
  .srodek { display: flex; flex-direction: column; gap: var(--sp-2); flex: 1; min-width: 0; }
  .strzalka { color: var(--text-2); flex: none; }

  .naglowek { display: flex; flex-direction: column; min-width: 0; }
  .tytul { display: block; }
  .naglowek.w-rzedzie { flex-direction: row; align-items: baseline; gap: var(--sp-2); }
  .tytul { font-size: var(--fs-base); font-weight: var(--fw-bold); line-height: var(--lh-tight); }
  .podpis { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-tight); }
  .w-rzedzie .podpis { margin-inline-start: auto; flex-shrink: 2; }
</style>
