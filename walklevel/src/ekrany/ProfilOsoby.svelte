<script lang="ts">
  // Ekran: Profil osoby z kontem online (trasa 'onlinePerson') - docs/wyglad.md 9.4,
  // docs/teksty.md 22.10. Dane: stan.online.person (ostatnio otwarty profil). Kolejność: dopisek
  // przy własnym profilu, karta poziomu (odznaka 70), osiągnięcia (4 najnowsze + Pokaż wszystkie),
  // profil (od kiedy, bieżnia, znajomość + zaproszenie albo usunięcie), ostatnie 7 dni, razem, gry.
  import Strona from '../lib/ui/Strona.svelte';
  import Karta from '../lib/ui/Karta.svelte';
  import Przycisk from '../lib/ui/Przycisk.svelte';
  import KartaPoziomu from './KontoOnline/KartaPoziomu.svelte';
  import WierszOsiagniecia from './KontoOnline/WierszOsiagniecia.svelte';
  import ListaGier from './KontoOnline/ListaGier.svelte';
  import * as api from '../lib/api';
  import { stan, wykonaj, type PropsEkranu } from '../lib/stan.svelte';
  import { czas, t, tytulIPodpis } from '../lib/teksty';
  import { liczba } from '../lib/format';
  import type { Achievement, PeriodTotals } from '../lib/typy';
  import { data, rangaPoziomu } from './KontoOnline/pomocnicze';

  let _props: PropsEkranu = $props();

  const NA_START = 4;
  const osoba = $derived(stan.online.person);
  const defs = $derived(stan.statics.definitions);
  const zajete = (a: 'invite' | 'removeFriend') => stan.account.busy.includes(a);

  // Poziom i XP: z serwera; przy własnym profilu, gdy serwer nie podał - z aplikacji
  const poziom = $derived(osoba?.level ?? (osoba?.me ? stan.levels?.level : null) ?? null);
  const xp = $derived(osoba?.xp ?? (osoba?.me ? stan.levels?.xp : null) ?? null);
  const ranga = $derived(poziom === null ? null : rangaPoziomu(defs, poziom));

  // Zdobyte osiągnięcia od najnowszego; null - serwer nie podał listy (własny: z aplikacji).
  // Cudzy profil nie ma czasów zdobycia (prywatność) - kolejność z listy serwera, bez dat.
  const zdobyte = $derived.by((): Array<{ a: Achievement; kiedy: number | null }> | null => {
    if (!osoba) return null;
    const lista =
      osoba.achievements ??
      (osoba.me && stan.levels
        ? stan.levels.progress.filter((p) => p.earnedAt !== null).map((p) => ({ id: p.id, earnedAt: p.earnedAt }))
        : null);
    if (!lista) return null;
    const wg = new Map(defs.achievements.map((a) => [a.id, a]));
    const wynik = lista
      .flatMap((x) => {
        const a = wg.get(x.id);
        return a ? [{ a, kiedy: x.earnedAt }] : [];
      });
    return lista.every((x) => x.earnedAt !== null) ? wynik.sort((p, q) => (q.kiedy ?? 0) - (p.kiedy ?? 0)) : wynik;
  });
  const osiagnieciaNaglowek = $derived(
    tytulIPodpis(t('poziomy.osiagniecia_tytul', { liczba: zdobyte?.length ?? 0, razem: defs.achievements.length })),
  );

  // Rozwinięcie listy, zaproszenie i usunięcie - stan lokalny jednego otwartego profilu
  let wszystkie = $state(false);
  let zaproszono = $state(false);
  let usunieto = $state(false);
  let poprzedni = '';
  $effect(() => {
    const id = osoba?.id ?? '';
    if (id !== poprzedni) {
      poprzedni = id;
      wszystkie = zaproszono = usunieto = false;
    }
  });
  const znajomy = $derived(!!osoba?.friend && !usunieto);

  async function zapros(id: string) {
    const wynik = await wykonaj(api.onlineInviteUser(id));
    if (wynik) zaproszono = true;
  }
  async function usun(id: string) {
    const wynik = await wykonaj(api.onlineFriendRemove(id).then(() => true));
    if (wynik) usunieto = true;
  }

  const wiersz = (s: PeriodTotals) => t('profil_online.wiersz', { kroki: s.steps, km: liczba(s.distanceKm, 2), czas: czas(s.activeS) });
  const sesje = (s: PeriodTotals) => (s.sessions === null ? '' : t('profil_online.sesje', { liczba: s.sessions }));
  const gry = $derived(tytulIPodpis(t('profil_online.gry')));
</script>

<Strona tytul={t('profil_online.tytul')} podpis={osoba?.name ?? ''}>
  {#if !osoba}
    <p class="opis uwaga">{stan.online.loading.includes('person') ? t('online.laduje') : '–'}</p>
  {:else}
    {#if osoba.me}<p class="opis uwaga">{t('profil_online.moj_opis')}</p>{/if}

    {#if poziom !== null}
      <KartaPoziomu {poziom} {xp} rozmiar={70} podpowiedz={ranga?.about ?? ''} />
    {/if}

    {#if zdobyte}
      <Karta tytul={osiagnieciaNaglowek.tytul} podpis={osiagnieciaNaglowek.podpis}>
        {#if zdobyte.length === 0}
          <p class="opis">{t('profil_online.brak_osiagniec')}</p>
        {:else}
          <ul class="osiagniecia">
            {#each wszystkie ? zdobyte : zdobyte.slice(0, NA_START) as z (z.a.id)}
              <WierszOsiagniecia osiagniecie={z.a} zdobyte={z.kiedy} />
            {/each}
          </ul>
          {#if !wszystkie && zdobyte.length > NA_START}
            <Przycisk szeroki napis={t('poziomy.pokaz_wszystkie', { liczba: zdobyte.length })} onclick={() => (wszystkie = true)} />
          {/if}
        {/if}
      </Karta>
    {/if}

    <Karta>
      <ul class="fakty">
        {#if osoba.description}<li class="opis-znajomego dane">{osoba.description}</li>{/if}
        {#if osoba.since}<li>{t('profil_online.od', { data: data(osoba.since) })}</li>{/if}
        {#if poziom === null && !osoba.me}<li>{t('online.poziom_ukryty')}</li>{/if}
        {#if osoba.treadmillModel}<li class="dane">{t('profil_online.bieznia', { model: osoba.treadmillModel })}</li>{/if}
        {#if !osoba.me}<li>{znajomy ? t('profil_online.znajomy') : t('profil_online.nieznajomy')}</li>{/if}
      </ul>
      {#if !osoba.me}
        <div class="przyciski">
          {#if znajomy}
            <Przycisk wariant="potwierdz" napis={t('znajomi.usun')} disabled={zajete('removeFriend')} onclick={() => usun(osoba.id)} />
          {:else}
            <Przycisk
              napisy={[t('osoby_online.zapros'), t('osoby_online.wyslano')]}
              napis={zaproszono ? t('osoby_online.wyslano') : t('osoby_online.zapros')}
              disabled={zaproszono || zajete('invite')}
              onclick={() => zapros(osoba.id)}
            />
          {/if}
        </div>
      {/if}
    </Karta>

    {#if osoba.today}
      <Karta tytul={t('profil_online.dzis')} podpis={sesje(osoba.today)} podpisZPrawej>
        <p class="wynik">{wiersz(osoba.today)}</p>
      </Karta>
    {/if}
    {#if osoba.week}
      <Karta tytul={t('profil_online.7dni')} podpis={sesje(osoba.week)} podpisZPrawej>
        <p class="wynik">{wiersz(osoba.week)}</p>
      </Karta>
    {/if}
    {#if osoba.total}
      <Karta tytul={t('profil_online.razem')} podpis={sesje(osoba.total)} podpisZPrawej>
        <p class="wynik">{wiersz(osoba.total)}</p>
      </Karta>
    {/if}

    <Karta tytul={gry.tytul} podpis={gry.podpis}>
      {#if osoba.gamesHidden}
        <p class="opis">{t('profil_online.gry_ukryte')}</p>
      {:else if osoba.games?.length || osoba.otherGames}
        <div class="gry"><ListaGier gry={osoba.games ?? []} inne={osoba.otherGames} /></div>
      {:else}
        <p class="opis">{t('profil_online.gry_brak')}</p>
      {/if}
    </Karta>

    <p class="opis uwaga">{osoba.me ? t('profil_online.moje_ukryte') : t('profil_online.cudze_ukryte')}</p>
  {/if}
</Strona>

<style>
  .opis { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-body); }
  /* notatki poza kartami - lekko wcięte jak w pozostałych ekranach; w kartach bez wcięcia */
  .uwaga { padding-inline: 2px; }
  .osiagniecia { display: flex; flex-direction: column; }
  .opis-znajomego { font-style: italic; color: var(--text); overflow-wrap: anywhere; }
  .fakty { display: flex; flex-direction: column; gap: 2px; font-size: var(--fs-md); color: var(--text-2); line-height: var(--lh-body); }
  .przyciski { display: flex; flex-wrap: wrap; gap: var(--sp-2); }
  .przyciski > :global(button) { flex: 0 1 auto; }
  .wynik { font-size: var(--fs-base); }
  .gry :global(.gra:first-child) { border-top: 0; }
</style>
