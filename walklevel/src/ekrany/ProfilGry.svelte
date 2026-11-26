<script lang="ts">
  // Ekran: Profil gry - edycja i nowy (trasa 'profile', 'profileNew') - zaślepka fundamentu 0.6.0.
  // Do napisania wg docs/wyglad.md 8.14; docs/teksty.md 11 (parametry.profileId).
  import Strona from '../lib/ui/Strona.svelte';
  import { t } from '../lib/teksty';
  import type { PropsEkranu } from '../lib/stan.svelte';

  let { parametry }: PropsEkranu = $props();
</script>

<Strona tytul={parametry.profileId ? t('profil.tytul_edycja') : t('profil.tytul_nowy')} />
