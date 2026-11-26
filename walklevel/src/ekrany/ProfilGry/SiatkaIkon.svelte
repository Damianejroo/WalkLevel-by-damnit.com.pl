<script lang="ts">
  // Siatka ikon profilu (docs/wyglad.md 8.14): 12 znaków w polach 48 x 48, 2 rzędy po 6.
  // Wybrana: ramka 2 px --ink i tło --surface-2 (neutralnie, wyglad.md 3). Ikona z gry = nic
  // nie jest wybrane. Strzałki chodzą po siatce.
  import Ikona from '../../lib/ui/Ikona.svelte';
  import { stan } from '../../lib/stan.svelte';
  import { jestKlucz, t } from '../../lib/teksty';

  interface Props {
    /** Wybrany znak (null - ikona z pliku gry). */
    wartosc: string | null;
    etykieta: string;
    onzmiana?: (znak: string) => void;
  }
  let { wartosc, etykieta, onzmiana }: Props = $props();

  const KOLUMNY = 6;
  let siatka: HTMLDivElement;
  let wybrany = $derived(wartosc);
  const znaki = $derived(stan.statics.glyphs);
  const indeks = $derived(znaki.findIndex((g) => g.code === wybrany));

  /** „Ogień” -> profil.ikona.ogien (docs/teksty.md 11). */
  function nazwa(n: string): string {
    const klucz = `profil.ikona.${n.normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/ł/g, 'l').toLowerCase()}`;
    return t('profil.ikona_dostepnosc', { nazwa: jestKlucz(klucz) ? t(klucz) : n });
  }

  function wybierz(i: number) {
    const g = znaki[i];
    if (!g || g.code === wybrany) return;
    wybrany = g.code;
    onzmiana?.(g.code);
  }

  function klawisz(e: KeyboardEvent) {
    const krok: Record<string, number> = { ArrowRight: 1, ArrowLeft: -1, ArrowDown: KOLUMNY, ArrowUp: -KOLUMNY };
    if (!(e.key in krok)) return;
    e.preventDefault();
    const nowy = Math.min(znaki.length - 1, Math.max(0, Math.max(indeks, 0) + krok[e.key]));
    wybierz(nowy);
    siatka.querySelectorAll<HTMLElement>('[role="radio"]')[nowy]?.focus();
  }
</script>

<div class="siatka" role="radiogroup" aria-label={etykieta} bind:this={siatka}>
  {#each znaki as g, i (g.code)}
    <button
      type="button"
      role="radio"
      class="pole"
      aria-checked={i === indeks}
      aria-label={nazwa(g.name)}
      title={g.name}
      tabindex={i === indeks || (indeks < 0 && i === 0) ? 0 : -1}
      onclick={() => wybierz(i)}
      onkeydown={klawisz}
    >
      <Ikona znak={g.code} rozmiar={20} />
    </button>
  {/each}
</div>

<style>
  .siatka {
    display: grid;
    grid-template-columns: repeat(6, 48px);
    gap: var(--sp-2);
  }
  .pole {
    display: grid;
    place-items: center;
    width: 48px;
    height: 48px;
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: var(--surface);
    color: var(--text-2);
    transition:
      background-color var(--t-color) linear,
      border-color var(--t-color) linear,
      color var(--t-color) linear;
  }
  .pole:hover { background: var(--surface-2); color: var(--text); }
  .pole[aria-checked='true'] {
    border: 2px solid var(--ink);
    background: var(--surface-2);
    color: var(--text);
  }
</style>
