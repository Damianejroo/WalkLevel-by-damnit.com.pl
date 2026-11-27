<script lang="ts">
  // Ekran: Kreator osoby (trasa 'wizard'), docs/texts.md 19; zachowanie jak Wizard z
  // bieznia/gui/ekrany/osoba.py. Trzy kroki: nazwa i kolor osoby, pierwszy profil gry, tryb
  // sterowania. Tryb 'first' (pierwsze uruchomienie, bez Wróć) albo 'add' (nowa osoba).
  // Przy każdym wejściu kreator zaczyna od nowa (api.wizardSeed). Akcent: Dalej/Gotowe.
  import { untrack } from 'svelte';
  import Strona from '../lib/ui/page.svelte';
  import Przycisk from '../lib/ui/button.svelte';
  import Pole from '../lib/ui/text-field.svelte';
  import Przelacznik from '../lib/ui/switch.svelte';
  import Ikona, { type NazwaIkony } from '../lib/ui/icon.svelte';
  import Opcja from './wizard/option-row.svelte';
  import Paleta from './wizard/color-palette.svelte';
  import { t, tl, type Klucz } from '../lib/texts';
  import { probka } from '../lib/theme';
  import * as api from '../lib/api';
  import { idz, stan, wroc, wykonaj, type PropsEkranu } from '../lib/state.svelte';
  import type { ColorName, Direction, GamePick, WizardMode } from '../lib/contract';

  let { parametry, aktywny }: PropsEkranu = $props();

  const tryb = $derived<WizardMode>(parametry.wizardMode ?? (stan.people.ready ? 'add' : 'first'));
  const pierwszy = $derived(tryb === 'first');
  const migracja = $derived(pierwszy && stan.app.legacy02);
  const osoba = $derived(stan.people.people.find((o) => o.id === stan.people.activePersonId) ?? null);

  // ---------------- stan kreatora ----------------
  let krok = $state<1 | 2 | 3>(1);
  let nazwa = $state('');
  let nazwaStartowa = '';
  let sugestie = $state<string[]>([]);
  let kolor = $state<ColorName>('Mięta');
  let gra = $state<GamePick | null>(null);
  let nazwaGry = $state('');
  let kolorGry = $state<ColorName>('Mięta');
  let uzyjGry = $state(false);
  let paletaGry = $state(false);
  let kopiuj = $state(false);
  let kierunek = $state<Direction>('keyboard');
  let pracuje = $state(false);

  let postep: HTMLDivElement | undefined;

  /** Nowy krok zaczyna się od góry (pasek postępu i tytuł widoczne także w niskim oknie). */
  function naKrok(n: 1 | 2 | 3) {
    krok = n;
    postep?.closest('.przewijanie')?.scrollTo({ top: 0 });
  }

  /** Nowy kreator przy wejściu na ekran (ekran jest montowany raz i tylko chowany). */
  async function zacznij(m: WizardMode) {
    naKrok(1);
    gra = null;
    nazwaGry = '';
    uzyjGry = false;
    paletaGry = false;
    kopiuj = false;
    pracuje = false;
    // jak w 0.5: przy dodawaniu osoby i po aktualizacji z 0.2 obecny tryb, inaczej Klawisze
    const d = stan.settings.direction;
    const zUstawien = m === 'add' || stan.app.legacy02;
    kierunek = zUstawien ? d : 'keyboard';
    const seed = await wykonaj(api.wizardSeed(m));
    if (!seed) return;
    nazwa = nazwaStartowa = seed.name;
    sugestie = seed.suggestions;
    kolor = seed.color;
  }

  /** Wskazana, ale niezapisana gra - tymczasowa ikona do usunięcia. */
  function porzucGre() {
    if (gra) void wykonaj(api.gamePickDiscard(gra.pickId));
    gra = null;
  }

  let byl = false;
  $effect(() => {
    const a = aktywny;
    const m = tryb;
    untrack(() => {
      if (a && !byl) void zacznij(m);
      if (!a && byl) porzucGre();
      byl = a;
    });
  });

  // ---------------- krok 1: nazwa i kolor ----------------
  const inicjal = $derived((nazwa.trim() || '?').charAt(0).toLocaleUpperCase('pl'));

  async function losuj() {
    const n = await wykonaj(api.nameRandom());
    if (n) nazwa = n;
  }

  // ---------------- krok 2: pierwszy profil gry ----------------
  async function wskazGre() {
    const g = await wykonaj(api.gamePick('folder'));
    if (!g) return;
    if (gra) void wykonaj(api.gamePickDiscard(gra.pickId));
    gra = g;
    nazwaGry = g.name;
    kolorGry = g.iconColor;
    uzyjGry = true;
    paletaGry = false;
  }

  const nazwaProfilu = $derived(uzyjGry && gra ? nazwaGry.trim() || gra.name : t('nazwy.profil_domyslny'));
  const [graTytul, graOpis] = $derived(tl('kreator.gra.wskaz'));
  const [domTytul, domOpis] = $derived(tl(migracja ? 'kreator.gra.zostaw' : 'kreator.gra.domyslny'));

  // ---------------- krok 3: tryb sterowania ----------------
  const TRYBY: Array<{ kierunek: Direction; ikona: NazwaIkony; klucz: Klucz }> = [
    { kierunek: 'keyboard', ikona: 'klawiatura', klucz: 'kreator.tryb.klawisze' },
    { kierunek: 'gamepad', ikona: 'pad', klucz: 'kreator.tryb.pad' },
    { kierunek: 'treadmill', ikona: 'chod', klucz: 'kreator.tryb.chodzenie' },
    { kierunek: 'training', ikona: 'trening', klucz: 'kreator.tryb.trening' },
  ];

  // ---------------- nawigacja ----------------
  const tytul = $derived(
    krok === 1
      ? t(migracja ? 'kreator.imie.tytul_migracja' : 'kreator.imie.tytul')
      : krok === 2 ? t('kreator.gra.tytul') : t('kreator.tryb.tytul'),
  );

  function wstecz() {
    if (krok > 1) {
      naKrok(krok === 3 ? 2 : 1);
      return;
    }
    porzucGre();
    wroc();
  }

  async function dalej() {
    if (krok === 1) {
      nazwa = nazwa.trim() || nazwaStartowa || t('kreator.imie.domyslna');
      naKrok(2);
    } else if (krok === 2) {
      naKrok(3);
    } else {
      await zakoncz();
    }
  }

  async function zakoncz() {
    if (pracuje) return;
    pracuje = true;
    const g = uzyjGry && gra ? { pickId: gra.pickId, name: nazwaGry.trim() || gra.name, color: kolorGry } : null;
    const ok = await wykonaj(
      api.wizardFinish({
        mode: tryb,
        name: nazwa.trim() || t('kreator.imie.domyslna'),
        color: kolor,
        copyProfiles: tryb === 'add' && kopiuj,
        game: g,
        direction: kierunek,
      }).then(() => true),
    );
    pracuje = false;
    if (!ok) return;
    gra = null; // ikona należy już do nowego profilu
    idz('main');
  }
</script>

<Strona {tytul} podpis={t('kreator.krok', { liczba: krok })} glowny={pierwszy}>
  <div class="postep" aria-hidden="true" bind:this={postep}>
    {#each [1, 2, 3] as n (n)}<span class="odcinek" class:zrobiony={n <= krok}></span>{/each}
  </div>

  {#if krok === 1}
    <p class="opis">{t(migracja ? 'kreator.imie.opis_migracja' : 'kreator.imie.opis')}</p>
    <div class="awatar" style:--probka={probka(kolor, stan.statics.palette)} aria-hidden="true">{inicjal}</div>
    <div class="nazwa">
      <Pole bind:wartosc={nazwa} etykietaDostepna={t('kreator.imie.pole')} maxlength={40} onenter={() => void dalej()} />
      <Przycisk wariant="ikona" ikona="losuj" etykieta={t('kreator.imie.losuj')} onclick={() => void losuj()} />
    </div>
    {#if sugestie.length}
      <p class="maly">{t('kreator.imie.propozycje')}</p>
      <div class="sugestie">
        {#each sugestie as s (s)}
          <span class="sugestia"><Przycisk napis={s} szeroki onclick={() => (nazwa = s)} /></span>
        {/each}
      </div>
    {/if}
    <h2 class="sekcja">{t('kreator.imie.kolor')}</h2>
    <Paleta paleta={stan.statics.palette} wartosc={kolor} etykieta={t('kreator.imie.kolor')} onzmiana={(k) => (kolor = k)} />
  {:else if krok === 2}
    <p class="opis">{t('kreator.gra.opis')}</p>
    <div class="opcje" role="radiogroup" aria-label={t('kreator.gra.tytul')}>
      <Opcja
        ikona="folder"
        tytul={graTytul}
        opis={graOpis}
        wybrany={uzyjGry && !!gra}
        onclick={() => (gra ? (uzyjGry = true) : void wskazGre())}
      >
        {#if gra}
          <div class="gra">
            <span class="plik">
              <span class="ikona-gry" style:--probka={probka(kolorGry, stan.statics.palette)}>
                {#if gra.icon.url}
                  <img src={gra.icon.url} alt="" width="28" height="28" />
                {:else}
                  <Ikona znak={gra.icon.glyph ?? stan.statics.defaultGlyph} rozmiar={20} />
                {/if}
              </span>
              <span class="plik-tekst">
                <span class="plik-nazwa jedna-linia" title={gra.exe}>{gra.exeName}</span>
                <span class="maly jedna-linia" title={gra.exe}>…\{gra.folderName}</span>
              </span>
            </span>
            <span class="gra-akcje">
              <Przycisk
                etykieta={t('kolor.dostepnosc', { kolor: kolorGry })}
                title={t('kolor.dostepnosc', { kolor: kolorGry })}
                onclick={() => (paletaGry = !paletaGry)}
              >
                <span class="kolor-przycisk"><span class="kropka-koloru" style:--probka={probka(kolorGry, stan.statics.palette)}></span>{t('kolor.przycisk')}</span>
              </Przycisk>
              <Przycisk napis={t('kreator.gra.zmien')} onclick={() => void wskazGre()} />
            </span>
          </div>
          <Pole
            bind:wartosc={nazwaGry}
            etykietaDostepna={t('kreator.gra.nazwa_dostepnosc')}
            podpowiedz={t('kreator.gra.nazwa')}
            maxlength={stan.statics.account.gameNameMax}
          />
          {#if paletaGry}
            <div class="rozwin">
              <Paleta
                paleta={stan.statics.palette}
                wartosc={kolorGry}
                etykieta={t('kolor.przycisk')}
                dopisek={kolorGry === gra.iconColor ? t('kolor.z_ikony') : t('kolor.recznie', { kolor: gra.iconColor })}
                onzmiana={(k) => (kolorGry = k)}
              />
            </div>
          {:else if kolorGry === gra.iconColor}
            <p class="maly">{t('kreator.gra.kolor_z_ikony', { kolor: kolorGry })}</p>
          {/if}
        {/if}
      </Opcja>
      <Opcja ikona="chod" tytul={domTytul} opis={domOpis} wybrany={!(uzyjGry && gra)} onclick={() => (uzyjGry = false)} />
    </div>
    {#if tryb === 'add' && osoba}
      <div class="kopiuj">
        <Przelacznik
          tytul={t('kreator.gra.kopiuj_od', { osoba: osoba.name })}
          opis={t('kreator.gra.kopiuj_opis')}
          bind:wlaczony={kopiuj}
        />
      </div>
    {/if}
  {:else}
    <p class="opis">{t('kreator.tryb.opis', { profil: nazwaProfilu })}</p>
    <div class="opcje" role="radiogroup" aria-label={t('kreator.tryb.tytul')}>
      {#each TRYBY as o (o.kierunek)}
        {@const [nazwaTrybu, opisTrybu] = tl(o.klucz)}
        <Opcja ikona={o.ikona} tytul={nazwaTrybu} opis={opisTrybu} wybrany={kierunek === o.kierunek} onclick={() => (kierunek = o.kierunek)} />
      {/each}
    </div>
  {/if}

  <div class="nawigacja">
    {#if krok > 1 || !pierwszy}
      <span class="wstecz">
        <Przycisk napis={krok > 1 ? t('kreator.wstecz') : t('kreator.anuluj')} szeroki onclick={wstecz} />
      </span>
    {/if}
    <span class="dalej">
      <Przycisk
        wariant="glowny"
        napis={krok === 3 ? t('kreator.gotowe') : t('kreator.dalej')}
        napisy={[t('kreator.dalej'), t('kreator.gotowe')]}
        szeroki
        disabled={pracuje}
        onclick={() => void dalej()}
      />
    </span>
  </div>
</Strona>

<style>
  .postep {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 6px;
  }
  .odcinek {
    height: 6px;
    border-radius: var(--r-xs);
    background: var(--surface-3);
  }
  .odcinek.zrobiony { background: var(--ink); }

  .opis { font-size: var(--fs-md); color: var(--text-2); }
  .maly { font-size: var(--fs-sm); color: var(--text-2); }
  .sekcja { margin-top: var(--sp-2); font-size: var(--fs-base); font-weight: var(--fw-bold); }

  .awatar {
    display: grid;
    place-items: center;
    align-self: center;
    width: 76px;
    height: 76px;
    border-radius: var(--r-round);
    background: var(--probka);
    color: #0f1f1a;              /* tekst na próbce koloru (design.md 2.4) */
    font-family: var(--font-num);
    font-size: 2rem;
    font-weight: var(--fw-bold);
    line-height: 1;
    user-select: none;
  }
  .nazwa {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: var(--sp-2);
  }
  /* równe kolumny: 3 w rzędzie albo 2 + 1 bez rozciągania ostatniej na całą szerokość */
  .sugestie {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(170px, 1fr));
    gap: var(--sp-2);
    margin-top: calc(var(--sp-1) * -1);
  }
  .sugestia { display: flex; min-width: 0; }
  .sugestia :global(.przycisk) { font-weight: var(--fw-regular); }

  .opcje { display: flex; flex-direction: column; gap: var(--sp-2); }

  .gra {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2) var(--sp-3);
    min-width: 0;
  }
  .plik { display: flex; align-items: center; gap: var(--sp-3); flex: 1 1 160px; min-width: 0; }
  .ikona-gry {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    flex: none;
    border-radius: var(--r-md);
    background: var(--surface-3);
    color: var(--probka);
  }
  .plik-tekst { display: flex; flex-direction: column; min-width: 0; }
  .plik-nazwa { font-weight: var(--fw-semibold); line-height: var(--lh-tight); }
  .gra-akcje { display: flex; gap: var(--sp-2); flex: none; }
  .kolor-przycisk { display: inline-flex; align-items: center; gap: var(--sp-2); }
  .kropka-koloru {
    width: 14px;
    height: 14px;
    flex: none;
    border-radius: var(--r-round);
    background: var(--probka);
  }
  .rozwin { animation: wl-rozwin var(--t-in) var(--ease-enter); }

  .kopiuj {
    padding: 0 14px;
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: var(--surface);
  }

  .nawigacja {
    display: flex;
    gap: var(--sp-2);
    margin-top: var(--sp-2);
  }
  .wstecz { display: flex; flex: 1 1 0; min-width: 0; }
  .dalej { display: flex; flex: 2 1 0; min-width: 0; }
</style>
