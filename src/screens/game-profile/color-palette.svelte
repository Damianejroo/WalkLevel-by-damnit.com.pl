<script lang="ts">
  // Paleta 20 kolorów (docs/design.md 8.14 „Paleta kolorów profilu”): próbki 32 px w odcieniu „d”,
  // wybrana z pierścieniem --text i znakiem zaznaczenia, opis pod spodem („Koral · dobrany z ikony
  // gry”). Szeroko 10 w rzędzie, w wąskiej karcie 7 (rzędy 7/7/6). Strzałki chodzą po siatce.
  // Używana w profilu gry i w edycji osoby.
  import Ikona from '../../lib/ui/icon.svelte';
  import { stan } from '../../lib/state.svelte';
  import { probka } from '../../lib/theme';
  import { t } from '../../lib/texts';
  import type { ColorName } from '../../lib/contract';

  interface Props {
    wartosc: ColorName;
    /** Kolor dobrany z ikony gry (podpowiedź w opisie). */
    zIkony?: ColorName | null;
    /** Nazwa grupy dla czytnika ekranu. */
    etykieta: string;
    onzmiana?: (kolor: ColorName) => void;
  }
  let { wartosc, zIkony = null, etykieta, onzmiana }: Props = $props();
  const id = $props.id();

  let siatka: HTMLDivElement;
  // wybór widać od razu, zanim rdzeń odeśle nowy stan
  let wybrany = $derived(wartosc);
  const kolory = $derived(stan.statics.palette);
  const indeks = $derived(Math.max(0, kolory.findIndex((k) => k.name === wybrany)));
  const opis = $derived(
    !zIkony
      ? wybrany
      : zIkony === wybrany
        ? `${wybrany} ${t('kolor.z_ikony')}`
        : `${wybrany} ${t('kolor.recznie', { kolor: zIkony })}`,
  );

  function wybierz(i: number) {
    const k = kolory[i];
    if (!k || k.name === wybrany) return;
    wybrany = k.name;
    onzmiana?.(k.name);
  }

  function klawisz(e: KeyboardEvent) {
    const przyciski = [...siatka.querySelectorAll<HTMLElement>('[role="radio"]')];
    const gora = przyciski[0]?.offsetTop ?? 0;
    const kolumny = Math.max(1, przyciski.filter((p) => p.offsetTop === gora).length);
    const krok: Record<string, number> = {
      ArrowRight: 1, ArrowLeft: -1, ArrowDown: kolumny, ArrowUp: -kolumny,
      Home: -indeks, End: kolory.length - 1 - indeks,
    };
    if (!(e.key in krok)) return;
    e.preventDefault();
    const nowy = Math.min(kolory.length - 1, Math.max(0, indeks + krok[e.key]));
    wybierz(nowy);
    przyciski[nowy]?.focus();
  }
</script>

<div class="paleta">
  <div class="siatka" role="radiogroup" aria-label={etykieta} aria-describedby="{id}-opis" bind:this={siatka}>
    {#each kolory as k, i (k.name)}
      <button
        type="button"
        role="radio"
        class="probka"
        aria-checked={i === indeks}
        aria-label={t('kolor.probka', { kolor: k.name })}
        title={k.name}
        tabindex={i === indeks ? 0 : -1}
        style:background={probka(k.name, kolory)}
        onclick={() => wybierz(i)}
        onkeydown={klawisz}
      >
        {#if i === indeks}<Ikona nazwa="zaznacz" rozmiar={14} />{/if}
      </button>
    {/each}
  </div>
  <p class="opis jedna-linia" id="{id}-opis" title={opis}>{opis}</p>
</div>

<style>
  .paleta {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    min-width: 0;
    container-type: inline-size;
  }
  .siatka {
    display: grid;
    grid-template-columns: repeat(10, 32px);
    gap: 10px;
    padding-block: 5px;        /* miejsce na pierścień wybranej próbki (w bok mieści się w odstępie karty) */
  }
  @container (max-width: 420px) {
    .siatka { grid-template-columns: repeat(7, 32px); }
  }
  .probka {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: var(--r-round);
    /* znak na próbce w odcieniu „d” - ciemny w obu motywach (design.md 8.14) */
    color: #0f1f1a;
  }
  .probka[aria-checked='true'] { box-shadow: 0 0 0 2px var(--surface), 0 0 0 5px var(--text); }
  .opis { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-body); }
</style>
