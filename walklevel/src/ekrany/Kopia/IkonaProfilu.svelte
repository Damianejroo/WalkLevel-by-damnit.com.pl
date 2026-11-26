<script lang="ts">
  // Ikona profilu gry w wierszu kopii: obrazek z gry albo znak w kolorze profilu (kolor danych)
  // na neutralnym tle; w jasnym motywie ciemniejszy odcień palety (czytelność).
  import type { IconRef } from '../../lib/typy';
  import { stan } from '../../lib/stan.svelte';
  import Ikona from '../../lib/ui/Ikona.svelte';

  let { ikona, rozmiar = 28 }: { ikona: IconRef; rozmiar?: number } = $props();
  const kolor = $derived(stan.statics?.palette?.find((p) => p.name === ikona.color));
</script>

<span
  class="ikona"
  style:width="{rozmiar}px"
  style:height="{rozmiar}px"
  style:--kolor-c={kolor?.dark}
  style:--kolor-j={kolor?.light}
  aria-hidden="true"
>
  {#if ikona.url}
    <img src={ikona.url} alt="" width={rozmiar - 6} height={rozmiar - 6} />
  {:else}
    <Ikona znak={ikona.glyph ?? ''} rozmiar={Math.round(rozmiar * 0.55)} />
  {/if}
</span>

<style>
  .ikona {
    display: grid;
    place-items: center;
    flex: none;
    border-radius: var(--r-sm);
    background: var(--surface-2);
    color: var(--kolor-c, var(--text-2));
  }
  :global(:root[data-theme='light']) .ikona { color: var(--kolor-j, var(--text-2)); }
  img { object-fit: contain; }
</style>
