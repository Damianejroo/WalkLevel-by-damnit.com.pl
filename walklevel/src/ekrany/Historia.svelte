<script lang="ts">
  // Ekran: Historia kroków (trasa 'history') - zaślepka fundamentu 0.6.0.
  // Do napisania wg docs/wyglad.md 8.15, 9.6; docs/teksty.md 20 (dane: api.historyQuery).
  import Strona from '../lib/ui/Strona.svelte';
  import { t } from '../lib/teksty';
  import type { PropsEkranu } from '../lib/stan.svelte';

  let _props: PropsEkranu = $props();
</script>

<Strona tytul={t('historia.tytul')} />
