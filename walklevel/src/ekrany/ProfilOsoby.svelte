<script lang="ts">
  // Ekran: Profil osoby z kontem online (trasa 'onlinePerson') - zaślepka fundamentu 0.6.0.
  // Do napisania wg docs/wyglad.md 9.4; docs/teksty.md 22.10 (dane: stan.online.person).
  import Strona from '../lib/ui/Strona.svelte';
  import { t } from '../lib/teksty';
  import { stan, type PropsEkranu } from '../lib/stan.svelte';

  let _props: PropsEkranu = $props();
</script>

<Strona tytul={t('profil_online.tytul')} podpis={stan.online.person?.name ?? ''} />
