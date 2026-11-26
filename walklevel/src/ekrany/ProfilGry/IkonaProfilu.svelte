<script lang="ts">
  // Ikona profilu gry (kontrakt IconRef): obrazek z pliku gry albo znak na kolorze profilu.
  // Kolor profilu to dane (wyglad.md 3), nie akcent.
  import Ikona from '../../lib/ui/Ikona.svelte';
  import { stan } from '../../lib/stan.svelte';
  import { probka } from '../../lib/wyglad';
  import type { ColorName } from '../../lib/typy';

  interface Props {
    kolor: ColorName;
    znak: string | null;
    url: string | null;
    rozmiar?: number;
  }
  let { kolor, znak, url, rozmiar = 44 }: Props = $props();
</script>

<span class="ikona-profilu" class:obrazek={!!url} style:--rozmiar="{rozmiar}px" style:background={url ? undefined : probka(kolor, stan.statics.palette)} aria-hidden="true">
  {#if url}
    <img src={url} alt="" width={rozmiar} height={rozmiar} />
  {:else}
    <Ikona znak={znak ?? stan.statics.defaultGlyph} rozmiar={Math.round(rozmiar * 0.46)} />
  {/if}
</span>

<style>
  .ikona-profilu {
    display: grid;
    place-items: center;
    flex: none;
    width: var(--rozmiar);
    height: var(--rozmiar);
    border-radius: var(--r-md);
    overflow: hidden;
    /* znak na próbce w odcieniu „d” - ciemny w obu motywach (wyglad.md 8.14) */
    color: #0f1f1a;
  }
  .obrazek { background: var(--surface-2); }
  img { width: 100%; height: 100%; object-fit: contain; }
</style>
