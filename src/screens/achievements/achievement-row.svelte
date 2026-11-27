<script lang="ts">
  // Wiersz osiągnięcia (docs/design.md 9.5): karta neutralna, odznaka 58 | nazwa (zawija się),
  // pod nią napis rzadkości, opis kursywą, warunek, pasek postępu z podpisem, stopka z XP.
  // Kolor niesie tylko odznaka, jej napis i pasek postępu (kolory danych, design.md 3).
  import OdznakaRzadkosci from '../../lib/ui/rarity-badge.svelte';
  import PasekXP from '../../lib/ui/xp-bar.svelte';
  import { t } from '../../lib/texts';
  import { liczba } from '../../lib/format';
  import { kiedy, podpisPostepu, type Pozycja } from './achievement-progress';

  interface Props {
    pozycja: Pozycja;
    xp: number;
    /** Bez opisu (niezdobyte w „Pozostałych” - krótsza lista). */
    zwiezly?: boolean;
    /** Dziś (do „zdobyte wczoraj”), odświeżane przy wejściu na ekran. */
    teraz: Date;
    /** Ile procent graczy ma to osiągnięcie (konto online; brak - bez dopisku). */
    procent?: number;
  }
  let { pozycja, xp, zwiezly = false, teraz, procent }: Props = $props();

  const a = $derived(pozycja.a);
  const p = $derived(pozycja.p);
  const rzadkosc = $derived(t(`rzadkosc.${a.rarity}`));
  const postep = $derived(pozycja.zdobyte ? '' : podpisPostepu(a, p));
  const stopka = $derived(
    [
      pozycja.zdobyte && p.earnedAt ? t('poziomy.osiagniecie.zdobyte', { kiedy: kiedy(p.earnedAt, teraz) }) : '',
      t('poziomy.osiagniecie.xp', { xp: liczba(xp) }),
      procent === undefined ? '' : t('osiagniecia.procent_graczy', { procent: liczba(procent, procent < 10 ? 1 : 0) }),
    ]
      .filter(Boolean)
      .join(' · '),
  );
  const dostepnosc = $derived(
    t('osiagniecia.dostepnosc', {
      nazwa: a.name,
      rzadkosc,
      stan: pozycja.zdobyte ? t('osiagniecia.zdobyte') : t('poziomy.osiagniecie.do_zdobycia'),
      warunek: a.condition,
    }),
  );
</script>

<li class="osiagniecie" aria-label={dostepnosc}>
  <OdznakaRzadkosci
    rzadkosc={a.rarity}
    znak={a.mark}
    rozmiar={58}
    zablokowana={!pozycja.zdobyte}
    postep={pozycja.zdobyte ? 0 : p.fraction}
  />
  <div class="tekst">
    <h3 class="nazwa">{a.name}</h3>
    <p class="rzadkosc hud" style:color="var(--rarity-{a.rarity}-text)">{rzadkosc}</p>
    {#if !zwiezly}<p class="opis">{a.flavor}</p>{/if}
    <p class="warunek">{a.condition}</p>
    {#if postep}
      <div class="postep">
        <PasekXP wartosc={p.fraction} wysokosc={8} odcinki={0} kolor="var(--rarity-{a.rarity})" opis={postep} />
        <span class="podpis-postepu" aria-hidden="true">{postep}</span>
      </div>
    {/if}
    <p class="stopka">{stopka}</p>
  </div>
</li>

<style>
  .osiagniecie {
    display: grid;
    grid-template-columns: 58px minmax(0, 1fr);
    gap: var(--sp-3);
    align-items: start;
    padding: var(--card-pad);
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: var(--surface);
    contain: layout paint;
    /* długa lista: wiersze poza widokiem nie są układane ani malowane (design.md 6.3) */
    content-visibility: auto;
    contain-intrinsic-size: auto 132px;
  }
  .tekst { display: flex; flex-direction: column; min-width: 0; }
  .nazwa {
    font-size: var(--fs-base);
    font-weight: var(--fw-bold);
    line-height: var(--lh-tight);
    overflow-wrap: anywhere;
  }
  .rzadkosc { margin-block: 2px 4px; padding-block-start: 0.12em; }
  .opis {
    font-size: var(--fs-sm);
    font-style: italic;
    color: var(--text-2);
    line-height: var(--lh-body);
  }
  .warunek { font-size: var(--fs-sm); line-height: var(--lh-body); }
  .postep {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: var(--sp-2);
    align-items: center;
    margin-block-start: 6px;
  }
  .podpis-postepu {
    font-family: var(--font-num);
    font-size: var(--fs-xs);
    font-variant-numeric: tabular-nums;
    color: var(--text-2);
    white-space: nowrap;
  }
  .stopka {
    margin-block-start: 4px;
    font-family: var(--font-num);
    font-size: var(--fs-xs);
    font-variant-numeric: tabular-nums;
    color: var(--text-2);
    line-height: var(--lh-caps);
  }
</style>
