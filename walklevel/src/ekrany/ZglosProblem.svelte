<script lang="ts">
  // Ekran: Zgłoś problem (trasa 'report') - zaślepka fundamentu 0.6.0.
  // Do napisania wg docs/teksty.md 18.3 (dane: stan.report, api.reportSend).
  import Strona from '../lib/ui/Strona.svelte';
  import { t } from '../lib/teksty';
  import type { PropsEkranu } from '../lib/stan.svelte';

  let _props: PropsEkranu = $props();
</script>

<Strona tytul={t('zgloszenie.tytul')} />
