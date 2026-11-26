<script lang="ts">
  // Przycisk (docs/wyglad.md 8.1). Warianty: zwykly, glowny (najwyżej jeden na ekranie), ikona,
  // link, stop, przytrzymaj (akcja niebezpieczna - onclick dopiero po 1 s przytrzymania).
  import type { Snippet } from 'svelte';
  import Ikona, { type NazwaIkony } from './Ikona.svelte';
  import { t } from '../teksty';

  type Wariant = 'zwykly' | 'glowny' | 'ikona' | 'link' | 'stop' | 'przytrzymaj';
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

  // --- przytrzymaj ---
  let trzyma = $state(false);
  let zegar: ReturnType<typeof setTimeout> | undefined;
  const czasTrzymania = () =>
    parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--t-hold')) || 1000;

  function zacznij() {
    if (disabled || trzyma) return;
    trzyma = true;
    zegar = setTimeout(() => {
      trzyma = false;
      onclick?.();
    }, czasTrzymania());
  }
  function przerwij() {
    clearTimeout(zegar);
    trzyma = false;
  }
  function klawisz(e: KeyboardEvent, wcisniety: boolean) {
    if (e.key !== ' ' && e.key !== 'Enter') return;
    e.preventDefault();
    if (wcisniety && !e.repeat) zacznij();
    if (!wcisniety) przerwij();
  }

  const tekst = $derived(napis ?? '');
  const przytrzymaj = $derived(wariant === 'przytrzymaj');
  const nazwaDostepna = $derived(
    przytrzymaj ? t('wspolne.przytrzymaj', { tekst: etykieta ?? tekst }) : (etykieta ?? undefined),
  );
  const podpowiedz = $derived(
    title ?? (przytrzymaj ? t('wspolne.przytrzymaj_podpowiedz') : wariant === 'ikona' ? etykieta : tekst || undefined),
  );
</script>

<button
  type={typ}
  class="przycisk {wariant}"
  class:szeroki
  class:trzyma
  {disabled}
  aria-label={nazwaDostepna}
  title={podpowiedz}
  onclick={przytrzymaj ? undefined : onclick}
  onpointerdown={przytrzymaj ? zacznij : undefined}
  onpointerup={przytrzymaj ? przerwij : undefined}
  onpointerleave={przytrzymaj ? przerwij : undefined}
  onpointercancel={przytrzymaj ? przerwij : undefined}
  onkeydown={przytrzymaj ? (e) => klawisz(e, true) : undefined}
  onkeyup={przytrzymaj ? (e) => klawisz(e, false) : undefined}
  onblur={przytrzymaj ? przerwij : undefined}
>
  {#if przytrzymaj}<span class="wypelnienie" aria-hidden="true"></span>{/if}
  {#if ikona}<Ikona nazwa={ikona} />{/if}
  {#if wariant !== 'ikona'}
    {#if children}
      <span class="napis jedna-linia">{@render children()}</span>
    {:else if napisy?.length}
      <span class="napisy">
        {#each napisy as n (n)}
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
  .przytrzymaj { border-color: var(--danger); color: var(--danger); font-weight: var(--fw-bold); }
  .stop:hover:not(:disabled) { background: var(--danger); color: var(--bg); }
  .stop:active:not(:disabled) { background: color-mix(in srgb, var(--danger) 80%, var(--text)); color: var(--bg); }
  .przytrzymaj:hover:not(:disabled) { background: var(--surface-2); }

  .wypelnienie {
    position: absolute;
    inset: 0;
    background: var(--danger);
    opacity: 0.22;
    transform: scaleX(0);
    transform-origin: left;
    transition: transform var(--t-color) linear;
    pointer-events: none;
  }
  .trzyma .wypelnienie { transform: scaleX(1); transition: transform var(--t-hold) linear; }
</style>
