<script lang="ts">
  // Ekran: Dziennik zdarzeń (trasa 'log') - zaślepka fundamentu 0.6.0.
  // Do napisania wg docs/teksty.md 18.2 (dane: ui.dziennik).
  import Strona from '../lib/ui/Strona.svelte';
  import { t } from '../lib/teksty';
  import type { PropsEkranu } from '../lib/stan.svelte';

  let _props: PropsEkranu = $props();
</script>

<Strona tytul={t('dziennik.tytul')} />
