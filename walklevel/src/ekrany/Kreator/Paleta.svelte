<script lang="ts">
  // Paleta 20 kolorów (docs/wyglad.md 8.14): próbki 32 px w siatce 2 × 10 albo 4 × 5 (wąsko), wybrana z
  // pierścieniem --text i znakiem zaznaczenia; opis pod spodem. Strzałki chodzą po siatce.
  import Ikona from '../../lib/ui/Ikona.svelte';
  import { t } from '../../lib/teksty';
  import type { ColorName, PaletteColor } from '../../lib/typy';

  interface Props {
    paleta: PaletteColor[];
    wartosc: ColorName;
    /** Nazwa grupy dla czytnika ekranu. */
    etykieta: string;
    /** Dopisek do opisu pod paletą (np. „· dobrany z ikony gry”). */
    dopisek?: string;
    onzmiana: (kolor: ColorName) => void;
  }
  let { paleta, wartosc, etykieta, dopisek = '', onzmiana }: Props = $props();

  let siatka: HTMLDivElement;

  function klawisz(e: KeyboardEvent, i: number) {
    const przyciski = [...siatka.querySelectorAll<HTMLButtonElement>('button')];
    const wRzedzie = Math.max(1, przyciski.filter((p) => p.offsetTop === przyciski[0].offsetTop).length);
    const krok = { ArrowRight: 1, ArrowLeft: -1, ArrowDown: wRzedzie, ArrowUp: -wRzedzie }[e.key];
    if (!krok) return;
    e.preventDefault();
    const nowy = Math.min(przyciski.length - 1, Math.max(0, i + krok));
    onzmiana(paleta[nowy].name);
    przyciski[nowy].focus();
  }
</script>

<div class="paleta">
  <div class="siatka" role="radiogroup" aria-label={etykieta} bind:this={siatka}>
    {#each paleta as k, i (k.name)}
      <button
        type="button"
        role="radio"
        class="probka"
        aria-checked={k.name === wartosc}
        aria-label={t('kolor.probka', { kolor: k.name })}
        title={k.name}
        tabindex={k.name === wartosc ? 0 : -1}
        style:--probka={k.dark}
        onclick={() => onzmiana(k.name)}
        onkeydown={(e) => klawisz(e, i)}
      >
        {#if k.name === wartosc}<Ikona nazwa="zaznacz" rozmiar={14} />{/if}
      </button>
    {/each}
  </div>
  <p class="opis">{wartosc}{dopisek ? ` ${dopisek}` : ''}</p>
</div>

<style>
  .paleta {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    min-width: 0;
    container: paleta / inline-size;
  }
  /* równe rzędy zamiast 9 + 9 + 2: 2 × 10, gdy się mieści, inaczej 4 × 5 */
  .siatka {
    display: grid;
    grid-template-columns: repeat(5, 32px);
    gap: 6px;
    padding: 3px;               /* miejsce na pierścień wybranej próbki */
  }
  .probka {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: var(--r-round);
    background: var(--probka);
    color: #0f1f1a;              /* znak na próbce (wyglad.md 8.14) */
  }
  .probka[aria-checked='true'] {
    outline: 3px solid var(--text);
    outline-offset: 2px;
  }
  .probka:focus-visible { outline: var(--focus-w) solid var(--focus); outline-offset: 3px; }
  .probka[aria-checked='true']:focus-visible { outline-width: 3px; }
  .opis { font-size: var(--fs-sm); color: var(--text-2); }
  @container paleta (min-width: 380px) {
    .siatka { grid-template-columns: repeat(10, 32px); }
  }
</style>
