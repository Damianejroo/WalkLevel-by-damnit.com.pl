<script lang="ts">
  // Ekran: Szczegóły sesji (trasa 'session') - zaślepka fundamentu 0.6.0.
  // Do napisania wg docs/teksty.md 20.1 (api.historySession(parametry.sessionId)).
  import Strona from '../lib/ui/Strona.svelte';
  import { t } from '../lib/teksty';
  import type { PropsEkranu } from '../lib/stan.svelte';

  let _props: PropsEkranu = $props();
</script>

<Strona tytul={t('sesja.tytul')} />
