<script lang="ts">
  // Ustawienia > Dane i kopia (docs/teksty.md 17).
  import Karta from '../../lib/ui/Karta.svelte';
  import Przycisk from '../../lib/ui/Przycisk.svelte';
  import Rzad from './Rzad.svelte';
  import { t } from '../../lib/teksty';
  import { stan, idz, wykonaj } from '../../lib/stan.svelte';
  import * as api from '../../lib/api';
</script>

<Karta tytul={t('ustawienia.dane.tytul')}>
  <p class="opis">{t('ustawienia.dane.opis')}</p>
  <Rzad>
    <Przycisk napis={t('ustawienia.dane.eksport')} onclick={() => idz('export')} />
    <Przycisk napis={t('ustawienia.dane.import')} onclick={() => idz('import')} />
  </Rzad>
  {#if stan.app.latestBackup}
    <Przycisk
      szeroki
      wariant="potwierdz"
      napis={t('ustawienia.dane.przywroc')}
      onclick={() => wykonaj(api.backupRestoreLatest())}
    />
  {/if}
  <Przycisk
    szeroki
    ikona="folder_otworz"
    napis={t('ustawienia.dane.folder')}
    title={t('ustawienia.dane.folder_podpowiedz')}
    onclick={() => wykonaj(api.openFolder('dane'))}
  />
</Karta>

<style>
  .opis { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-body); }
</style>
