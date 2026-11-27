<script lang="ts">
  // Panel znajomych z boku okna (0.6.2, docs/design.md 8.16): montowany raz obok kolumny ekranów.
  // Pełny 288 px albo wąski pasek 64 px (same odznaki); zwinięcie zwęża okno (window.rs). Nagłówek
  // 60 px: „Znajomi” + kto chodzi; ikony: plus (szukaj/zaproś) i zwiń (zwiń zawsze od strony
  // głównej kolumny; w pasku pod nim licznik osób, które idą teraz). Zaproszenia do mnie,
  // „Chodzą teraz”, „Pozostali” (po krokach tygodnia), stopka ze statusem. Bez akcentu
  // (design.md 3). Dane: stan.online.friends z jednego źródła odświeżania (friends.svelte.ts).
  import { SvelteSet } from 'svelte/reactivity';
  import Ikona from '../lib/ui/icon.svelte';
  import Pole from '../lib/ui/text-field.svelte';
  import Przycisk from '../lib/ui/button.svelte';
  import OdznakaRzadkosci from '../lib/ui/rarity-badge.svelte';
  import InitialAvatar from '../lib/ui/initial-avatar.svelte';
  import WierszZnajomego from './friends-panel/friend-row.svelte';
  import StopkaPanelu from './friends-panel/panel-footer.svelte';
  import * as api from '../lib/api';
  import { idz, stan, wykonaj } from '../lib/state.svelte';
  import { t } from '../lib/texts';
  import { odswiezZnajomych, potrzebujeZnajomych, xpTeraz, znajomi } from '../lib/friends.svelte';
  import KartaBledu from '../lib/ui/error-card.svelte';
  import { panel as ukladPanelu, zmienPanel } from '../lib/friends-panel.svelte';
  import { bladPseudonimu, rangaPoziomu, sprobuj } from './online/helpers';
  import type { Friend } from '../lib/contract';

  let { waski }: { waski: boolean } = $props();

  const konto = $derived(stan.account);
  const online = $derived(stan.online);

  // lista potrzebna, póki panel jest w oknie i osoba ma konto (odświeżanie co 10 s / 60 s)
  $effect(() => {
    if (!konto.loggedIn) return;
    return potrzebujeZnajomych();
  });

  // --- usuwanie z „Cofnij”: wiersz znika od razu, serwer dostaje usunięcie dopiero po
  //     zniknięciu komunikatu (Cofnij = nic nie poszło)
  const ukryci = new SvelteSet<string>();
  const wszyscy = $derived((online.friends ?? []).filter((f) => !ukryci.has(f.id)));

  let szukaj = $state(false);
  let fraza = $state('');
  let bladZaproszenia = $state('');
  const pasuje = (f: Friend) => !fraza.trim() || f.name.toLowerCase().includes(fraza.trim().toLowerCase());
  const chodza = $derived(wszyscy.filter((f) => f.live && pasuje(f)));
  const pozostali = $derived(wszyscy.filter((f) => !f.live && pasuje(f)).sort((a, b) => b.week.steps - a.week.steps));
  const ileChodzi = $derived(wszyscy.filter((f) => f.live).length);
  const podpis = $derived(
    !konto.loggedIn
      ? t('panel.bez_konta')
      : online.friends === null
        ? ''
        : ileChodzi
        ? t('panel.chodza', { liczba: ileChodzi, razem: wszyscy.length })
        : t('panel.na_liscie', { liczba: wszyscy.length }),
  );
  const zaproszenia = $derived(fraza.trim() ? [] : (online.invitations?.incoming ?? []));
  const doZaproszenia = $derived.by(() => {
    const n = fraza.trim();
    if (n.length < 3) return '';
    return wszyscy.some((f) => f.name.toLowerCase() === n.toLowerCase()) ? '' : n;
  });

  function przelaczSzukaj() {
    szukaj = !szukaj;
    if (!szukaj) fraza = '';
    bladZaproszenia = '';
    if (szukaj) queueMicrotask(() => pole?.querySelector('input')?.focus());
  }
  let pole = $state<HTMLElement>();

  async function zapros() {
    const nazwa = doZaproszenia;
    if (!nazwa) return;
    const blad = bladPseudonimu(nazwa, stan.statics.account);
    if (blad) {
      bladZaproszenia = blad;
      return;
    }
    const w = await sprobuj(api.onlineInviteName(nazwa));
    bladZaproszenia = w.blad || Object.values(w.pola)[0] || '';
    if (w.ok) {
      fraza = '';
      szukaj = false;
    }
  }

  // --- komunikat w panelu (jak Toast: --ink), 4 s; „Cofnij” przy usuwaniu
  let toast = $state<{ tekst: string; cofnij?: () => void } | null>(null);
  let zegarToastu: ReturnType<typeof setTimeout> | undefined;
  let poToascie: (() => void) | undefined;
  function komunikatPanelu(tekst: string, cofnij?: () => void, potem?: () => void) {
    clearTimeout(zegarToastu);
    poToascie?.();
    poToascie = potem;
    toast = { tekst, cofnij };
    zegarToastu = setTimeout(() => {
      toast = null;
      const p = poToascie;
      poToascie = undefined;
      p?.();
    }, 4000);
  }
  function cofnij() {
    clearTimeout(zegarToastu);
    toast?.cofnij?.();
    toast = null;
    poToascie = undefined;
  }

  function usun(f: Friend) {
    ukryci.add(f.id);
    komunikatPanelu(
      t('panel.usunieto', { pseudonim: f.name }),
      () => ukryci.delete(f.id),
      () => {
        void wykonaj(api.onlineFriendRemove(f.id)).finally(() => ukryci.delete(f.id));
      },
    );
  }

  // --- awans znajomego: nowa liczba na odznace + komunikat w panelu
  const poziomy = new Map<string, number>();
  $effect(() => {
    const defs = stan.statics.definitions;
    for (const f of wszyscy) {
      const { xp } = xpTeraz(f, defs.xpPerKm);
      if (xp === null) continue;
      let n = 1;
      while (n < defs.levelsXp.length && defs.levelsXp[n] <= xp) n++;
      // highest level seen: live XP drops back after a session ends, until the server counts it
      const byl = poziomy.get(f.id);
      poziomy.set(f.id, Math.max(byl ?? n, n));
      if (byl !== undefined && n > byl) {
        komunikatPanelu(t('panel.osoba', { pseudonim: f.name, poziom: n, ranga: rangaPoziomu(defs, n)?.name ?? '' }));
      }
    }
  });

  // --- profil i menu prawego przycisku
  function otworz(f: Friend) {
    menu = null;
    if (konto.busy.includes('person')) return;
    void wykonaj(api.onlinePersonOpen(f.id));
  }
  let menu = $state<{ f: Friend; x: number; y: number } | null>(null);
  let statusOtwarty = $state(false);
  let panel = $state<HTMLElement>();
  function otworzMenu(f: Friend, x: number, y: number) {
    statusOtwarty = false;
    const r = panel?.getBoundingClientRect();
    if (!r) return;
    menu = { f, x: Math.max(8, Math.min(x - r.left, r.width - 208)), y: Math.max(8, Math.min(y - r.top, r.height - 120)) };
  }
  $effect(() => {
    if (!menu && !statusOtwarty) return;
    const obok = (e: PointerEvent) => {
      const cel = e.target as Element;
      if (!cel.closest('.menu-znajomego, .stopka')) {
        menu = null;
        statusOtwarty = false;
      }
    };
    const klawisz = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        menu = null;
        statusOtwarty = false;
      }
    };
    window.addEventListener('pointerdown', obok, true);
    window.addEventListener('keydown', klawisz);
    return () => {
      window.removeEventListener('pointerdown', obok, true);
      window.removeEventListener('keydown', klawisz);
    };
  });

  // zwinięcie / rozwinięcie z przejściem bez drgań (lib/friends-panel.svelte.ts)
  const zwin = () => zmienPanel('friendsPanelCollapsed', !waski);
  const tytulZwijania = $derived(waski ? t('panel.rozwin') : t('panel.zwin'));
  const onRight = $derived(ukladPanelu.strona === 'right');
</script>

{#snippet collapseButton()}
  <button type="button" class="ib zwijanie" aria-label={tytulZwijania} title={tytulZwijania} onclick={zwin}>
    <Ikona nazwa={waski ? 'zwin_panel' : 'rozwin_panel'} />
  </button>
{/snippet}

<aside class="panel" class:waski class:right={onRight} bind:this={panel} aria-label={t('panel.tytul')}>
  <header class="naglowek">
    {#if waski}
      {@render collapseButton()}
      {#if konto.loggedIn && ileChodzi}
        {@const walking = t('panel.pasek_licznik', { liczba: ileChodzi })}
        <span class="walking-count" role="img" aria-label={walking} title={walking}>
          <span class="status-dot"></span><span class="liczba">{ileChodzi}</span>
        </span>
      {/if}
    {:else}
      <!-- the collapse button sits at the edge next to the main column, where the strip has it -->
      {#if onRight}{@render collapseButton()}{/if}
      <div class="tytul">
        <h2>{t('panel.tytul')}</h2>
        <p class="jedna-linia" title={podpis}>{podpis}</p>
      </div>
      <div class="ikony">
        {#if konto.loggedIn}
          <button type="button" class="ib" aria-expanded={szukaj} aria-label={t('panel.szukaj')} title={t('panel.szukaj')} onclick={przelaczSzukaj}>
            <Ikona nazwa="dodaj" />
          </button>
        {/if}
        {#if !onRight}{@render collapseButton()}{/if}
      </div>
    {/if}
  </header>

  {#if !konto.loggedIn}
    {#if !waski}
      <div class="bez-konta">
        <p>{t('panel.bez_konta_opis')}</p>
        <p>{t('panel.bez_konta_opis2')}</p>
        <Przycisk szeroki napis={t('panel.konto')} onclick={() => idz('account', { accountMode: 'login' })} />
      </div>
    {/if}
  {:else}
    {#if szukaj && !waski}
      <div class="szukaj" bind:this={pole}>
        <Pole
          bind:wartosc={fraza}
          typ="search"
          etykietaDostepna={t('panel.pole_dostepnosc')}
          podpowiedz={t('panel.pole')}
          maxlength={stan.statics.account.nameMax}
          autocomplete="off"
          blad={bladZaproszenia}
          oninput={() => (bladZaproszenia = '')}
          onenter={zapros}
        />
        {#if doZaproszenia}
          <Przycisk szeroki napis={t('panel.zapros', { pseudonim: doZaproszenia })} disabled={konto.busy.includes('invite')} onclick={zapros} />
        {:else}
          <p class="podpowiedz">{t('panel.pole_opis')}</p>
        {/if}
      </div>
    {/if}

    <div class="lista">
      {#if zaproszenia.length}
        {#if !waski}<p class="grupa">{t('znajomi.zaproszenia_do_mnie')}</p>{/if}
        {#each zaproszenia as z (z.id)}
          {#if waski}
            <button type="button" class="zapr-waski" title={`${z.person.name} · ${t('panel.zaprasza')}`} onclick={zwin}>
              <OdznakaRzadkosci rzadkosc="pospolite" liczba="?" rozmiar={34} />
            </button>
          {:else}
            <div class="zaproszenie">
              <InitialAvatar name={z.person.name} />
              <span class="tresc">
                <span class="nick jedna-linia dane" title={z.person.name}>{z.person.name}</span>
                <span class="linia">{t('panel.zaprasza')}</span>
              </span>
              <span class="odpowiedz">
                <button type="button" class="ib mala" aria-label={t('znajomi.odrzuc')} title={t('znajomi.odrzuc')} disabled={konto.busy.includes('answer')} onclick={() => wykonaj(api.onlineInvitationAnswer(z.id, false))}>
                  <Ikona nazwa="zamknij" rozmiar={14} />
                </button>
                <button type="button" class="ib mala" aria-label={t('znajomi.przyjmij')} title={t('znajomi.przyjmij')} disabled={konto.busy.includes('answer')} onclick={() => wykonaj(api.onlineInvitationAnswer(z.id, true))}>
                  <Ikona nazwa="zaznacz" rozmiar={14} />
                </button>
              </span>
            </div>
          {/if}
        {/each}
        {#if waski}<span class="rozdzielacz"></span>{/if}
      {/if}

      {#if chodza.length}
        {#if !waski}<p class="grupa">{t('panel.chodza_teraz', { liczba: chodza.length })}</p>{/if}
        {#each chodza as f (f.id)}
          <WierszZnajomego {f} {waski} rozwiniety={menu?.f.id === f.id} onotworz={otworz} onmenu={otworzMenu} />
        {/each}
        {#if waski && pozostali.length}<span class="rozdzielacz"></span>{/if}
      {/if}
      {#if pozostali.length}
        {#if !waski}<p class="grupa">{t('panel.pozostali', { liczba: pozostali.length })}</p>{/if}
        {#each pozostali as f (f.id)}
          <WierszZnajomego {f} {waski} rozwiniety={menu?.f.id === f.id} onotworz={otworz} onmenu={otworzMenu} />
        {/each}
      {/if}
      {#if !waski && online.friends === null && znajomi.blad}
        <div class="blad-panelu">
          <KartaBledu
            zwarta
            tytul={t('online.blad.znajomi')}
            opis={znajomi.blad}
            ponowiZa={znajomi.nastepna ? (znajomi.nastepna - znajomi.teraz) / 1000 : null}
            onponow={() => odswiezZnajomych(true)}
          />
        </div>
      {/if}
      {#if !waski && online.friends !== null && !chodza.length && !pozostali.length}
        <p class="podpowiedz pusto">{fraza.trim() ? t('panel.nikt', { fraza: fraza.trim() }) : t('panel.pusto')}</p>
      {/if}
    </div>

    <StopkaPanelu
      {waski}
      otwarte={statusOtwarty}
      onprzelacz={() => {
        menu = null;
        statusOtwarty = !statusOtwarty;
      }}
      onzamknij={() => (statusOtwarty = false)}
    />
  {/if}

  {#if menu}
    <div class="menu-znajomego" role="menu" style:left="{menu.x}px" style:top="{menu.y}px">
      <button type="button" role="menuitem" class="pozycja" onclick={() => menu && otworz(menu.f)}>{t('panel.menu.profil')}</button>
      <button
        type="button"
        role="menuitem"
        class="pozycja"
        onclick={() => {
          const f = menu?.f;
          menu = null;
          if (f) usun(f);
        }}
      >{t('panel.menu.usun')}</button>
    </div>
  {/if}

  {#if toast && !waski}
    <div class="toast" role="status">
      <span class="dane">{toast.tekst}</span>
      {#if toast.cofnij}<button type="button" onclick={cofnij}>{t('panel.cofnij')}</button>{/if}
    </div>
  {/if}
</aside>

<style>
  .panel {
    position: relative;
    display: flex;
    flex-direction: column;
    min-width: 0;
    height: 100%;
    overflow: hidden;
    border-inline-start: 1px solid var(--line);
    background: var(--surface);
    color: var(--text);
  }
  :global([data-strona-panelu='left']) .panel { border-inline-start: 0; border-inline-end: 1px solid var(--line); }

  .naglowek {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 4px;
    min-height: var(--pagebar-h);
    padding: 8px 12px 4px 14px;
  }
  /* left: title ...... [+] [collapse], right: [collapse] title ...... [+]; the collapse button is
     12 px from the column side, centred in the 64 px strip the same way, so it does not move when
     the panel collapses */
  .right .naglowek { grid-template-columns: auto minmax(0, 1fr) auto; gap: 6px; padding-inline: 12px 6px; }
  .waski .naglowek {
    grid-template-columns: 1fr;
    grid-template-rows: calc(var(--pagebar-h) - 12px) auto;
    justify-items: center;
    padding: 8px 0 4px;
  }
  .walking-count {
    display: flex;
    align-items: center;
    gap: 4px;
    padding-bottom: 2px;
    font-size: var(--fs-sm);
    font-weight: var(--fw-bold);
    line-height: var(--lh-tight);
    color: var(--text);
  }
  .walking-count .status-dot { width: 8px; height: 8px; border-radius: var(--r-round); background: var(--zone-walk); }
  .tytul { min-width: 0; }
  h2 { font-size: var(--fs-base); font-weight: var(--fw-bold); line-height: var(--lh-tight); }
  .tytul p { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-tight); }
  .ikony { display: flex; }
  .ib {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border-radius: var(--r-md);
    color: var(--text);
    transition: background-color var(--t-color) linear;
  }
  .ib:hover,
  .ib[aria-expanded='true'] { background: var(--surface-2); }
  .ib:active { transform: scale(0.96); }
  .ib:disabled { color: var(--text-2); }
  .ib.mala { width: 36px; height: 36px; border: 1px solid var(--control); }
  /* the arrow points the way the window's outer edge moves: towards the main column when
     collapsing, away from it in the strip; drawn for a panel on the left, mirrored on the right */
  .right .zwijanie :global(.ikona) { transform: scaleX(-1); }

  .szukaj { display: grid; gap: 6px; padding: 4px 10px 8px; min-width: 0; }
  .podpowiedz { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-tight); padding-inline: 2px; }
  .pusto { padding: 12px 6px; }
  .blad-panelu { padding: 8px 6px 0 0; }

  .lista {
    flex: 1 1 0;
    min-height: 0;
    overflow-y: auto;
    scrollbar-gutter: stable;
    padding: 0 2px 8px 8px;
  }
  .waski .lista { padding: 4px 0; scrollbar-width: none; }
  .grupa {
    padding: 12px 6px 4px;
    font-size: var(--fs-sm);
    font-weight: var(--fw-semibold);
    color: var(--text-2);
    line-height: var(--lh-tight);
  }
  .rozdzielacz { display: block; height: 1px; margin: 6px 14px; background: var(--line); }

  .zaproszenie {
    display: grid;
    grid-template-columns: 34px minmax(0, 1fr) auto;
    gap: 10px;
    align-items: center;
    margin: 0 4px 2px 0;
    padding: 6px 6px 6px 8px;
    border: 1px solid var(--line);
    border-radius: var(--r-md);
  }
  .zapr-waski { display: grid; place-items: center; width: 100%; min-height: 50px; }
  .tresc { display: grid; gap: 2px; min-width: 0; }
  .nick { font-size: 0.9375rem; font-weight: var(--fw-semibold); line-height: var(--lh-tight); }
  .linia { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-tight); }
  .odpowiedz { display: flex; gap: 4px; }

  .bez-konta {
    display: grid;
    gap: 10px;
    align-content: start;
    padding: 8px 16px;
    color: var(--text-2);
    font-size: var(--fs-md);
    line-height: var(--lh-body);
  }

  .menu-znajomego {
    position: absolute;
    z-index: 30;
    display: grid;
    gap: 2px;
    width: min(200px, calc(100% - 16px));
    padding: 6px;
    border: 1px solid var(--control);
    border-radius: var(--r-lg);
    background: var(--surface-2);
    box-shadow: var(--shadow-pop);
    animation: wl-rozwin var(--t-in) var(--ease-enter);
  }
  .pozycja {
    min-height: 42px;
    padding: 8px 10px;
    border-radius: 8px;
    font-size: var(--fs-md);
    text-align: start;
    color: var(--text);
  }
  .pozycja:hover { background: color-mix(in srgb, var(--text) 10%, transparent); }

  .toast {
    position: absolute;
    inset-inline: 10px;
    bottom: 74px;
    z-index: 25;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 8px;
    align-items: center;
    padding: 10px 12px;
    border-radius: var(--r-md);
    background: var(--ink);
    color: var(--on-ink);
    box-shadow: var(--shadow-pop);
    font-size: var(--fs-md);
    font-weight: var(--fw-semibold);
    line-height: var(--lh-tight);
    overflow-wrap: anywhere;
    animation: wl-wjazd-w-gore var(--t-in) var(--ease-enter);
  }
  .toast button { color: inherit; font-weight: var(--fw-bold); text-decoration: underline; }
</style>
