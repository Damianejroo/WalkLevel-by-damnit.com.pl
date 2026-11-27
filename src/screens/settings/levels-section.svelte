<script lang="ts">
  // Ustawienia > Poziomy i powiadomienia (docs/texts.md 15).
  import Karta from '../../lib/ui/card.svelte';
  import Przycisk from '../../lib/ui/button.svelte';
  import Przelacznik from '../../lib/ui/switch.svelte';
  import { t } from '../../lib/texts';
  import { stan, wykonaj, succeeded } from '../../lib/state.svelte';
  import * as api from '../../lib/api';
</script>

<Karta tytul={t('ustawienia.poziomy.tytul')}>
  <Przelacznik
    tytul={t('ustawienia.poziomy.wlacz')}
    opis={t('ustawienia.poziomy.wlacz_opis')}
    wlaczony={stan.settings.achievementPopups}
    onzmiana={(v) => succeeded(api.settingsSet('achievementPopups', v))}
  />
  <Przelacznik
    tytul={t('ustawienia.poziomy.dzwiek')}
    opis={t('ustawienia.poziomy.dzwiek_opis')}
    wlaczony={stan.settings.achievementSound}
    onzmiana={(v) => succeeded(api.settingsSet('achievementSound', v))}
  />
  <Przycisk szeroki napis={t('ustawienia.poziomy.przyklad')} onclick={() => wykonaj(api.levelsSamplePopup())} />
</Karta>
