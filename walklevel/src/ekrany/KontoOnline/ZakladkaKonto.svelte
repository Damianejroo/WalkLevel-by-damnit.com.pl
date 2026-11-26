<script lang="ts">
  // Konto online > Moje konto (docs/teksty.md 22.3, 22.9): wysyłanie sesji, kto je widzi, na żywo,
  // Wyślij teraz, mój profil; pseudonim, widoczność profilu i gier; hasło; adres e-mail i jego
  // potwierdzenie, zapamiętanie logowania, regulamin, pobranie danych, wylogowanie, usunięcie konta.
  // Bez akcentu - żadna z tych akcji nie jest główną akcją zakładki.
  import Karta from '../../lib/ui/Karta.svelte';
  import Przycisk from '../../lib/ui/Przycisk.svelte';
  import Przelacznik from '../../lib/ui/Przelacznik.svelte';
  import Segmentowy from '../../lib/ui/Segmentowy.svelte';
  import Pole from '../../lib/ui/Pole.svelte';
  import * as api from '../../lib/api';
  import { komunikat, stan, wykonaj } from '../../lib/stan.svelte';
  import { t } from '../../lib/teksty';
  import type { AccountAction, GamesVisibility, ProfileVisibility, SessionVisibility } from '../../lib/typy';
  import { bladHasla, bladPseudonimu, sprobuj, wersjaRegulaminu } from './pomocnicze';

  let { onotworz }: { onotworz: (userId: string) => void } = $props();

  const konto = $derived(stan.account);
  const zasady = $derived(stan.statics.account);
  const zajete = (a: AccountAction) => konto.busy.includes(a);

  // --- wysyłanie ---
  async function wyslijTeraz() {
    const ile = await wykonaj(api.accountUploadNow());
    if (ile !== undefined) komunikat(ile > 0 ? t('moje.wyslano', { liczba: ile }) : t('moje.wyslano_nic'));
  }

  // --- pseudonim (pole wraca do nazwy z konta, gdy ta się zmieni) ---
  let pseudonim = $state('');
  let bladNazwy = $state('');
  $effect(() => {
    pseudonim = konto.name;
  });
  async function zapiszPseudonim() {
    const nazwa = pseudonim.trim();
    bladNazwy = bladPseudonimu(nazwa, zasady);
    if (bladNazwy) return;
    const w = await sprobuj(api.accountRename(nazwa));
    bladNazwy = w.pola.name ?? w.blad;
    if (w.ok) komunikat(t('moje.profil.zapisano'));
  }

  // --- hasło ---
  let stare = $state('');
  let nowe = $state('');
  let nowe2 = $state('');
  let bledyHasla = $state({ stare: '', nowe: '', nowe2: '', ogolny: '' });
  async function zmienHaslo() {
    if (!stare || !nowe) return;
    bledyHasla = { stare: '', nowe: bladHasla(nowe, zasady), nowe2: nowe === nowe2 ? '' : t('moje.haslo.rozne'), ogolny: '' };
    if (bledyHasla.nowe || bledyHasla.nowe2) return;
    const w = await sprobuj(api.accountChangePassword(stare, nowe));
    bledyHasla = { stare: w.pola.oldPassword ?? '', nowe: w.pola.newPassword ?? '', nowe2: '', ogolny: w.blad };
    if (w.ok) {
      stare = nowe = nowe2 = '';
      komunikat(t('moje.haslo.zmienione'));
    }
  }

  // --- konto ---
  async function pobierzDane() {
    const plik = await wykonaj(api.accountExportMyData());
    if (plik) komunikat(t('moje.konto.pobrano', { plik }));
  }
  let hasloUsuniecia = $state('');
  let bladUsuniecia = $state('');
  async function usunKonto() {
    if (!hasloUsuniecia) {
      bladUsuniecia = t('moje.konto.usun_brak_hasla');
      return;
    }
    const w = await sprobuj(api.accountDelete(hasloUsuniecia));
    bladUsuniecia = w.pola.password ?? w.blad;
    if (w.ok) hasloUsuniecia = '';
  }
</script>

<div class="zakladka">
  <Karta tytul={t('moje.wyniki.tytul')}>
    <Przelacznik
      wlaczony={konto.upload}
      tytul={t('moje.wyniki.wysylaj')}
      opis={t('moje.wyniki.wysylaj_opis')}
      onzmiana={(on) => wykonaj(api.accountSetUpload(on))}
    />
    <div class="grupa">
      <p class="etykieta">{t('moje.wyniki.kto')}</p>
      <Segmentowy
        etykieta={t('moje.wyniki.kto')}
        wartosc={konto.visibility}
        onzmiana={(v: SessionVisibility) => wykonaj(api.accountSetVisibility(v))}
        opcje={[
          { wartosc: 'prywatna', napis: t('moje.widocznosc.ja') },
          { wartosc: 'znajomi', napis: t('moje.widocznosc.znajomi') },
          { wartosc: 'publiczna', napis: t('moje.widocznosc.wszyscy') },
        ]}
      />
      <p class="opis">{t('moje.wyniki.jedna_sesja')}</p>
    </div>
    <Przelacznik
      wlaczony={konto.live}
      tytul={t('moje.na_zywo')}
      opis={t('moje.na_zywo_opis')}
      disabled={zajete('live')}
      onzmiana={(on) => wykonaj(api.accountSetLive(on))}
    />
    <div class="przyciski">
      <Przycisk napis={t('moje.wyslij_teraz')} disabled={zajete('upload') || !konto.upload} onclick={wyslijTeraz} />
      <Przycisk napis={t('znajomi.moj_profil')} disabled={!konto.userId || zajete('person')} onclick={() => onotworz(konto.userId)} />
    </div>
    {#if konto.pendingUploads > 0}<p class="opis">{t('moje.czeka', { liczba: konto.pendingUploads })}</p>{/if}
  </Karta>

  <Karta tytul={t('moje.profil.etykieta')}>
    <div class="grupa">
      <p class="etykieta">{t('moje.profil.pseudonim')}</p>
      <div class="rzad-pola">
        <div class="pole">
          <Pole
            etykietaDostepna={t('moje.profil.pseudonim')}
            bind:wartosc={pseudonim}
            maxlength={zasady.nameMax}
            blad={bladNazwy}
            oninput={() => (bladNazwy = '')}
            onenter={zapiszPseudonim}
          />
        </div>
        <Przycisk
          napis={t('moje.profil.zapisz')}
          disabled={zajete('profile') || pseudonim.trim() === konto.name}
          onclick={zapiszPseudonim}
        />
      </div>
      <p class="opis">{t('konto.pseudonim_zasady')}</p>
    </div>
    <div class="grupa">
      <p class="etykieta">{t('moje.profil.lista')}</p>
      <Segmentowy
        etykieta={t('moje.profil.lista_dostepnosc')}
        wartosc={konto.profileVisibility}
        disabled={zajete('profile')}
        onzmiana={(v: ProfileVisibility) => wykonaj(api.accountSetProfileVisibility(v))}
        opcje={[
          { wartosc: 'ukryty', napis: t('moje.profil.ukryty') },
          { wartosc: 'znajomi', napis: t('moje.widocznosc.znajomi') },
          { wartosc: 'publiczny', napis: t('moje.widocznosc.wszyscy') },
        ]}
      />
    </div>
    <div class="grupa">
      <p class="etykieta">{t('moje.gry.kto')}</p>
      <Segmentowy
        etykieta={t('moje.gry.kto_dostepnosc')}
        wartosc={konto.gamesVisibility}
        disabled={zajete('profile') || !konto.gamesAllowed}
        onzmiana={(v: GamesVisibility) => wykonaj(api.accountSetGamesVisibility(v))}
        opcje={[
          { wartosc: 'ja', napis: t('moje.widocznosc.ja') },
          { wartosc: 'znajomi', napis: t('moje.widocznosc.znajomi') },
          { wartosc: 'wszyscy', napis: t('moje.widocznosc.wszyscy') },
        ]}
      />
      <p class="opis">{t('moje.gry.opis')}</p>
    </div>
  </Karta>

  <Karta tytul={t('moje.haslo.tytul')}>
    <Pole etykieta={t('moje.haslo.obecne')} typ="password" autocomplete="current-password" bind:wartosc={stare} blad={bledyHasla.stare} etykietaPokaz={t('online.pokaz_haslo')} />
    <Pole etykieta={t('moje.haslo.nowe')} typ="password" autocomplete="new-password" bind:wartosc={nowe} blad={bledyHasla.nowe} opis={t('konto.haslo_zasady', { liczba: zasady.passwordMin })} etykietaPokaz={t('online.pokaz_haslo')} />
    <Pole etykieta={t('moje.haslo.nowe2')} typ="password" autocomplete="new-password" bind:wartosc={nowe2} blad={bledyHasla.nowe2} etykietaPokaz={t('online.pokaz_haslo')} onenter={zmienHaslo} />
    {#if bledyHasla.ogolny}<p class="blad" role="alert">{bledyHasla.ogolny}</p>{/if}
    <div class="przyciski">
      <Przycisk napis={t('moje.haslo.zmien')} disabled={zajete('password') || !stare || !nowe || !nowe2} onclick={zmienHaslo} />
    </div>
    <p class="opis">{t('moje.haslo.opis')}</p>
  </Karta>

  <Karta tytul={t('moje.konto.tytul')}>
    <p class="adres dane">{t('moje.konto.adres', { email: konto.email })}</p>
    {#if konto.verified}
      <p class="opis">{t('konto.email.juz')}</p>
    {:else}
      <p class="opis">{t('konto.email.niepotwierdzony')}</p>
      {#if konto.verifyPolling}<p class="opis">{t('konto.email.wyslano')}</p>{/if}
      <div class="przyciski">
        <Przycisk
          napisy={[t('konto.email.wyslij_link'), t('konto.trwa.wysylanie')]}
          napis={zajete('verify') ? t('konto.trwa.wysylanie') : t('konto.email.wyslij_link')}
          disabled={zajete('verify')}
          onclick={() => wykonaj(api.accountSendVerification())}
        />
      </div>
    {/if}
    <Przelacznik
      wlaczony={konto.remember}
      tytul={t('moje.konto.zapamietaj')}
      opis={t('moje.konto.zapamietaj_wyl')}
      onzmiana={(on) => wykonaj(api.accountSetRemember(on))}
    />
    <div class="regulamin">
      {#if konto.terms}<p class="opis">{t('moje.konto.regulamin_wersja', { data: wersjaRegulaminu(konto.terms) })}</p>{/if}
      <Przycisk wariant="link" napis={t('konto.regulamin_czytaj')} title={konto.termsUrl || undefined} onclick={() => wykonaj(api.openLink('regulamin'))} />
    </div>
    <div class="przyciski">
      <Przycisk napis={t('moje.konto.pobierz')} disabled={zajete('export')} onclick={pobierzDane} />
      <Przycisk napis={t('moje.konto.wyloguj')} disabled={zajete('logout')} onclick={() => wykonaj(api.accountLogout())} />
    </div>
    <div class="usun">
      <p class="opis">{t('moje.konto.usun_opis')}</p>
      <Pole
        etykieta={t('moje.konto.usun_haslo')}
        typ="password"
        autocomplete="current-password"
        bind:wartosc={hasloUsuniecia}
        blad={bladUsuniecia}
        etykietaPokaz={t('online.pokaz_haslo')}
        oninput={() => (bladUsuniecia = '')}
      />
      <div class="przyciski">
        <Przycisk wariant="przytrzymaj" napis={t('moje.konto.usun')} disabled={zajete('delete')} onclick={usunKonto} />
      </div>
    </div>
  </Karta>
</div>

<style>
  .zakladka { display: flex; flex-direction: column; gap: var(--sp-3); min-width: 0; }
  .grupa { display: flex; flex-direction: column; gap: 6px; padding-block: 4px; }
  .etykieta { font-size: var(--fs-md); font-weight: var(--fw-semibold); }
  .opis { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-body); }
  .blad { font-size: var(--fs-sm); color: var(--danger); }
  .adres { font-size: var(--fs-base); }
  .przyciski { display: flex; flex-wrap: wrap; gap: var(--sp-2); }
  .przyciski > :global(button) { flex: 0 1 auto; }
  .rzad-pola { display: flex; flex-wrap: wrap; align-items: flex-start; gap: var(--sp-2); }
  .pole { flex: 1 1 200px; min-width: 0; }
  .rzad-pola > :global(button) { flex: 0 0 auto; }
  .regulamin { display: flex; flex-direction: column; }
  .regulamin > :global(button) { align-self: flex-start; }
  .usun {
    display: flex;
    flex-direction: column;
    gap: var(--sp-2);
    margin-top: var(--sp-2);
    padding-top: var(--sp-3);
    border-top: 1px solid var(--line);
  }
</style>
