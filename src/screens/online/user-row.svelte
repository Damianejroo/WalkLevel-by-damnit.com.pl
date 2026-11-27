<script lang="ts">
  // Wiersz osoby z kontem (docs/design.md 8.11): [miejsce] | odznaka poziomu 34 | pseudonim jako
  // link (podkreślenie przy najechaniu, bez tła) + ranga wersalikami + dodatkowe linie | koniec
  // (statystyki albo przycisk). Cały wiersz nie jest przyciskiem - klik tylko w pseudonim.
  // Bez poziomu (serwer nie podaje go bez zgody tej osoby) - inicjał pseudonimu w kółku
  // z podpowiedzią „Poziom ukryty” zamiast pustej odznaki.
  import type { Snippet } from 'svelte';
  import OdznakaRzadkosci, { rzadkoscPoziomu } from '../../lib/ui/rarity-badge.svelte';
  import InitialAvatar from '../../lib/ui/initial-avatar.svelte';
  import { stan } from '../../lib/state.svelte';
  import { t } from '../../lib/texts';
  import { rangaPoziomu } from './helpers';

  interface Props {
    nazwa: string;
    poziom: number | null;
    /** Miejsce w rankingu (dodaje kolumnę z numerem). */
    miejsce?: number;
    /** Własny wiersz: tło --surface-2 i dopisek „(Ty)”. */
    ja?: boolean;
    /** Bez `onotworz` pseudonim jest zwykłym tekstem. */
    onotworz?: () => void;
    /** Linie pod pseudonimem i rangą (opis, „idzie teraz”). */
    opis?: Snippet;
    koniec?: Snippet;
  }
  let { nazwa, poziom: zSerwera, miejsce, ja = false, onotworz, opis, koniec }: Props = $props();

  // serwer nie zawsze odsyła poziom (np. we własnym wierszu rankingu kroków) - własny bierzemy
  // z aplikacji, jak w 0.5.x; bez tego zostawała pusta odznaka
  const poziom = $derived(zSerwera ?? (ja ? (stan.levels?.level ?? null) : null));
  const ranga = $derived(poziom === null ? null : rangaPoziomu(stan.statics.definitions, poziom));
  const rzadkosc = $derived(rzadkoscPoziomu(poziom ?? 1));
</script>

<li class="wiersz" class:ja class:z-miejscem={miejsce !== undefined}>
  {#if miejsce !== undefined}
    <span class="miejsce liczba">{t('ranking.miejsce', { liczba: miejsce })}</span>
  {/if}
  {#if poziom === null}
    <InitialAvatar name={nazwa} label={t('online.poziom_ukryty')} title={t('online.poziom_ukryty')} />
  {:else}
    <OdznakaRzadkosci
      {rzadkosc}
      liczba={poziom}
      rozmiar={34}
      etykieta={t('poziomy.ranga_dostepnosc', { poziom, ranga: ranga?.name ?? '' })}
    />
  {/if}
  <span class="osoba">
    <span class="nazwa">
      {#if onotworz}
        <!-- title z pełnym pseudonimem: przy wielokropku to jedyne miejsce z całą nazwą -->
        <button type="button" class="link jedna-linia" title={nazwa} onclick={onotworz}>{nazwa}</button>
      {:else}
        <span class="tekst-nazwy jedna-linia" title={nazwa}>{nazwa}</span>
      {/if}
      {#if ja}<span class="ty">{t('ranking.ty')}</span>{/if}
    </span>
    {#if ranga}
      <span class="ranga hud jedna-linia" style:color="var(--rarity-{rzadkosc}-text)" title={ranga.name}>{ranga.name}</span>
    {/if}
    {@render opis?.()}
  </span>
  {#if koniec}<span class="koniec">{@render koniec()}</span>{/if}
</li>

<style>
  .wiersz {
    display: grid;
    grid-template-columns: 34px minmax(0, 1fr) auto;
    gap: 10px;
    align-items: center;
    min-height: 52px;
    padding: 6px;
    border-top: 1px solid var(--line);
    border-radius: 0;
  }
  .wiersz:first-child,
  .ja,
  .ja + :global(li) { border-top-color: transparent; }
  .z-miejscem { grid-template-columns: 32px 34px minmax(0, 1fr) auto; }
  .ja { background: var(--surface-2); border-radius: 8px; }

  .miejsce { font-size: var(--fs-base); text-align: end; }
  .ja :global(.initial-avatar) { background: var(--surface-3); }
  .osoba { display: flex; flex-direction: column; min-width: 0; }
  .nazwa { display: flex; align-items: baseline; gap: 6px; min-width: 0; }
  .link,
  .tekst-nazwy {
    max-width: 100%;
    font-size: var(--fs-base);
    font-weight: var(--fw-semibold);
    line-height: var(--lh-tight);
    color: var(--text);
  }
  .link { border-radius: 2px; text-underline-offset: 3px; }
  .link:hover,
  .link:focus-visible { text-decoration: underline; }
  .ty { flex: none; font-size: var(--fs-sm); color: var(--text-2); }
  .ranga { padding-block-start: 0.12em; }
  .koniec { display: flex; justify-content: flex-end; min-width: 0; max-width: 11rem; }
</style>
