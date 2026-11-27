<script lang="ts">
  // Zdobyte osiągnięcie na profilu osoby (wersja zwarta wiersza z docs/design.md 9.5): odznaka 44 |
  // nazwa (zawija się), pod nią rzadkość wersalikami, warunek, „zdobyte wczoraj · +250 XP”
  // (cudzy profil bez czasu zdobycia - samo „+250 XP”).
  // Karta neutralna - kolor niesie tylko odznaka i napis rzadkości.
  import OdznakaRzadkosci from '../../lib/ui/rarity-badge.svelte';
  import type { Achievement } from '../../lib/contract';
  import { stan } from '../../lib/state.svelte';
  import { t } from '../../lib/texts';
  import { kiedy } from './helpers';

  let { osiagniecie: a, zdobyte }: { osiagniecie: Achievement; zdobyte: number | null } = $props();
  const stopka = $derived(
    [
      zdobyte === null ? '' : t('poziomy.osiagniecie.zdobyte', { kiedy: kiedy(zdobyte) }),
      t('poziomy.osiagniecie.xp', { xp: stan.statics.definitions.rarityXp[a.rarity] }),
    ]
      .filter(Boolean)
      .join(' · '),
  );
  const nazwaRzadkosci = $derived(t(`rzadkosc.${a.rarity}`));
</script>

<li class="wiersz">
  <OdznakaRzadkosci rzadkosc={a.rarity} znak={a.mark} rozmiar={44} etykieta={nazwaRzadkosci} />
  <span class="tekst">
    <span class="nazwa dane">{a.name}</span>
    <span class="hud rzadkosc" style:color="var(--rarity-{a.rarity}-text)">{nazwaRzadkosci}</span>
    <span class="warunek">{a.condition}</span>
    <span class="stopka liczba">{stopka}</span>
  </span>
</li>

<style>
  .wiersz {
    display: grid;
    grid-template-columns: 44px minmax(0, 1fr);
    gap: var(--sp-3);
    align-items: start;
    padding-block: 10px;
    border-top: 1px solid var(--line);
    content-visibility: auto;
    contain-intrinsic-size: auto 96px;
  }
  .wiersz:first-child { border-top: 0; padding-top: 2px; }
  .tekst { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .nazwa { font-size: var(--fs-base); font-weight: var(--fw-bold); line-height: var(--lh-tight); }
  .rzadkosc { padding-block-start: 0.12em; }
  .warunek { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-body); }
  .stopka { font-size: var(--fs-xs); font-weight: var(--fw-regular); color: var(--text-2); }
</style>
