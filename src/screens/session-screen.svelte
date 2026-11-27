<script lang="ts">
  // Ekran: Szczegóły sesji (trasa 'session'), docs/texts.md 20.1. Dane z history_session -
  // przy wejściu i ponownie po zmianie history.stamp (np. ukrycie online). Tytuł „Sesja”, data
  // i godziny jako podpis. Usunięcie dwoma kliknięciami, potem powrót do Historii (toast wysyła rdzeń).
  // Akcentu brak: jedyna mocna akcja (usuń) ma kolor ostrzeżenia, nie akcent.
  import { untrack } from 'svelte';
  import Strona from '../lib/ui/page.svelte';
  import Karta from '../lib/ui/card.svelte';
  import Przycisk from '../lib/ui/button.svelte';
  import Przelacznik from '../lib/ui/switch.svelte';
  import KafelekStatystyki from '../lib/ui/stat-tile.svelte';
  import IkonaProfilu from './history/profile-icon.svelte';
  import { czas, t } from '../lib/texts';
  import { liczba, zJednostka } from '../lib/format';
  import { stan, succeeded, wroc, wykonaj, type PropsEkranu } from '../lib/state.svelte';
  import * as api from '../lib/api';
  import type * as K from '../lib/contract';
  import { data, godzina } from './history/dates';

  let { parametry, aktywny }: PropsEkranu = $props();

  let sesja = $state.raw<K.SessionDetail | null>(null);
  let wczytana = $state(false);

  /** Id sesji z wejścia; w podglądzie bez parametru (?ekran=session) - najnowsza sesja z atrapy. */
  async function ktora(): Promise<string | undefined> {
    if (parametry.sessionId || api.wTauri) return parametry.sessionId;
    const v = await api.historyQuery({ scope: 'me', range: 'all' });
    return v.recent[0]?.id;
  }

  let numer = 0;
  /** Właśnie usunięta sesja - przy wyjściu nie pokazujemy „Tej sesji już nie ma”. */
  let usunieta = '';
  $effect(() => {
    if (!aktywny) return;
    const wejscie = parametry.sessionId;
    void stan.history.stamp;
    // inna sesja niż ostatnio pokazana - bez pokazywania starej do czasu odpowiedzi
    untrack(() => {
      if (wejscie && sesja && sesja.id !== wejscie) {
        sesja = null;
        wczytana = false;
      }
    });
    const moj = ++numer;
    (async () => {
      const id = await ktora();
      const s = id ? await wykonaj(api.historySession(id)) : null;
      if (moj !== numer || s === undefined || (s === null && id === usunieta)) return;
      sesja = s;
      wczytana = true;
    })();
  });

  const kiedy = $derived(
    sesja ? t('sesja.kiedy', { data: data(sesja.start), godzina: godzina(sesja.start), godzina_koniec: godzina(sesja.end) }) : '',
  );

  const zrodlo = $derived.by(() => {
    if (!sesja) return '';
    if (sesja.source === 'symulator') return t('sesja.skad.symulator');
    const czesci = [sesja.deviceModel || t('sesja.skad.bieznia')];
    if (sesja.firmware) czesci.push(t('sesja.skad.oprogramowanie', { wersja_bt: sesja.firmware }));
    if (sesja.appVersion) czesci.push(t('sesja.skad.aplikacja', { wersja: sesja.appVersion }));
    return czesci.join(' · ');
  });

  const stanOnline = $derived.by(() => {
    const o = sesja?.online;
    if (!o) return '';
    const s = o.hidden
      ? t('sesja.online.ukryta')
      : o.rejected
        ? t('sesja.online.odrzucona')
        : o.sent ? t('sesja.online.wyslana') : t('sesja.online.czeka');
    return t('sesja.online.etykieta', { stan: s });
  });

  /** Kto widzi sesję - ustawienie konta, gdy sesja należy do wybranej osoby z kontem. */
  const kto = $derived.by(() => {
    const a = stan.account;
    if (!sesja?.online || sesja.online.hidden || !a.loggedIn || a.personId !== sesja.personId) return '';
    const nazwa = { prywatna: 'moje.widocznosc.ja', znajomi: 'moje.widocznosc.znajomi', publiczna: 'moje.widocznosc.wszyscy' } as const;
    return t('sesja.online.kto', { kto: t(nazwa[a.visibility]) });
  });

  function ukryj(ukryta: boolean) {
    return sesja ? succeeded(api.historySessionHideOnline(sesja.id, ukryta)) : false;
  }

  async function usun() {
    if (!sesja) return;
    const id = sesja.id;
    usunieta = id;
    const ok = await wykonaj(api.historySessionDelete(id).then(() => true));
    if (ok) wroc();
    else usunieta = '';
  }
</script>

<Strona tytul={t('sesja.tytul')} podpis={kiedy}>
  {#if sesja}
    <div class="kto">
      <IkonaProfilu ikona={sesja.icon} rozmiar={38} />
      <div class="kto-tekst">
        <span class="profil jedna-linia dane" title={sesja.profileName}>{sesja.profileName}</span>
        <span class="osoba jedna-linia dane" title={sesja.personName}>{sesja.personName}</span>
      </div>
    </div>

    <div class="kafelki">
      <div class="siatka">
        <KafelekStatystyki tytul={t('sesja.kroki')} wartosc={liczba(sesja.steps)} />
        <KafelekStatystyki tytul={t('sesja.dystans')} wartosc={zJednostka(sesja.distanceKm, 'km', 2)} />
        <KafelekStatystyki tytul={t('sesja.czas')} wartosc={czas(sesja.activeS)} />
        <KafelekStatystyki tytul={t('sesja.srednio')} wartosc={zJednostka(sesja.avgKmh, 'km/h', 1)} />
        <KafelekStatystyki
          tytul={t('sesja.kadencja')}
          etykieta={t('glowny.licznik.kadencja')}
          wartosc={sesja.cadence === null ? null : liczba(sesja.cadence)}
        />
        <KafelekStatystyki tytul={t('sesja.wykres')} wartosc={czas(sesja.wholeS)} />
      </div>
    </div>

    <Karta tytul={t('sesja.skad')}>
      <p class="opis dane">{zrodlo}</p>
      {#if sesja.source === 'symulator'}
        <p class="opis">{t('sesja.online.symulator')}</p>
      {/if}
    </Karta>

    {#if sesja.online}
      <Karta>
        <Przelacznik
          wlaczony={sesja.online.hidden}
          tytul={t('sesja.online.ukryj')}
          opis={stanOnline}
          onzmiana={ukryj}
        />
        {#if (sesja.online.rejected && !sesja.online.hidden) || kto}
          <div class="dopiski">
            {#if sesja.online.rejected && !sesja.online.hidden}
              <p class="opis dane">{sesja.online.rejected}</p>
            {/if}
            {#if kto}<p class="opis">{kto}</p>{/if}
          </div>
        {/if}
      </Karta>
    {/if}

    <Karta tytul={t('sesja.usun.tytul')}>
      <p class="opis">{t(sesja.linked && sesja.source === 'bieznia' ? 'sesja.usun.opis' : 'sesja.usun.opis_lokalnie')}</p>
      <div class="akcja">
        <Przycisk wariant="potwierdz" napis={t('sesja.usun.przycisk')} onclick={usun} />
      </div>
    </Karta>
  {:else if wczytana}
    <p class="info">{t('sesja.brak')}</p>
  {/if}
</Strona>

<style>
  .info { color: var(--text-2); font-size: var(--fs-md); }
  .kto {
    display: grid;
    grid-template-columns: 38px minmax(0, 1fr);
    gap: var(--sp-3);
    align-items: center;
    min-width: 0;
    padding-block: var(--sp-1);
  }
  .kto-tekst { display: flex; flex-direction: column; min-width: 0; }
  .profil { font-size: var(--fs-lg); font-weight: var(--fw-bold); line-height: var(--lh-tight); }
  .osoba { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-tight); }

  /* 6 kafelków: 3 × 2, gdy „1 h 05 min” mieści się w trzeciej części (próg w rem - rośnie z
     „Dużym tekstem”), węziej 2 × 3; wysokość kafelka bez zmian */
  .kafelki { container-type: inline-size; min-width: 0; }
  .siatka { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: var(--sp-2); }
  @container (min-width: 25rem) {
    .siatka { grid-template-columns: repeat(3, minmax(0, 1fr)); }
  }

  .opis { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-body); }
  .akcja { display: flex; flex-wrap: wrap; }
  /* dopiski pod opisem przełącznika: odstęp jak między liniami opisu (wiersz ma 10 px dołu) */
  .dopiski {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
    margin-block-start: calc(-1 * var(--sp-3));
    padding-block-end: var(--sp-1);
  }
</style>
