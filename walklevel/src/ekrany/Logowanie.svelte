<script lang="ts">
  // Ekran: Konto online - logowanie, rejestracja z kodem zaproszenia, przypomnienie hasła i
  // „potwierdź e-mail” po rejestracji (trasa 'account', tryb z parametry.accountMode) -
  // docs/teksty.md 22.1-22.4, kontrakt 6.8. Pola sprawdzane od razu przy wysłaniu (te same zasady
  // co w rdzeniu), błędy rdzenia przy polach albo pod formularzem. Jedna główna akcja na ekranie.
  //
  // Konto dla nowej osoby (new_person): gdy wybrana osoba ma już konto albo ekran otwarto z
  // parametry.personId === NOWA_OSOBA (Ustawienia > Konto i osoby > Dodaj osobę z kontem online).
  import { untrack } from 'svelte';
  import Strona from '../lib/ui/Strona.svelte';
  import Karta from '../lib/ui/Karta.svelte';
  import Przycisk from '../lib/ui/Przycisk.svelte';
  import Przelacznik from '../lib/ui/Przelacznik.svelte';
  import Pole from '../lib/ui/Pole.svelte';
  import * as api from '../lib/api';
  import { idz, stan, wykonaj, type PropsEkranu } from '../lib/stan.svelte';
  import { t } from '../lib/teksty';
  import type { AccountFormMode } from '../lib/typy';
  import { bladEmaila, bladHasla, bladPseudonimu, sprobuj, wersjaRegulaminu } from './KontoOnline/pomocnicze';

  const NOWA_OSOBA = 'nowa';
  const TRYBY: AccountFormMode[] = ['login', 'register', 'reset', 'registered'];

  let { parametry, aktywny }: PropsEkranu = $props();

  // podgląd w przeglądarce i zrzuty: ?formularz=register (w aplikacji tryb przychodzi w parametrach)
  const zAdresu = new URLSearchParams(location.search).get('formularz') as AccountFormMode | null;
  const tryb = $derived<AccountFormMode>(parametry.accountMode ?? (zAdresu && TRYBY.includes(zAdresu) ? zAdresu : 'login'));

  const TYTULY: Record<AccountFormMode, string> = {
    login: t('konto.logowanie.tytul'),
    register: t('konto.rejestracja.tytul'),
    reset: t('konto.reset.tytul'),
    registered: t('konto.potwierdz.tytul'),
  };

  const konto = $derived(stan.account);
  const zasady = $derived(stan.statics.account);
  const nowaOsoba = $derived(parametry.personId === NOWA_OSOBA || konto.loggedIn);
  const osoba = $derived(stan.people.people.find((o) => o.id === stan.people.activePersonId)?.name ?? '');
  const zajete = $derived(konto.busy.includes('login') || konto.busy.includes('register') || konto.busy.includes('reset'));

  // --- pola formularza ---
  let kod = $state('');
  let email = $state('');
  let pseudonim = $state('');
  let haslo = $state('');
  let haslo2 = $state('');
  let zapamietaj = $state(true);
  let regulamin = $state(false);
  let bledy = $state<Record<string, string>>({});
  let bladOgolny = $state('');

  // adres wybranej osoby jako podpowiedź (logowanie i przypomnienie hasła)
  $effect(() => {
    const adres = nowaOsoba ? '' : konto.email;
    if (adres) untrack(() => email || (email = adres));
  });
  // hasła nie zostają w polach po wyjściu z ekranu; zmiana trybu czyści błędy
  $effect(() => {
    if (!aktywny) haslo = haslo2 = '';
  });
  $effect(() => {
    void tryb;
    bledy = {};
    bladOgolny = '';
  });

  const czysc = (pole: string) => () => {
    if (bledy[pole]) bledy = { ...bledy, [pole]: '' };
    bladOgolny = '';
  };

  async function poZalogowaniu() {
    haslo = haslo2 = '';
    if (zapamietaj !== stan.account.remember) await wykonaj(api.accountSetRemember(zapamietaj));
  }

  async function zaloguj() {
    bledy = { email: bladEmaila(email, zasady), password: haslo ? '' : t('konto.blad.logowanie') };
    if (bledy.email || bledy.password) return;
    const w = await sprobuj(api.accountLogin(email.trim(), haslo, nowaOsoba));
    bledy = w.pola;
    bladOgolny = w.blad;
    if (w.ok) await poZalogowaniu(); // rdzeń sam przechodzi na Konto online
  }

  async function zaloz() {
    bledy = {
      code: kod.trim().length >= zasady.codeMin ? '' : t('konto.blad.kod'),
      email: bladEmaila(email, zasady),
      name: bladPseudonimu(pseudonim, zasady),
      password: bladHasla(haslo, zasady),
      password2: haslo === haslo2 ? '' : t('konto.blad.hasla_rozne'),
      terms: regulamin ? '' : t('konto.blad.regulamin'),
    };
    if (Object.values(bledy).some(Boolean)) return;
    const w = await sprobuj(api.accountRegister(kod.trim(), email.trim(), haslo, pseudonim, nowaOsoba));
    bledy = w.pola;
    bladOgolny = w.blad;
    if (w.ok) {
      await poZalogowaniu();
      idz('account', { accountMode: 'registered' });
    }
  }

  async function przypomnij() {
    bledy = { email: bladEmaila(email, zasady) };
    if (bledy.email) return;
    const w = await sprobuj(api.accountRemindPassword(email.trim()));
    bledy = w.pola;
    bladOgolny = w.blad;
  }

  const wyslij = () => (tryb === 'login' ? zaloguj() : tryb === 'register' ? zaloz() : przypomnij());
  const napisy = $derived(
    tryb === 'login'
      ? [t('konto.przycisk.zaloguj'), t('konto.trwa.logowanie')]
      : tryb === 'register'
        ? [t('konto.przycisk.zaloz'), t('konto.trwa.zakladanie')]
        : [t('konto.reset.wyslij'), t('konto.trwa.wysylanie')],
  );
  const przejdz = (accountMode: AccountFormMode) => () => idz('account', { ...parametry, accountMode });
</script>

<Strona tytul={TYTULY[tryb]}>
  {#if tryb === 'registered'}
    <Karta>
      <p class="tresc">{t('konto.rejestracja.gotowe')}</p>
      <div class="przyciski">
        <Przycisk wariant="glowny" napis={t('konto.link.przejdz')} onclick={przejdz('login')} />
      </div>
    </Karta>
  {:else}
    <Karta>
      <form class="formularz" novalidate onsubmit={(e) => { e.preventDefault(); wyslij(); }}>
        {#if tryb === 'reset'}
          <p class="opis">{t('konto.reset.opis')}</p>
        {:else}
          <p class="opis dane">{nowaOsoba ? t('konto.nowa_osoba_opis') : t('konto.dla_osoby', { osoba })}</p>
        {/if}

        {#if tryb === 'register'}
          <Pole etykieta={t('konto.pole.kod')} bind:wartosc={kod} blad={bledy.code} opis={t('konto.kod_opis')} autocomplete="off" oninput={czysc('code')} />
        {/if}
        <Pole etykieta={t('konto.pole.email')} typ="email" autocomplete="email" bind:wartosc={email} blad={bledy.email} oninput={czysc('email')} />
        {#if tryb === 'register'}
          <Pole
            etykieta={t('konto.pole.pseudonim')}
            bind:wartosc={pseudonim}
            maxlength={zasady.nameMax}
            autocomplete="off"
            blad={bledy.name}
            opis={t('konto.pseudonim_zasady')}
            oninput={czysc('name')}
          />
        {/if}
        {#if tryb !== 'reset'}
          <Pole
            etykieta={t('konto.pole.haslo')}
            typ="password"
            autocomplete={tryb === 'register' ? 'new-password' : 'current-password'}
            bind:wartosc={haslo}
            blad={bledy.password}
            opis={tryb === 'register' ? t('konto.haslo_zasady', { liczba: zasady.passwordMin }) : undefined}
            etykietaPokaz={t('online.pokaz_haslo')}
            oninput={czysc('password')}
          />
        {/if}
        {#if tryb === 'register'}
          <Pole
            etykieta={t('konto.pole.haslo2')}
            typ="password"
            autocomplete="new-password"
            bind:wartosc={haslo2}
            blad={bledy.password2}
            etykietaPokaz={t('online.pokaz_haslo')}
            oninput={czysc('password2')}
          />
        {/if}

        {#if tryb !== 'reset'}
          <Przelacznik bind:wlaczony={zapamietaj} tytul={t('konto.zapamietaj')} opis={t('konto.zapamietaj_opis')} />
        {/if}
        {#if tryb === 'register'}
          <div class="regulamin">
            <Przelacznik
              bind:wlaczony={regulamin}
              tytul={t('konto.regulamin_zgoda')}
              opis={konto.termsCurrent ? t('konto.regulamin_wersja', { data: wersjaRegulaminu(konto.termsCurrent) }) : undefined}
              onzmiana={() => czysc('terms')()}
            />
            {#if bledy.terms}<p class="blad" role="alert">{bledy.terms}</p>{/if}
            <Przycisk wariant="link" napis={t('konto.regulamin_czytaj')} title={konto.termsUrl || undefined} onclick={() => wykonaj(api.openLink('regulamin'))} />
          </div>
        {/if}

        {#if bladOgolny}<p class="blad" role="alert">{bladOgolny}</p>{/if}
        {#if tryb === 'reset' && konto.message}<p class="opis" role="status">{konto.message}</p>{/if}

        <Przycisk wariant="glowny" typ="submit" szeroki {napisy} napis={zajete ? napisy[1] : napisy[0]} disabled={zajete} />
      </form>
    </Karta>

    <div class="linki">
      {#if tryb === 'login'}
        <Przycisk wariant="link" napis={t('konto.link.nie_pamietam')} onclick={przejdz('reset')} />
        <Przycisk wariant="link" napis={t('konto.link.zaloz')} onclick={przejdz('register')} />
      {:else if tryb === 'register'}
        <Przycisk wariant="link" napis={t('konto.link.mam_konto')} onclick={przejdz('login')} />
      {:else}
        <Przycisk wariant="link" napis={t('konto.link.do_logowania')} onclick={przejdz('login')} />
      {/if}
    </div>

    {#if tryb !== 'reset'}
      <Karta tytul={t('konto.co_wysylamy.tytul')}>
        <p class="opis">{t('konto.co_wysylamy')}</p>
      </Karta>
    {/if}
  {/if}
</Strona>

<style>
  .formularz { display: flex; flex-direction: column; gap: var(--sp-3); min-width: 0; }
  .tresc { font-size: var(--fs-base); line-height: var(--lh-body); }
  .opis { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-body); }
  .blad { font-size: var(--fs-sm); color: var(--danger); line-height: var(--lh-body); }
  .przyciski { display: flex; flex-wrap: wrap; gap: var(--sp-2); }
  .regulamin { display: flex; flex-direction: column; gap: 2px; }
  .regulamin > :global(button) { align-self: flex-start; }
  .linki { display: flex; flex-wrap: wrap; justify-content: space-between; column-gap: var(--sp-4); padding-inline: 2px; }
</style>
