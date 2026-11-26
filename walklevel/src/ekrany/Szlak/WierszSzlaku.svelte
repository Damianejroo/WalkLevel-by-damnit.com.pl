<script lang="ts">
  // Wiersz szlaku (docs/wyglad.md 8.10): km | szyna z węzłem | panel. Szyna przebyta w --text,
  // przyszła w --control; węzeł przebyty - heksagon w kolorze rzadkości, przyszły - pusty,
  // „tu jesteś” - kropka w akcencie z pierścieniem (jedyny akcent ekranu, bez poświaty).
  import type { Snippet } from 'svelte';
  import type { Rarity } from '../../lib/typy';

  interface Props {
    /** Napis kolumny km („1 257 km”). */
    km: string;
    wezel: 'przebyty' | 'przyszly' | 'tu';
    /** Kolor węzła przebytego. */
    rzadkosc?: Rarity;
    /** Odcinek szyny nad węzłem: brak (pierwszy wiersz), przebyty albo przyszły. */
    gora: 'brak' | 'przebyta' | 'przyszla';
    dol: 'brak' | 'przebyta' | 'przyszla';
    children: Snippet;
  }
  let { km, wezel, rzadkosc, gora, dol, children }: Props = $props();
</script>

<li class="wiersz" class:tu={wezel === 'tu'}>
  <span class="km" class:przed={wezel === 'przyszly'}>{km}</span>
  <span class="szyna" aria-hidden="true">
    {#if gora !== 'brak'}<span class="odcinek gora {gora}"></span>{/if}
    {#if dol !== 'brak'}<span class="odcinek dol {dol}"></span>{/if}
    {#if wezel === 'tu'}
      <span class="kropka"></span>
    {:else}
      <svg class="heks {wezel}" viewBox="-1 -1 2 2" width="14" height="14" style:--r="var(--rarity-{rzadkosc ?? 'pospolite'})">
        <path d="M0 -0.86L0.745 -0.43L0.745 0.43L0 0.86L-0.745 0.43L-0.745 -0.43Z" />
      </svg>
    {/if}
  </span>
  <div class="panel">{@render children()}</div>
</li>

<style>
  .wiersz {
    display: grid;
    grid-template-columns: 64px 26px minmax(0, 1fr);
    gap: 10px;
    align-items: center;
    min-height: 52px;
  }
  .km {
    font-family: var(--font-num);
    font-size: var(--fs-md);
    font-weight: var(--fw-bold);
    font-variant-numeric: tabular-nums;
    text-align: end;
    white-space: nowrap;
    color: var(--text);
  }
  .km.przed { color: var(--text-2); }

  /* Szyna: pełna wysokość wiersza + połowa odstępu między wierszami (4 px) z każdej strony */
  .szyna { position: relative; align-self: stretch; display: grid; place-items: center; }
  .odcinek {
    position: absolute;
    left: 50%;
    width: 3px;
    margin-left: -1.5px;
  }
  .gora { top: -2px; bottom: 50%; }
  .dol { top: 50%; bottom: -2px; }
  .przebyta { background: var(--text); }
  .przyszla { background: var(--control); }
  /* przy „tu jesteś” linia kończy się na pierścieniu */
  .tu .gora { bottom: calc(50% + 10px); }
  .tu .dol { top: calc(50% + 10px); }

  .heks { position: relative; overflow: visible; }
  .heks path { stroke-width: 2px; vector-effect: non-scaling-stroke; stroke-linejoin: round; }
  .heks.przebyty path { fill: var(--r); stroke: var(--r); }
  .heks.przyszly path { fill: var(--surface-2); stroke: var(--control); }

  .kropka {
    position: relative;
    width: 10px;
    height: 10px;
    border-radius: var(--r-round);
    background: var(--accent);
    outline: 2px solid var(--accent);
    outline-offset: 3px;
  }

  .panel {
    min-width: 0;
    padding: 6px 12px;
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: var(--surface);
  }
  .tu .panel { background: var(--surface-2); }
</style>
