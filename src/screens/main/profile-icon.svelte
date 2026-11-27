<script lang="ts">
  // Ikona profilu gry: obrazek z gry albo znak Segoe Fluent na neutralnym polu (kolor profilu
  // jest akcentem ekranu, więc ikona go nie powtarza).
  import type { IconRef } from '../../lib/contract';
  import Ikona from '../../lib/ui/icon.svelte';

  let { ikona, rozmiar = 40 }: { ikona: IconRef; rozmiar?: number } = $props();
</script>

<span class="ikona-profilu" aria-hidden="true" style:width="{rozmiar}px" style:height="{rozmiar}px">
  {#if ikona.url}
    <img src={ikona.url} alt="" width={rozmiar} height={rozmiar} />
  {:else}
    <Ikona znak={ikona.glyph ?? ''} rozmiar={Math.round(rozmiar * 0.5)} />
  {/if}
</span>

<style>
  .ikona-profilu {
    display: inline-grid;
    place-items: center;
    flex: none;
    overflow: hidden;
    border-radius: var(--r-md);
    background: var(--surface-2);
    color: var(--text);
  }
  img { width: 100%; height: 100%; object-fit: cover; }
</style>
