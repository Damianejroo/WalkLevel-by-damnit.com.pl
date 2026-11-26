<script lang="ts">
  // Ustawienia > Wygląd i okno: kolor profilu gry, motyw, duży tekst, okno, przycisk na ekranie,
  // panel znajomych (docs/teksty.md 11.1, 16, dodatki 0.6.2). Kolor zapisuje się w aktywnym profilu (profile_update).
  import Karta from '../../lib/ui/Karta.svelte';
  import Przelacznik from '../../lib/ui/Przelacznik.svelte';
  import Segmentowy from '../../lib/ui/Segmentowy.svelte';
  import Paleta from './Paleta.svelte';
  import { t, tytulIPodpis } from '../../lib/teksty';
  import { stan, ui, wykonaj } from '../../lib/stan.svelte';
  import { rozwinMotyw, ustawMotyw } from '../../lib/wyglad';
  import * as api from '../../lib/api';
  import { zmienPanel } from '../../lib/panel.svelte';
  import type { PanelSide, SettingKey, ThemeChoice } from '../../lib/typy';

  const osoba = $derived(stan.people.people.find((o) => o.id === stan.people.activePersonId));
  const profil = $derived(osoba?.profiles.find((p) => p.id === stan.people.activeProfileId));
  const kolor = $derived(tytulIPodpis(t('ustawienia.wyglad.kolor', { profil: profil?.name ?? '' })));

  const motywy: { wartosc: ThemeChoice; napis: string }[] = [
    { wartosc: 'system', napis: t('ustawienia.wyglad.motyw.system') },
    { wartosc: 'light', napis: t('ustawienia.wyglad.motyw.jasny') },
    { wartosc: 'dark', napis: t('ustawienia.wyglad.motyw.ciemny') },
  ];
  const strony: { wartosc: PanelSide; napis: string }[] = [
    { wartosc: 'left', napis: t('ustawienia.wyglad.panel_strona.lewa') },
    { wartosc: 'right', napis: t('ustawienia.wyglad.panel_strona.prawa') },
  ];
  const PRZELACZNIKI = [
    ['largeText', 'ustawienia.wyglad.duzy_tekst', 'ustawienia.wyglad.duzy_tekst_opis'],
    ['alwaysOnTop', 'ustawienia.wyglad.na_wierzchu', 'ustawienia.wyglad.na_wierzchu_opis'],
    ['floatingButton', 'ustawienia.wyglad.przycisk', 'ustawienia.wyglad.przycisk_opis'],
    ['showPersonSwitch', 'ustawienia.wyglad.zmiana_osoby', 'ustawienia.wyglad.zmiana_osoby_opis'],
  ] as const satisfies ReadonlyArray<readonly [SettingKey, string, string]>;
</script>

<Karta tytul={t('ustawienia.wyglad.tytul')}>
  {#if profil}
    <div class="grupa">
      <span class="etykieta">{kolor.tytul}</span>
      {#if kolor.podpis}<span class="opis dane">{kolor.podpis}</span>{/if}
      <Paleta
        paleta={stan.statics.palette}
        wybrany={profil.color}
        zIkony={profil.iconColor}
        etykieta={t('kolor.dostepnosc', { kolor: profil.color })}
        onwybor={(k) => wykonaj(api.profileUpdate(profil.id, { color: k }))}
      />
    </div>
  {/if}
  <div class="grupa">
    <span class="etykieta">{t('ustawienia.wyglad.motyw')}</span>
    <Segmentowy
      opcje={motywy}
      wartosc={stan.settings.theme}
      etykieta={t('ustawienia.wyglad.motyw')}
      onzmiana={(v) => {
        // od razu, bez czekania na rdzeń (zapis idzie w tle, stan wróci tym samym motywem)
        ustawMotyw(rozwinMotyw(v, ui.ciemnySystem));
        void wykonaj(api.settingsSet('theme', v));
      }}
    />
  </div>
  {#each PRZELACZNIKI as [klucz, tytul, opis] (klucz)}
    <Przelacznik
      tytul={t(tytul)}
      opis={t(opis)}
      wlaczony={stan.settings[klucz]}
      onzmiana={(v) => wykonaj(api.settingsSet(klucz, v))}
    />
  {/each}
  <!-- panel znajomych (0.6.2): wł./wył., strona, powiadomienie (domyślnie wyłączone) -->
  <Przelacznik
    tytul={t('ustawienia.wyglad.panel')}
    opis={t('ustawienia.wyglad.panel_opis')}
    wlaczony={stan.settings.friendsPanel}
    onzmiana={(v) => zmienPanel('friendsPanel', v)}
  />
  {#if stan.settings.friendsPanel}
    <div class="grupa">
      <span class="etykieta">{t('ustawienia.wyglad.panel_strona')}</span>
      <Segmentowy
        opcje={strony}
        wartosc={stan.settings.friendsPanelSide}
        etykieta={t('ustawienia.wyglad.panel_strona')}
        onzmiana={(v) => wykonaj(api.settingsSet('friendsPanelSide', v))}
      />
    </div>
    <Przelacznik
      tytul={t('ustawienia.wyglad.panel_powiadom')}
      opis={t('ustawienia.wyglad.panel_powiadom_opis')}
      wlaczony={stan.settings.friendsNotify}
      onzmiana={(v) => wykonaj(api.settingsSet('friendsNotify', v))}
    />
  {/if}
</Karta>

<style>
  .grupa { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
  .etykieta { font-size: var(--fs-base); line-height: var(--lh-tight); }
  .opis { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-tight); }
</style>
