<script lang="ts">
  // Konto online > Znajomi (docs/texts.md 22.6): karta mojego poziomu (klik - mój profil oczami
  // innych), wyniki tygodnia z podziałem na gry, zaproszenia do mnie, lista znajomych („idzie
  // teraz”), zaproszenie po pseudonimie i wysłane zaproszenia, kod na nowe konto.
  import Karta from '../../lib/ui/card.svelte';
  import Przycisk from '../../lib/ui/button.svelte';
  import Pole from '../../lib/ui/text-field.svelte';
  import KartaPoziomu from './level-card.svelte';
  import WierszOsoby from './user-row.svelte';
  import ListaGier from './game-list.svelte';
  import * as api from '../../lib/api';
  import { komunikat, stan, wykonaj } from '../../lib/state.svelte';
  import { czas, t, tytulIPodpis } from '../../lib/texts';
  import { liczba } from '../../lib/format';
  import { bladPseudonimu, data, sprobuj } from './helpers';

  interface Props {
    /** „Zaproś” jest główną akcją widoku (nie ma karty nowego regulaminu). */
    glowna: boolean;
    onotworz: (userId: string) => void;
  }
  let { glowna, onotworz }: Props = $props();

  const online = $derived(stan.online);
  const konto = $derived(stan.account);
  const zajete = (a: (typeof konto.busy)[number]) => konto.busy.includes(a);
  const laduje = (co: (typeof online.loading)[number]) => online.loading.includes(co);

  const wyniki = $derived(tytulIPodpis(t('znajomi.moje_wyniki')));
  let pokazGry = $state(false);

  // --- zaproszenie po pseudonimie ---
  let pseudonim = $state('');
  let bladZaproszenia = $state('');
  async function zapros() {
    const nazwa = pseudonim.trim();
    if (!nazwa) return;
    const blad = bladPseudonimu(nazwa, stan.statics.account);
    if (blad) {
      bladZaproszenia = blad;
      return;
    }
    const w = await sprobuj(api.onlineInviteName(nazwa));
    bladZaproszenia = w.blad || Object.values(w.pola)[0] || '';
    if (w.ok) pseudonim = '';
  }

  // --- kod na nowe konto ---
  const [przedKodem, poKodzie] = t('znajomi.kod.kod', { kod: '\u0000' }).split('\u0000');
  async function kopiujKod(kod: string) {
    try {
      await navigator.clipboard.writeText(kod);
      komunikat(t('znajomi.kod.skopiowano'));
    } catch (e) {
      console.error('Schowek niedostępny', e);
    }
  }
</script>

<div class="zakladka">
  {#if stan.levels}
    <KartaPoziomu
      poziom={stan.levels.level}
      xp={stan.levels.xp}
      podpowiedz={t('znajomi.moj_profil')}
      etykieta={t('znajomi.moj_profil')}
      onclick={() => onotworz(konto.userId)}
    />
  {/if}

  <Karta tytul={wyniki.tytul} podpis={wyniki.podpis}>
    {#if online.stats}
      <p class="wiersz-wynikow">
        {t('znajomi.moje_wyniki_wiersz', { kroki: online.stats.steps, km: liczba(online.stats.distanceKm, 2), czas: czas(online.stats.activeS) })}
      </p>
      {#if online.stats.games?.length}
        <div class="przyciski">
          <Przycisk
            napisy={[t('znajomi.podzial_pokaz'), t('znajomi.podzial_ukryj')]}
            napis={pokazGry ? t('znajomi.podzial_ukryj') : t('znajomi.podzial_pokaz')}
            onclick={() => (pokazGry = !pokazGry)}
          />
        </div>
        {#if pokazGry}<div class="rozwiniete"><ListaGier gry={online.stats.games} /></div>{/if}
      {/if}
    {:else}
      <p class="wyciszony">{laduje('stats') ? t('online.laduje') : '–'}</p>
    {/if}
  </Karta>

  {#if online.invitations?.incoming.length}
    <Karta tytul={t('znajomi.zaproszenia_do_mnie')}>
      <ul class="lista">
        {#each online.invitations.incoming as z (z.id)}
          <li class="zaproszenie">
            <span class="kto">
              <span class="nazwa jedna-linia" title={z.person.name}>{z.person.name}</span>
              {#if z.createdAt}<span class="kiedy">{data(z.createdAt)}</span>{/if}
            </span>
            <span class="odpowiedz">
              <Przycisk napis={t('znajomi.odrzuc')} disabled={zajete('answer')} onclick={() => wykonaj(api.onlineInvitationAnswer(z.id, false))} />
              <Przycisk napis={t('znajomi.przyjmij')} disabled={zajete('answer')} onclick={() => wykonaj(api.onlineInvitationAnswer(z.id, true))} />
            </span>
          </li>
        {/each}
      </ul>
    </Karta>
  {/if}

  <Karta tytul={t('znajomi.lista')} podpis={online.friends?.length ? String(online.friends.length) : ''} podpisZPrawej>
    {#if online.friends === null}
      <p class="wyciszony">{laduje('friends') ? t('online.laduje') : '–'}</p>
    {:else if online.friends.length === 0}
      <p class="wyciszony">{t('znajomi.pusto')}</p>
    {:else}
      <ul class="lista osoby">
        {#each online.friends as f (f.id)}
          <WierszOsoby nazwa={f.name} poziom={f.level} onotworz={() => onotworz(f.id)}>
            {#snippet opis()}
              {#if f.live}
                <span class="idzie">{t('znajomi.idzie', { kmh: liczba(f.live.kmh, 1), kroki: f.live.steps })}</span>
              {:else}
                <span class="linia">{t('znajomi.tydzien', { kroki: f.week.steps, czas: czas(f.week.activeS) })}</span>
              {/if}
            {/snippet}
          </WierszOsoby>
        {/each}
      </ul>
    {/if}
  </Karta>

  <Karta tytul={t('znajomi.dodaj')}>
    <p class="opis">{t('znajomi.dodaj_opis')}</p>
    <div class="rzad-pola">
      <div class="pole">
        <Pole
          bind:wartosc={pseudonim}
          etykietaDostepna={t('znajomi.dodaj_pole')}
          podpowiedz={t('znajomi.dodaj_pole')}
          maxlength={stan.statics.account.nameMax}
          blad={bladZaproszenia}
          oninput={() => (bladZaproszenia = '')}
          onenter={zapros}
        />
      </div>
      <Przycisk
        wariant={glowna ? 'glowny' : 'zwykly'}
        napis={t('znajomi.zapros')}
        disabled={zajete('invite')}
        onclick={zapros}
      />
    </div>
    {#if online.invitations?.sent.length}
      <p class="podtytul">{t('znajomi.wyslane')}</p>
      <ul class="lista">
        {#each online.invitations.sent as z (z.id)}
          <li class="zaproszenie">
            <span class="kto">
              <span class="nazwa jedna-linia" title={z.person.name}>{z.person.name}</span>
              {#if z.createdAt}<span class="kiedy">{data(z.createdAt)}</span>{/if}
            </span>
            <span class="odpowiedz">
              <Przycisk napis={t('znajomi.wycofaj')} disabled={zajete('answer')} onclick={() => wykonaj(api.onlineInvitationWithdraw(z.id))} />
            </span>
          </li>
        {/each}
      </ul>
    {/if}
  </Karta>

  <Karta tytul={t('znajomi.kod.tytul')}>
    <p class="opis">{t('znajomi.kod.opis')}</p>
    {#if online.regCode}
      <p class="kod-wiersz">{przedKodem}<span class="kod liczba">{online.regCode.code}</span>{poKodzie}</p>
      <p class="opis">{t('znajomi.kod.zasady')}</p>
      <div class="przyciski">
        <Przycisk napis={t('znajomi.kod.kopiuj')} onclick={() => online.regCode && kopiujKod(online.regCode.code)} />
        <Przycisk napis={t('znajomi.kod.kolejny')} disabled={zajete('regCode')} onclick={() => wykonaj(api.onlineRegistrationCode())} />
      </div>
    {:else}
      <div class="przyciski">
        <Przycisk napis={t('znajomi.kod.pokaz')} disabled={zajete('regCode')} onclick={() => wykonaj(api.onlineRegistrationCode())} />
      </div>
    {/if}
  </Karta>
</div>

<style>
  .zakladka { display: flex; flex-direction: column; gap: var(--sp-3); min-width: 0; }
  .wiersz-wynikow { font-size: var(--fs-base); }
  .opis,
  .linia,
  .kiedy { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-body); }
  .idzie { font-size: var(--fs-sm); font-weight: var(--fw-semibold); line-height: var(--lh-body); }
  .podtytul { margin-top: var(--sp-2); font-size: var(--fs-md); font-weight: var(--fw-semibold); }

  .przyciski { display: flex; flex-wrap: wrap; gap: var(--sp-2); }
  .przyciski > :global(*) { flex: 0 1 auto; }
  .rozwiniete { animation: wl-rozwin var(--t-in) var(--ease-enter); }

  .lista { display: flex; flex-direction: column; }
  .osoby { margin-inline: -6px; }
  .zaproszenie {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2) var(--sp-3);
    padding-block: 8px;
    border-top: 1px solid var(--line);
  }
  .zaproszenie:first-child { border-top: 0; padding-top: 0; }
  .kto { display: flex; flex-direction: column; flex: 1 1 140px; min-width: 0; }
  .nazwa { font-size: var(--fs-base); font-weight: var(--fw-semibold); }
  .odpowiedz { display: flex; flex-wrap: wrap; gap: var(--sp-2); margin-inline-start: auto; }

  .rzad-pola { display: flex; flex-wrap: wrap; align-items: flex-start; gap: var(--sp-2); }
  .pole { flex: 1 1 200px; min-width: 0; }
  .rzad-pola > :global(button) { flex: 0 0 auto; min-width: 120px; }

  .kod-wiersz { font-size: var(--fs-base); overflow-wrap: anywhere; }
  .kod {
    font-size: var(--fs-lg);
    letter-spacing: 0.04em;
    user-select: all;
  }
</style>
