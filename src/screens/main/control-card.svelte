<script lang="ts">
  // Karta sterowania (docs/design.md 9.2, docs/texts.md 8): profil gry z listą profili, klawisz
  // włączania z opisem (control.subtitle), przełącznik sterowania, kafelki trybu i podpowiedź
  // trybu (control.hint). Podtytuł jednoliniowy, podpowiedź zawsze na dwie linie - zmiana trybu
  // nie zmienia wysokości karty.
  import Karta from '../../lib/ui/card.svelte';
  import Ikona from '../../lib/ui/icon.svelte';
  import Przelacznik from '../../lib/ui/switch.svelte';
  import * as api from '../../lib/api';
  import { stan, idz, wykonaj, succeeded } from '../../lib/state.svelte';
  import { t } from '../../lib/texts';
  import IkonaProfilu from './profile-icon.svelte';
  import KafelkiTrybu from './direction-tiles.svelte';
  import ListaRozwijana from './dropdown.svelte';

  const osoba = $derived(stan.people.people.find((o) => o.id === stan.people.activePersonId));
  const profil = $derived(osoba?.profiles.find((p) => p.id === stan.people.activeProfileId));
  const wiele = $derived(!!osoba && osoba.profiles.length > 1 && stan.settings.showProfileSwitch);
  const nazwa = $derived(profil ? t('glowny.sterowanie.profil', { profil: profil.name }) : t('glowny.sterowanie.bez_profilu'));
  const ikona = $derived(profil?.icon ?? { color: stan.statics.defaultColor, glyph: stan.statics.defaultGlyph, url: null });
  const pozycje = $derived(
    (osoba?.profiles ?? []).map((p) => ({ id: p.id, tytul: p.name, aktywna: p.id === stan.people.activeProfileId })),
  );

  let listaOtwarta = $state(false);
  let wyzwalacz = $state<HTMLButtonElement>();
  // lista otwarta myszą zamyka się bez fokusu na wyzwalaczu (zgłoszenie: strzałki i Enter od
  // programu do pada zmieniały profil w oknie WalkLevel); z klawiatury fokus wraca jak zwykle
  let zKlawiatury = $state(false);
</script>

<Karta>
  <div class="naglowek">
    <IkonaProfilu {ikona} />
    <div class="nazwy">
      {#if wiele}
        <button
          type="button"
          class="profil"
          bind:this={wyzwalacz}
          aria-haspopup="dialog"
          aria-expanded={listaOtwarta}
          aria-label={t('glowny.sterowanie.profil_dostepnosc', { profil: nazwa })}
          title={nazwa}
          onclick={(e) => {
            zKlawiatury = e.detail === 0;
            listaOtwarta = !listaOtwarta;
          }}
        >
          <span class="nazwa jedna-linia dane">{nazwa}</span>
          <Ikona nazwa="w_dol" rozmiar={12} />
        </button>
      {:else}
        <h2 class="nazwa jedna-linia dane" title={nazwa}>{nazwa}</h2>
      {/if}
      <div class="klawisz">
        <button
          type="button"
          class="klawisz-kapsel"
          aria-label={t('glowny.sterowanie.klawisz_dostepnosc', { klawisz_wlaczania: stan.control.toggleLabel })}
          title={t('glowny.sterowanie.klawisz_podpowiedz')}
          onclick={() => idz('settings', { kategoria: 'sterowanie' })}
        >{stan.control.toggleLabel}</button>
        <span class="podtytul jedna-linia" title={stan.control.subtitle}>{stan.control.subtitle}</span>
      </div>
    </div>
    <Przelacznik
      wlaczony={stan.control.enabled}
      etykieta={t('glowny.sterowanie.przelacznik')}
      bezFokusuPoKliknieciu
      onzmiana={(on, zKlawiatury) => succeeded(api.controlSet(on, zKlawiatury))}
    />
  </div>

  <KafelkiTrybu />

  <div class="miejsce-podpowiedzi">
    <p class="podpowiedz" title={stan.control.hint}>{stan.control.hint}</p>
  </div>
</Karta>

{#if wiele}
  <ListaRozwijana
    bind:otwarta={listaOtwarta}
    {wyzwalacz}
    etykieta={t('glowny.sterowanie.lista')}
    {pozycje}
    linki={[{ tytul: t('glowny.sterowanie.zarzadzaj'), ikona: 'ustawienia', onclick: () => idz('settings', { kategoria: 'sterowanie' }) }]}
    fokusPoZamknieciu={zKlawiatury}
    onwybierz={(id, klawiatura) => id !== stan.people.activeProfileId && wykonaj(api.profileSwitch(id, 'main', klawiatura))}
  >
    {#snippet poczatek(id)}
      {@const p = osoba?.profiles.find((x) => x.id === id)}
      {#if p}<IkonaProfilu ikona={p.icon} rozmiar={28} />{/if}
    {/snippet}
  </ListaRozwijana>
{/if}

<style>
  .naglowek {
    display: grid;
    grid-template-columns: 40px minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .nazwy { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
  .profil {
    display: flex;
    align-items: center;
    gap: 6px;
    max-width: 100%;
    min-width: 0;
    margin-inline-start: -6px;
    padding: 0 6px;
    border-radius: var(--r-sm);
    color: var(--text);
    transition: background-color var(--t-color) linear;
  }
  .profil:hover { background: var(--surface-2); }
  .profil :global(.ikona) { color: var(--text-2); }
  .nazwa { font-size: var(--fs-lg); font-weight: var(--fw-bold); line-height: var(--lh-tight); overflow-wrap: normal; }
  .klawisz { display: flex; align-items: center; gap: 8px; min-width: 0; }
  .klawisz-kapsel {
    flex: none;
    padding: 1px 8px;
    border: 1px solid var(--control);
    border-bottom-width: 3px;
    border-radius: var(--r-sm);
    background: var(--surface);
    color: var(--text);
    font-size: var(--fs-sm);
    font-weight: var(--fw-bold);
    line-height: var(--lh-body);
    white-space: nowrap;
    transition: background-color var(--t-color) linear;
  }
  .klawisz-kapsel:hover { background: var(--surface-2); }
  .podtytul { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-tight); }
  /* Podpowiedź ma stałą liczbę linii (2, w wąskiej karcie 3 - tyle zajmuje najdłuższa,
     o trybie próbnym), więc zmiana trybu nie zmienia wysokości karty. */
  .miejsce-podpowiedzi { container-type: inline-size; min-width: 0; --linie: 2; }
  @container (max-width: 459px) {
    .podpowiedz { --linie: 3; }
  }
  .podpowiedz {
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: var(--linie);
    line-clamp: var(--linie);
    overflow: hidden;
    height: calc(var(--linie) * var(--lh-body) * var(--fs-sm));
    font-size: var(--fs-sm);
    color: var(--text-2);
    line-height: var(--lh-body);
  }
</style>
