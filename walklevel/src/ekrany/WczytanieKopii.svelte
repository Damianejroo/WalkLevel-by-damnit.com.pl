<script lang="ts">
  // Ekran: Kopia danych - wczytanie (trasa 'import'), docs/teksty.md 17.2, kontrakt 6.10.
  // Etapy: wybór pliku (okno systemowe otwiera się samo przy wejściu) -> podgląd (osoby, co
  // wczytać, osoby, które już są: połącz / dodaj jako nową, zachowanie moich ustawień) -> wynik.
  // Przy wyborze i po wczytaniu: przywrócenie danych sprzed ostatniego wczytania (przytrzymanie).
  // Akcent: tylko główna akcja etapu („Wybierz plik kopii…”, „Wczytaj”, „Gotowe”).
  import { tick, untrack } from 'svelte';
  import Strona from '../lib/ui/Strona.svelte';
  import Karta from '../lib/ui/Karta.svelte';
  import Przycisk from '../lib/ui/Przycisk.svelte';
  import Segmentowy from '../lib/ui/Segmentowy.svelte';
  import Przelacznik from '../lib/ui/Przelacznik.svelte';
  import Ikona from '../lib/ui/Ikona.svelte';
  import KafelekStatystyki from '../lib/ui/KafelekStatystyki.svelte';
  import SiatkaKafelkow from '../lib/ui/SiatkaKafelkow.svelte';
  import PoleWyboru from './Kopia/PoleWyboru.svelte';
  import Awatar from './Kopia/Awatar.svelte';
  import * as api from '../lib/api';
  import { t, tl } from '../lib/teksty';
  import { liczba } from '../lib/format';
  import { stan, wroc, wykonaj, type PropsEkranu } from '../lib/stan.svelte';
  import type { ImportChoice, ImportPreview, ImportReport } from '../lib/typy';

  let { aktywny }: PropsEkranu = $props();

  type Etap = 'wybor' | 'podglad' | 'wynik';
  type Tryb = 'polacz' | 'nowa';

  let etap = $state<Etap>('wybor');
  let podglad = $state<ImportPreview | null>(null);
  let raport = $state<ImportReport | null>(null);
  let wybraneOsoby = $state<string[]>([]);
  let jakoNowe = $state<string[]>([]);
  let ustawienia = $state(true);
  let wyniki = $state(true);
  let zachowaj = $state(false);
  let otwiera = $state(false);
  let wczytuje = $state(false);
  let przywraca = $state(false);
  let etapEl = $state<HTMLElement>();

  // Przycisk, który miał fokus, znika ze zmianą etapu - fokus z klawiatury trafia na pierwszą
  // akcję nowego etapu zamiast na początek dokumentu.
  async function fokusNaEtap() {
    await tick();
    if (document.activeElement === document.body) etapEl?.querySelector<HTMLElement>('button:not(:disabled)')?.focus();
  }

  // fokus=false przy samoczynnym otwarciu okna przy wejściu (nic nie miało fokusu na tym ekranie)
  async function otworz(fokus = true) {
    if (otwiera) return;
    otwiera = true;
    const p = await wykonaj(api.backupImportOpen());
    otwiera = false;
    if (!p) return; // okno zamknięte bez wyboru albo zły plik (komunikat już jest) - zostaje, co było
    podglad = p;
    wybraneOsoby = p.people.map((o) => o.id);
    jakoNowe = [];
    ustawienia = p.hasSettings;
    wyniki = p.hasResults;
    zachowaj = false;
    raport = null;
    etap = 'podglad';
    if (fokus) void fokusNaEtap();
  }

  // Każde wejście na ekran: od początku i od razu okno wyboru pliku („Wczytaj kopię…”).
  let byl = false;
  $effect(() => {
    const teraz = aktywny;
    untrack(() => {
      if (teraz && !byl) {
        etap = 'wybor';
        podglad = null;
        raport = null;
        void otworz(false);
      }
    });
    byl = teraz;
  });

  const plikOpis = $derived(
    podglad
      ? t('kopia.import.plik_opis', {
          wersja: podglad.appVersion,
          liczba: podglad.people.length,
          profile: podglad.people.reduce((s, o) => s + o.profiles, 0),
          sesje: podglad.people.reduce((s, o) => s + o.sessions, 0),
        })
      : '',
  );
  const istniejace = $derived(podglad?.people.filter((o) => o.exists) ?? []);
  const laczone = $derived(istniejace.filter((o) => wybraneOsoby.includes(o.id) && !jakoNowe.includes(o.id)));
  // tytuł karty w jednej linii; nazwy osób są w wierszach pod opisem
  const tytulKonfliktu = $derived(t(istniejace.length === 1 ? 'kopia.import.osoba_jest_tytul' : 'kopia.import.osoby_sa'));
  const brakOsoby = $derived(wybraneOsoby.length === 0);

  function przelaczOsobe(id: string, wlacz: boolean) {
    wybraneOsoby = wlacz ? [...wybraneOsoby.filter((x) => x !== id), id] : wybraneOsoby.filter((x) => x !== id);
  }
  function ustawTryb(id: string, tryb: Tryb) {
    jakoNowe = tryb === 'nowa' ? [...jakoNowe.filter((x) => x !== id), id] : jakoNowe.filter((x) => x !== id);
  }

  async function wczytaj() {
    if (!podglad || brakOsoby || wczytuje) return;
    const wybor: ImportChoice = {
      people: [...wybraneOsoby],
      settings: podglad.hasSettings && ustawienia,
      results: podglad.hasResults && wyniki,
      newPerson: jakoNowe.filter((id) => wybraneOsoby.includes(id)),
      keepMySettings: ustawienia && zachowaj && laczone.length > 0,
    };
    wczytuje = true;
    const r = await wykonaj(api.backupImportApply(wybor));
    wczytuje = false;
    if (!r) return;
    raport = r;
    etap = 'wynik';
    void fokusNaEtap();
  }

  async function przywroc() {
    if (przywraca) return;
    przywraca = true;
    const ok = await wykonaj(api.backupRestoreLatest().then(() => true));
    przywraca = false;
    if (ok) wroc();
  }

  const [ustawieniaTytul, ustawieniaOpis] = tl('kopia.import.ustawienia');
  const [wynikiTytul, wynikiOpis] = tl('kopia.import.wyniki');
  const [zachowajTytul, zachowajOpis] = tl('kopia.import.zachowaj');
  const tryby: { wartosc: Tryb; napis: string }[] = [
    { wartosc: 'polacz', napis: t('kopia.import.polacz') },
    { wartosc: 'nowa', napis: t('kopia.import.jako_nowa') },
  ];
</script>

<Strona tytul={t('kopia.import.tytul')}>
  <div class="etap" bind:this={etapEl}>
    {#if etap === 'wybor'}
      <Karta tytul={t('kopia.import.plik')}>
        <p class="opis">{t('kopia.import.wybierz_opis')}</p>
        <div class="prawo">
          <span class="akcja">
            <Przycisk wariant="glowny" szeroki napis={t('kopia.import.wybierz')} disabled={otwiera} onclick={() => otworz()} />
          </span>
        </div>
      </Karta>
    {:else if etap === 'podglad' && podglad}
      <Karta tytul={t('kopia.import.plik')}>
        <div class="plik">
          <span class="znak-pliku"><Ikona nazwa="plik" rozmiar={20} /></span>
          <span class="plik-tekst">
            <span class="nazwa-pliku jedna-linia dane" title={podglad.file}>{podglad.file}</span>
            <span class="opis">{plikOpis}</span>
          </span>
          <span class="inny">
            <Przycisk napis={t('kopia.import.inny_plik')} disabled={otwiera || wczytuje} onclick={() => otworz()} />
          </span>
        </div>
      </Karta>

      <Karta tytul={t('kopia.import.co')}>
        <div class="lista">
          {#each podglad.people as o (o.id)}
            <PoleWyboru
              zaznaczenie={wybraneOsoby.includes(o.id) ? 'tak' : 'nie'}
              tytul={o.name}
              opis={t('kopia.import.osoba_opis', {
                stan: t(o.exists ? 'kopia.import.osoba_jest' : 'kopia.import.osoba_nowa'),
                profile: o.profiles,
                sesje: o.sessions,
              })}
              onzmiana={(w) => przelaczOsobe(o.id, w)}
            >
              {#snippet poczatek()}<Awatar nazwa={o.name} kolor={o.color} />{/snippet}
            </PoleWyboru>
          {/each}
          {#if podglad.hasSettings || podglad.hasResults}
            <div class="kreska" role="presentation"></div>
          {/if}
          {#if podglad.hasSettings}
            <PoleWyboru zaznaczenie={ustawienia ? 'tak' : 'nie'} tytul={ustawieniaTytul} opis={ustawieniaOpis} onzmiana={(w) => (ustawienia = w)} />
          {/if}
          {#if podglad.hasResults}
            <PoleWyboru zaznaczenie={wyniki ? 'tak' : 'nie'} tytul={wynikiTytul} opis={wynikiOpis} onzmiana={(w) => (wyniki = w)} />
          {/if}
        </div>
      </Karta>

      {#if istniejace.length > 0}
        <Karta tytul={tytulKonfliktu}>
          <p class="opis">{t('kopia.import.polacz_opis')}</p>
          <p class="opis">{t('kopia.import.jako_nowa_opis')}</p>
          {#each istniejace as o (o.id)}
            {@const wybrana = wybraneOsoby.includes(o.id)}
            <div class="konflikt" class:pominieta={!wybrana}>
              <span class="kto">
                <Awatar nazwa={o.name} kolor={o.color} rozmiar={28} />
                <span class="nazwa jedna-linia dane" title={o.name}>{o.name}</span>
              </span>
              <span class="tryb">
                <Segmentowy
                  opcje={tryby}
                  wartosc={jakoNowe.includes(o.id) ? 'nowa' : 'polacz'}
                  etykieta={t('kopia.import.konflikt_dostepnosc', { osoba: o.name })}
                  disabled={!wybrana}
                  onzmiana={(w) => ustawTryb(o.id, w)}
                />
              </span>
            </div>
          {/each}
          {#if podglad.hasSettings}
            <div class="kreska" role="presentation"></div>
            <Przelacznik
              bind:wlaczony={zachowaj}
              tytul={zachowajTytul}
              opis={zachowajOpis}
              disabled={!ustawienia || laczone.length === 0}
            />
          {/if}
        </Karta>
      {/if}

      <div class="stopka">
        <p class="podsumowanie" class:blad={brakOsoby} aria-live="polite">
          {brakOsoby ? t('kopia.blad.brak_osoby') : t('kopia.import.kopia_przed')}
        </p>
        <span class="akcja">
          <Przycisk wariant="glowny" szeroki napis={t('kopia.import.wczytaj')} disabled={brakOsoby || wczytuje || otwiera} onclick={wczytaj} />
        </span>
      </div>
    {:else if etap === 'wynik' && raport}
      <Karta tytul={t('kopia.import.wynik.tytul')}>
        <SiatkaKafelkow>
          <KafelekStatystyki tytul={t('kopia.import.wynik.nowe_osoby')} wartosc={liczba(raport.peopleAdded)} />
          <KafelekStatystyki tytul={t('kopia.import.wynik.polaczone')} wartosc={liczba(raport.peopleMerged)} />
          <KafelekStatystyki tytul={t('kopia.import.wynik.nowe_profile')} wartosc={liczba(raport.profilesAdded)} />
          <KafelekStatystyki tytul={t('kopia.import.wynik.nowe_sesje')} wartosc={liczba(raport.sessionsAdded)} />
        </SiatkaKafelkow>
        <div class="prawo">
          <span class="akcja">
            <Przycisk wariant="glowny" szeroki napis={t('kopia.import.zakoncz')} onclick={wroc} />
          </span>
        </div>
      </Karta>
    {/if}
  </div>

  {#if etap !== 'podglad' && stan.app?.latestBackup}
    <Karta tytul={t('kopia.import.przed.tytul')}>
      <p class="opis">{t('kopia.import.przed.opis')}</p>
      <div class="prawo">
        <span class="akcja">
          <Przycisk wariant="przytrzymaj" szeroki napis={t('kopia.import.przed.przywroc')} disabled={przywraca} onclick={przywroc} />
        </span>
      </div>
    </Karta>
  {/if}
</Strona>

<style>
  /* tylko do szukania akcji etapu - karty zostają w kolumnie strony */
  .etap { display: contents; }
  .opis { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-body); }

  .plik {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2) var(--sp-3);
    min-width: 0;
  }
  .znak-pliku {
    display: grid;
    place-items: center;
    flex: none;
    width: 38px;
    height: 38px;
    border-radius: var(--r-md);
    background: var(--surface-2);
    color: var(--text-2);
  }
  .plik-tekst { display: flex; flex-direction: column; flex: 1 1 200px; min-width: 0; }
  .inny { display: flex; margin-inline-start: auto; }
  .nazwa-pliku { font-weight: var(--fw-semibold); line-height: var(--lh-tight); }

  /* wiersze z polami mają własny odstęp wewnętrzny - wyrównanie do krawędzi tytułu karty */
  .lista { display: flex; flex-direction: column; margin-inline: calc(-1 * var(--sp-2)); }
  .kreska { height: 1px; margin-block: var(--sp-1); background: var(--line); }
  .lista .kreska { margin-inline: var(--sp-2); }
  .opis + .konflikt { margin-top: var(--sp-1); }

  .konflikt {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2) var(--sp-3);
    min-width: 0;
  }
  /* nazwa osoby dostaje część wolnego miejsca (nie skraca się do kilku liter obok przełącznika);
     w wąskim oknie przełącznik przechodzi pod nazwę na całą szerokość */
  .kto { display: flex; align-items: center; gap: var(--sp-2); flex: 1 1 200px; min-width: 0; }
  .nazwa { font-weight: var(--fw-semibold); }
  .pominieta .kto { color: var(--text-2); }
  .pominieta .tryb { opacity: 0.6; }
  .tryb { flex: 1 1 300px; min-width: 0; }

  .stopka {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2) var(--sp-4);
  }
  /* tekst zabiera wolne miejsce w rzędzie; po zawinięciu przycisk ma całą szerokość */
  .podsumowanie { flex: 999 1 220px; min-width: 0; }
  .podsumowanie { font-size: var(--fs-sm); color: var(--text-2); }
  .podsumowanie.blad { color: var(--text); font-weight: var(--fw-semibold); font-size: var(--fs-md); }
  .akcja { display: flex; flex: 1 0 auto; min-width: 0; }

  /* sam przycisk: z prawej, w wąskiej karcie na całą szerokość */
  .prawo { container-type: inline-size; min-width: 0; }
  .prawo .akcja { width: max-content; max-width: 100%; margin-inline-start: auto; }
  @container (max-width: 439px) {
    .prawo .akcja { width: 100%; }
  }
</style>
