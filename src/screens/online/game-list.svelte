<script lang="ts">
  // Podział wyników na gry (docs/design.md 9.4): nazwa gry z wielokropkiem | kroki i „km · czas”
  // w dwóch liniach do prawej. Od największej liczby kroków; na końcu „Inne gry (N)” (profil osoby:
  // gry poza listą, bez nazw).
  import type { GameTotals, OtherGames } from '../../lib/contract';
  import Statystyki from './stats.svelte';
  import { czas, t } from '../../lib/texts';
  import { liczba } from '../../lib/format';
  import { km1 } from './helpers';

  let { gry, inne = null }: { gry: GameTotals[]; inne?: OtherGames | null } = $props();
  const posortowane = $derived([...gry].sort((a, b) => b.steps - a.steps));
</script>

<ul class="gry">
  {#each posortowane as g (g.gameId ?? g.game)}
    {@const nazwa = g.game.trim() || t('profil_online.bez_gry')}
    <li class="gra">
      <span class="nazwa jedna-linia" class:bez-nazwy={!g.game.trim()} title={nazwa}>{nazwa}</span>
      <Statystyki
        liczba={liczba(g.steps)}
        jednostka={t('ranking.wiersz.kroki_slowo', { kroki: g.steps })}
        dol={`${t('ranking.wiersz.km', { km: km1(g.distanceKm) })} · ${czas(g.activeS)}`}
      />
    </li>
  {/each}
  {#if inne}
    {@const nazwa = t('profil_online.inne_gry', { liczba: inne.games })}
    <li class="gra">
      <span class="nazwa jedna-linia bez-nazwy" title={nazwa}>{nazwa}</span>
      <Statystyki
        liczba={liczba(inne.steps)}
        jednostka={t('ranking.wiersz.kroki_slowo', { kroki: inne.steps })}
        dol={`${t('ranking.wiersz.km', { km: km1(inne.distanceKm) })} · ${czas(inne.activeS)}`}
      />
    </li>
  {/if}
</ul>

<style>
  .gry { display: flex; flex-direction: column; }
  .gra {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: var(--sp-3);
    align-items: center;
    min-height: 48px;
    padding-block: 6px;
    border-top: 1px solid var(--line);
  }
  .nazwa { font-size: var(--fs-base); }
  .bez-nazwy { color: var(--text-2); }
</style>
