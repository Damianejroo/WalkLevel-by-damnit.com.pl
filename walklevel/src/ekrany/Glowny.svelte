<script lang="ts">
  // Ekran główny (trasa 'main') - szkielet fundamentu 0.6.0: pasek z nazwą i przyciskami
  // nawigacji. Treść do napisania wg docs/wyglad.md 9.2 i 8.14 oraz docs/teksty.md 2-8:
  // karta stanu -> karta poziomu -> wskaźnik + slot 56 -> kafelki -> karta sterowania.
  // W podglądzie (atrapa w przeglądarce) pod paskiem jest lista wszystkich ekranów.
  import Strona from '../lib/ui/Strona.svelte';
  import Przycisk from '../lib/ui/Przycisk.svelte';
  import WierszListy from '../lib/ui/WierszListy.svelte';
  import { t } from '../lib/teksty';
  import { stan, idz, wykonaj, type PropsEkranu, type Trasa } from '../lib/stan.svelte';
  import * as api from '../lib/api';
  import { EKRANY } from './rejestr';

  let _props: PropsEkranu = $props();

  const osoba = $derived(stan.people.people.find((o) => o.id === stan.people.activePersonId));
  const podpis = $derived(
    stan.people.people.length > 1 && osoba
      ? t('app.osoba_i_wersja', { osoba: osoba.name, wersja: stan.app.version })
      : t('app.wersja', { wersja: stan.app.version }),
  );
  const aktualizacja = $derived(stan.update.available);
  const podglad = (Object.keys(EKRANY) as Trasa[]).filter((x) => x !== 'main');

  function otworz(trasa: Trasa) {
    idz(trasa, trasa === 'profile' ? { profileId: stan.people.activeProfileId } : {});
  }
</script>

<Strona tytul={t('app.nazwa')} {podpis} glowny>
  {#snippet akcje()}
    {#if aktualizacja || stan.update.busy}
      <Przycisk
        wariant="ikona"
        ikona="pobierz"
        etykieta={aktualizacja ? t('glowny.pasek.aktualizuj_do', { nowa_wersja: aktualizacja }) : t('glowny.pasek.aktualizuj')}
        disabled={stan.update.busy}
        onclick={() => wykonaj(api.updateCheck(true))}
      />
    {/if}
    {#if stan.account.loggedIn}
      <Przycisk wariant="ikona" ikona="osoby" etykieta={t('glowny.pasek.online')} onclick={() => idz('online')} />
    {/if}
    <Przycisk wariant="ikona" ikona="wykres" etykieta={t('glowny.pasek.historia')} onclick={() => idz('history')} />
    <Przycisk wariant="ikona" ikona="ustawienia" etykieta={t('glowny.pasek.ustawienia')} onclick={() => idz('settings')} />
  {/snippet}

  {#if !api.wTauri}
    <nav class="podglad">
      {#each podglad as trasa (trasa)}
        <WierszListy tytul={t(EKRANY[trasa].nazwa)} opis={trasa} onclick={() => otworz(trasa)} />
      {/each}
    </nav>
  {/if}
</Strona>

<style>
  .podglad { display: flex; flex-direction: column; gap: var(--sp-1); }
</style>
