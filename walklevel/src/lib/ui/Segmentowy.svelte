<script lang="ts" module>
  export interface Opcja<W> {
    wartosc: W;
    napis: string;
    /** Kropka „aktywny tryb” przed napisem. */
    aktywny?: boolean;
    title?: string;
  }
</script>

<script lang="ts" generics="T extends string | number">
  // Przełącznik segmentowany (docs/wyglad.md 8.3): segmenty równej szerokości, neutralny
  // wskaźnik przesuwany transformem. Za wąsko na rząd: 4 opcje -> 2 x 2, 3+ -> kolumna,
  // 2 opcje zawsze w rzędzie. Wysokość nie zależy od wybranej wartości.
  import { onMount } from 'svelte';

  interface Props {
    opcje: Opcja<T>[];
    wartosc: T;
    /** Nazwa grupy dla czytnika ekranu. */
    etykieta: string;
    disabled?: boolean;
    onzmiana?: (wartosc: T) => void;
  }
  let { opcje, wartosc = $bindable(), etykieta, disabled = false, onzmiana }: Props = $props();

  let tor: HTMLDivElement;
  let kolumny = $state(0);
  const ile = $derived(opcje.length);
  const k = $derived(kolumny || ile);
  const indeks = $derived(Math.max(0, opcje.findIndex((o) => o.wartosc === wartosc)));

  function dopasuj() {
    if (!tor) return;
    const napisy = [...tor.querySelectorAll<HTMLElement>('.napis')];
    const potrzeba = Math.max(0, ...napisy.map((n) => n.scrollWidth)) + 20 + (opcje.some((o) => o.aktywny) ? 12 : 0);
    const szer = tor.clientWidth - 6;
    const komorka = (szer - (ile - 1) * 2) / ile;
    kolumny = komorka >= potrzeba || ile <= 2 ? ile : ile === 4 ? 2 : 1;
  }

  onMount(() => {
    // w następnej klatce: zmiana liczby kolumn zmienia rozmiar toru, a zmiana w tej samej
    // klatce kończyła się błędem „ResizeObserver loop completed with undelivered notifications”
    const ro = new ResizeObserver(() => requestAnimationFrame(dopasuj));
    ro.observe(tor);
    document.fonts?.ready.then(dopasuj);
    return () => ro.disconnect();
  });
  $effect(() => {
    void opcje.map((o) => o.napis).join();
    dopasuj();
  });

  function wybierz(i: number) {
    if (disabled) return;
    const o = opcje[i];
    if (!o || o.wartosc === wartosc) return;
    wartosc = o.wartosc;
    onzmiana?.(o.wartosc);
  }

  function klawisz(e: KeyboardEvent) {
    const krok = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[e.key];
    if (!krok) return;
    e.preventDefault();
    const nowy = (indeks + krok + ile) % ile;
    wybierz(nowy);
    tor.querySelectorAll<HTMLElement>('[role="radio"]')[nowy]?.focus();
  }
</script>

<div
  class="tor"
  role="radiogroup"
  aria-label={etykieta}
  aria-disabled={disabled || undefined}
  bind:this={tor}
  style:--kolumny={k}
>
  <span
    class="wskaznik"
    aria-hidden="true"
    style:transform="translate(calc({indeks % k} * (100% + 2px)), calc({Math.floor(indeks / k)} * (100% + 2px)))"
  ></span>
  {#each opcje as o, i (o.wartosc)}
    <button
      type="button"
      role="radio"
      class="segment"
      aria-checked={i === indeks}
      tabindex={i === indeks ? 0 : -1}
      title={o.title ?? o.napis}
      {disabled}
      onclick={() => wybierz(i)}
      onkeydown={klawisz}
    >
      {#if o.aktywny}<span class="kropka" aria-hidden="true"></span>{/if}
      <span class="napis jedna-linia">{o.napis}</span>
    </button>
  {/each}
</div>

<style>
  .tor {
    position: relative;
    display: grid;
    grid-template-columns: repeat(var(--kolumny), minmax(0, 1fr));
    gap: 2px;
    min-height: var(--control-h);
    padding: 3px;
    border: 1px solid var(--control);
    border-radius: var(--r-md);
    background: var(--bg);
  }
  .wskaznik {
    position: absolute;
    top: 3px;
    left: 3px;
    width: calc((100% - 6px - (var(--kolumny) - 1) * 2px) / var(--kolumny));
    height: var(--segment-h);
    border: 1px solid var(--sel-border);
    border-radius: var(--r-seg);
    background: var(--sel-bg);
    transition: transform var(--t-slide) var(--ease-smooth);
    pointer-events: none;
  }
  .segment {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    min-width: 0;
    height: var(--segment-h);
    padding: 0 10px;
    border-radius: var(--r-seg);
    color: var(--text-2);
    font-size: var(--fs-md);
    font-weight: var(--fw-semibold);
    transition: color var(--t-color) linear;
  }
  .segment:hover:not(:disabled),
  .segment[aria-checked='true'] { color: var(--text); }
  .segment:disabled { cursor: default; }
  .kropka {
    width: 6px;
    height: 6px;
    flex: none;
    border-radius: var(--r-round);
    background: var(--text);
  }
</style>
