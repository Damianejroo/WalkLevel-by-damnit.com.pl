<script lang="ts">
  // Ustawienia > Konto i osoby: lista osób (wybór, edycja, dodawanie) i karta konta online
  // wybranej osoby (docs/texts.md 10, 22.1).
  import Karta from '../../lib/ui/card.svelte';
  import Przycisk from '../../lib/ui/button.svelte';
  import WierszWyboru from './select-row.svelte';
  import WierszDodaj from './add-row.svelte';
  import Awatar from './avatar.svelte';
  import Rzad from './wrap-row.svelte';
  import { t, tytulIPodpis } from '../../lib/texts';
  import { stan, idz, wykonaj, NEW_PERSON_ID } from '../../lib/state.svelte';
  import * as api from '../../lib/api';
  import type { Person } from '../../lib/contract';

  const osoby = $derived(stan.people.people);
  const aktywna = $derived(osoby.find((o) => o.id === stan.people.activePersonId));
  const karta = $derived(tytulIPodpis(t('konto.karta.tytul', { osoba: aktywna?.name ?? '' })));

  function opis(o: Person) {
    const zm = { liczba: o.profiles.length, kroki: o.todaySteps, pseudonim: o.accountName ?? '' };
    return o.hasAccount && o.accountName ? t('ustawienia.osoby.wiersz', zm) : t('ustawienia.osoby.wiersz_bez_konta', zm);
  }
</script>

<Karta tytul={t('ustawienia.osoby.tytul')}>
  <p class="opis">{t('ustawienia.osoby.opis')}</p>
  {#each osoby as o (o.id)}
    <WierszWyboru
      tytul={o.name}
      opis={opis(o)}
      wybrany={o.id === stan.people.activePersonId}
      etykietaEdycji={t('ustawienia.osoby.edytuj', { osoba: o.name })}
      onwybierz={() => o.id !== stan.people.activePersonId && wykonaj(api.personSwitch(o.id))}
      onedytuj={() => idz('person', { personId: o.id })}
    >
      {#snippet poczatek()}<Awatar imie={o.name} kolor={o.color} />{/snippet}
    </WierszWyboru>
  {/each}
  <WierszDodaj napis={t('ustawienia.osoby.dodaj')} onclick={() => idz('wizard', { wizardMode: 'add' })} />
  {#if stan.account.enabled}
    <WierszDodaj
      napis={t('ustawienia.osoby.dodaj_z_kontem')}
      onclick={() => idz('account', { accountMode: 'login', personId: NEW_PERSON_ID })}
    />
  {/if}
</Karta>

{#if stan.account.enabled}
  <Karta tytul={karta.tytul} podpis={karta.podpis}>
    {#if stan.account.loggedIn}
      <div class="konto">
        <span class="pseudonim dane">{stan.account.name}</span>
        <span class="opis dane">{stan.account.email}</span>
      </div>
      {#if !stan.account.verified}<p class="opis">{t('konto.email.niepotwierdzony')}</p>{/if}
      <Przycisk szeroki napis={t('konto.karta.otworz')} onclick={() => idz('online')} />
    {:else}
      <p class="opis">{t('konto.karta.opis')}</p>
      <Rzad>
        <Przycisk napis={t('konto.karta.zaloz')} onclick={() => idz('account', { accountMode: 'register' })} />
        <Przycisk napis={t('konto.karta.zaloguj')} onclick={() => idz('account', { accountMode: 'login' })} />
      </Rzad>
    {/if}
  </Karta>
{/if}

<style>
  .opis { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-body); }
  .konto { display: flex; flex-direction: column; min-width: 0; }
  .pseudonim { font-size: var(--fs-base); font-weight: var(--fw-semibold); line-height: var(--lh-tight); }
</style>
