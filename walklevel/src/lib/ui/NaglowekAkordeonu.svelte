<script lang="ts">
  // Nagłówek akordeonu - kategoria Ustawień (docs/wyglad.md 8.5): stała wysokość 64, tytuł i opis
  // po jednej linii z wielokropkiem, wartość z prawej ustępuje miejsca pierwsza (< 440 px ukryta).
  // Kategorie otwierają się niezależnie; kliknięty nagłówek nie rusza się z miejsca - zmienia się
  // tylko to, co pod nim (treść wjeżdża, elementy niżej zjeżdżają FLIP-em na transform).
  import { tick, type Snippet } from 'svelte';
  import Ikona, { type NazwaIkony } from './Ikona.svelte';

  interface Props {
    ikona: NazwaIkony;
    tytul: string;
    opis: string;
    wartosc?: string;
    otwarty?: boolean;
    onprzelacz?: (otwarty: boolean) => void;
    /** Treść kategorii (pokazywana pod nagłówkiem, gdy otwarta). */
    children?: Snippet;
  }
  let { ikona, tytul, opis, wartosc, otwarty = $bindable(false), onprzelacz, children }: Props = $props();
  const id = $props.id();
  let korzen: HTMLElement;

  async function przelacz() {
    const rodzenstwo = korzen.parentElement ? [...korzen.parentElement.children] : [];
    const nizej = rodzenstwo.slice(rodzenstwo.indexOf(korzen) + 1);
    const przed = nizej.map((el) => el.getBoundingClientRect().top);
    otwarty = !otwarty;
    onprzelacz?.(otwarty);
    await tick();
    if (matchMedia('(prefers-reduced-motion: reduce)').matches) return;
    const styl = getComputedStyle(document.documentElement);
    const czas = parseFloat(styl.getPropertyValue(otwarty ? '--t-in' : '--t-out')) || 0;
    const krzywa = styl.getPropertyValue(otwarty ? '--ease-enter' : '--ease-exit').trim() || 'ease';
    nizej.forEach((el, i) => {
      const roznica = przed[i] - el.getBoundingClientRect().top;
      if (roznica) el.animate([{ transform: `translateY(${roznica}px)` }, { transform: 'none' }], { duration: czas, easing: krzywa });
    });
  }
</script>

<div class="akordeon" bind:this={korzen}>
  <button
    type="button"
    class="naglowek"
    class:otwarty
    aria-expanded={otwarty}
    aria-controls="{id}-tresc"
    id="{id}-naglowek"
    title={opis}
    onclick={przelacz}
  >
    <span class="ikona"><Ikona nazwa={ikona} rozmiar={20} /></span>
    <span class="srodek">
      <span class="tekst">
        <span class="tytul jedna-linia">{tytul}</span>
        <span class="opis jedna-linia">{opis}</span>
      </span>
      {#if wartosc}<span class="wartosc jedna-linia" title={wartosc}>{wartosc}</span>{/if}
    </span>
    <span class="strzalka"><Ikona nazwa="w_prawo" rozmiar={12} /></span>
  </button>
  {#if otwarty && children}
    <div class="tresc" id="{id}-tresc" role="region" aria-labelledby="{id}-naglowek">
      {@render children()}
    </div>
  {/if}
</div>

<style>
  .akordeon { container-type: inline-size; min-width: 0; }
  .naglowek {
    display: grid;
    grid-template-columns: 40px minmax(0, 1fr) 16px;
    gap: 14px;
    align-items: center;
    width: 100%;
    height: var(--header-h);
    padding: 0 14px;
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: var(--surface);
    transition:
      background-color var(--t-color) linear,
      border-color var(--t-color) linear;
  }
  .naglowek:hover { background: var(--surface-2); }
  .naglowek.otwarty { border-color: var(--control); }
  .ikona {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border-radius: var(--r-md);
    background: var(--surface-2);
    color: var(--text-2);
  }
  .srodek { display: flex; align-items: center; gap: 14px; min-width: 0; }
  .tekst { display: flex; flex-direction: column; flex: 0 1 auto; min-width: 0; }
  .tytul { font-size: var(--fs-base); font-weight: var(--fw-semibold); line-height: var(--lh-tight); }
  .opis { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-tight); }
  .wartosc {
    flex: 0 100 auto;
    max-width: 40%;
    margin-inline-start: auto;
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .strzalka {
    display: grid;
    place-items: center;
    color: var(--text-2);
    transition: transform var(--t-switch) var(--ease-smooth);
  }
  .otwarty .strzalka { transform: rotate(90deg); }
  .tresc {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    padding-top: var(--sp-3);
    animation: wl-rozwin var(--t-in) var(--ease-enter);
  }
  @container (max-width: 439px) {
    .wartosc { display: none; }
  }
  @media (prefers-reduced-motion: reduce) {
    .strzalka { transition: none; }
  }
</style>
