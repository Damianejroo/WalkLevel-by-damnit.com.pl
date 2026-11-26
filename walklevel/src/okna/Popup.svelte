<script lang="ts">
  // Stos powiadomień nad grą (docs/wyglad.md 8.13): najwyżej 3 karty 380 × 96, najnowsza na
  // dole, starsze wyżej (odstęp 10). Wejście opacity + translateY(12px -> 0) 220 ms, wyjście
  // (leaving z rdzenia) opacity 180 ms, pozostałe karty dosuwają się transformem (FLIP).
  // Akcent: tylko kolor rzadkości odznaki i nadtytułu; bez gradientu i paska z lewej.
  import { onMount } from 'svelte';
  import { flip } from 'svelte/animate';
  import { cubicOut } from 'svelte/easing';
  import type { PopupItem } from '../lib/typy';
  import * as api from '../lib/api';
  import { t } from '../lib/teksty';
  import OdznakaRzadkosci from '../lib/ui/OdznakaRzadkosci.svelte';
  import PasekXP from '../lib/ui/PasekXP.svelte';

  let items = $state<PopupItem[]>([]);
  const spokojnie = matchMedia('(prefers-reduced-motion: reduce)').matches;
  const przesuniecie = { duration: spokojnie ? 0 : 220, easing: cubicOut };

  onMount(() => {
    let zamkniete = false;
    let koniec: (() => void) | undefined;
    let poZdarzeniu = false;
    (async () => {
      const stop = await api.sluchaj('wl:popupy', (v) => {
        poZdarzeniu = true;
        items = v.items;
      });
      if (zamkniete) return stop();
      koniec = stop;
      const start = await api.popupReady();
      // Zdarzenie, które przyszło w trakcie, jest nowsze niż stan startowy.
      if (!poZdarzeniu) items = start.items;
      await api.uiReady('popup');
    })()
      .catch((e) => console.error('Start okna powiadomień nie powiódł się', e))
      .finally(() => (document.documentElement.dataset.gotowe = '1'));
    return () => {
      zamkniete = true;
      koniec?.();
    };
  });

  // Tytuł okna (np. dla czytnika ekranu): najnowsze powiadomienie
  $effect(() => {
    const najnowszy = items.at(-1);
    document.title = najnowszy ? t('powiadomienie.tytul_okna', { tytul: najnowszy.title }) : t('app.nazwa');
  });
</script>

<div class="stos" aria-live="polite">
  {#each items as it (it.id)}
    <div class="miejsce" animate:flip={przesuniecie}>
      <article class="karta" class:wychodzi={it.leaving} aria-label={t('powiadomienie.dostepnosc')} style:--kolor={it.color}>
        <OdznakaRzadkosci rzadkosc={it.rarity} znak={it.mark || undefined} liczba={it.badgeText ?? undefined} rozmiar={64} />
        <div class="tresc">
          <p class="nad hud jedna-linia">{it.kicker}</p>
          <p class="tytul jedna-linia">{it.title}</p>
          {#if it.progress !== null}
            <PasekXP wartosc={it.progress} wysokosc={8} kolor={it.color} opis={it.line} />
          {/if}
          <p class="linia jedna-linia">{it.line}</p>
        </div>
      </article>
    </div>
  {/each}
</div>

<style>
  /* Okno przezroczyste: widać tylko karty */
  :global(html.okno-przezroczyste),
  :global(html.okno-przezroczyste body) { background: transparent; }
  :global(html.okno-przezroczyste #app) { position: relative; }

  /* Podgląd w przeglądarce (?demo=1): okno 380 × 308 w prawym dolnym rogu, margines 16 */
  :global(html.podglad-okna body) { background: var(--control); }
  :global(html.podglad-okna #app) {
    position: fixed;
    right: var(--sp-4);
    bottom: var(--sp-4);
    width: 380px;
    height: 308px;
  }

  .stos {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    gap: 10px;
    overflow: hidden;
    pointer-events: none;
    user-select: none;
  }
  .miejsce { flex: none; }

  .karta {
    display: grid;
    grid-template-columns: 64px minmax(0, 1fr);
    gap: var(--sp-3);
    align-items: center;
    width: 100%;
    height: 96px;
    padding: 8px 16px 8px 14px;
    background: var(--popup-bg);
    border: 1px solid var(--popup-line);
    border-radius: 8px;
    color: var(--popup-text);
    animation: wejscie var(--t-in) var(--ease-enter) backwards;
    transition: opacity var(--t-out) var(--ease-exit);
  }
  .karta.wychodzi { opacity: 0; }

  .tresc {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .nad {
    color: var(--kolor);
    letter-spacing: 0.15em;
    padding-block-start: 0.12em;   /* kreski nad Ó, Ś, Ż w wersalikach Bahnschrift */
  }
  .tytul {
    font-family: var(--font-num);
    font-size: 1.0625rem;
    font-weight: var(--fw-bold);
    line-height: 1.2;
    padding-block-start: 0.06em;
  }
  .linia {
    color: var(--popup-text-2);
    font-size: var(--fs-xs);
    line-height: var(--lh-body);
  }

  @keyframes wejscie {
    from { opacity: 0; transform: translateY(12px); }
    to { opacity: 1; transform: none; }
  }
</style>
