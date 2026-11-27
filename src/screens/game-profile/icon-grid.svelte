<script lang="ts">
  // Siatka ikon profilu (docs/design.md 8.14): 12 znaków w polach 48 x 48, 2 rzędy po 6
  // (w kontenerze węższym niż 6 pól - 3 rzędy po 4). Wybrana: ramka 2 px --ink i tło --surface-2
  // (neutralnie, design.md 3). Ikona z gry = nic nie jest wybrane. Strzałki, Home i End chodzą
  // po siatce.
  import Ikona from '../../lib/ui/icon.svelte';
  import { stan } from '../../lib/state.svelte';
  import { jestKlucz, t } from '../../lib/texts';

  interface Props {
    /** Wybrany znak (null - ikona z pliku gry). */
    wartosc: string | null;
    etykieta: string;
    onzmiana?: (znak: string) => void;
  }
  let { wartosc, etykieta, onzmiana }: Props = $props();

  let siatka: HTMLDivElement;
  let wybrany = $derived(wartosc);
  const znaki = $derived(stan.statics.glyphs);
  const indeks = $derived(znaki.findIndex((g) => g.code === wybrany));

  /** „Ogień” -> profil.ikona.ogien (docs/texts.md 11). */
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
    const przyciski = [...siatka.querySelectorAll<HTMLElement>('[role="radio"]')];
    // od pola z fokusem - przy ikonie z gry nic nie jest wybrane, a fokus stoi na pierwszym
    const biezacy = Math.max(0, przyciski.indexOf(e.currentTarget as HTMLElement));
    const gora = przyciski[0]?.offsetTop ?? 0;
    const kolumny = Math.max(1, przyciski.filter((p) => p.offsetTop === gora).length);
    const krok: Record<string, number> = {
      ArrowRight: 1, ArrowLeft: -1, ArrowDown: kolumny, ArrowUp: -kolumny,
      Home: -biezacy, End: znaki.length - 1 - biezacy,
    };
    if (!(e.key in krok)) return;
    e.preventDefault();
    const nowy = Math.min(znaki.length - 1, Math.max(0, biezacy + krok[e.key]));
    wybierz(nowy);
    przyciski[nowy]?.focus();
  }
</script>

<div class="kontener">
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
</div>

<style>
  .kontener { min-width: 0; container-type: inline-size; }
  .siatka {
    display: grid;
    grid-template-columns: repeat(6, 48px);
    gap: var(--sp-2);
  }
  /* 6 x 48 + 5 odstępów = 328 px; węższy kontener (design.md 7.1: 320) - po 4 w rzędzie */
  @container (max-width: 327px) {
    .siatka { grid-template-columns: repeat(4, 48px); }
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
