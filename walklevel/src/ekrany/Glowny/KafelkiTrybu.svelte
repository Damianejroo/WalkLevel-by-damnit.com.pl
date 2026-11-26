<script lang="ts">
  // Kafelki trybu (docs/wyglad.md 8.14, docs/teksty.md 8.1): Klawisze, Pad, Chodzenie, Trening.
  // Wysokość 78, wybrany neutralnie (tło --sel-bg, ramka 2 px --ink). 4 w rzędzie od 440 px
  // kontenera, węziej 2 x 2. Grupa radiowa: strzałki zmieniają wybór, Tab wchodzi na wybrany.
  import Ikona, { type NazwaIkony } from '../../lib/ui/Ikona.svelte';
  import type { Direction } from '../../lib/typy';
  import * as api from '../../lib/api';
  import { stan, wykonaj } from '../../lib/stan.svelte';
  import { t, type Klucz } from '../../lib/teksty';

  const TRYBY: Array<{ tryb: Direction; ikona: NazwaIkony; nazwa: Klucz; opis: Klucz }> = [
    { tryb: 'keyboard', ikona: 'klawiatura', nazwa: 'tryb.klawisze.nazwa', opis: 'tryb.klawisze.opis' },
    { tryb: 'gamepad', ikona: 'pad', nazwa: 'tryb.pad.nazwa', opis: 'tryb.pad.opis' },
    { tryb: 'treadmill', ikona: 'chod', nazwa: 'tryb.chodzenie.nazwa', opis: 'tryb.chodzenie.opis' },
    { tryb: 'training', ikona: 'trening', nazwa: 'tryb.trening.nazwa', opis: 'tryb.trening.opis' },
  ];

  const wybrany = $derived(stan.settings.direction);
  const kafelki = $derived(
    TRYBY.map((k) => {
      const proba = k.tryb === 'treadmill' && stan.settings.output === 'dry';
      const nazwa = t(k.nazwa);
      const opis = t(proba ? 'tryb.chodzenie.opis_proba' : k.opis, { klawisz_chodu: stan.settings.walkKey.name });
      return { ...k, nazwaTekst: nazwa, opisTekst: opis, dostepnosc: t('tryb.dostepnosc', { tryb: nazwa, opis }) };
    }),
  );

  let grupa = $state<HTMLElement>();

  function wybierz(tryb: Direction) {
    if (tryb !== wybrany) wykonaj(api.settingsSet('direction', tryb));
  }

  function klawisz(e: KeyboardEvent) {
    const krok = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[e.key];
    if (!krok) return;
    e.preventDefault();
    const i = TRYBY.findIndex((k) => k.tryb === wybrany);
    const nowy = (i + krok + TRYBY.length) % TRYBY.length;
    wybierz(TRYBY[nowy].tryb);
    grupa?.querySelectorAll<HTMLButtonElement>('button')[nowy]?.focus();
  }
</script>

<div class="kontener">
  <div class="kafelki" role="radiogroup" aria-label={t('glowny.sterowanie.tryby')} bind:this={grupa} onkeydown={klawisz} tabindex="-1">
    {#each kafelki as k (k.tryb)}
      <button
        type="button"
        class="kafelek"
        role="radio"
        aria-checked={k.tryb === wybrany}
        aria-label={k.dostepnosc}
        tabindex={k.tryb === wybrany ? 0 : -1}
        title={k.opisTekst}
        onclick={() => wybierz(k.tryb)}
      >
        <Ikona nazwa={k.ikona} rozmiar={20} />
        <span class="nazwa jedna-linia">{k.nazwaTekst}</span>
        <span class="opis jedna-linia">{k.opisTekst}</span>
      </button>
    {/each}
  </div>
</div>

<style>
  .kontener { container-type: inline-size; min-width: 0; }
  .kafelki { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: var(--sp-2); }
  @container (min-width: 440px) {
    .kafelki { grid-template-columns: repeat(4, minmax(0, 1fr)); }
  }
  .kafelek {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
    min-width: 0;
    height: 78px;
    padding: 0 8px;
    border: 1px solid var(--control);
    border-radius: var(--r-md);
    background: var(--surface);
    color: var(--text);
    text-align: center;
    transition:
      background-color var(--t-color) linear,
      border-color var(--t-color) linear;
  }
  .kafelek:hover { background: var(--surface-2); }
  .kafelek[aria-checked='true'] {
    background: var(--sel-bg);
    border-color: var(--ink);
    box-shadow: inset 0 0 0 1px var(--ink); /* ramka 2 px bez przesunięcia treści */
  }
  .nazwa { max-width: 100%; margin-top: 2px; font-size: 0.9375rem; font-weight: var(--fw-bold); line-height: var(--lh-tight); }
  .opis { max-width: 100%; font-size: var(--fs-cap); color: var(--text-2); line-height: var(--lh-tight); }
</style>
