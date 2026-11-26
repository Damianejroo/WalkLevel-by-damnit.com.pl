<script lang="ts">
  // Ekran: Kopia danych - wczytanie (trasa 'import') - zaślepka fundamentu 0.6.0.
  // Do napisania wg docs/teksty.md 17.2 (api.backupImportOpen, api.backupImportApply).
  import Strona from '../lib/ui/Strona.svelte';
  import { t } from '../lib/teksty';
  import type { PropsEkranu } from '../lib/stan.svelte';

  let _props: PropsEkranu = $props();
</script>

<Strona tytul={t('kopia.import.tytul')} />
