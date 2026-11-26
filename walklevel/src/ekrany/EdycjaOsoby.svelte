<script lang="ts">
  // Ekran: Edycja osoby (trasa 'person', parametry.personId; bez niego - wybrana osoba).
  // Wg bieznia/gui/ekrany/osoba.py i docs/teksty.md 19: nazwa (zapis 500 ms po ostatniej literze
  // i przy wyjściu z pola, kostka losuje), kolor osoby, konto online tej osoby, usunięcie
  // (przytrzymanie, nie ostatniej osoby). Wszystko zapisuje się samo - ekran nie ma akcentu.
  import { untrack } from 'svelte';
  import Strona from '../lib/ui/Strona.svelte';
  import Karta from '../lib/ui/Karta.svelte';
  import Przycisk from '../lib/ui/Przycisk.svelte';
  import Pole from '../lib/ui/Pole.svelte';
  import Awatar from './EdycjaOsoby/Awatar.svelte';
  import Paleta from './ProfilGry/Paleta.svelte';
  import Zapis from './ProfilGry/Zapis.svelte';
  import * as api from '../lib/api';
  import { idz, stan, wroc, wykonaj, type PropsEkranu } from '../lib/stan.svelte';
  import { t } from '../lib/teksty';
  import type * as K from '../lib/typy';

  let { parametry, aktywny }: PropsEkranu = $props();

  const idOsoby = $derived(parametry.personId ?? stan.people.activePersonId);
  const osoba = $derived(stan.people.people.find((o) => o.id === idOsoby));
  // po usunięciu zostaje ostatni widok na czas wyjścia z ekranu
  let ostatnia = $state<K.Person | null>(null);
  $effect(() => {
    if (osoba) ostatnia = osoba;
  });
  const widok = $derived(osoba ?? ostatnia);
  const wybrana = $derived(!!widok && widok.id === stan.people.activePersonId);
  const ostatniaOsoba = $derived(stan.people.people.length < 2);

  let nazwa = $state('');
  let zapisano = $state(0);
  let pracuje = $state(false);
  let zegarNazwy: ReturnType<typeof setTimeout> | undefined;

  $effect(() => {
    if (!aktywny) return;
    const id = idOsoby;
    untrack(() => {
      clearTimeout(zegarNazwy);
      nazwa = stan.people.people.find((o) => o.id === id)?.name ?? '';
    });
  });
  $effect(() => {
    if (aktywny) return;
    untrack(() => void zapiszNazwe());
  });

  async function zmien(patch: K.PersonPatch) {
    if (!osoba) return;
    const ok = await wykonaj(api.personUpdate(osoba.id, patch).then(() => true));
    if (ok) zapisano++;
  }

  function wpisanoNazwe() {
    clearTimeout(zegarNazwy);
    zegarNazwy = setTimeout(zapiszNazwe, 500);
  }

  async function zapiszNazwe() {
    clearTimeout(zegarNazwy);
    const n = nazwa.trim();
    if (osoba && n && n !== osoba.name) await zmien({ name: n });
  }

  async function losuj() {
    const n = await wykonaj(api.nameRandom());
    if (!n) return;
    nazwa = n;
    await zapiszNazwe();
  }

  async function wybierzOsobe() {
    if (!osoba || pracuje) return;
    pracuje = true;
    await wykonaj(api.personSwitch(osoba.id));
    pracuje = false;
  }

  async function usun() {
    if (!osoba || pracuje) return;
    pracuje = true;
    const ok = await wykonaj(api.personRemove(osoba.id).then(() => true));
    pracuje = false;
    if (ok) wroc();
  }
</script>

<Strona tytul={t('osoba.edycja.tytul')} podpis={widok?.name ?? ''}>
  {#if widok}
    <Karta tytul={t('osoba.edycja.nazwa')}>
      <div class="nazwa">
        <Awatar nazwa={nazwa || widok.name} kolor={widok.color} />
        <div class="pole">
          <Pole
            bind:wartosc={nazwa}
            etykietaDostepna={t('kreator.imie.pole')}
            podpowiedz={t('kreator.imie.domyslna')}
            oninput={wpisanoNazwe}
            onblur={zapiszNazwe}
            onenter={zapiszNazwe}
          />
        </div>
        <Przycisk wariant="ikona" ikona="losuj" etykieta={t('kreator.imie.losuj')} onclick={losuj} />
      </div>
    </Karta>

    <Karta tytul={t('osoba.edycja.kolor')}>
      <Paleta wartosc={widok.color} etykieta={t('osoba.edycja.kolor')} onzmiana={(k) => void zmien({ color: k })} />
    </Karta>

    <Zapis licznik={zapisano} />

    {#if stan.app.accountsEnabled}
      <Karta tytul={t('osoba.edycja.konto')}>
        <p class="tekst dane">
          {widok.hasAccount
            ? t('osoba.edycja.konto_jest', { pseudonim: widok.accountName ?? '' })
            : t('osoba.edycja.konto_brak')}
        </p>
        {#if !wybrana}
          <p class="podpowiedz">{t('osoba.edycja.konto_wybierz')}</p>
          <div class="rzad">
            <Przycisk napis={t('osoba.edycja.wybierz')} disabled={pracuje} onclick={wybierzOsobe} />
          </div>
        {:else if widok.hasAccount}
          <div class="rzad">
            <Przycisk ikona="osoby" napis={t('konto.karta.otworz')} onclick={() => idz('online')} />
          </div>
        {:else}
          <div class="rzad">
            <Przycisk napis={t('konto.karta.zaloz')} onclick={() => idz('account', { accountMode: 'register' })} />
            <Przycisk napis={t('konto.karta.zaloguj')} onclick={() => idz('account', { accountMode: 'login' })} />
          </div>
        {/if}
      </Karta>
    {/if}

    {#if !ostatniaOsoba}
      <Karta>
        <p class="podpowiedz">{t('osoba.edycja.usun_opis')}</p>
        <div class="rzad">
          <Przycisk wariant="przytrzymaj" napis={t('osoba.edycja.usun')} disabled={pracuje} onclick={usun} />
        </div>
      </Karta>
    {/if}
  {/if}
</Strona>

<style>
  .nazwa { display: flex; align-items: center; gap: var(--sp-3); min-width: 0; }
  .pole { flex: 1; min-width: 0; }
  .pole :global(input) { text-overflow: ellipsis; }
  .tekst { line-height: var(--lh-body); }
  .podpowiedz { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-body); }
  .rzad { display: flex; flex-wrap: wrap; gap: var(--sp-2); }
  .rzad > :global(*) { flex: 1 1 160px; }
</style>
