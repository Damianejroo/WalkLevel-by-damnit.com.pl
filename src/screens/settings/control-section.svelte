<script lang="ts">
  // Ustawienia > Gry i sterowanie: profile gier, sterowanie (przełącznik trybów + ustawienia
  // wybranego trybu), klawisz włączania, klawisze szybciej/wolniej (docs/texts.md 11-13). Zmiana podglądanego trybu zmienia
  // tylko kartę sterowania; to, co pod nią, zjeżdża FLIP-em (bez skoku).
  import { untrack } from 'svelte';
  import Karta from '../../lib/ui/card.svelte';
  import Przycisk from '../../lib/ui/button.svelte';
  import Przelacznik from '../../lib/ui/switch.svelte';
  import Segmentowy from '../../lib/ui/segmented-control.svelte';
  import WierszWyboru from './select-row.svelte';
  import WierszDodaj from './add-row.svelte';
  import IkonaProfilu from './profile-icon.svelte';
  import PrzyciskKlawisza from './key-capture-button.svelte';
  import UstawieniaTrybu from './direction-settings.svelte';
  import Rzad from './wrap-row.svelte';
  import { flip } from './flip';
  import { t, tytulIPodpis } from '../../lib/texts';
  import { liczba } from '../../lib/format';
  import { stan, idz, wykonaj, succeeded } from '../../lib/state.svelte';
  import * as api from '../../lib/api';
  import type { Direction, GameProfile } from '../../lib/contract';

  const TRYBY: Direction[] = ['keyboard', 'gamepad', 'treadmill', 'training'];
  const kmh = (x: number) => liczba(x, 1);

  const osoba = $derived(stan.people.people.find((o) => o.id === stan.people.activePersonId));
  const profil = $derived(osoba?.profiles.find((p) => p.id === stan.people.activeProfileId));
  const kartaProfili = $derived(tytulIPodpis(t('ustawienia.profile.tytul', { osoba: osoba?.name ?? '' })));
  const kartaSterowania = $derived(tytulIPodpis(t('ustawienia.sterowanie.tytul', { profil: profil?.name ?? '' })));
  const aktywny = $derived(stan.settings.direction);
  const nazwy = $derived(stan.statics.modeNames);

  /** Tryb, którego ustawienia widać na karcie (domyślnie aktywny; po zmianie trybu - nowy). */
  let widok = $state<Direction>(untrack(() => stan.settings.direction));
  $effect(() => {
    const d = aktywny;
    untrack(() => (widok = d));
  });
  let cialo = $state<HTMLElement>();
  const opcje = $derived(
    TRYBY.map((d) => ({ wartosc: d, napis: nazwy[d], aktywny: d === aktywny, title: d === aktywny ? t('ustawienia.sterowanie.aktywny_tryb') : undefined })),
  );

  function podsumowanie(p: GameProfile) {
    const z = p.summary;
    switch (z.direction) {
      case 'gamepad':
        return t('ustawienia.profile.podsumowanie.pad', { liczba: z.padStepCount, kmh: kmh(z.padMaxKmh) });
      case 'keyboard':
        return t('ustawienia.profile.podsumowanie.klawisze', { kmh: kmh(z.walkSpeed), kmh_bieg: kmh(z.runSpeed) });
      case 'training':
        return t('ustawienia.profile.podsumowanie.trening', { min_kmh: kmh(z.trainingMin), max_kmh: kmh(z.trainingMax) });
      default:
        return t('ustawienia.profile.podsumowanie.chodzenie', { kmh: kmh(z.standBelow) });
    }
  }

  // --- przycisk pada ---
  const napisPada = $derived(
    stan.control.padCapturing
      ? t('ustawienia.wlaczanie.pad_czekam')
      : stan.settings.togglePad && stan.control.padButtonLabel
        ? t('klawisz.przycisk', { rola: t('ustawienia.wlaczanie.pad_rola'), klawisz: stan.control.padButtonLabel })
        : t('ustawienia.wlaczanie.pad_przypisz'),
  );
  const napisyPada = $derived([
    ...new Set([napisPada, t('ustawienia.wlaczanie.pad_czekam'), t('ustawienia.wlaczanie.pad_przypisz')]),
  ]);
</script>

<Karta tytul={kartaProfili.tytul} podpis={kartaProfili.podpis}>
  <p class="opis">{t('ustawienia.profile.opis')}</p>
  {#each osoba?.profiles ?? [] as p (p.id)}
    <WierszWyboru
      tytul={p.name}
      opis={podsumowanie(p)}
      wybrany={p.id === stan.people.activeProfileId}
      etykietaEdycji={t('wspolne.edytuj_co', { nazwa: p.name })}
      onwybierz={() => p.id !== stan.people.activeProfileId && wykonaj(api.profileSwitch(p.id, 'settings'))}
      onedytuj={() => idz('profile', { profileId: p.id })}
    >
      {#snippet poczatek()}<IkonaProfilu ikona={p.icon} />{/snippet}
    </WierszWyboru>
  {/each}
  <WierszDodaj napis={t('ustawienia.profile.nowy')} onclick={() => idz('profileNew')} />
</Karta>

<Karta tytul={kartaSterowania.tytul} podpis={kartaSterowania.podpis}>
  <Segmentowy
    {opcje}
    wartosc={widok}
    etykieta={t('ustawienia.sterowanie.wybor')}
    onzmiana={(d) => flip(cialo, () => (widok = d))}
  />
  <div class="cialo" bind:this={cialo}>
    <div class="stan-trybu">
      {#if widok === aktywny}
        <p class="opis">{t('ustawienia.sterowanie.aktywny')}</p>
      {:else}
        <p class="opis">{t('ustawienia.sterowanie.nieaktywny', { tryb_aktywny: nazwy[aktywny], tryb: nazwy[widok] })}</p>
        <span class="przelacz">
          <Przycisk
            napis={t('ustawienia.sterowanie.wlacz_tryb', { tryb: nazwy[widok] })}
            onclick={() => wykonaj(api.settingsSet('direction', widok))}
          />
        </span>
      {/if}
    </div>
    {#key widok}
      <div class="tryb">
        <UstawieniaTrybu tryb={widok} />
      </div>
    {/key}
  </div>
</Karta>

<Karta tytul={t('ustawienia.wlaczanie.tytul')}>
  <p class="opis">{t('ustawienia.wlaczanie.opis')}</p>
  <PrzyciskKlawisza rola="toggle" nazwaRoli={t('ustawienia.wlaczanie.rola')} klawisz={stan.settings.toggleKey.label} kombinacje />
  <p class="opis">{t('ustawienia.wlaczanie.pad_naglowek')}</p>
  <Rzad>
    <Przycisk
      napis={napisPada}
      napisy={napisyPada}
      title={t('ustawienia.wlaczanie.pad_podpowiedz')}
      onclick={() => wykonaj(stan.control.padCapturing ? api.padCaptureCancel() : api.padCaptureStart())}
    />
    {#if stan.settings.togglePad}
      <span class="wasko">
        <Przycisk napis={t('ustawienia.wlaczanie.pad_usun')} onclick={() => wykonaj(api.settingsSet('togglePad', ''))} />
      </span>
    {/if}
  </Rzad>
  <Przelacznik
    tytul={t('ustawienia.wlaczanie.zmiana_profilu')}
    opis={t('ustawienia.wlaczanie.zmiana_profilu_opis')}
    wlaczony={stan.settings.showProfileSwitch}
    onzmiana={(v) => succeeded(api.settingsSet('showProfileSwitch', v))}
  />
</Karta>

<Karta tytul={t('ustawienia.tempo.tytul')}>
  <p class="opis">{t('ustawienia.tempo.opis')}</p>
  <Rzad>
    <PrzyciskKlawisza rola="faster" nazwaRoli={t('ustawienia.tempo.szybciej')} klawisz={stan.settings.fasterKey.label} kombinacje />
    <PrzyciskKlawisza rola="slower" nazwaRoli={t('ustawienia.tempo.wolniej')} klawisz={stan.settings.slowerKey.label} kombinacje />
  </Rzad>
</Karta>

<style>
  .opis { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-body); }
  .cialo { display: flex; flex-direction: column; gap: var(--sp-3); min-width: 0; }
  /* Ta sama wysokość z przyciskiem „Przełącz na …” i bez niego; przycisk schodzi pod tekst */
  .stan-trybu {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2) var(--sp-3);
    min-height: var(--control-h);
    min-width: 0;
  }
  .stan-trybu .opis { flex: 1 1 200px; min-width: 0; }
  .przelacz { flex: 0 1 auto; min-width: 0; max-width: 100%; display: flex; }
  .tryb {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    min-width: 0;
    animation: wl-rozwin var(--t-in) var(--ease-enter);
  }
</style>
