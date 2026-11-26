<script lang="ts">
  // Ekran: Ustawienia (trasa 'settings') - zaślepka fundamentu 0.6.0.
  // Do napisania wg docs/wyglad.md 8.5, 9.3; docs/teksty.md 9-18.
  import Strona from '../lib/ui/Strona.svelte';
  import { t } from '../lib/teksty';
  import type { PropsEkranu } from '../lib/stan.svelte';

  let _props: PropsEkranu = $props();
</script>

<Strona tytul={t('ustawienia.tytul')} />
