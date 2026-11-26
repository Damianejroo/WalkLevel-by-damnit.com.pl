<script lang="ts">
  // Karta poziomu (docs/wyglad.md 8.14, docs/teksty.md 5): cała karta prowadzi do Osiągnięć.
  // Odznaka 62 z numerem | POZIOM + XP, ranga, pasek XP (--ink, bez akcentu), ile km do
  // następnego poziomu | puchar z liczbą osiągnięć. Bez gradientu i koloru tła.
  import Karta from '../../lib/ui/Karta.svelte';
  import OdznakaRzadkosci, { rzadkoscPoziomu } from '../../lib/ui/OdznakaRzadkosci.svelte';
  import PasekXP from '../../lib/ui/PasekXP.svelte';
  import type { LevelsView } from '../../lib/typy';
  import { liczba } from '../../lib/format';
  import { stan, idz } from '../../lib/stan.svelte';
  import { t } from '../../lib/teksty';

  let { poziomy }: { poziomy: LevelsView } = $props();

  const najwyzszy = $derived(poziomy.level >= poziomy.maxLevel);
  const xp = $derived(
    najwyzszy
      ? t('glowny.poziom.xp_max', { xp: poziomy.xp })
      : t('glowny.poziom.xp', { xp: poziomy.xp, xp_max: poziomy.levelCeilingXp }),
  );
  const podpowiedz = $derived(
    stan.settings.simulator
      ? t('glowny.poziom.symulator')
      : najwyzszy
        ? t('glowny.poziom.najwyzszy')
        : t('glowny.poziom.do_nastepnego', { km: liczba(poziomy.kmToNextLevel, 1), poziom: poziomy.level + 1 }),
  );
  const ranga = $derived(t('glowny.poziom.podpowiedz', { ranga: poziomy.rank.name, opis_rangi: poziomy.rank.about }));
</script>

<Karta etykieta={t('glowny.poziom.dostepnosc')} onclick={() => idz('achievements')}>
  <span class="poziom" title={ranga}>
    <OdznakaRzadkosci rzadkosc={rzadkoscPoziomu(poziomy.level)} liczba={poziomy.level} rozmiar={62} />
    <span class="srodek">
      <span class="gora">
        <span class="hud wyciszony">{t('glowny.poziom.naglowek', { poziom: poziomy.level })}</span>
        <span class="xp liczba wyciszony">{xp}</span>
      </span>
      <span class="ranga jedna-linia">{poziomy.rank.name}</span>
      <PasekXP wartosc={najwyzszy ? 1 : poziomy.levelFraction} opis={xp} />
      <span class="podpowiedz jedna-linia">{podpowiedz}</span>
    </span>
    <span class="puchar">
      <OdznakaRzadkosci rzadkosc="legendarne" znak="korona" rozmiar={40} />
      <span class="licznik liczba">{t('glowny.poziom.licznik', { liczba: poziomy.earnedCount, razem: poziomy.total })}</span>
    </span>
  </span>
</Karta>

<style>
  .poziom {
    display: grid;
    grid-template-columns: 62px minmax(0, 1fr) auto;
    align-items: center;
    gap: 12px;
    min-width: 0;
  }
  .srodek { display: flex; flex-direction: column; gap: 3px; min-width: 0; }
  .gora {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    column-gap: var(--sp-2);
    min-width: 0;
  }
  .hud { font-size: var(--fs-xs); }
  .xp { font-size: var(--fs-xs); font-weight: var(--fw-regular); line-height: var(--lh-caps); white-space: nowrap; }
  .ranga { font-size: var(--fs-lg); font-weight: var(--fw-bold); line-height: var(--lh-tight); }
  .podpowiedz { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-tight); }
  .puchar { display: flex; flex-direction: column; align-items: center; gap: 2px; }
  .licznik { font-size: var(--fs-sm); line-height: 1.1; }
</style>
