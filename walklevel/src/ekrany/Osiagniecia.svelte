<script lang="ts">
  // Ekran: Osiągnięcia (trasa 'achievements'), docs/wyglad.md 8.8-8.9, 9.5; docs/teksty.md 21.1.
  // Pasek rzadkości, przejście do szlaku, grupy: Ostatnio zdobyte, Najbliżej zdobycia, Pozostałe.
  // Bez akcentu (kolory niosą odznaki). Listy liczone z definicji i postępu (kontrakt 6.7).
  import { untrack } from 'svelte';
  import Strona from '../lib/ui/Strona.svelte';
  import Przycisk from '../lib/ui/Przycisk.svelte';
  import OdznakaRzadkosci from '../lib/ui/OdznakaRzadkosci.svelte';
  import WierszOsiagniecia from './Osiagniecia/WierszOsiagniecia.svelte';
  import { kmKrotko, type Pozycja } from './Osiagniecia/poziomy';
  import { t } from '../lib/teksty';
  import * as api from '../lib/api';
  import { stan, ui, idz, wykonaj, type PropsEkranu } from '../lib/stan.svelte';

  let { aktywny }: PropsEkranu = $props();

  let tresc = $state<HTMLElement>();
  let teraz = $state(new Date());

  // Wejście na ekran: przeliczenie bez okienek (levels_refresh); z ekranu głównego od góry,
  // powrót ze szlaku zostawia miejsce na liście.
  $effect(() => {
    if (!aktywny) return;
    untrack(() => {
      teraz = new Date();
      if (ui.kierunek === 1) tresc?.closest('.przewijanie')?.scrollTo({ top: 0 });
      void wykonaj(api.levelsRefresh());
      // „Ma to X% graczy”: z konta online, rdzeń pobiera najwyżej raz na dobę; bez konta albo
      // sieci - bez dopisku
      if (stan.account.loggedIn) {
        api.onlineLoad('achievements', false).catch((e: unknown) => console.warn('Statystyki osiągnięć', e));
      }
    });
  });

  const defs = $derived(stan.statics.definitions);
  const poziomy = $derived(stan.levels);

  const pozycje = $derived.by((): Pozycja[] => {
    if (!poziomy) return [];
    const wgId = new Map(defs.achievements.map((a) => [a.id, a]));
    const wynik: Pozycja[] = [];
    for (const p of poziomy.progress) {
      const a = wgId.get(p.id);
      if (a) wynik.push({ a, p, zdobyte: p.earnedAt !== null });
    }
    return wynik;
  });

  const ostatnie = $derived(
    pozycje
      .filter((x) => x.zdobyte)
      .sort((x, y) => (y.p.earnedAt ?? 0) - (x.p.earnedAt ?? 0))
      .slice(0, 3),
  );
  const najblizej = $derived(
    pozycje
      .filter((x) => !x.zdobyte && x.p.current > 0)
      .sort((x, y) => y.p.fraction - x.p.fraction)
      .slice(0, 3),
  );
  const pozostale = $derived.by(() => {
    const pokazane = new Set([...ostatnie, ...najblizej].map((x) => x.a.id));
    return pozycje
      .filter((x) => !pokazane.has(x.a.id))
      .sort((x, y) => Number(y.zdobyte) - Number(x.zdobyte) || y.p.fraction - x.p.fraction);
  });

  const rzadkosci = $derived(
    defs.rarities.map((r) => {
      const wszystkie = pozycje.filter((x) => x.a.rarity === r);
      return { r, zdobyte: wszystkie.filter((x) => x.zdobyte).length, razem: wszystkie.length };
    }),
  );

  const podpis = $derived(
    poziomy ? t('osiagniecia.podpis', { liczba: poziomy.earnedCount, razem: poziomy.total }) : '',
  );
</script>

{#snippet grupa(tytul: string, lista: Pozycja[], zwiezle: boolean)}
  {#if lista.length}
    <section class="grupa">
      <h2 class="tytul-grupy">{tytul}</h2>
      <ul class="lista">
        {#each lista as pozycja (pozycja.a.id)}
          <WierszOsiagniecia
            {pozycja}
            {teraz}
            procent={stan.online.achievementStats?.percent[pozycja.a.id]}
            xp={defs.rarityXp[pozycja.a.rarity]}
            zwiezly={zwiezle && !pozycja.zdobyte}
          />
        {/each}
      </ul>
    </section>
  {/if}
{/snippet}

<Strona tytul={t('osiagniecia.tytul')} {podpis}>
  <div class="ekran-osiagniec" bind:this={tresc}>
    {#if poziomy}
      <ul class="rzadkosci karta" aria-label={t('osiagniecia.lista_rzadkosci')}>
        {#each rzadkosci as { r, zdobyte, razem } (r)}
          {@const nazwa = t(`rzadkosc.${r}`)}
          <li class="rzadkosc" aria-label={t('osiagniecia.rzadkosc_dostepnosc', { rzadkosc: nazwa, liczba: zdobyte, razem })}>
            <OdznakaRzadkosci rzadkosc={r} liczba={zdobyte} rozmiar={34} zablokowana={zdobyte === 0} />
            <span class="nazwa-rzadkosci hud jedna-linia" style:color="var(--rarity-{r}-text)" aria-hidden="true">{nazwa}</span>
            <span class="licznik" aria-hidden="true">{t('poziomy.grupa_licznik', { liczba: zdobyte, razem })}</span>
          </li>
        {/each}
      </ul>

      <Przycisk szeroki napis={t('szlak.tytul_km', { km: kmKrotko(poziomy.km) })} onclick={() => idz('trail')} />

      {#if poziomy.earnedCount === 0}
        <p class="pusto">{t('poziomy.brak_osiagniec')}</p>
      {/if}

      {@render grupa(t('poziomy.ostatnie'), ostatnie, false)}
      {@render grupa(t('poziomy.najblizej'), najblizej, false)}
      {@render grupa(t('poziomy.pozostale'), pozostale, true)}

      <p class="uwaga">{t('poziomy.symulator')}</p>
    {:else}
      <p class="pusto">{t('poziomy.brak_osiagniec')}</p>
    {/if}
  </div>
</Strona>

<style>
  .ekran-osiagniec {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    min-width: 0;
    container-type: inline-size;
  }

  .karta {
    padding: var(--card-pad);
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: var(--surface);
  }

  /* Pasek rzadkości: 5 równych kolumn; w wąskim kontenerze 3 + 2 (wyglad.md 7.3), drugi rząd
     przesunięty o pół komórki - komórki pod sobą stoją na przemian, bez nierównych odstępów */
  .rzadkosci {
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: var(--sp-3) var(--sp-2);
  }
  .rzadkosc {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    min-width: 0;
  }
  @container (max-width: 459px) {
    .rzadkosci { grid-template-columns: repeat(6, minmax(0, 1fr)); }
    .rzadkosc { grid-column: span 2; }
    .rzadkosc:nth-child(4) { grid-column: 2 / span 2; }
  }
  .rzadkosc :global(.odznaka) { margin-block-end: 2px; }
  .nazwa-rzadkosci {
    max-width: 100%;
    font-size: 0.625rem;
    letter-spacing: 0.08em;
    padding-block-start: 0.12em;
  }
  .licznik {
    font-family: var(--font-num);
    font-size: var(--fs-cap);
    font-variant-numeric: tabular-nums;
    color: var(--text-2);
    line-height: var(--lh-caps);
  }

  .grupa { display: flex; flex-direction: column; gap: var(--sp-2); min-width: 0; }
  .tytul-grupy {
    margin-block-start: var(--sp-2);
    font-size: var(--fs-base);
    font-weight: var(--fw-bold);
    line-height: var(--lh-tight);
  }
  .lista { display: flex; flex-direction: column; gap: var(--sp-2); }

  .pusto { color: var(--text-2); font-size: var(--fs-md); }
  .uwaga {
    margin-block-start: var(--sp-2);
    color: var(--text-2);
    font-size: var(--fs-sm);
    text-align: center;
  }
</style>
