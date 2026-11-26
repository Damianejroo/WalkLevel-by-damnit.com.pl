<script lang="ts">
  // Ikona profilu gry (IconRef): obrazek z gry albo znak Segoe Fluent na kolorze profilu (dane).
  import type { IconRef } from '../../lib/typy';
  import Ikona from '../../lib/ui/Ikona.svelte';
  import { stan } from '../../lib/stan.svelte';
  import { probka } from '../../lib/wyglad';

  interface Props {
    ikona: IconRef;
    rozmiar?: number;
  }
  let { ikona, rozmiar = 38 }: Props = $props();
</script>

{#if ikona.url}
  <img class="ikona" src={ikona.url} alt="" width={rozmiar} height={rozmiar} />
{:else}
  <span
    class="ikona znak"
    aria-hidden="true"
    style:width="{rozmiar}px"
    style:height="{rozmiar}px"
    style:background={probka(ikona.color, stan.statics.palette)}
  >
    <Ikona znak={ikona.glyph ?? stan.statics.defaultGlyph} rozmiar={Math.round(rozmiar * 0.5)} />
  </span>
{/if}

<style>
  .ikona { flex: none; border-radius: var(--r-md); object-fit: contain; }
  .znak { display: grid; place-items: center; color: var(--on-rarity); }
</style>
