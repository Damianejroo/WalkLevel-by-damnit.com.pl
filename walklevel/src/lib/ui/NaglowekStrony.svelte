<script lang="ts">
  // Pasek tytułu strony (docs/wyglad.md 9.1): [wstecz] tytuł w jednej linii + podpis pod spodem
  // (dynamiczna część, np. pseudonim) | akcje. Stała wysokość z podpisem i bez. Nie przewija się.
  import type { Snippet } from 'svelte';
  import Przycisk from './Przycisk.svelte';
  import { t } from '../teksty';

  interface Props {
    tytul: string;
    podpis?: string;
    /** Bez `onwroc` nie ma przycisku Wróć (ekran główny). */
    onwroc?: () => void;
    /** Tytuł nigdy nie skracany (nazwa aplikacji na ekranie głównym). */
    pelnyTytul?: boolean;
    akcje?: Snippet;
  }
  let { tytul, podpis = '', onwroc, pelnyTytul = false, akcje }: Props = $props();
</script>

<header class="pasek" class:z-powrotem={!!onwroc} class:pelny={pelnyTytul}>
  {#if onwroc}
    <Przycisk wariant="ikona" ikona="wstecz" etykieta={t('app.wroc')} onclick={onwroc} />
  {/if}
  <div class="tytuly">
    <h1 class="tytul jedna-linia" title={tytul}>{tytul}</h1>
    <p class="podpis jedna-linia" title={podpis || undefined}>{podpis}</p>
  </div>
  {#if akcje}<div class="akcje">{@render akcje()}</div>{/if}
</header>

<style>
  .pasek {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: var(--sp-2);
    align-items: center;
    height: max(var(--pagebar-h), calc(var(--fs-xl) * 1.25 + var(--fs-sm) * 1.25 + 20px));
    padding: 8px var(--sp-4) 8px;
  }
  .z-powrotem { grid-template-columns: var(--icon-btn) minmax(0, 1fr) auto; padding-inline-start: var(--sp-2); }
  .tytuly { display: flex; flex-direction: column; min-width: 0; }
  /* Nazwa aplikacji nigdy nie skracana: kolumna co najmniej tak szeroka jak tytuł, podpis
     (width 0 + min-width 100%) nie poszerza jej i skraca się wielokropkiem */
  .pelny { grid-template-columns: minmax(min-content, 1fr) auto; }
  .pelny .tytul { overflow: visible; }
  .pelny .podpis { width: 0; min-width: 100%; }
  .tytul { font-size: var(--fs-xl); font-weight: var(--fw-semibold); line-height: 1.25; }
  .podpis { font-size: var(--fs-sm); color: var(--text-2); line-height: 1.25; min-height: 1.25em; }
  .akcje { display: flex; align-items: center; gap: 2px; min-width: 0; }
</style>
