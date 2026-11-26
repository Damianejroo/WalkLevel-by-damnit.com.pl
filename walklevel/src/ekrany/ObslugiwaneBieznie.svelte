<script lang="ts">
  // Ekran: Obsługiwane bieżnie (trasa 'models') - zaślepka fundamentu 0.6.0.
  // Do napisania wg docs/teksty.md 14.1 (dane: stan.models, przy wejściu api.modelsRefresh).
  import Strona from '../lib/ui/Strona.svelte';
  import { t } from '../lib/teksty';
  import type { PropsEkranu } from '../lib/stan.svelte';

  let _props: PropsEkranu = $props();
</script>

<Strona tytul={t('modele.tytul')} />
