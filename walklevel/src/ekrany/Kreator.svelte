<script lang="ts">
  // Ekran: Kreator osoby (pierwsze uruchomienie i nowa osoba) (trasa 'wizard') - zaślepka fundamentu 0.6.0.
  // Do napisania wg docs/teksty.md 19 (api.wizardSeed, api.wizardFinish; parametry.wizardMode).
  import Strona from '../lib/ui/Strona.svelte';
  import { t } from '../lib/teksty';
  import type { PropsEkranu } from '../lib/stan.svelte';

  let _props: PropsEkranu = $props();
</script>

<Strona tytul={t('kreator.imie.tytul')} podpis={t('kreator.krok', { liczba: 1 })} />
