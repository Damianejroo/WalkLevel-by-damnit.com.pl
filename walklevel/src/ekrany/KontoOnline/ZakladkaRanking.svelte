<script lang="ts">
  // Konto online > Ranking (docs/teksty.md 22.7, wyglad.md 8.11): zakres (2 x 2 w wąskim oknie),
  // wiersze: miejsce, odznaka poziomu, pseudonim jako link + ranga, statystyki w dwóch liniach.
  // Bez akcentu - własny wiersz wyróżniony tłem. Przy zmianie zakresu stare wiersze zostają
  // przygaszone do czasu nowych (bez skoku wysokości).
  import Karta from '../../lib/ui/Karta.svelte';
  import Segmentowy from '../../lib/ui/Segmentowy.svelte';
  import WierszOsoby from './WierszOsoby.svelte';
  import Statystyki from './Statystyki.svelte';
  import * as api from '../../lib/api';
  import { stan, wykonaj } from '../../lib/stan.svelte';
  import { t, tytulIPodpis } from '../../lib/teksty';
  import { liczba } from '../../lib/format';
  import type { RankingScope } from '../../lib/typy';
  import { km1 } from './pomocnicze';

  let { onotworz }: { onotworz: (userId: string) => void } = $props();

  const ranking = $derived(stan.online.ranking);
  // wybrany zakres zmienia się od razu; dane przychodzą z rdzenia
  let wybrany = $state<RankingScope | null>(null);
  const zakres = $derived(wybrany ?? ranking?.scope ?? 'tydzien');
  const czeka = $derived(!!ranking && ranking.scope !== zakres);

  const naglowek = $derived(
    tytulIPodpis(
      ranking?.scope === 'poziom'
        ? t('ranking.tytul.poziomy')
        : ranking?.scope === 'razem'
          ? t('ranking.tytul.kroki_zawsze')
          : t('ranking.tytul.kroki'),
    ),
  );

  async function zmien(s: RankingScope) {
    wybrany = s;
    // błąd (toast z wykonaj) - wraca zakres z danych, żeby wiersze nie zostały przygaszone
    const ok = await wykonaj(api.onlineRankingScope(s).then(() => true));
    if (!ok && wybrany === s) wybrany = null;
  }
</script>

<div class="zakladka">
  <Segmentowy
    etykieta={t('ranking.zakres')}
    wartosc={zakres}
    onzmiana={zmien}
    opcje={[
      { wartosc: 'tydzien', napis: t('ranking.zakres.tydzien') },
      { wartosc: 'miesiac', napis: t('ranking.zakres.miesiac') },
      { wartosc: 'razem', napis: t('ranking.zakres.zawsze') },
      { wartosc: 'poziom', napis: t('ranking.zakres.poziomy') },
    ]}
  />

  <Karta tytul={naglowek.tytul} podpis={naglowek.podpis}>
    {#if ranking === null}
      <p class="wyciszony">{stan.online.loading.includes('ranking') ? t('online.laduje') : '–'}</p>
    {:else if ranking.rows.length === 0}
      <p class="wyciszony" class:czeka>{t('ranking.pusto')}</p>
    {:else}
      <ol class="lista" class:czeka aria-busy={czeka}>
        {#each ranking.rows as w (w.id || `${w.place}-${w.name}`)}
          {@const id = w.me ? stan.account.userId : w.id}
          <WierszOsoby nazwa={w.name} poziom={w.level} miejsce={w.place} ja={w.me} onotworz={id ? () => onotworz(id) : undefined}>
            {#snippet koniec()}
              {#if ranking.scope === 'poziom'}
                <Statystyki liczba={t('ranking.wiersz.km', { km: liczba(w.distanceKm) })} jednostka="" dol={t('ranking.wiersz.lacznie')} />
              {:else}
                <Statystyki
                  liczba={liczba(w.steps)}
                  jednostka={t('ranking.wiersz.kroki_slowo', { kroki: w.steps })}
                  dol={t('ranking.wiersz.km', { km: km1(w.distanceKm) })}
                />
              {/if}
            {/snippet}
          </WierszOsoby>
        {/each}
      </ol>
    {/if}
  </Karta>

  <p class="opis">{zakres === 'poziom' ? t('ranking.opis.poziomy') : t('ranking.opis.zasady')}</p>
</div>

<style>
  .zakladka { display: flex; flex-direction: column; gap: var(--sp-3); min-width: 0; }
  .lista { display: flex; flex-direction: column; margin-inline: -6px; }
  .czeka { opacity: 0.5; transition: opacity var(--t-color) linear; }
  .opis { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-body); padding-inline: 2px; }
</style>
