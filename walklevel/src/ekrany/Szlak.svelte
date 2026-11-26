<script lang="ts">
  // Ekran: Twój szlak (trasa 'trail'), docs/wyglad.md 8.10; docs/teksty.md 21.2; definicje v2.
  // Pionowa oś przystanków (osiągnięcia „dystans” wg km) z wierszem „TU JESTEŚ” na miejscu
  // przebytych km. Po wejściu „Tu jesteś” w 1/3 wysokości widoku, ustawione przed malowaniem.
  import { untrack } from 'svelte';
  import Strona from '../lib/ui/Strona.svelte';
  import OdznakaRzadkosci from '../lib/ui/OdznakaRzadkosci.svelte';
  import WierszSzlaku from './Szlak/WierszSzlaku.svelte';
  import { kmKrotko } from './Osiagniecia/poziomy';
  import type { Achievement } from '../lib/typy';
  import { t } from '../lib/teksty';
  import { liczba } from '../lib/format';
  import { stan, type PropsEkranu } from '../lib/stan.svelte';

  let { aktywny }: PropsEkranu = $props();

  type Wiersz =
    | { typ: 'przystanek'; a: Achievement; km: number; przebyty: boolean }
    | { typ: 'tu'; km: number; nastepny: number | null };

  const poziomy = $derived(stan.levels);
  const kmTeraz = $derived(poziomy?.km ?? 0);

  const szlak = $derived(
    stan.statics.definitions.achievements
      .filter((a) => a.kind === 'dystans' && a.km !== null)
      .sort((x, y) => (x.km ?? 0) - (y.km ?? 0)),
  );
  const zdobyte = $derived(new Set(poziomy?.progress.filter((p) => p.earnedAt !== null).map((p) => p.id) ?? []));

  const wiersze = $derived.by((): Wiersz[] => {
    const wynik: Wiersz[] = [];
    const nastepny = szlak.find((a) => (a.km ?? 0) > kmTeraz);
    let tu = false;
    for (const a of szlak) {
      const km = a.km ?? 0;
      if (!tu && km > kmTeraz) {
        wynik.push({ typ: 'tu', km: kmTeraz, nastepny: km });
        tu = true;
      }
      // zdobyte „przed czasem” (próg v1 niższy niż v2) też jest przebyte
      wynik.push({ typ: 'przystanek', a, km, przebyty: km <= kmTeraz || zdobyte.has(a.id) });
    }
    if (!tu) wynik.push({ typ: 'tu', km: kmTeraz, nastepny: nastepny?.km ?? null });
    return wynik;
  });

  const przebyte = $derived(szlak.filter((a) => (a.km ?? 0) <= kmTeraz || zdobyte.has(a.id)).length);

  /** Odcinek szyny od wiersza do wiersza jest przebyty, gdy jego koniec leży w przebytych km. */
  const odcinek = (i: number, j: number): 'brak' | 'przebyta' | 'przyszla' => {
    const w = wiersze[i];
    const v = wiersze[j];
    if (!w || !v) return 'brak';
    return Math.max(w.km, v.km) <= kmTeraz ? 'przebyta' : 'przyszla';
  };

  function zdanieTu(nastepny: number | null): string {
    if (nastepny === null) return t('szlak.koniec');
    const zostalo = nastepny - kmTeraz;
    return t('szlak.nastepny', { km: kmKrotko(zostalo), dni: Math.max(1, Math.round(zostalo / 5)) });
  }

  let lista = $state<HTMLElement>();

  // Wejście na ekran: „Tu jesteś” w 1/3 wysokości kontenera przewijania (bez animacji).
  $effect(() => {
    if (!aktywny || !lista) return;
    untrack(() => {
      const tu = lista?.querySelector<HTMLElement>('.tu');
      const kontener = lista?.closest<HTMLElement>('.przewijanie');
      if (!tu || !kontener) return;
      const k = kontener.getBoundingClientRect();
      const r = tu.getBoundingClientRect();
      kontener.scrollTop += r.top + r.height / 2 - (k.top + k.height / 3);
    });
  });
</script>

<Strona tytul={t('szlak.tytul')} podpis={poziomy ? t('szlak.km', { km: kmKrotko(kmTeraz) }) : ''}>
  <section class="naglowek">
    <span class="suma">{t('szlak.km', { km: liczba(kmTeraz, 1) })}</span>
    <span class="opis">{t('szlak.lacznie', { liczba: przebyte, razem: szlak.length })}</span>
  </section>

  <ol class="szlak" bind:this={lista} aria-label={t('szlak.przystanki')}>
    {#each wiersze as w, i (w.typ === 'tu' ? 'tu' : w.a.id)}
      {#if w.typ === 'tu'}
        <WierszSzlaku km={t('szlak.km', { km: kmKrotko(w.km) })} wezel="tu" gora={odcinek(i - 1, i)} dol={odcinek(i, i + 1)}>
          <p class="tu-jestes hud">{t('szlak.tu_jestes')}</p>
          <p class="zdanie">{zdanieTu(w.nastepny)}</p>
        </WierszSzlaku>
      {:else}
        <WierszSzlaku
          km={t('szlak.km', { km: kmKrotko(w.km) })}
          wezel={w.przebyty ? 'przebyty' : 'przyszly'}
          rzadkosc={w.a.rarity}
          gora={odcinek(i - 1, i)}
          dol={odcinek(i, i + 1)}
        >
          <div class="przystanek">
            <OdznakaRzadkosci
              rzadkosc={w.a.rarity}
              znak={w.a.mark}
              rozmiar={44}
              zablokowana={!w.przebyty}
              etykieta={w.przebyty ? t('osiagniecia.zdobyte') : t('poziomy.osiagniecie.do_zdobycia')}
            />
            <div class="tekst">
              <p class="nazwa" title={w.a.name}>{w.a.name}</p>
              <p class="rzadkosc hud" style:color="var(--rarity-{w.a.rarity}-text)">{t(`rzadkosc.${w.a.rarity}`)}</p>
              <p class="opis-przystanku" class:przebyty={w.przebyty}>{w.a.flavor}</p>
            </div>
          </div>
        </WierszSzlaku>
      {/if}
    {/each}
  </ol>
</Strona>

<style>
  .naglowek {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 2px var(--sp-3);
    min-width: 0;
    padding: var(--card-pad);
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: var(--surface);
  }
  .suma {
    font-family: var(--font-num);
    font-size: var(--fs-num-lg);
    font-weight: var(--fw-bold);
    font-variant-numeric: tabular-nums;
    line-height: 1.1;
    white-space: nowrap;
  }
  .opis { min-width: 0; font-size: var(--fs-sm); color: var(--text-2); }

  .szlak { display: flex; flex-direction: column; gap: 4px; min-width: 0; }

  .przystanek {
    display: grid;
    grid-template-columns: 44px minmax(0, 1fr);
    gap: 10px;
    align-items: center;
  }
  .tekst { display: flex; flex-direction: column; min-width: 0; }
  .nazwa {
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    overflow: hidden;
    overflow-wrap: anywhere;
    font-size: 0.9375rem;
    font-weight: var(--fw-bold);
    line-height: var(--lh-tight);
  }
  .rzadkosc { padding-block-start: 0.12em; }
  .opis-przystanku { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-body); }
  .opis-przystanku.przebyty { font-style: italic; }

  .tu-jestes {
    padding-block-start: 0.12em;
    font-size: var(--fs-xs);
    color: var(--accent);
  }
  .zdanie { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-body); }
</style>
