<script lang="ts">
  // Wiersz znajomego w panelu (docs/design.md 8.16): odznaka poziomu 34 | pseudonim, czynność,
  // pasek XP 8 px (--ink) z licznikiem. Chodzący: „World of Warcraft · 3,4 km/h”, „Trening · …”,
  // „W grze · …” (gra ukryta), licznik „+152 XP” z sesji; pozostali: kroki z tygodnia albo opis
  // kursywą i % do następnego poziomu. Poziom ukryty - inicjał pseudonimu w kółku, bez paska.
  // Wąski pasek: sama odznaka (kropka „idzie teraz” w rogu), reszta w podpowiedzi. Klik - profil,
  // prawy przycisk - menu (obsługuje panel). Wartości zmieniają się w miejscu (lista z kluczem).
  import OdznakaRzadkosci, { rzadkoscPoziomu } from '../../lib/ui/rarity-badge.svelte';
  import Ikona from '../../lib/ui/icon.svelte';
  import InitialAvatar from '../../lib/ui/initial-avatar.svelte';
  import PasekXP from '../../lib/ui/xp-bar.svelte';
  import { stan } from '../../lib/state.svelte';
  import { t } from '../../lib/texts';
  import { liczba } from '../../lib/format';
  import { xpTeraz } from '../../lib/friends.svelte';
  import { granicePoziomu, rangaPoziomu } from '../online/helpers';
  import type { Friend } from '../../lib/contract';

  interface Props {
    f: Friend;
    waski: boolean;
    /** Menu kontekstowe otwarte dla tego wiersza. */
    rozwiniety?: boolean;
    onotworz: (f: Friend) => void;
    onmenu: (f: Friend, x: number, y: number) => void;
  }
  let { f, waski, rozwiniety = false, onotworz, onmenu }: Props = $props();

  const defs = $derived(stan.statics.definitions);
  const zywo = $derived(xpTeraz(f, defs.xpPerKm));
  // poziom z XP na żywo (awans w trakcie sesji widać od razu); bez XP - poziom z serwera
  const poziom = $derived.by(() => {
    if (zywo.xp === null) return f.level;
    let n = 1;
    while (n < defs.levelsXp.length && defs.levelsXp[n] <= zywo.xp) n++;
    return Math.max(n, f.level ?? 1);
  });
  const ranga = $derived(poziom === null ? null : rangaPoziomu(defs, poziom));
  const postep = $derived.by(() => {
    if (poziom === null || zywo.xp === null) return null;
    const g = granicePoziomu(defs, poziom);
    return g.do > g.od ? Math.min(1, Math.max(0, (zywo.xp - g.od) / (g.do - g.od))) : 1;
  });
  const kmh = $derived(f.live ? liczba(f.live.kmh, 1) : '');
  // „World of Warcraft · 3,4 km/h”: nazwa gry pogrubiona (wzór z texts.md, podział po znaczniku)
  const [przedGra, poGrze] = t('panel.gra', { gra: '{GRA}', kmh: '{KMH}' }).split('{GRA}');
  const czynnosc = $derived.by(() => {
    if (!f.live) return '';
    if (f.live.mode === 'trening') return t('panel.trening', { kmh });
    if (f.live.mode === 'gra') return f.live.game ? t('panel.gra', { gra: f.live.game, kmh }) : t('panel.w_grze', { kmh });
    return t('panel.chodzi', { kmh });
  });
  // „dziś 3 200 kroków · tydzień 31 240”, gdy serwer podał dzisiejsze kroki (> 0)
  const tydzien = $derived(
    f.today && f.today.steps > 0
      ? t('panel.dzis_tydzien', { dzis: f.today.steps, tydzien: f.week.steps })
      : t('panel.tydzien', { kroki: f.week.steps }),
  );
  const licznik = $derived(
    f.live && zywo.zSesji !== null
      ? t('panel.xp_sesja', { xp: liczba(zywo.zSesji) })
      : postep === null
        ? ''
        : t('panel.xp_procent', { procent: Math.floor(postep * 100) }),
  );
  const podpowiedz = $derived(
    [
      poziom === null ? `${f.name} · ${t('online.poziom_ukryty')}` : t('panel.osoba', { pseudonim: f.name, poziom, ranga: ranga?.name ?? '' }),
      czynnosc,
    ]
      .filter(Boolean)
      .join(' · '),
  );

  // collapsed strip: walking - "Ola · idzie teraz · World of Warcraft · 3,4 km/h" (as far as the
  // server tells), otherwise "Ola · w tym tygodniu 31 240 kroków"
  const stripTitle = $derived.by(() => {
    if (!f.live) return t('panel.pasek_tydzien', { pseudonim: f.name, kroki: f.week.steps });
    const activity = f.live.mode === null ? t('pole.kmh', { kmh }) : czynnosc;
    return `${t('panel.pasek_idzie', { pseudonim: f.name })} · ${activity}`;
  });

  function menu(e: MouseEvent) {
    e.preventDefault();
    onmenu(f, e.clientX, e.clientY);
  }
</script>

<button
  type="button"
  class="wiersz"
  class:waski
  data-id={f.id}
  data-idzie={f.live ? '1' : '0'}
  aria-expanded={rozwiniety ? 'true' : undefined}
  aria-label={waski ? stripTitle : undefined}
  title={waski ? stripTitle : podpowiedz}
  onclick={() => onotworz(f)}
  oncontextmenu={menu}
>
  <span class="odznaka">
    {#if poziom === null}
      <InitialAvatar name={f.name} label={waski ? undefined : t('online.poziom_ukryty')} />
    {:else}
      <OdznakaRzadkosci
        rzadkosc={rzadkoscPoziomu(poziom)}
        liczba={poziom}
        rozmiar={34}
        etykieta={t('poziomy.ranga_dostepnosc', { poziom, ranga: ranga?.name ?? '' })}
      />
    {/if}
    {#if waski && f.live}<span class="status-dot" aria-hidden="true"></span>{/if}
  </span>
  {#if !waski}
    <span class="tresc">
      <span class="nick jedna-linia dane">{f.name}</span>
      <span class="linia">
        {#if f.live}
          <Ikona nazwa={f.live.mode === 'trening' ? 'trening' : 'chod'} rozmiar={12} />
          {#if f.live.mode === 'gra' && f.live.game}
            {#if przedGra}<span class="stale">{przedGra}</span>{/if}
            <b class="jedna-linia dane gra" title={f.live.game}>{f.live.game}</b>
            <span class="stale">{poGrze.replace('{KMH}', kmh)}</span>
          {:else}
            <span class="jedna-linia dane">{czynnosc}</span>
          {/if}
        {:else if f.description}
          <span class="jedna-linia dane opis">{f.description}</span>
        {:else}
          <span class="jedna-linia" title={tydzien}>{tydzien}</span>
        {/if}
      </span>
      {#if postep !== null && zywo.xp !== null}
        <span class="xp">
          <PasekXP
            wartosc={postep}
            wysokosc={8}
            odcinki={0}
            opis={t('panel.xp_dostepnosc', { xp: liczba(zywo.xp), procent: Math.floor(postep * 100), nastepny: (poziom ?? 1) + 1 })}
          />
          <span class="licznik liczba" class:idzie={!!f.live}>{licznik}</span>
        </span>
      {/if}
    </span>
  {/if}
</button>

<style>
  .wiersz {
    display: grid;
    grid-template-columns: 34px minmax(0, 1fr);
    gap: 10px;
    align-items: center;
    width: 100%;
    min-height: 56px;
    padding: 7px 8px;
    border-radius: 8px;
    text-align: start;
    color: var(--text);
    transition: background-color var(--t-color) linear;
  }
  .wiersz:hover,
  .wiersz[aria-expanded='true'] { background: var(--surface-2); }
  .waski {
    grid-template-columns: 34px;
    justify-content: center;
    min-height: 50px;
    padding: 8px 0;
    border-radius: 0;
  }
  .odznaka { position: relative; display: grid; place-items: center; width: 34px; height: 34px; }
  /* "walking now" in the collapsed strip: status dot in the walk colour with a ring in the panel
     colour, in the corner of the badge */
  .status-dot {
    position: absolute;
    right: -3px;
    bottom: -3px;
    width: 14px;
    height: 14px;
    border: 2px solid var(--surface);
    border-radius: var(--r-round);
    background: var(--zone-walk);
  }
  .tresc { display: grid; gap: 2px; min-width: 0; }
  .nick { font-size: 0.9375rem; font-weight: var(--fw-semibold); line-height: var(--lh-tight); }
  .linia {
    display: flex;
    align-items: center;
    gap: 5px;
    min-width: 0;
    font-size: var(--fs-sm);
    color: var(--text-2);
    line-height: var(--lh-tight);
  }
  .linia :global(.ikona) { color: var(--text); }
  .opis { font-style: italic; }
  /* nazwa gry skraca się wielokropkiem, tempo zostaje zawsze widoczne */
  .gra { flex: 0 1 auto; color: var(--text); font-weight: var(--fw-semibold); }
  .stale { flex: none; white-space: pre; }
  .xp { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 8px; align-items: center; margin-top: 3px; }
  .licznik {
    min-width: 52px;
    font-size: var(--fs-xs);
    font-weight: var(--fw-bold);
    color: var(--text-2);
    text-align: end;
  }
  .licznik.idzie { color: var(--text); }
</style>
