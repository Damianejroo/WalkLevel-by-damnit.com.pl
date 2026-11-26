<script lang="ts">
  // Przycisk (docs/wyglad.md 8.1). Warianty: zwykly, glowny (najwyżej jeden na ekranie), ikona,
  // link, stop, potwierdz (akcja niebezpieczna: pierwsze kliknięcie pyta „Na pewno?”, drugie
  // w ciągu 4 s wykonuje).
  import type { Snippet } from 'svelte';
  import Ikona, { type NazwaIkony } from './Ikona.svelte';
  import { t } from '../teksty';

  type Wariant = 'zwykly' | 'glowny' | 'ikona' | 'link' | 'stop' | 'potwierdz';
  interface Props {
    wariant?: Wariant;
    /** Bieżący napis (albo children). */
    napis?: string;
    /** Wszystkie napisy, które przycisk może pokazać - szerokość = najdłuższy (7.2 pkt 6). */
    napisy?: string[];
    ikona?: NazwaIkony;
    /** Nazwa dostępna i podpowiedź; dla wariantu „ikona” obowiązkowa. */
    etykieta?: string;
    title?: string;
    disabled?: boolean;
    typ?: 'button' | 'submit';
    /** Rozciąga przycisk na całą szerokość kontenera. */
    szeroki?: boolean;
    onclick?: () => void;
    children?: Snippet;
  }
  let {
    wariant = 'zwykly', napis, napisy, ikona, etykieta, title, disabled = false, typ = 'button',
    szeroki = false, onclick, children,
  }: Props = $props();

  // --- potwierdz: dwa kliknięcia ---
  const CZAS_PYTANIA_MS = 4000;
  let pyta = $state(false);
  let zegar: ReturnType<typeof setTimeout> | undefined;

  function klikPotwierdz() {
    if (disabled) return;
    clearTimeout(zegar);
    if (pyta) {
      pyta = false;
      onclick?.();
      return;
    }
    pyta = true;
    zegar = setTimeout(() => (pyta = false), CZAS_PYTANIA_MS);
  }
  $effect(() => () => clearTimeout(zegar));

  const potwierdz = $derived(wariant === 'potwierdz');
  const pytanie = t('wspolne.potwierdz');
  const tekst = $derived(potwierdz && pyta ? pytanie : (napis ?? ''));
  // szerokość = dłuższy z napisów, żeby przycisk nie skakał przy pytaniu
  const wszystkie = $derived(potwierdz && !children ? [napis ?? '', pytanie] : napisy);
  const nazwaDostepna = $derived(
    potwierdz && !pyta ? t('wspolne.potwierdz_dostepnosc', { tekst: etykieta ?? napis ?? '' }) : (etykieta ?? undefined),
  );
  const podpowiedz = $derived(title ?? (wariant === 'ikona' ? etykieta : tekst || undefined));
</script>

<button
  type={typ}
  class="przycisk {wariant}"
  class:szeroki
  class:pyta
  {disabled}
  aria-label={nazwaDostepna}
  title={podpowiedz}
  onclick={potwierdz ? klikPotwierdz : onclick}
  onblur={potwierdz ? () => { clearTimeout(zegar); pyta = false; } : undefined}
>
  {#if ikona}<Ikona nazwa={ikona} />{/if}
  {#if wariant !== 'ikona'}
    {#if children}
      <span class="napis jedna-linia">{@render children()}</span>
    {:else if wszystkie?.length}
      <span class="napisy">
        {#each wszystkie as n (n)}
          <span class="napis jedna-linia" class:ukryty={n !== tekst} aria-hidden={n !== tekst}>{n}</span>
        {/each}
      </span>
    {:else if tekst}
      <span class="napis jedna-linia">{tekst}</span>
    {/if}
  {/if}
</button>

<style>
  .przycisk {
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--sp-2);
    min-width: var(--control-h);
    max-width: 100%;
    height: var(--control-h);
    padding: 0 var(--sp-4);
    border: 1px solid var(--control);
    border-radius: var(--r-md);
    background: var(--surface);
    color: var(--text);
    font-size: var(--fs-base);
    font-weight: var(--fw-semibold);
    line-height: var(--lh-tight);
    white-space: nowrap;
    overflow: hidden;
    transition:
      background-color var(--t-color) linear,
      border-color var(--t-color) linear,
      color var(--t-color) linear;
  }
  .szeroki { width: 100%; }
  .przycisk:hover:not(:disabled) { background: var(--surface-2); }
  .przycisk:active:not(:disabled) { background: var(--surface-3); transition-duration: var(--t-press); }
  .przycisk:disabled { color: var(--text-2); border-color: var(--line); }

  .napis { display: block; }
  /* Wszystkie napisy w jednej komórce: szerokość najdłuższego, widoczny tylko bieżący */
  .napisy { display: grid; min-width: 0; }
  .napisy > .napis { grid-area: 1 / 1; text-align: center; }
  .ukryty { visibility: hidden; }

  .glowny { background: var(--accent); border-color: var(--accent); color: var(--on-accent); }
  .glowny:hover:not(:disabled) { background: color-mix(in srgb, var(--accent) 85%, var(--text)); }
  .glowny:active:not(:disabled) { background: color-mix(in srgb, var(--accent) 75%, var(--text)); }
  .glowny:disabled { background: var(--surface); }

  .ikona {
    width: var(--icon-btn);
    padding: 0;
    border-color: transparent;
    background: transparent;
  }
  .ikona:hover:not(:disabled) { border-color: var(--control); }

  .link {
    min-width: 0;
    height: auto;
    min-height: var(--control-h);
    padding: 0;
    border: 0;
    background: transparent;
    font-weight: var(--fw-regular);
  }
  .link:hover:not(:disabled),
  .link:focus-visible { background: transparent; text-decoration: underline; }
  .link:active:not(:disabled) { background: transparent; }

  .stop,
  .potwierdz { border-color: var(--danger); color: var(--danger); font-weight: var(--fw-bold); }
  .stop:hover:not(:disabled) { background: var(--danger); color: var(--bg); }
  .stop:active:not(:disabled) { background: color-mix(in srgb, var(--danger) 80%, var(--text)); color: var(--bg); }
  .potwierdz:hover:not(:disabled) { background: var(--surface-2); }
  /* pytanie „Na pewno?”: pełne tło ostrzeżenia - drugie kliknięcie usuwa */
  .potwierdz.pyta,
  .potwierdz.pyta:hover:not(:disabled) { background: var(--danger); color: var(--bg); }
</style>
