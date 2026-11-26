<script lang="ts">
  // Ekran: Twój szlak (trasa 'trail') - zaślepka fundamentu 0.6.0.
  // Do napisania wg docs/wyglad.md 8.10; docs/teksty.md 21.2; docs/analiza-szlak.md.
  import Strona from '../lib/ui/Strona.svelte';
  import { t } from '../lib/teksty';
  import { liczba } from '../lib/format';
  import { stan, type PropsEkranu } from '../lib/stan.svelte';

  let _props: PropsEkranu = $props();
</script>

<Strona tytul={t('szlak.tytul')} podpis={stan.levels ? t('szlak.km', { km: liczba(stan.levels.km, 1) }) : ''} />
