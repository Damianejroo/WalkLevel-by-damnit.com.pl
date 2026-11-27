<script lang="ts">
  // Karta poziomu (docs/design.md 8.14, 9.4): odznaka z liczbą poziomu | „POZIOM 12” + XP z prawej,
  // ranga, pasek XP (--ink, neutralnie), podpowiedź. Bez gradientu i kolorowego tła.
  // Z `onclick` cała karta jest przyciskiem (Znajomi: mój profil oczami innych).
  import Karta from '../../lib/ui/card.svelte';
  import OdznakaRzadkosci, { rzadkoscPoziomu } from '../../lib/ui/rarity-badge.svelte';
  import PasekXP from '../../lib/ui/xp-bar.svelte';
  import { stan } from '../../lib/state.svelte';
  import { t } from '../../lib/texts';
  import { granicePoziomu, rangaPoziomu } from './helpers';

  interface Props {
    poziom: number;
    /** Łączne XP (null - serwer nie podał: bez paska). */
    xp: number | null;
    podpowiedz?: string;
    rozmiar?: 62 | 70;
    etykieta?: string;
    onclick?: () => void;
  }
  let { poziom, xp: xpLaczne, podpowiedz = '', rozmiar = 62, etykieta, onclick }: Props = $props();

  const defs = $derived(stan.statics.definitions);
  const ranga = $derived(rangaPoziomu(defs, poziom));
  const granice = $derived(granicePoziomu(defs, poziom));
  // XP niezgodne z poziomem (np. stara odpowiedź serwera) - bez paska, żeby nie pokazywać bzdur
  const xp = $derived(xpLaczne !== null && xpLaczne >= granice.od ? xpLaczne : null);
  const najwyzszy = $derived(granice.do <= granice.od);
  const ulamek = $derived(xp === null || najwyzszy ? 1 : (xp - granice.od) / (granice.do - granice.od));
  const tekstXp = $derived(
    xp === null ? '' : najwyzszy ? t('glowny.poziom.xp_max', { xp }) : t('poziomy.xp', { xp, xp_max: granice.do }),
  );
</script>

<Karta {onclick} {etykieta}>
  <span class="poziom">
    <OdznakaRzadkosci
      rzadkosc={rzadkoscPoziomu(poziom)}
      liczba={poziom}
      {rozmiar}
      etykieta={t('poziomy.ranga_dostepnosc', { poziom, ranga: ranga?.name ?? '' })}
    />
    <span class="kolumna">
      <span class="gora">
        <span class="hud wyciszony">{t('poziomy.naglowek', { poziom })}</span>
        {#if tekstXp}<span class="xp liczba wyciszony">{tekstXp}</span>{/if}
      </span>
      {#if ranga}<span class="ranga dane">{ranga.name}</span>{/if}
      {#if xp !== null}<PasekXP wartosc={ulamek} opis={tekstXp} />{/if}
      {#if podpowiedz}<span class="podpowiedz">{podpowiedz}</span>{/if}
    </span>
  </span>
</Karta>

<style>
  .poziom { display: flex; align-items: center; gap: 14px; min-width: 0; }
  .kolumna { display: flex; flex-direction: column; gap: 4px; flex: 1; min-width: 0; }
  .gora { display: flex; flex-wrap: wrap; justify-content: space-between; column-gap: var(--sp-3); }
  .xp { font-size: var(--fs-xs); line-height: var(--lh-caps); }
  .ranga { font-size: var(--fs-lg); font-weight: var(--fw-bold); line-height: var(--lh-tight); }
  .podpowiedz { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-body); }
</style>
