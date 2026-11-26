<script lang="ts">
  // Podział wyników na gry (docs/wyglad.md 9.4): nazwa gry z wielokropkiem | kroki i „km · czas”
  // w dwóch liniach do prawej. Od największej liczby kroków.
  import type { GameTotals } from '../../lib/typy';
  import Statystyki from './Statystyki.svelte';
  import { czas, t } from '../../lib/teksty';
  import { liczba } from '../../lib/format';
  import { km1 } from './pomocnicze';

  let { gry }: { gry: GameTotals[] } = $props();
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
