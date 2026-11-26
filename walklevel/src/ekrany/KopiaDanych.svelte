<script lang="ts">
  // Ekran: Kopia danych - zapis (trasa 'export') - zaślepka fundamentu 0.6.0.
  // Do napisania wg docs/teksty.md 17.1 (api.backupExportSummary, api.backupExport).
  import Strona from '../lib/ui/Strona.svelte';
  import { t } from '../lib/teksty';
  import type { PropsEkranu } from '../lib/stan.svelte';

  let _props: PropsEkranu = $props();
</script>

<Strona tytul={t('kopia.eksport.tytul')} />
