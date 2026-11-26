<script lang="ts">
  // Ekran: Konto online - logowanie, rejestracja, hasło (trasa 'account') - zaślepka fundamentu 0.6.0.
  // Do napisania wg docs/teksty.md 22.2-22.4 (parametry.accountMode).
  import Strona from '../lib/ui/Strona.svelte';
  import { t } from '../lib/teksty';
  import type { PropsEkranu } from '../lib/stan.svelte';

  let { parametry }: PropsEkranu = $props();
  const TYTULY = {
    login: t('konto.logowanie.tytul'),
    register: t('konto.rejestracja.tytul'),
    reset: t('konto.reset.tytul'),
    registered: t('konto.potwierdz.tytul'),
  };
</script>

<Strona tytul={TYTULY[parametry.accountMode ?? 'login']} />
