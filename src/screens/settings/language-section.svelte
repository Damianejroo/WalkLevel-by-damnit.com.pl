<script lang="ts">
  // Settings > Language (docs/i18n.md): every language the app knows,
  // installed or to download. A click shows an installed language at once; any other one is
  // downloaded first (progress in the row). A failed download shows the error card.
  import { onMount } from 'svelte';
  import Card from '../../lib/ui/card.svelte';
  import ListRow from '../../lib/ui/list-row.svelte';
  import ErrorCard from '../../lib/ui/error-card.svelte';
  import { t } from '../../lib/texts';
  import { stan, wykonaj } from '../../lib/state.svelte';
  import * as api from '../../lib/api';
  import type { LanguageEntry } from '../../lib/contract';

  const view = $derived(stan.language);

  // languages newer than this version, from the repository (silently local without internet)
  onMount(() => {
    api.languagesRefresh().catch(() => undefined);
  });

  function caption(l: LanguageEntry): string {
    const d = view.download;
    const state =
      d && d.code === l.code
        ? t('language.downloading', { procent: d.total ? Math.floor((d.done / d.total) * 100) : 0 })
        : l.code === view.chosen
          ? t('language.selected')
          : l.availability === 'download'
            ? t('language.download')
            : t('language.installed');
    return l.status === 'in-progress' ? `${state} · ${t('language.in-progress')}` : state;
  }
</script>

<Card tytul={t('language.title')}>
  <p class="description">{t('language.description')}</p>
  <ul class="languages" aria-label={t('language.list')}>
    {#each view.languages as l (l.code)}
      <li lang={l.code}>
        <ListRow
          tytul={l.name}
          opis={caption(l)}
          wybrany={l.code === view.chosen}
          etykieta={`${l.name}: ${caption(l)}`}
          onclick={() => void wykonaj(api.languageSet(l.code))}
        />
      </li>
    {/each}
  </ul>
  {#if view.error}
    <ErrorCard
      tytul={t('language.error.title')}
      opis={view.error}
      zajete={view.download !== null}
      onponow={() => void wykonaj(api.languageSet(view.chosen))}
    />
  {:else if view.shown !== view.chosen && view.download === null}
    <p class="description">{t('language.shown-instead')}</p>
  {/if}
</Card>

<style>
  .description { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-tight); }
  .languages { display: flex; flex-direction: column; gap: 6px; min-width: 0; margin: 0; padding: 0; list-style: none; }
</style>
