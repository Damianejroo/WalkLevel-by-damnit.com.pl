<script lang="ts">
  // Ekran: Profil gry - nowy (trasa 'profileNew') i edycja (trasa 'profile', parametry.profileId,
  // bez niego - aktywny profil). Wg bieznia/gui/ekrany/profil.py, docs/design.md 3 i 8.14,
  // docs/texts.md 11. Edycja zapisuje się sama (nazwa 500 ms po ostatniej literze i przy wyjściu
  // z pola, reszta od razu); nowy profil powstaje dopiero przyciskiem „Utwórz…” - jedynym
  // akcentem ekranu. Ekran montowany raz: przy każdym wejściu formularz bierze dane od nowa.
  import { untrack } from 'svelte';
  import Strona from '../lib/ui/page.svelte';
  import Karta from '../lib/ui/card.svelte';
  import Przycisk from '../lib/ui/button.svelte';
  import Przelacznik from '../lib/ui/switch.svelte';
  import Segmentowy from '../lib/ui/segmented-control.svelte';
  import Pole from '../lib/ui/text-field.svelte';
  import Ikona from '../lib/ui/icon.svelte';
  import Paleta from './game-profile/color-palette.svelte';
  import SiatkaIkon from './game-profile/icon-grid.svelte';
  import IkonaProfilu from './game-profile/profile-icon.svelte';
  import Zapis from './game-profile/save-status.svelte';
  import * as api from '../lib/api';
  import { stan, ui, wroc, wykonaj, type PropsEkranu } from '../lib/state.svelte';
  import { t } from '../lib/texts';
  import type * as K from '../lib/contract';

  let { parametry, aktywny }: PropsEkranu = $props();

  // Ten sam komponent obsługuje dwie trasy; ekran montuje się w chwili, gdy jego trasa jest bieżąca.
  const nowy = untrack(() => ui.trasa) === 'profileNew';

  const osoba = $derived(stan.people.people.find((o) => o.id === stan.people.activePersonId));
  const aktywnyProfil = $derived(osoba?.profiles.find((p) => p.id === stan.people.activeProfileId));

  // --- edycja: profil ze stanu rdzenia (po usunięciu zostaje ostatni widok na czas wyjścia) ---
  const idProfilu = $derived(nowy ? '' : (parametry.profileId ?? stan.people.activeProfileId));
  const wlasciciel = $derived(stan.people.people.find((o) => o.profiles.some((p) => p.id === idProfilu)));
  const profil = $derived(wlasciciel?.profiles.find((p) => p.id === idProfilu));
  let ostatni = $state<K.GameProfile | null>(null);
  $effect(() => {
    if (profil) ostatni = profil;
  });
  const edytowany = $derived(profil ?? ostatni);

  // --- formularz ---
  let nazwa = $state('');
  let kolor = $state<K.ColorName>('Mięta');
  let kolorZIkony = $state<K.ColorName | null>(null);
  let znak = $state<string | null>(null);
  let wybor = $state<K.GamePick | null>(null);
  let zacznij = $state<'kopia' | 'domyslne'>('kopia');
  let pracuje = $state(false);
  let zapisano = $state(0);
  let zegarNazwy: ReturnType<typeof setTimeout> | undefined;

  function wczytaj(id: string) {
    clearTimeout(zegarNazwy);
    if (nowy) {
      nazwa = '';
      kolor = stan.statics.defaultColor;
      kolorZIkony = null;
      znak = stan.statics.glyphs[0]?.code ?? null;
      wybor = null;
      zacznij = 'kopia';
    } else {
      nazwa = stan.people.people.flatMap((o) => o.profiles).find((p) => p.id === id)?.name ?? '';
    }
  }

  // wejście na ekran (albo inny profil do edycji) - dane od nowa
  $effect(() => {
    if (!aktywny) return;
    const id = idProfilu;
    untrack(() => wczytaj(id));
  });
  // wyjście: zaległa nazwa zapisuje się, niewykorzystana gra znika z plików tymczasowych
  $effect(() => {
    if (aktywny) return;
    untrack(() => {
      void zapiszNazwe();
      porzucWybor();
    });
  });

  function porzucWybor() {
    if (wybor) void wykonaj(api.gamePickDiscard(wybor.pickId));
    wybor = null;
  }

  // --- widok (nowy: szkic; edycja: stan rdzenia) ---
  const widokKolor = $derived(nowy ? kolor : (edytowany?.color ?? kolor));
  const widokZIkony = $derived(nowy ? kolorZIkony : (edytowany?.iconColor ?? null));
  const widokUrl = $derived(nowy ? (znak ? null : (wybor?.icon.url ?? null)) : (edytowany?.icon.url ?? null));
  const widokZnak = $derived(nowy ? (znak ?? wybor?.icon.glyph ?? null) : (edytowany?.icon.glyph ?? null));
  const wybranyZnak = $derived(widokUrl ? null : widokZnak);
  const gra = $derived.by(() => {
    if (nowy) return wybor ? { plik: wybor.exeName, folder: wybor.folderName } : null;
    const czesci = edytowany?.exe?.split(/[\\/]/).filter(Boolean) ?? [];
    return czesci.length ? { plik: czesci[czesci.length - 1], folder: czesci[czesci.length - 2] ?? '' } : null;
  });
  const zGry = $derived(nowy ? !!wybor && znak === null : !!edytowany?.icon.url);

  // --- zapis w edycji ---
  async function zmien(patch: K.ProfilePatch): Promise<boolean> {
    if (nowy || !edytowany) return false;
    const ok = await wykonaj(api.profileUpdate(edytowany.id, patch).then(() => true));
    if (ok) zapisano++;
    return !!ok;
  }

  function wpisanoNazwe() {
    if (nowy) return;
    clearTimeout(zegarNazwy);
    zegarNazwy = setTimeout(zapiszNazwe, 500);
  }

  async function zapiszNazwe() {
    clearTimeout(zegarNazwy);
    if (nowy || !profil) return;
    const n = nazwa.trim();
    if (n && n !== profil.name) await zmien({ name: n });
  }

  // wyjście z pola: pusta nazwa się nie zapisuje, więc wraca zapisana (pole nie zostaje puste)
  async function skonczonoNazwe() {
    await zapiszNazwe();
    if (!nowy && profil && !nazwa.trim()) nazwa = profil.name;
  }

  function zmienKolor(k: K.ColorName) {
    if (nowy) kolor = k;
    else void zmien({ color: k });
  }

  function zmienZnak(z: string) {
    if (nowy) znak = z;
    else void zmien({ glyph: z });
  }

  async function wskazGre(kind: K.GamePickKind) {
    if (pracuje) return;
    pracuje = true;
    const w = await wykonaj(api.gamePick(kind));
    pracuje = false;
    if (!w) return;
    if (nowy) {
      porzucWybor();
      wybor = w;
      nazwa = w.name;
      kolor = w.iconColor;
      kolorZIkony = w.iconColor;
      znak = null;
    } else {
      const bezNazwy = !nazwa.trim();
      if (bezNazwy) nazwa = w.name;
      const ok = await zmien({ pickId: w.pickId, color: w.iconColor, ...(bezNazwy ? { name: w.name } : {}) });
      // nieprzyjęta gra nie zostaje w plikach tymczasowych
      if (!ok) void wykonaj(api.gamePickDiscard(w.pickId));
    }
  }

  async function utworz() {
    if (pracuje) return;
    pracuje = true;
    const id = await wykonaj(
      api.profileCreate({
        name: nazwa.trim() || t('profil.domyslna_nazwa'),
        color: kolor,
        iconColor: kolorZIkony,
        glyph: znak,
        pickId: wybor?.pickId ?? null,
        copyFromActive: zacznij === 'kopia',
      }),
    );
    pracuje = false;
    if (id === undefined) return;
    wybor = null; // ikona należy już do profilu
    wroc();
  }

  async function duplikuj() {
    if (!edytowany || pracuje) return;
    pracuje = true;
    const id = await wykonaj(api.profileDuplicate(edytowany.id));
    pracuje = false;
    if (id !== undefined) wroc();
  }

  async function usun() {
    if (!edytowany || pracuje) return;
    pracuje = true;
    const ok = await wykonaj(api.profileRemove(edytowany.id).then(() => true));
    pracuje = false;
    if (ok) wroc();
  }
</script>

<Strona
  tytul={nowy ? t('profil.tytul_nowy') : t('profil.tytul_edycja')}
  podpis={nowy ? (osoba?.name ?? '') : (edytowany?.name ?? '')}
>
  {#if nowy || edytowany}
    <Karta tytul={t('profil.gra')}>
      {#if gra}
        <div class="gra">
          <span class="plik"><Ikona nazwa="plik" rozmiar={20} /></span>
          <span class="gra-tekst">
            <span class="gra-plik jedna-linia dane" title={gra.plik}>{gra.plik}</span>
            {#if gra.folder}
              <span class="gra-folder jedna-linia" title={gra.folder}>{t('profil.folder_gry', { folder: gra.folder })}</span>
            {/if}
          </span>
          <Przycisk napis={t('profil.zmien')} disabled={pracuje} onclick={() => wskazGre('folder')} />
        </div>
        {#if zGry}<p class="podpowiedz">{t('profil.z_gry')}</p>{/if}
      {:else}
        <p class="podpowiedz">{t('profil.wskaz_opis')}</p>
        <div class="rzad szerokie">
          <Przycisk ikona="folder" napis={t('profil.wskaz_folder')} disabled={pracuje} onclick={() => wskazGre('folder')} />
          <Przycisk
            ikona="plik"
            napis={t('profil.wskaz_plik')}
            title={t('profil.wskaz_podpowiedz')}
            disabled={pracuje}
            onclick={() => wskazGre('file')}
          />
        </div>
      {/if}
    </Karta>

    <Karta tytul={t('profil.nazwa')}>
      <div class="nazwa">
        <IkonaProfilu kolor={widokKolor} znak={widokZnak} url={widokUrl} />
        <div class="pole">
          <Pole
            bind:wartosc={nazwa}
            etykietaDostepna={t('profil.nazwa')}
            podpowiedz={t('profil.nazwa_przyklad')}
            oninput={wpisanoNazwe}
            onblur={skonczonoNazwe}
            onenter={skonczonoNazwe}
          />
        </div>
      </div>
      {#if !nowy && edytowany && stan.app.accountsEnabled}
        <Przelacznik
          wlaczony={edytowany.online}
          tytul={t('profil.online')}
          opis={t('profil.online_opis')}
          onzmiana={(v) => zmien({ online: v })}
        />
      {/if}
    </Karta>

    <Karta tytul={t('profil.kolor')}>
      <Paleta wartosc={widokKolor} zIkony={widokZIkony} etykieta={t('profil.kolor')} onzmiana={zmienKolor} />
    </Karta>

    <Karta tytul={t('profil.ikona')}>
      <SiatkaIkon wartosc={wybranyZnak} etykieta={t('profil.ikona')} onzmiana={zmienZnak} />
    </Karta>

    {#if nowy}
      <Karta tytul={t('profil.zacznij_od')}>
        <Segmentowy
          etykieta={t('profil.zacznij_od')}
          bind:wartosc={zacznij}
          opcje={[
            { wartosc: 'kopia', napis: t('profil.zacznij_kopia') },
            { wartosc: 'domyslne', napis: t('profil.zacznij_domyslne') },
          ]}
        />
        <p class="podpowiedz warianty">
          <span class:ukryty={zacznij !== 'kopia'}>{t('profil.zacznij_opis', { profil: aktywnyProfil?.name ?? '' })}</span>
          <span class:ukryty={zacznij !== 'domyslne'}>{t('profil.zacznij_domyslne_opis')}</span>
        </p>
      </Karta>
      <Przycisk wariant="glowny" szeroki napis={t('profil.utworz')} disabled={pracuje} onclick={utworz} />
    {:else}
      <Zapis licznik={zapisano} />
      <div class="rzad">
        <Przycisk napis={t('profil.duplikuj')} disabled={pracuje} onclick={duplikuj} />
        {#if (wlasciciel?.profiles.length ?? 0) > 1}
          <Przycisk wariant="potwierdz" napis={t('profil.usun')} disabled={pracuje} onclick={usun} />
        {/if}
      </div>
    {/if}
  {/if}
</Strona>

<style>
  .podpowiedz { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-body); }
  .rzad { display: flex; flex-wrap: wrap; gap: var(--sp-2); }
  .rzad > :global(*) { flex: 1 1 160px; }
  /* dłuższe napisy: w wąskim oknie jeden pod drugim zamiast ucinania */
  .szerokie > :global(*) { flex-basis: 200px; }

  .gra {
    display: grid;
    grid-template-columns: 38px minmax(0, 1fr) auto;
    gap: var(--sp-3);
    align-items: center;
  }
  .plik {
    display: grid;
    place-items: center;
    width: 38px;
    height: 38px;
    border-radius: var(--r-md);
    background: var(--surface-2);
    color: var(--text-2);
  }
  .gra-tekst { display: flex; flex-direction: column; min-width: 0; }
  .gra-plik { font-weight: var(--fw-semibold); line-height: var(--lh-tight); }
  .gra-folder { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-body); }

  .nazwa { display: flex; align-items: center; gap: var(--sp-3); min-width: 0; }
  .pole { flex: 1; min-width: 0; }
  /* długa nazwa gry w wąskim oknie: wielokropek, gdy pole nie ma fokusu */
  .pole :global(input) { text-overflow: ellipsis; }

  /* oba opisy w jednej komórce - zmiana wyboru nie zmienia wysokości karty */
  .warianty { display: grid; }
  .warianty > span { grid-area: 1 / 1; }
  .ukryty { visibility: hidden; }
</style>
