<script lang="ts">
  // Układ ekranu (docs/design.md 6.3, 7.1, 9.1): stały pasek tytułu + własny kontener przewijania
  // z kolumną treści maks. 680 px. Każdy ekran w src/screens/ zaczyna się od <Strona>.
  import type { Snippet } from 'svelte';
  import NaglowekStrony from './page-header.svelte';
  import { wroc } from '../state.svelte';

  interface Props {
    tytul: string;
    podpis?: string;
    /** Ekran główny: bez przycisku Wróć, tytuł nie skracany. */
    glowny?: boolean;
    akcje?: Snippet;
    children?: Snippet;
  }
  let { tytul, podpis, glowny = false, akcje, children }: Props = $props();
</script>

<div class="strona">
  <div class="gora">
    <div class="kolumna">
      <NaglowekStrony {tytul} {podpis} {akcje} pelnyTytul={glowny} onwroc={glowny ? undefined : wroc} />
    </div>
  </div>
  <div class="przewijanie">
    <div class="kolumna tresc">
      {@render children?.()}
    </div>
  </div>
</div>

<style>
  .strona {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr);
    height: 100%;
    min-width: 0;
  }
  /* to samo miejsce na pasek przewijania co niżej - pasek tytułu w osi treści */
  .gora { padding-inline-end: var(--scrollbar-w); min-width: 0; }
  .kolumna {
    width: min(100%, var(--content-max));
    min-width: 0;
    margin-inline: auto;
  }
  .przewijanie {
    min-height: 0;
    overflow-y: auto;
    scrollbar-gutter: stable;
    overscroll-behavior: contain;
  }
  .tresc {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    padding: 4px var(--sp-4) var(--sp-5);
  }
</style>
