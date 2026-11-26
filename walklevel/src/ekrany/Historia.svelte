<script lang="ts">
  // Ekran: Historia kroków (trasa 'history'), docs/wyglad.md 8.15 i 9.6, docs/teksty.md 20.
  // Zakres i „czyja” zostają w ekranie (montowany raz). Dane z history_query - pobierane przy
  // wejściu i ponownie, gdy zmieni się history.stamp albo zapytanie; do czasu odpowiedzi zostaje
  // poprzedni widok (bez migania i bez zmiany wysokości). Akcentu brak: słupki w kolorach profili.
  import Strona from '../lib/ui/Strona.svelte';
  import Karta from '../lib/ui/Karta.svelte';
  import Segmentowy from '../lib/ui/Segmentowy.svelte';
  import SiatkaKafelkow from '../lib/ui/SiatkaKafelkow.svelte';
  import KafelekStatystyki from '../lib/ui/KafelekStatystyki.svelte';
  import WykresKrokow from './Historia/WykresKrokow.svelte';
  import WierszSesji from './Historia/WierszSesji.svelte';
  import IkonaProfilu from './Historia/IkonaProfilu.svelte';
  import { czas, t, tl } from '../lib/teksty';
  import { liczba, zJednostka } from '../lib/format';
  import { stan, idz, wykonaj, type PropsEkranu } from '../lib/stan.svelte';
  import * as api from '../lib/api';
  import type * as K from '../lib/typy';

  let { aktywny }: PropsEkranu = $props();

  let zakres = $state<K.HistoryRange>('week'); // jak w 0.5.x
  let czyja = $state<K.HistoryScope>('me');
  let widok = $state.raw<K.HistoryView | null>(null);
  let teraz = $state.raw(new Date());

  const osoba = $derived(stan.people.people.find((o) => o.id === stan.people.activePersonId));
  const wiele = $derived(stan.people.people.length > 1);
  const dostepna = $derived(stan.people.ready && stan.history.available);

  let numer = 0;
  $effect(() => {
    if (!aktywny || !dostepna) return;
    const zapytanie: K.HistoryQuery = { scope: czyja, range: zakres };
    void stan.history.stamp;
    const moj = ++numer;
    wykonaj(api.historyQuery(zapytanie)).then((v) => {
      if (!v || moj !== numer) return;
      widok = v;
      teraz = new Date();
    });
  });

  const wszyscy = $derived(widok?.everyone ?? false);
  const podpis = $derived(wiele ? (czyja === 'all' ? t('historia.czyja.wszyscy') : (osoba?.name ?? '')) : '');
  const sumy = $derived(widok?.totals ?? null);
  /** Tytuł wykresu z pokazanych danych (nie z wybranego zakresu - do odpowiedzi zostaje stary wykres). */
  const godzinowy = $derived(widok ? widok.chart.hourly : zakres === 'day');

  const ZAKRESY: Array<{ wartosc: K.HistoryRange; napis: string }> = [
    { wartosc: 'day', napis: t('historia.zakres.dzis') },
    { wartosc: 'week', napis: t('historia.zakres.7') },
    { wartosc: 'month', napis: t('historia.zakres.30') },
    { wartosc: 'all', napis: t('historia.zakres.wszystko') },
  ];
  const CZYJA: Array<{ wartosc: K.HistoryScope; napis: string }> = [
    { wartosc: 'me', napis: t('historia.czyja.moja') },
    { wartosc: 'all', napis: t('historia.czyja.wszyscy') },
  ];

  /** Osoba grupy (podział na osoby) - kolor awatara z kontraktu, nazwa z grupy. */
  const osobaGrupy = (g: K.HistoryGroup) => (g.avatarColor ? { name: g.name, color: g.avatarColor } : null);

  function ukryjOnline(s: K.SessionRow, ukryta: boolean) {
    wykonaj(api.historySessionHideOnline(s.id, ukryta));
  }
</script>

<Strona tytul={t('historia.tytul')} {podpis}>
  {#if !stan.people.ready}
    <p class="info">{t('historia.brak_osoby')}</p>
  {:else if !stan.history.available}
    <p class="info">{t('historia.niedostepna')}</p>
  {:else}
    <div class="wybor">
      {#if wiele}
        <Segmentowy etykieta={t('historia.czyja')} opcje={CZYJA} bind:wartosc={czyja} />
      {/if}
      <Segmentowy etykieta={t('historia.zakres')} opcje={ZAKRESY} bind:wartosc={zakres} />
    </div>

    <SiatkaKafelkow>
      <KafelekStatystyki tytul={t('historia.suma.kroki')} wartosc={sumy ? liczba(sumy.steps) : null} />
      <KafelekStatystyki tytul={t('historia.suma.dystans')} wartosc={sumy ? zJednostka(sumy.distanceKm, 'km', 2) : null} />
      <KafelekStatystyki tytul={t('historia.suma.czas')} wartosc={sumy ? czas(sumy.activeS) : null} />
      <KafelekStatystyki tytul={t('historia.suma.srednio')} wartosc={sumy ? zJednostka(sumy.avgKmh, 'km/h', 1) : null} />
    </SiatkaKafelkow>

    <Karta tytul={t(godzinowy ? 'historia.wykres.dzien' : 'historia.wykres.dni')}>
      {#if widok}
        <WykresKrokow wykres={widok.chart} {teraz} />
      {:else}
        <div class="miejsce-wykresu"></div>
      {/if}
    </Karta>

    {#if widok?.groups.length}
      <Karta tytul={t(widok.groupKey === 'person' ? 'historia.podzial.osoby' : 'historia.podzial.profile')}>
        <ul class="grupy">
          {#each widok.groups as g (g.name)}
            {@const [kroki, reszta] = tl('historia.podzial.wiersz', {
              kroki: g.totals.steps,
              km: liczba(g.totals.distanceKm, 1),
              czas: czas(g.totals.activeS),
              procent: Math.round(g.share * 100),
            })}
            <li class="grupa">
              <IkonaProfilu ikona={g.icon} osoba={osobaGrupy(g)} rozmiar={36} />
              <div class="grupa-tekst">
                <div class="grupa-gora">
                  <span class="grupa-nazwa jedna-linia dane" title={g.name}>{g.name}</span>
                  <span class="grupa-kroki liczby">{kroki}</span>
                </div>
                <span class="udzial" aria-hidden="true">
                  <span class="udzial-pasek" style:background={g.color} style:transform="scaleX({Math.min(1, Math.max(0, g.share))})"></span>
                </span>
                <span class="grupa-opis">{reszta}</span>
              </div>
            </li>
          {/each}
        </ul>
      </Karta>
    {/if}

    <!-- „Jeszcze nie ma sesji” tylko dla zakresu Wszystko - w krótszym zakresie starsze sesje mogą
         być (np. Dziś bez sesji po wczorajszym spacerze), a pusty okres opisuje już wykres -->
    {#if widok && (widok.recent.length || widok.query.range === 'all')}
      <Karta tytul={t('historia.sesje.tytul')}>
        {#if widok.recent.length}
          <ul class="sesje">
            {#each widok.recent as s (s.id)}
              <WierszSesji
                sesja={widok.online ? s : { ...s, online: null }}
                {wszyscy}
                {teraz}
                onotworz={() => idz('session', { sessionId: s.id })}
                onoko={(ukryta) => ukryjOnline(s, ukryta)}
              />
            {/each}
          </ul>
        {:else}
          <p class="wyciszony opis-karty">{t('historia.sesje.pusto')}</p>
        {/if}
      </Karta>
    {/if}
  {/if}
</Strona>

<style>
  .info { color: var(--text-2); font-size: var(--fs-md); }
  .wybor { display: flex; flex-direction: column; gap: var(--sp-2); }
  .miejsce-wykresu { height: 190px; }
  .opis-karty { font-size: var(--fs-md); }

  .grupy { display: flex; flex-direction: column; gap: var(--sp-3); }
  .grupa {
    display: grid;
    grid-template-columns: 36px minmax(0, 1fr);
    gap: var(--sp-3);
    align-items: center;
  }
  .grupa-tekst { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
  .grupa-gora { display: flex; align-items: baseline; gap: var(--sp-2); min-width: 0; }
  .grupa-nazwa { flex: 1; font-weight: var(--fw-semibold); line-height: var(--lh-tight); }
  .grupa-kroki {
    flex: none;
    font-size: var(--fs-md);
    color: var(--text);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .udzial {
    display: block;
    height: 6px;
    overflow: hidden;
    border-radius: var(--r-xs);
    background: var(--surface-2);
  }
  .udzial-pasek {
    display: block;
    height: 100%;
    border-radius: var(--r-xs);
    transform-origin: left;
    transition: transform var(--t-gauge) var(--ease-enter);
  }
  .grupa-opis { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-body); }

  .sesje { display: flex; flex-direction: column; }
</style>
