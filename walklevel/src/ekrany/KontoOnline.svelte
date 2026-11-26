<script lang="ts">
  // Ekran: Konto online (Znajomi, Ranking, Osoby z kontem, Moje konto) (trasa 'online') - zaślepka fundamentu 0.6.0.
  // Do napisania wg docs/wyglad.md 8.11, 9.4; docs/teksty.md 22.5-22.9.
  import Strona from '../lib/ui/Strona.svelte';
  import { t, tytulIPodpis } from '../lib/teksty';
  import { stan, type PropsEkranu } from '../lib/stan.svelte';

  let _props: PropsEkranu = $props();
  const naglowek = $derived(
    stan.account.loggedIn ? tytulIPodpis(t('online.tytul', { pseudonim: stan.account.name })) : { tytul: t('online.tytul_ogolny'), podpis: '' },
  );
</script>

<Strona tytul={naglowek.tytul} podpis={naglowek.podpis} />
