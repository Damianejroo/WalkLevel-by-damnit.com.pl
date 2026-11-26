<script lang="ts">
  // Ekran: Ustawienia (trasa 'settings'), docs/wyglad.md 8.5, 9.3; docs/teksty.md 9-18.
  // Akordeon 7 kategorii: otwierają się niezależnie, kliknięty nagłówek stoi w miejscu (zmienia się
  // tylko to, co pod nim), bez automatycznego przewijania. Wartości przy nagłówkach liczone na
  // żywo ze stanu. Otwarte kategorie zostają po powrocie (ekran montowany raz). Bez akcentu.
  import Strona from '../lib/ui/Strona.svelte';
  import NaglowekAkordeonu from '../lib/ui/NaglowekAkordeonu.svelte';
  import type { NazwaIkony } from '../lib/ui/Ikona.svelte';
  import KatOsoby from './Ustawienia/KatOsoby.svelte';
  import KatSterowanie from './Ustawienia/KatSterowanie.svelte';
  import KatBieznia from './Ustawienia/KatBieznia.svelte';
  import KatPoziomy from './Ustawienia/KatPoziomy.svelte';
  import KatWyglad from './Ustawienia/KatWyglad.svelte';
  import KatDane from './Ustawienia/KatDane.svelte';
  import KatPomoc from './Ustawienia/KatPomoc.svelte';
  import { t, tl, type Klucz } from '../lib/teksty';
  import { stan, type PropsEkranu } from '../lib/stan.svelte';
  import * as api from '../lib/api';

  let { parametry, aktywny }: PropsEkranu = $props();

  type Kategoria = 'osoby' | 'sterowanie' | 'bieznia' | 'poziomy' | 'wyglad' | 'dane' | 'pomoc';
  interface OpisKategorii {
    klucz: Kategoria;
    ikona: NazwaIkony;
    nazwa: Klucz;
    opis: Klucz;
  }
  const KATEGORIE: OpisKategorii[] = [
    { klucz: 'osoby', ikona: 'konto', nazwa: 'ustawienia.kat.osoby.nazwa', opis: 'ustawienia.kat.osoby.opis' },
    { klucz: 'sterowanie', ikona: 'pad', nazwa: 'ustawienia.kat.sterowanie.nazwa', opis: 'ustawienia.kat.sterowanie.opis' },
    { klucz: 'bieznia', ikona: 'bieznia', nazwa: 'ustawienia.kat.bieznia.nazwa', opis: 'ustawienia.kat.bieznia.opis' },
    { klucz: 'poziomy', ikona: 'gwiazda', nazwa: 'ustawienia.kat.poziomy.nazwa', opis: 'ustawienia.kat.poziomy.opis' },
    { klucz: 'wyglad', ikona: 'wyglad', nazwa: 'ustawienia.kat.wyglad.nazwa', opis: 'ustawienia.kat.wyglad.opis' },
    { klucz: 'dane', ikona: 'dane', nazwa: 'ustawienia.kat.dane.nazwa', opis: 'ustawienia.kat.dane.opis' },
    { klucz: 'pomoc', ikona: 'pomoc', nazwa: 'ustawienia.kat.pomoc.nazwa', opis: 'ustawienia.kat.pomoc.opis' },
  ];

  /**
   * Podgląd w przeglądarce (atrapa, test zrzutów): `?otwarte=osoby,pomoc` albo `?otwarte=wszystkie`;
   * scenariusze atrapy `ustawienia*` otwierają wszystkie kategorie. W aplikacji wszystkie zwinięte.
   */
  function naStart(): Record<Kategoria, boolean> {
    const wynik = Object.fromEntries(KATEGORIE.map((k) => [k.klucz, false])) as Record<Kategoria, boolean>;
    if (api.wTauri) return wynik;
    const p = new URLSearchParams(location.search);
    const lista = p.get('otwarte') ?? (p.get('stan')?.startsWith('ustawienia') ? 'wszystkie' : '');
    for (const k of KATEGORIE) wynik[k.klucz] = lista === 'wszystkie' || lista.split(',').includes(k.klucz);
    return wynik;
  }
  const otwarte = $state(naStart());
  // Wejście z odnośnika („Zarządzaj profilami gier”, klawisz włączania na ekranie głównym) rozwija
  // wskazaną kategorię; pozostałe zostają, jak były.
  $effect(() => {
    if (aktywny && parametry.kategoria) otwarte[parametry.kategoria] = true;
  });

  const osoba = $derived(stan.people.people.find((o) => o.id === stan.people.activePersonId));
  const profil = $derived(osoba?.profiles.find((p) => p.id === stan.people.activeProfileId));
  const motyw = $derived.by(() => {
    const [system, jasny, ciemny] = tl('ustawienia.wartosc.motyw');
    return { system, light: jasny, dark: ciemny }[stan.settings.theme];
  });
  const bieznia = $derived(
    stan.settings.simulator
      ? t('ustawienia.wartosc.bieznia_symulator')
      : !stan.device.remembered
        ? t('ustawienia.wartosc.bieznia_brak')
        : t('ustawienia.wartosc.bieznia', {
            model: stan.device.device?.model || stan.device.label.split('\n')[0] || stan.settings.deviceName,
          }),
  );
  const wartosci = $derived<Record<Kategoria, string | undefined>>({
    osoby: !osoba
      ? undefined
      : stan.account.loggedIn && stan.account.name
        ? t('ustawienia.wartosc.osoby', { osoba: osoba.name, pseudonim: stan.account.name })
        : osoba.name,
    sterowanie: t('ustawienia.wartosc.sterowanie', {
      profil: profil?.name ?? '',
      tryb: stan.statics.modeNames[stan.settings.direction],
      klawisz_wlaczania: stan.settings.toggleKey.label,
    }),
    bieznia,
    poziomy: t(stan.settings.achievementPopups ? 'ustawienia.wartosc.poziomy_wl' : 'ustawienia.wartosc.poziomy_wyl'),
    wyglad: motyw,
    dane: undefined,
    pomoc: t('ustawienia.wartosc.pomoc', { wersja: stan.app.version }),
  });
</script>

<Strona tytul={t('ustawienia.tytul')}>
  <div class="kategorie">
    {#each KATEGORIE as k (k.klucz)}
      <NaglowekAkordeonu
        ikona={k.ikona}
        tytul={t(k.nazwa)}
        opis={t(k.opis)}
        wartosc={wartosci[k.klucz]}
        bind:otwarty={otwarte[k.klucz]}
      >
        {#if k.klucz === 'osoby'}
          <KatOsoby />
        {:else if k.klucz === 'sterowanie'}
          <KatSterowanie />
        {:else if k.klucz === 'bieznia'}
          <KatBieznia />
        {:else if k.klucz === 'poziomy'}
          <KatPoziomy />
        {:else if k.klucz === 'wyglad'}
          <KatWyglad />
        {:else if k.klucz === 'dane'}
          <KatDane />
        {:else}
          <KatPomoc />
        {/if}
      </NaglowekAkordeonu>
    {/each}
  </div>
</Strona>

<style>
  .kategorie { display: flex; flex-direction: column; gap: var(--sp-3); min-width: 0; }
</style>
