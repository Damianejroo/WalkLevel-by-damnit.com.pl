<script lang="ts">
  // Ekran główny (trasa 'main', docs/wyglad.md 9.2): pasek z nazwą, osobą i ikonami -> karta
  // stanu bieżni -> karta poziomu -> wskaźnik prędkości z podpisem i slotem 56 (żetony stref albo
  // pasek Treningu) -> kafelki liczników -> karta sterowania. Ekran stały: telemetria (10-20 Hz)
  // zmienia tylko liczby, kąt łuku i aktywną strefę; zmiana trybu nie zmienia wysokości układu.
  import Strona from '../lib/ui/Strona.svelte';
  import Karta from '../lib/ui/Karta.svelte';
  import Przycisk from '../lib/ui/Przycisk.svelte';
  import Ikona from '../lib/ui/Ikona.svelte';
  import KafelekStatystyki from '../lib/ui/KafelekStatystyki.svelte';
  import SiatkaKafelkow from '../lib/ui/SiatkaKafelkow.svelte';
  import WierszListy from '../lib/ui/WierszListy.svelte';
  import { liczba } from '../lib/format';
  import { czas, t } from '../lib/teksty';
  import { stan, zywo, idz, wykonaj, type PropsEkranu, type Trasa } from '../lib/stan.svelte';
  import * as api from '../lib/api';
  import { EKRANY } from './rejestr';
  import Awatar from './Glowny/Awatar.svelte';
  import KartaStanu from './Glowny/KartaStanu.svelte';
  import KartaPoziomu from './Glowny/KartaPoziomu.svelte';
  import KartaSterowania from './Glowny/KartaSterowania.svelte';
  import ListaRozwijana from './Glowny/ListaRozwijana.svelte';
  import SlotStref from './Glowny/SlotStref.svelte';
  import Wskaznik from './Glowny/Wskaznik.svelte';
  import { ukladStref } from './Glowny/strefy';

  let _props: PropsEkranu = $props();

  // --- pasek ---
  const osoba = $derived(stan.people.people.find((o) => o.id === stan.people.activePersonId));
  const wieleOsob = $derived(stan.people.people.length > 1);
  const aktualizacja = $derived(stan.update.available);
  // Pod nazwą: postęp aktualizacji („Sprawdzam…”, „Pobieram 0.6.1: 45%”) albo „Dostępna wersja
  // 0.6.1” (docs/teksty.md 18.1, jak w 0.5.x), inaczej wersja z osobą.
  const statusAktualizacji = $derived(
    stan.update.busy || stan.update.installing
      ? stan.update.status
      : aktualizacja
        ? stan.update.status || t('aktualizacja.dostepna_krotko', { nowa_wersja: aktualizacja })
        : '',
  );
  const podpis = $derived(
    statusAktualizacji ||
      (wieleOsob && osoba
        ? t('app.osoba_i_wersja', { osoba: osoba.name, wersja: stan.app.version })
        : t('app.wersja', { wersja: stan.app.version })),
  );
  const zmianaOsoby = $derived(wieleOsob && !!osoba && stan.settings.showPersonSwitch);
  const pokazAktualizacje = $derived(!!aktualizacja || stan.update.busy);
  // przy 420 px pełny pasek (osoba + 5 ikon) ledwo się mieści - ikony trochę węższe
  const ciasno = $derived(zmianaOsoby && pokazAktualizacje && stan.account.loggedIn);

  let listaOsob = $state(false);
  let przyciskOsoby = $state<HTMLButtonElement>();
  const osoby = $derived(
    stan.people.people.map((o) => ({
      id: o.id,
      tytul: o.name,
      opis: t('glowny.osoby.dzis', { kroki: o.todaySteps }),
      aktywna: o.id === stan.people.activePersonId,
    })),
  );

  // --- wskaźnik i liczniki ---
  const uklad = $derived(ukladStref(stan.settings, stan.statics.maxSpeed));
  const tel = $derived(zywo.telemetria);
  const tryb = $derived(tel?.mode ?? 'offline');
  const aktywna = $derived(tel?.active ?? -1);
  const trening = $derived(stan.settings.direction === 'training');

  const kroki = $derived(tel?.steps == null ? null : liczba(tel.steps));
  const kadencja = $derived(tel?.cadence == null ? null : liczba(tel.cadence));
  const dystans = $derived(tel ? t('glowny.licznik.km', { km: liczba(tel.distanceKm, 2) }) : null);
  const czasRuchu = $derived(tel ? czas(tel.activeS) : null);

  // --- podgląd w przeglądarce (?ekrany): lista wszystkich ekranów pod treścią ---
  const listaEkranow = !api.wTauri && new URLSearchParams(location.search).has('ekrany');
  const podglad = (Object.keys(EKRANY) as Trasa[]).filter((x) => x !== 'main');
  function otworz(trasa: Trasa) {
    idz(trasa, trasa === 'profile' ? { profileId: stan.people.activeProfileId } : {});
  }
</script>

<Strona tytul={t('app.nazwa')} {podpis} glowny>
  {#snippet akcje()}
    <div class="akcje" class:ciasno>
      {#if zmianaOsoby && osoba}
        <button
          type="button"
          class="osoba"
          bind:this={przyciskOsoby}
          aria-haspopup="dialog"
          aria-expanded={listaOsob}
          aria-label={t('glowny.pasek.osoba')}
          title={t('glowny.pasek.osoba')}
          onclick={() => (listaOsob = !listaOsob)}
        >
          <Awatar imie={osoba.name} kolor={osoba.color} />
          <Ikona nazwa="w_dol" rozmiar={12} />
        </button>
      {/if}
      {#if pokazAktualizacje}
        <Przycisk
          wariant="ikona"
          ikona="pobierz"
          etykieta={aktualizacja ? t('glowny.pasek.aktualizuj_do', { nowa_wersja: aktualizacja }) : t('glowny.pasek.aktualizuj')}
          disabled={stan.update.busy}
          onclick={() => wykonaj(api.updateCheck(true))}
        />
      {/if}
      <Przycisk wariant="ikona" ikona="zglos" etykieta={t('glowny.pasek.zglos')} onclick={() => idz('report')} />
      {#if stan.account.loggedIn}
        <Przycisk wariant="ikona" ikona="osoby" etykieta={t('glowny.pasek.online')} onclick={() => idz('online')} />
      {/if}
      <Przycisk wariant="ikona" ikona="wykres" etykieta={t('glowny.pasek.historia')} onclick={() => idz('history')} />
      <Przycisk wariant="ikona" ikona="ustawienia" etykieta={t('glowny.pasek.ustawienia')} onclick={() => idz('settings')} />
    </div>
  {/snippet}

  <KartaStanu />

  {#if stan.levels}
    <KartaPoziomu poziomy={stan.levels} />
  {/if}

  <Karta>
    <div class="wskaznik">
      <Wskaznik {uklad} predkosc={tel?.speedKmh ?? 0} {tryb} {aktywna} podpis={tel?.caption ?? ''} />
      <p class="podpis jedna-linia" title={tel?.caption}>{tel?.caption ?? ''}</p>
      <SlotStref {trening} zetony={uklad.zetony} {aktywna} />
    </div>
  </Karta>

  <SiatkaKafelkow>
    <KafelekStatystyki tytul={t('glowny.licznik.kroki')} wartosc={kroki} />
    <KafelekStatystyki tytul={t('glowny.licznik.kadencja_krotko')} etykieta={t('glowny.licznik.kadencja')} wartosc={kadencja} />
    <KafelekStatystyki tytul={t('glowny.licznik.dystans')} wartosc={dystans} />
    <KafelekStatystyki tytul={t('glowny.licznik.czas')} wartosc={czasRuchu} />
  </SiatkaKafelkow>

  <KartaSterowania />

  {#if listaEkranow}
    <nav class="podglad">
      {#each podglad as trasa (trasa)}
        <WierszListy tytul={t(EKRANY[trasa].nazwa)} opis={trasa} onclick={() => otworz(trasa)} />
      {/each}
    </nav>
  {/if}
</Strona>

{#if zmianaOsoby}
  <ListaRozwijana
    bind:otwarta={listaOsob}
    wyzwalacz={przyciskOsoby}
    strona="prawo"
    etykieta={t('glowny.osoby.lista')}
    pozycje={osoby}
    linki={[
      { tytul: t('glowny.osoby.dodaj'), ikona: 'dodaj', onclick: () => idz('wizard', { wizardMode: 'add' }) },
      { tytul: t('glowny.osoby.zarzadzaj'), ikona: 'ustawienia', onclick: () => idz('settings', { kategoria: 'osoby' }) },
    ]}
    onwybierz={(id) => id !== stan.people.activePersonId && wykonaj(api.personSwitch(id))}
  >
    {#snippet poczatek(id)}
      {@const o = stan.people.people.find((x) => x.id === id)}
      {#if o}<Awatar imie={o.name} kolor={o.color} />{/if}
    {/snippet}
  </ListaRozwijana>
{/if}

<style>
  .akcje { display: flex; align-items: center; min-width: 0; }
  /* pełny pasek w wąskim oknie: ikony 40 px zamiast 44 */
  @media (max-width: 479px) {
    .ciasno { --icon-btn: 40px; --control-h: 40px; }
  }
  .osoba {
    display: flex;
    align-items: center;
    gap: 4px;
    height: var(--control-h);
    padding: 0 6px;
    border: 1px solid transparent;
    border-radius: var(--r-md);
    color: var(--text-2);
    transition:
      background-color var(--t-color) linear,
      border-color var(--t-color) linear;
  }
  .osoba:hover { background: var(--surface-2); border-color: var(--control); }

  .wskaznik { display: flex; flex-direction: column; gap: var(--sp-2); min-width: 0; }
  .podpis {
    height: 22px;
    font-size: 0.9375rem;
    font-weight: var(--fw-bold);
    line-height: 22px;
    text-align: center;
  }
  .podglad { display: flex; flex-direction: column; gap: var(--sp-1); }
</style>
