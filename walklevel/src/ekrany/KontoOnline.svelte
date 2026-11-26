<script lang="ts">
  // Ekran: Konto online (trasa 'online') - docs/wyglad.md 8.11, 9.4; docs/teksty.md 22.4-22.9.
  // Tytuł „Konto online”, pseudonim jako podpis. Zakładki Znajomi / Ranking / Osoby z kontem /
  // Moje konto: każda montowana przy pierwszym otwarciu i potem tylko chowana (stały ekran).
  // Dane z rdzenia (stan.online, stan.account); przy wejściu na ekran i zmianie zakładki ciche
  // odświeżenie (online_load bez force), a na otwartej zakładce Znajomi odświeżanie wspólne
  // z panelem znajomych (znajomi.svelte.ts): dane podmieniane w miejscu, jedno zapytanie naraz. Błąd wczytania - komunikat
  // z „Spróbuj ponownie” w miejscu (błąd cichego odświeżenia zostawia ostatnią listę).
  import { untrack } from 'svelte';
  import { SvelteSet } from 'svelte/reactivity';
  import Strona from '../lib/ui/Strona.svelte';
  import Karta from '../lib/ui/Karta.svelte';
  import Przycisk from '../lib/ui/Przycisk.svelte';
  import Segmentowy from '../lib/ui/Segmentowy.svelte';
  import ZakladkaZnajomi from './KontoOnline/ZakladkaZnajomi.svelte';
  import ZakladkaRanking from './KontoOnline/ZakladkaRanking.svelte';
  import ZakladkaOsoby from './KontoOnline/ZakladkaOsoby.svelte';
  import ZakladkaKonto from './KontoOnline/ZakladkaKonto.svelte';
  import * as api from '../lib/api';
  import { idz, stan, wykonaj, type PropsEkranu } from '../lib/stan.svelte';
  import { t, tytulIPodpis } from '../lib/teksty';
  import type { OnlineTab } from '../lib/typy';
  import { sprobuj } from './KontoOnline/pomocnicze';
  import { odswiezZnajomych, potrzebujeZnajomych, znajomi } from '../lib/znajomi.svelte';
  import KartaBledu from '../lib/ui/KartaBledu.svelte';
  import type { Klucz } from '../lib/teksty';

  let { aktywny }: PropsEkranu = $props();

  const ZAKLADKI: OnlineTab[] = ['friends', 'ranking', 'people', 'me'];
  const jestZakladka = (v: unknown): v is OnlineTab => ZAKLADKI.includes(v as OnlineTab);
  // podgląd w przeglądarce i zrzuty: ?zakladka=ranking (w aplikacji adres jest bez parametrów)
  const zAdresu = new URLSearchParams(location.search).get('zakladka');

  let zakladka = $state<OnlineTab>(jestZakladka(zAdresu) ? zAdresu : 'friends');
  // wybrana zakładka renderuje się od razu (bez pustej klatki przed efektem), potem zostaje
  // zamontowana i przy kolejnych zmianach tylko chowana
  const odwiedzone = new SvelteSet<OnlineTab>();
  $effect(() => {
    odwiedzone.add(zakladka);
  });

  const konto = $derived(stan.account);
  const naglowek = $derived(
    konto.loggedIn ? tytulIPodpis(t('online.tytul', { pseudonim: konto.name })) : { tytul: t('online.tytul_ogolny'), podpis: '' },
  );

  // --- wczytywanie: przy wejściu, zmianie zakładki i zmianie osoby ---
  let bledy = $state<Partial<Record<OnlineTab, string>>>({});
  // trwające pobranie (zakładka) - odświeżenie co 10 s nie dubluje zapytań
  let trwa: OnlineTab | null = null;
  async function wczytaj(z: OnlineTab, force: boolean) {
    if (z === 'me') return; // Moje konto to wycinek account - nic do pobrania
    if (trwa === z) return;
    trwa = z;
    try {
      const w = await sprobuj(api.onlineLoad(z, force));
      bledy[z] = w.ok ? '' : w.blad || Object.values(w.pola)[0] || '';
    } finally {
      if (trwa === z) trwa = null;
    }
  }
  $effect(() => {
    if (!aktywny || !konto.loggedIn) return;
    const z = zakladka;
    void stan.online.personId;
    untrack(() => wczytaj(z, false));
  });
  // „idzie teraz” na otwartej zakładce Znajomi: to samo odświeżanie co panel znajomych
  // (src/lib/znajomi.svelte.ts - 10 s przy widocznym oknie, 60 s przy zminimalizowanym)
  $effect(() => {
    if (!aktywny || !konto.loggedIn || zakladka !== 'friends') return;
    return potrzebujeZnajomych();
  });

  // Karta błędu tylko wtedy, gdy nie ma czego pokazać (błąd odświeżenia przy istniejących
  // danych zostawia ostatnią listę). Znajomi: błąd wspólnego odświeżania (znajomi.svelte.ts).
  const TYTUL_BLEDU: Partial<Record<OnlineTab, Klucz>> = {
    friends: 'online.blad.znajomi',
    ranking: 'online.blad.ranking',
    people: 'online.blad.osoby',
  };
  const bladZakladki = $derived.by(() => {
    const o = stan.online;
    if (zakladka === 'friends') return o.friends === null ? znajomi.blad || bledy.friends || '' : '';
    if (zakladka === 'ranking') return o.ranking === null ? bledy.ranking || '' : '';
    if (zakladka === 'people') return o.people === null ? bledy.people || '' : '';
    return '';
  });

  function otworzProfil(userId: string) {
    if (!userId || konto.busy.includes('person')) return;
    wykonaj(api.onlinePersonOpen(userId));
  }
</script>

<Strona tytul={naglowek.tytul} podpis={naglowek.podpis}>
  {#if !konto.loggedIn}
    <Karta>
      <p class="opis">{t('konto.blad.niezalogowany')}</p>
      <div class="przyciski">
        <Przycisk wariant="glowny" napis={t('konto.karta.zaloguj')} onclick={() => idz('account', { accountMode: 'login' })} />
        <Przycisk napis={t('konto.karta.zaloz')} onclick={() => idz('account', { accountMode: 'register' })} />
      </div>
    </Karta>
  {:else}
    <Segmentowy
      etykieta={t('online.tytul_ogolny')}
      bind:wartosc={zakladka}
      opcje={[
        { wartosc: 'friends', napis: t('online.zakladka.znajomi') },
        { wartosc: 'ranking', napis: t('online.zakladka.ranking') },
        { wartosc: 'people', napis: t('online.zakladka.osoby') },
        { wartosc: 'me', napis: t('online.zakladka.konto') },
      ]}
    />

    {#if konto.termsOutdated}
      <Karta tytul={t('konto.regulamin.zmiana_tytul')}>
        <p class="opis">{t('konto.regulamin.zmiana_opis')}</p>
        <div class="przyciski">
          <Przycisk napis={t('konto.regulamin.czytaj')} title={konto.termsUrl || undefined} onclick={() => wykonaj(api.openLink('regulamin'))} />
          <Przycisk
            wariant="glowny"
            napis={t('konto.regulamin.akceptuj')}
            disabled={konto.busy.includes('terms')}
            onclick={() => wykonaj(api.accountAcceptTerms())}
          />
        </div>
      </Karta>
    {/if}

    {#if bladZakladki}
      <KartaBledu
        tytul={t(TYTUL_BLEDU[zakladka] ?? 'online.blad.znajomi')}
        opis={bladZakladki}
        zajete={stan.online.loading.length > 0}
        ponowiZa={zakladka === 'friends' && znajomi.nastepna ? (znajomi.nastepna - znajomi.teraz) / 1000 : null}
        onponow={() => (zakladka === 'friends' ? odswiezZnajomych(true) : wczytaj(zakladka, true))}
      />
    {/if}

    {#each ZAKLADKI as z (z)}
      {#if odwiedzone.has(z) || z === zakladka}
        <div class="panel" hidden={z !== zakladka}>
          {#if z === 'friends'}
            <ZakladkaZnajomi glowna={!konto.termsOutdated} onotworz={otworzProfil} />
          {:else if z === 'ranking'}
            <ZakladkaRanking onotworz={otworzProfil} />
          {:else if z === 'people'}
            <ZakladkaOsoby onotworz={otworzProfil} />
          {:else}
            <ZakladkaKonto widoczna={aktywny && zakladka === 'me'} onotworz={otworzProfil} />
          {/if}
        </div>
      {/if}
    {/each}
  {/if}
</Strona>

<style>
  .opis { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-body); }
  .przyciski { display: flex; flex-wrap: wrap; gap: var(--sp-2); }
  .przyciski > :global(button) { flex: 0 1 auto; }
  .panel { display: flex; flex-direction: column; min-width: 0; animation: wl-rozwin var(--t-in) var(--ease-enter); }
  .panel[hidden] { display: none; }
</style>
