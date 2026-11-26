<script lang="ts">
  // Konto online > Osoby z kontem (docs/teksty.md 22.8): wyszukiwanie po pseudonimie, strony,
  // „Zaproś” przy osobach spoza listy znajomych.
  import Karta from '../../lib/ui/Karta.svelte';
  import Przycisk from '../../lib/ui/Przycisk.svelte';
  import Pole from '../../lib/ui/Pole.svelte';
  import WierszOsoby from './WierszOsoby.svelte';
  import * as api from '../../lib/api';
  import { stan, wykonaj } from '../../lib/stan.svelte';
  import { t } from '../../lib/teksty';
  import { SvelteSet } from 'svelte/reactivity';

  let { onotworz }: { onotworz: (userId: string) => void } = $props();

  const online = $derived(stan.online);
  const strona = $derived(online.people?.page ?? online.peopleQuery.page);
  const stronicowanie = $derived(!!online.people && (online.people.hasNext || online.people.page > 1));

  let szukaj = $state(stan.online.peopleQuery.search);
  const zaproszeni = new SvelteSet<string>();

  function wyszukaj(page = 1) {
    wykonaj(api.onlinePeopleSearch(page === 1 ? szukaj.trim() : online.peopleQuery.search, page));
  }

  async function zapros(id: string) {
    const stanZaproszenia = await wykonaj(api.onlineInviteUser(id));
    if (stanZaproszenia) zaproszeni.add(id);
  }
</script>

<div class="zakladka">
  <div class="szukaj">
    <div class="pole">
      <Pole
        typ="search"
        bind:wartosc={szukaj}
        etykietaDostepna={t('osoby_online.szukaj_pole')}
        podpowiedz={t('osoby_online.szukaj_pole')}
        maxlength={stan.statics.account.nameMax}
        onenter={() => wyszukaj()}
      />
    </div>
    <Przycisk napis={t('osoby_online.szukaj')} disabled={stan.account.busy.includes('loadPeople')} onclick={() => wyszukaj()} />
  </div>

  <Karta
    tytul={t('osoby_online.tytul')}
    podpis={stronicowanie ? t('osoby_online.strona', { liczba: strona }) : ''}
    podpisZPrawej
  >
    {#if online.people === null}
      <p class="wyciszony">{online.loading.includes('people') ? t('online.laduje') : '–'}</p>
    {:else if online.people.people.length === 0}
      <p class="wyciszony">{online.peopleQuery.search ? t('osoby_online.pusto') : t('osoby_online.pusto_lista')}</p>
    {:else}
      <ul class="lista">
        {#each online.people.people as o (o.id)}
          {@const ja = o.id === stan.account.userId}
          <WierszOsoby nazwa={o.name} poziom={o.level} {ja} onotworz={() => onotworz(o.id)}>
            {#snippet opis()}
              <span class="linia">
                {o.place
                  ? t('osoby_online.wiersz', { kroki: o.weekSteps, liczba: o.place }).replace(/(\d)\. /, '$1. ')
                  : t('osoby_online.wiersz_bez_miejsca', { kroki: o.weekSteps })}
              </span>
              {#if o.friend}<span class="linia">{t('profil_online.znajomy')}</span>{/if}
            {/snippet}
            {#snippet koniec()}
              {#if !o.friend && !ja}
                <Przycisk
                  napisy={[t('osoby_online.zapros'), t('osoby_online.wyslano')]}
                  napis={zaproszeni.has(o.id) ? t('osoby_online.wyslano') : t('osoby_online.zapros')}
                  disabled={zaproszeni.has(o.id) || stan.account.busy.includes('invite')}
                  onclick={() => zapros(o.id)}
                />
              {/if}
            {/snippet}
          </WierszOsoby>
        {/each}
      </ul>
    {/if}
    {#if stronicowanie && online.people}
      <div class="strony">
        <Przycisk napis={t('osoby_online.poprzednie')} disabled={online.people.page <= 1} onclick={() => wyszukaj(strona - 1)} />
        <Przycisk napis={t('osoby_online.nastepne')} disabled={!online.people.hasNext} onclick={() => wyszukaj(strona + 1)} />
      </div>
    {/if}
  </Karta>

  <p class="opis">{t('osoby_online.opis')}</p>
</div>

<style>
  .zakladka { display: flex; flex-direction: column; gap: var(--sp-3); min-width: 0; }
  .szukaj { display: flex; flex-wrap: wrap; gap: var(--sp-2); }
  .pole { flex: 1 1 200px; min-width: 0; }
  .szukaj > :global(button) { flex: 0 0 auto; min-width: 120px; }
  .lista { display: flex; flex-direction: column; margin-inline: -6px; }
  .linia { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-body); }
  .strony { display: flex; flex-wrap: wrap; gap: var(--sp-2); margin-top: var(--sp-1); }
  .strony > :global(button) { flex: 1 1 140px; }
  .opis { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-body); padding-inline: 2px; }
</style>
