<script lang="ts">
  // Ustawienia > Bieżnia: zapamiętana bieżnia, start i stop taśmy, symulator, obsługiwane modele
  // (docs/texts.md 14). Opis bieżni liczy rdzeń (device.label, dwie linie).
  import Karta from '../../lib/ui/card.svelte';
  import Przycisk from '../../lib/ui/button.svelte';
  import Przelacznik from '../../lib/ui/switch.svelte';
  import Rzad from './wrap-row.svelte';
  import { t } from '../../lib/texts';
  import { stan, idz, wykonaj, succeeded } from '../../lib/state.svelte';
  import * as api from '../../lib/api';

  const linie = $derived(stan.device.label.split('\n'));
</script>

<Karta tytul={t('ustawienia.bieznia.tytul')}>
  <p class="bieznia" title={stan.settings.deviceName || undefined}>
    <span class="model dane" class:brak={!stan.device.remembered && !stan.settings.simulator}>{linie[0]}</span>
    {#if linie[1]}<span class="szczegoly">{linie[1]}</span>{/if}
  </p>
  <Przelacznik
    tytul={t('ustawienia.bieznia.start_stop', { klawisz_wlaczania: stan.settings.toggleKey.label })}
    opis={t('ustawienia.bieznia.start_stop_opis')}
    wlaczony={stan.settings.beltControl}
    onzmiana={(v) => succeeded(api.settingsSet('beltControl', v))}
  />
  <Przelacznik
    tytul={t('ustawienia.bieznia.symulator')}
    opis={t('ustawienia.bieznia.symulator_przelacznik_opis', {
      klawisz_szybciej: stan.settings.fasterKey.label,
      klawisz_wolniej: stan.settings.slowerKey.label,
    })}
    wlaczony={stan.settings.simulator}
    onzmiana={(v) => succeeded(api.settingsSet('simulator', v))}
  />
  <Rzad>
    <Przycisk napis={t('ustawienia.bieznia.modele')} title={t('ustawienia.bieznia.modele_podpowiedz')} onclick={() => idz('models')} />
    <Przycisk
      wariant="potwierdz"
      napis={t('ustawienia.bieznia.zapomnij')}
      disabled={!stan.device.remembered}
      onclick={() => wykonaj(api.deviceForget())}
    />
  </Rzad>
</Karta>

<style>
  .bieznia { display: flex; flex-direction: column; min-width: 0; line-height: var(--lh-body); }
  .model { font-size: var(--fs-base); }
  .brak { font-size: var(--fs-sm); color: var(--text-2); }
  .szczegoly { font-size: var(--fs-sm); color: var(--text-2); }
</style>
