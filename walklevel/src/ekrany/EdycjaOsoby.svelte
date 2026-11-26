<script lang="ts">
  // Ekran: Edycja osoby (trasa 'person') - zaślepka fundamentu 0.6.0.
  // Do napisania wg docs/teksty.md 19 (parametry.personId).
  import Strona from '../lib/ui/Strona.svelte';
  import { t } from '../lib/teksty';
  import type { PropsEkranu } from '../lib/stan.svelte';

  let _props: PropsEkranu = $props();
</script>

<Strona tytul={t('osoba.edycja.tytul')} />
