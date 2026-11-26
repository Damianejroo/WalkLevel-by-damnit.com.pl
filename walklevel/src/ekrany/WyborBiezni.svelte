<script lang="ts">
  // Ekran: Wybierz bieżnię (wyniki skanowania) (trasa 'devices') - zaślepka fundamentu 0.6.0.
  // Do napisania wg docs/teksty.md 4.2 (dane: stan.device.devices).
  import Strona from '../lib/ui/Strona.svelte';
  import { t } from '../lib/teksty';
  import { stan, type PropsEkranu } from '../lib/stan.svelte';

  let _props: PropsEkranu = $props();
</script>

<Strona tytul={t('bieznia.lista.tytul')} podpis={stan.device.scanText} />
