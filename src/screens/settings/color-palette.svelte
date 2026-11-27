<script lang="ts">
  // Paleta kolorów profilu (docs/design.md 8.14): próbki 32 px w zawijającej siatce, wybrana z
  // pierścieniem --text i znakiem zaznaczenia; pod spodem nazwa i skąd kolor. Próbki to dane
  // (kolory profili), nie akcent. Strzałki po siatce.
  import Ikona from '../../lib/ui/icon.svelte';
  import { t } from '../../lib/texts';
  import type { ColorName, PaletteColor } from '../../lib/contract';

  interface Props {
    paleta: PaletteColor[];
    wybrany: ColorName;
    zIkony: ColorName | null;
    etykieta: string;
    onwybor: (kolor: ColorName) => void;
  }
  let { paleta, wybrany, zIkony, etykieta, onwybor }: Props = $props();
  let siatka: HTMLDivElement;

  const opis = $derived(
    zIkony === wybrany
      ? `${wybrany} ${t('kolor.z_ikony')}`
      : zIkony
        ? `${wybrany} ${t('kolor.recznie', { kolor: zIkony })}`
        : wybrany,
  );
  const indeks = $derived(Math.max(0, paleta.findIndex((k) => k.name === wybrany)));

  function klawisz(e: KeyboardEvent) {
    const kolumny = Math.max(1, Math.floor((siatka.clientWidth + 6) / 38));
    const krok = { ArrowRight: 1, ArrowLeft: -1, ArrowDown: kolumny, ArrowUp: -kolumny }[e.key];
    if (!krok) return;
    e.preventDefault();
    const nowy = Math.min(paleta.length - 1, Math.max(0, indeks + krok));
    onwybor(paleta[nowy].name);
    siatka.querySelectorAll<HTMLElement>('[role="radio"]')[nowy]?.focus();
  }
</script>

<div class="paleta">
  <div class="siatka" role="radiogroup" aria-label={etykieta} bind:this={siatka}>
    {#each paleta as k, i (k.name)}
      <button
        type="button"
        role="radio"
        class="probka"
        aria-checked={k.name === wybrany}
        aria-label={t('kolor.probka', { kolor: k.name })}
        title={k.name}
        tabindex={i === indeks ? 0 : -1}
        style:background={k.dark}
        onclick={() => k.name !== wybrany && onwybor(k.name)}
        onkeydown={klawisz}
      >
        {#if k.name === wybrany}<Ikona nazwa="zaznacz" rozmiar={14} />{/if}
      </button>
    {/each}
  </div>
  <p class="opis">{opis}</p>
</div>

<style>
  .paleta { display: flex; flex-direction: column; gap: var(--sp-2); min-width: 0; }
  .siatka {
    display: grid;
    grid-template-columns: repeat(auto-fill, 32px);
    gap: 6px;
    padding: 5px;
    margin-inline-start: -5px; /* próbki w linii z tekstem; miejsce na pierścień zostaje */
  }
  .probka {
    position: relative;
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: var(--r-round);
    color: var(--on-rarity);
  }
  .probka[aria-checked='true']::after {
    content: '';
    position: absolute;
    inset: -5px;
    border: 3px solid var(--text);
    border-radius: var(--r-round);
  }
  .opis { font-size: var(--fs-sm); color: var(--text-2); }
</style>
