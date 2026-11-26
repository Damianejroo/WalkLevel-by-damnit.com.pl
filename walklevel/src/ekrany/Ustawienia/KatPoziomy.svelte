<script lang="ts">
  // Ustawienia > Poziomy i powiadomienia (docs/teksty.md 15).
  import Karta from '../../lib/ui/Karta.svelte';
  import Przycisk from '../../lib/ui/Przycisk.svelte';
  import Przelacznik from '../../lib/ui/Przelacznik.svelte';
  import { t } from '../../lib/teksty';
  import { stan, wykonaj } from '../../lib/stan.svelte';
  import * as api from '../../lib/api';
</script>

<Karta tytul={t('ustawienia.poziomy.tytul')}>
  <Przelacznik
    tytul={t('ustawienia.poziomy.wlacz')}
    opis={t('ustawienia.poziomy.wlacz_opis')}
    wlaczony={stan.settings.achievementPopups}
    onzmiana={(v) => wykonaj(api.settingsSet('achievementPopups', v))}
  />
  <Przelacznik
    tytul={t('ustawienia.poziomy.dzwiek')}
    opis={t('ustawienia.poziomy.dzwiek_opis')}
    wlaczony={stan.settings.achievementSound}
    onzmiana={(v) => wykonaj(api.settingsSet('achievementSound', v))}
  />
  <Przycisk szeroki napis={t('ustawienia.poziomy.przyklad')} onclick={() => wykonaj(api.levelsSamplePopup())} />
</Karta>
