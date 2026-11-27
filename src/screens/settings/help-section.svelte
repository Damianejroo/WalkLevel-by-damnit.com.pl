<script lang="ts">
  // Ustawienia > Pomoc i diagnostyka: zgłoś problem, dziennik, nagrywanie, aktualizacje i
  // informacje o aplikacji (docs/texts.md 18). „Zgłoś problem…” to zwykły przycisk (bez akcentu).
  import Karta from '../../lib/ui/card.svelte';
  import Przycisk from '../../lib/ui/button.svelte';
  import Przelacznik from '../../lib/ui/switch.svelte';
  import Rzad from './wrap-row.svelte';
  import { t } from '../../lib/texts';
  import { stan, idz, wykonaj, succeeded } from '../../lib/state.svelte';
  import * as api from '../../lib/api';
  import type { LinkKind } from '../../lib/contract';

  const LINKI: [LinkKind, 'app.link.strona' | 'app.link.prywatnosc' | 'app.link.kod'][] = [
    ['strona', 'app.link.strona'],
    ['prywatnosc', 'app.link.prywatnosc'],
    ['kod', 'app.link.kod'],
  ];
  const stanAktualizacji = $derived(
    stan.update.status ||
      (stan.update.available ? t('aktualizacja.dostepna', { nowa_wersja: stan.update.available }) : ''),
  );
</script>

<Karta tytul={t('ustawienia.pomoc.tytul')}>
  <Przycisk szeroki ikona="zglos" napis={t('ustawienia.pomoc.zglos')} onclick={() => idz('report')} />
  <Przelacznik
    tytul={t('ustawienia.pomoc.logi_bt')}
    opis={t('ustawienia.pomoc.logi_bt_opis')}
    wlaczony={stan.settings.detailedLogs}
    onzmiana={(v) => succeeded(api.settingsSet('detailedLogs', v))}
  />
  <Przelacznik
    tytul={t('ustawienia.pomoc.nagrywanie')}
    opis={t('ustawienia.pomoc.nagrywanie_opis')}
    wlaczony={stan.settings.record}
    onzmiana={(v) => succeeded(api.settingsSet('record', v))}
  />
  <Rzad>
    <Przycisk napis={t('ustawienia.pomoc.dziennik')} onclick={() => idz('log')} />
    <Przycisk napis={t('ustawienia.pomoc.folder_logow')} onclick={() => wykonaj(api.openFolder('logi'))} />
  </Rzad>
</Karta>

<Karta tytul={t('ustawienia.pomoc.aktualizacje_tytul')}>
  {#if stanAktualizacji}<p class="opis" aria-live="polite">{stanAktualizacji}</p>{/if}
  <Przycisk
    szeroki
    napis={t('ustawienia.pomoc.sprawdz')}
    disabled={stan.update.busy || !stan.app.installed}
    title={stan.app.installed ? undefined : t('glowny.pasek.aktualizuj_niedostepne')}
    onclick={() => wykonaj(api.updateCheck(false))}
  />
  <p class="o-aplikacji">{t('app.o_aplikacji', { wersja: stan.app.version, autor: stan.statics.author })}</p>
  <p class="o-aplikacji">{t('ustawienia.pomoc.podziekowania')}</p>
  <div class="linki">
    {#each LINKI as [rodzaj, klucz] (rodzaj)}
      <Przycisk wariant="link" napis={t(klucz)} onclick={() => wykonaj(api.openLink(rodzaj))} />
    {/each}
  </div>
</Karta>

<style>
  .opis { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-body); }
  .o-aplikacji { font-size: var(--fs-sm); color: var(--text-2); overflow-wrap: anywhere; }
  .linki { display: flex; flex-wrap: wrap; column-gap: var(--sp-5); min-width: 0; margin-block: -8px -6px; }
</style>
