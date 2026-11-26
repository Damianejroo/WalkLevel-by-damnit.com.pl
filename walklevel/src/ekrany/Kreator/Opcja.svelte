<script lang="ts">
  // Opcja konfiguratora (wybór jednej z kilku): ikona 40 | tytuł + opis | znak zaznaczenia.
  // Zaznaczenie neutralne (wyglad.md 3): ramka 2 px --ink, tło --sel-bg; grubsza ramka nie
  // zmienia wymiarów. Dodatkowa treść (np. wskazana gra) pod przyciskiem,
  // wewnątrz tej samej ramki - pola nie mogą być w środku przycisku.
  import type { Snippet } from 'svelte';
  import Ikona, { type NazwaIkony } from '../../lib/ui/Ikona.svelte';

  interface Props {
    ikona: NazwaIkony;
    tytul: string;
    opis: string;
    wybrany: boolean;
    onclick: () => void;
    children?: Snippet;
  }
  let { ikona, tytul, opis, wybrany, onclick, children }: Props = $props();

  /** Strzałki przenoszą fokus na sąsiednią opcję tej samej grupy (wybór dopiero Spacją/Enterem,
   *  bo wybranie „Wskaż folder gry” otwiera okno systemowe). */
  function strzalki(e: KeyboardEvent) {
    const krok = { ArrowDown: 1, ArrowRight: 1, ArrowUp: -1, ArrowLeft: -1 }[e.key];
    const ten = e.currentTarget as HTMLButtonElement;
    const grupa = ten.closest('[role="radiogroup"]');
    if (!krok || !grupa) return;
    const opcje = [...grupa.querySelectorAll<HTMLButtonElement>(':scope > .opcja > .wybor')];
    const nowy = opcje[opcje.indexOf(ten) + krok];
    if (!nowy) return;
    e.preventDefault();
    nowy.focus();
  }
</script>

<div class="opcja" class:wybrany>
  <button type="button" class="wybor" role="radio" aria-checked={wybrany} {onclick} onkeydown={strzalki}>
    <span class="ikona"><Ikona nazwa={ikona} rozmiar={20} /></span>
    <span class="tekst">
      <span class="tytul">{tytul}</span>
      <span class="opis">{opis}</span>
    </span>
    <span class="znak" class:widoczny={wybrany}><Ikona nazwa="zaznacz" rozmiar={16} /></span>
  </button>
  {#if children}<div class="dodatek">{@render children()}</div>{/if}
</div>

<style>
  .opcja {
    display: flex;
    flex-direction: column;
    min-width: 0;
    border: 1px solid var(--control);
    border-radius: var(--r-md);
    background: var(--surface);
    transition: border-color var(--t-color) linear, background-color var(--t-color) linear;
  }
  .opcja:hover:not(.wybrany) { background: var(--surface-2); }
  /* ramka 2 px bez zmiany wymiarów: druga linia jako cień wewnętrzny */
  .wybrany {
    border-color: var(--ink);
    box-shadow: inset 0 0 0 1px var(--ink);
    background: var(--sel-bg);
  }
  .wybor {
    display: grid;
    grid-template-columns: 40px minmax(0, 1fr) 16px;
    gap: var(--sp-3);
    align-items: center;
    width: 100%;
    min-height: 64px;
    padding: 12px 14px;
    border-radius: var(--r-md);
    text-align: start;
  }
  .ikona {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border-radius: var(--r-md);
    background: var(--surface-2);
    color: var(--text-2);
  }
  /* w ciemnym motywie --sel-bg = --surface-2: kafelek ikony o stopień ciemniej/jaśniej */
  .wybrany .ikona { background: var(--surface-3); color: var(--text); }
  .tekst { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .tytul { font-size: var(--fs-base); font-weight: var(--fw-bold); line-height: var(--lh-tight); }
  .opis { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-body); }
  .znak { color: var(--text); opacity: 0; }
  .znak.widoczny { opacity: 1; }
  .dodatek {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    min-width: 0;
    padding: 0 14px 14px;
  }
</style>
