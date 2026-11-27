<script lang="ts">
  // Wzornik komponentów (trasa 'wzornik', tylko podgląd: /?ekran=wzornik). Wszystkie komponenty
  // z src/lib/ui w najdłuższych wariantach tekstów - test „nic nie wystaje” sprawdza je przy
  // 420/740/1400 px. Teksty ze słownika (docs/texts.md); to nie jest ekran aplikacji.
  import Strona from '../lib/ui/page.svelte';
  import Przycisk from '../lib/ui/button.svelte';
  import Przelacznik from '../lib/ui/switch.svelte';
  import Segmentowy from '../lib/ui/segmented-control.svelte';
  import Karta from '../lib/ui/card.svelte';
  import NaglowekAkordeonu from '../lib/ui/accordion-header.svelte';
  import Pole from '../lib/ui/text-field.svelte';
  import KafelekStatystyki from '../lib/ui/stat-tile.svelte';
  import SiatkaKafelkow from '../lib/ui/tile-grid.svelte';
  import OdznakaRzadkosci, { rzadkoscPoziomu } from '../lib/ui/rarity-badge.svelte';
  import PasekXP from '../lib/ui/xp-bar.svelte';
  import WierszListy from '../lib/ui/list-row.svelte';
  import Ikona, { IKONY, type NazwaIkony } from '../lib/ui/icon.svelte';
  import { NAZWY_ZNAKOW } from '../lib/ui/badge-glyphs';
  import { t, tytulIPodpis } from '../lib/texts';
  import { liczba, zJednostka } from '../lib/format';
  import { komunikat, stan, type PropsEkranu } from '../lib/state.svelte';
  import type { Rarity, ThemeChoice, HistoryRange, RankingScope } from '../lib/contract';

  let _props: PropsEkranu = $props();

  const RZADKOSCI: Rarity[] = ['pospolite', 'rzadkie', 'epickie', 'heroiczne', 'legendarne'];
  const STATUSY = ['brak', 'skanowanie', 'polaczono', 'ponawiam', 'symulator'] as const;
  const napisyStatusu = [
    t('glowny.status.brak.akcja'), t('glowny.status.skanowanie.akcja'), t('glowny.status.polaczono.akcja'),
    t('glowny.status.ponawiam.akcja'), t('glowny.status.symulator.akcja'),
  ];
  let status = $state(0);
  let wlaczony = $state(true);
  let motyw = $state<ThemeChoice>('system');
  let zakres = $state<HistoryRange>('week');
  let ranking = $state<RankingScope>('tydzien');
  let dwa = $state('me');
  let otwarte = $state([true, false, false]);
  let opisZgloszenia = $state('Za');
  let haslo = $state('');
  const poziom = $derived(stan.levels?.level ?? 12);
  const profile = $derived(tytulIPodpis(t('ustawienia.profile.tytul', { osoba: stan.people.people[0]?.name ?? '' })));
</script>

<Strona tytul="Wzornik" podpis={t('app.haslo')}>
  <Karta tytul={profile.tytul} podpis={profile.podpis}>
    <div class="rzad">
      <Przycisk wariant="glowny" napis={t('zgloszenie.wyslij')} />
      <Przycisk napis={t('ustawienia.dane.eksport')} />
      <Przycisk wariant="stop" napis={t('glowny.trening.stop')} />
      <Przycisk wariant="potwierdz" napis={t('sesja.usun.przycisk')} onclick={() => komunikat(t('sesja.usunieta'))} />
      <Przycisk wariant="link" napis={t('app.link.prywatnosc')} />
      <Przycisk disabled napis={t('wspolne.anuluj')} />
      <Przycisk ikona="dodaj" napis={t('ustawienia.osoby.dodaj_z_kontem')} />
    </div>
    <div class="rzad">
      <Przycisk napisy={napisyStatusu} napis={napisyStatusu[status]} onclick={() => (status = (status + 1) % STATUSY.length)} />
      <Przycisk wariant="ikona" ikona="ustawienia" etykieta={t('glowny.pasek.ustawienia')} />
      <Przycisk wariant="ikona" ikona="wykres" etykieta={t('glowny.pasek.historia')} />
    </div>
  </Karta>

  <Karta>
    <Przelacznik bind:wlaczony tytul={t('ustawienia.wyglad.przycisk')} opis={t('ustawienia.wyglad.przycisk_opis')} />
    <Przelacznik wlaczony={false} tytul={t('ustawienia.pomoc.logi_bt')} opis={t('ustawienia.pomoc.logi_bt_opis')} />
    <Przelacznik wlaczony disabled tytul={t('ustawienia.wyglad.duzy_tekst')} opis={t('ustawienia.wyglad.duzy_tekst_opis')} />
  </Karta>

  <Karta tytul={t('ustawienia.wyglad.motyw')}>
    <Segmentowy
      etykieta={t('ustawienia.wyglad.motyw')}
      bind:wartosc={motyw}
      opcje={[
        { wartosc: 'system', napis: t('ustawienia.wyglad.motyw.system') },
        { wartosc: 'light', napis: t('ustawienia.wyglad.motyw.jasny') },
        { wartosc: 'dark', napis: t('ustawienia.wyglad.motyw.ciemny') },
      ]}
    />
    <Segmentowy
      etykieta={t('historia.zakres')}
      bind:wartosc={zakres}
      opcje={[
        { wartosc: 'day', napis: t('historia.zakres.dzis') },
        { wartosc: 'week', napis: t('historia.zakres.7') },
        { wartosc: 'month', napis: t('historia.zakres.30') },
        { wartosc: 'all', napis: t('historia.zakres.wszystko') },
      ]}
    />
    <Segmentowy
      etykieta={t('ranking.zakres')}
      bind:wartosc={ranking}
      opcje={[
        { wartosc: 'tydzien', napis: t('ranking.zakres.tydzien'), aktywny: true },
        { wartosc: 'miesiac', napis: t('ranking.zakres.miesiac') },
        { wartosc: 'razem', napis: t('ranking.zakres.zawsze') },
        { wartosc: 'poziom', napis: t('ranking.zakres.poziomy') },
      ]}
    />
    <Segmentowy
      etykieta={t('historia.czyja')}
      bind:wartosc={dwa}
      opcje={[
        { wartosc: 'me', napis: t('historia.czyja.moja') },
        { wartosc: 'all', napis: t('historia.czyja.wszyscy') },
      ]}
    />
  </Karta>

  <div class="akordeony">
    <NaglowekAkordeonu ikona="konto" tytul={t('ustawienia.kat.osoby.nazwa')} opis={t('ustawienia.kat.osoby.opis')} wartosc={t('ustawienia.wartosc.osoby', { osoba: 'Damianejro_bardzo_dlugi1', pseudonim: 'Damianejro_bardzo_dlugi1' })} bind:otwarty={otwarte[0]}>
      <Karta>
        <Przelacznik wlaczony tytul={t('ustawienia.wyglad.na_wierzchu')} opis={t('ustawienia.wyglad.na_wierzchu_opis')} />
      </Karta>
    </NaglowekAkordeonu>
    <NaglowekAkordeonu ikona="pad" tytul={t('ustawienia.kat.sterowanie.nazwa')} opis={t('ustawienia.kat.sterowanie.opis')} wartosc={t('ustawienia.wartosc.sterowanie', { profil: 'Gra o bardzo długiej nazwie, czterdzieści', tryb: t('tryb.trening.nazwa'), klawisz_wlaczania: 'Ctrl+Shift+F8' })} bind:otwarty={otwarte[1]}>
      <Karta>
        <Pole etykieta={t('zgloszenie.opis_etykieta')} podpowiedz={t('zgloszenie.opis_przyklad')} typ="wielowierszowe" bind:wartosc={opisZgloszenia} blad={opisZgloszenia.trim().length < 5 ? t('zgloszenie.blad.krotki', { liczba: 5 - opisZgloszenia.trim().length }) : ''} />
      </Karta>
    </NaglowekAkordeonu>
    <NaglowekAkordeonu ikona="bieznia" tytul={t('ustawienia.kat.bieznia.nazwa')} opis={t('ustawienia.kat.bieznia.opis')} wartosc="WalkingPad X21 (KS-ST-X21C)" bind:otwarty={otwarte[2]} />
  </div>

  <Karta>
    <Pole etykieta={t('konto.pole.email')} podpowiedz="imie@poczta.pl" typ="email" blad={t('konto.blad.email')} />
    <Pole etykieta={t('konto.pole.haslo')} typ="password" bind:wartosc={haslo} etykietaPokaz={t('konto.pole.haslo')} opis={t('konto.haslo_zasady', { liczba: 10 })} />
    <Pole etykietaDostepna={t('znajomi.dodaj_pole')} podpowiedz={t('znajomi.dodaj_pole')} />
  </Karta>

  <SiatkaKafelkow>
    <KafelekStatystyki tytul={t('glowny.licznik.kroki')} wartosc={liczba(123456)} />
    <KafelekStatystyki tytul="Kroki/min" etykieta={t('glowny.licznik.kadencja')} wartosc={null} />
    <KafelekStatystyki tytul={t('glowny.licznik.dystans')} wartosc={zJednostka(1234.56, 'km', 2)} />
    <KafelekStatystyki tytul={t('glowny.licznik.czas')} wartosc="12 h 05 min" />
  </SiatkaKafelkow>

  <Karta tytul={t('poziomy.ranga_wiersz', { poziom, ranga: stan.levels?.rank.name ?? '' })} podpis={t('poziomy.xp', { xp: stan.levels?.xp ?? 0, xp_max: stan.levels?.levelCeilingXp ?? 0 })}>
    <div class="odznaki">
      <OdznakaRzadkosci rzadkosc={rzadkoscPoziomu(poziom)} liczba={poziom} rozmiar={62} etykieta={t('poziomy.ranga_dostepnosc', { poziom, ranga: stan.levels?.rank.name ?? '' })} />
      <OdznakaRzadkosci rzadkosc="legendarne" liczba={100} rozmiar={62} />
      {#each RZADKOSCI as r, i (r)}
        <OdznakaRzadkosci rzadkosc={r} znak="korona" rozmiar={44} etykieta={t(`rzadkosc.${r}`)} />
        <OdznakaRzadkosci rzadkosc={r} znak="stopy" rozmiar={44} zablokowana postep={(i + 1) / 6} />
      {/each}
    </div>
    <PasekXP wartosc={stan.levels?.levelFraction ?? 0.62} opis={t('poziomy.xp', { xp: stan.levels?.xp ?? 0, xp_max: stan.levels?.levelCeilingXp ?? 0 })} />
    <PasekXP wartosc={0.4} wysokosc={8} odcinki={0} kolor="var(--rarity-epickie)" opis={t('poziomy.postep.km', { liczba: '12,3', razem: 42 })} />
    <p class="hud" style:color="var(--rarity-epickie-text)">{t('rzadkosc.epickie')}</p>
  </Karta>

  <Karta>
    <div class="znaki">
      {#each NAZWY_ZNAKOW as z (z)}
        <OdznakaRzadkosci rzadkosc={RZADKOSCI[z.length % 5]} znak={z} rozmiar={44} etykieta={z} />
      {/each}
    </div>
  </Karta>

  <div class="lista">
    <WierszListy tytul="Damianejro_bardzo_dlugi1_bez_spacji" opis={t('ustawienia.osoby.wiersz', { liczba: 6, kroki: 12345, pseudonim: 'Damianejro' })} ikona="konto" wybrany />
    <WierszListy tytul="Gra o bardzo długiej nazwie, czterdzieści" opis={t('historia.sesje.wiersz', { dzien: t('historia.sesje.dzis'), godzina: '18:20', osoba: 'Damian', profil: 'Skyrim', kroki: 5432, czas: '42 min', kmh: '3,1' })} znak={''} onclick={() => komunikat(t('wspolne.blad_nieznany'), 'error')} />
    <WierszListy tytul={t('kopia.import.konflikt', { osoba: 'Damian' })} opis={t('kopia.import.polacz_opis')}>
      {#snippet koniec()}
        <Przelacznik etykieta={t('kopia.import.polacz')} wlaczony />
      {/snippet}
    </WierszListy>
  </div>

  <Karta>
    <div class="ikony">
      {#each Object.keys(IKONY) as nazwa (nazwa)}
        <span title={nazwa}><Ikona nazwa={nazwa as NazwaIkony} rozmiar={20} /></span>
      {/each}
    </div>
    <Przycisk napis={t('app.zapis.zapisano')} onclick={() => komunikat(t('kopia.import.kopia_przed'))} />
  </Karta>
</Strona>

<style>
  .rzad { display: flex; flex-wrap: wrap; gap: var(--sp-2); }
  .rzad > :global(*) { flex: 1 1 160px; }
  .akordeony, .lista { display: flex; flex-direction: column; gap: var(--sp-2); }
  .odznaki, .znaki, .ikony { display: flex; flex-wrap: wrap; gap: var(--sp-2); align-items: center; }
  .ikony { gap: var(--sp-3); color: var(--text-2); }
</style>
