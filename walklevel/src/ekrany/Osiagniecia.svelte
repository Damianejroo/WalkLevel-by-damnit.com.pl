<script lang="ts">
  // Ekran: Poziomy i osiągnięcia (trasa 'achievements') - zaślepka fundamentu 0.6.0.
  // Do napisania wg docs/wyglad.md 8.8-8.9, 9.5; docs/teksty.md 21.1.
  import Strona from '../lib/ui/Strona.svelte';
  import { t } from '../lib/teksty';
  import { stan, type PropsEkranu } from '../lib/stan.svelte';

  let _props: PropsEkranu = $props();
</script>

<Strona tytul={t('poziomy.tytul')} podpis={stan.levels ? t('poziomy.grupa_licznik', { liczba: stan.levels.earnedCount, razem: stan.levels.total }) : ''} />
