<script lang="ts">
  // Ekran: Wybierz bieżnię (trasa 'devices'), docs/texts.md 4.2. Wyniki skanowania z
  // stan.device: bieżnie, inne urządzenia, wskazówki przy pustej liście, symulator.
  // „Szukaj ponownie” stoi na górze - wyniki pojawiają się pod nim, przycisk nie ucieka.
  // Akcent: „Szukaj ponownie”.
  import Strona from '../lib/ui/page.svelte';
  import Karta from '../lib/ui/card.svelte';
  import Przycisk from '../lib/ui/button.svelte';
  import Przelacznik from '../lib/ui/switch.svelte';
  import WierszListy from '../lib/ui/list-row.svelte';
  import { t, type Klucz } from '../lib/texts';
  import * as api from '../lib/api';
  import { idz, stan, wykonaj, type PropsEkranu, succeeded } from '../lib/state.svelte';
  import type { FoundDevice } from '../lib/contract';

  let _props: PropsEkranu = $props();

  const d = $derived(stan.device);
  const bieznie = $derived(d.devices.filter((u) => u.treadmill));
  const inne = $derived(d.devices.filter((u) => !u.treadmill).slice(0, 12));

  /** Progi z docs/texts.md 0.3: >= -60 silny, -61…-75 średni, < -75 słaby. */
  const sygnal = (rssi: number) =>
    t(rssi >= -60 ? 'bieznia.sygnal.silny' : rssi >= -75 ? 'bieznia.sygnal.sredni' : 'bieznia.sygnal.slaby');
  const nazwa = (u: FoundDevice) => u.name || t('bieznia.lista.bez_nazwy');
  const zapamietana = (u: FoundDevice) => d.remembered && u.address === stan.settings.deviceAddress;

  function opisBiezni(u: FoundDevice) {
    const model = u.model && u.model !== u.name ? u.model : t('bieznia.lista.bieznia_ogolna');
    const zm = { model, sygnal: sygnal(u.rssi) };
    return zapamietana(u) ? t('bieznia.lista.twoja_podpis', zm) : t('bieznia.lista.bieznia_podpis', zm);
  }

  async function polacz(u: FoundDevice) {
    const ok = await wykonaj(api.deviceConnect(u.address, u.name).then(() => true));
    if (ok) idz('main');
  }

  const pusta = $derived(d.bluetoothOff ? 'bieznia.lista.bt_wylaczony' : 'bieznia.lista.brak');
  const kroki = $derived(
    (d.bluetoothOff ? ['krok1', 'krok2', 'krok3'] : ['krok1', 'krok2', 'krok3', 'krok4']).map((k) => `${pusta}.${k}` as Klucz),
  );
</script>

<Strona tytul={t('bieznia.lista.tytul')} podpis={d.scanText}>
  <Przycisk
    wariant="glowny"
    napis={t('bieznia.lista.szukaj_ponownie')}
    szeroki
    disabled={d.scanning}
    onclick={() => void wykonaj(api.deviceScan())}
  />

  {#if d.scanning}
    <div class="szukam" role="status">
      <span class="kropka" aria-hidden="true"></span>
      <span>{t('glowny.status.skanowanie.podtytul')}</span>
    </div>
  {:else}
    {#if bieznie.length}
      <ul class="lista">
        {#each bieznie as u (u.address)}
          <li>
            <WierszListy
              tytul={nazwa(u)}
              opis={opisBiezni(u)}
              ikona="bieznia"
              wybrany={zapamietana(u)}
              onclick={() => void polacz(u)}
            />
          </li>
        {/each}
      </ul>
    {:else}
      <Karta tytul={t(`${pusta}.tytul` as Klucz)}>
        <ol class="kroki">
          {#each kroki as k (k)}
            <li>{t(k)}</li>
          {/each}
        </ol>
      </Karta>
    {/if}

    {#if inne.length}
      <h2 class="naglowek">{t('bieznia.lista.inne_naglowek')}</h2>
      <ul class="lista">
        {#each inne as u (u.address)}
          <li>
            <WierszListy
              tytul={nazwa(u)}
              opis={t('bieznia.lista.inne_podpis', { sygnal: sygnal(u.rssi) })}
              ikona="bluetooth"
              onclick={() => void polacz(u)}
            />
          </li>
        {/each}
      </ul>
    {/if}
  {/if}

  <Karta>
    <Przelacznik
      tytul={t('ustawienia.bieznia.symulator')}
      opis={t('ustawienia.bieznia.symulator_przelacznik_opis', {
        klawisz_szybciej: stan.settings.fasterKey.label,
        klawisz_wolniej: stan.settings.slowerKey.label,
      })}
      wlaczony={stan.settings.simulator}
      onzmiana={(v) => succeeded(api.settingsSet('simulator', v))}
    />
  </Karta>
</Strona>

<style>
  .lista { display: flex; flex-direction: column; gap: var(--sp-2); }
  .naglowek {
    margin-top: var(--sp-2);
    font-size: var(--fs-base);
    font-weight: var(--fw-bold);
  }
  .kroki {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .szukam {
    display: flex;
    align-items: flex-start;
    gap: var(--sp-3);
    min-height: var(--row-h);
    padding: 8px 12px;
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .kropka {
    width: 10px;
    height: 10px;
    flex: none;
    margin-top: calc((1lh - 10px) / 2);   /* środek pierwszej linii */
    border-radius: var(--r-round);
    background: var(--status-busy);
    animation: wl-puls 1000ms var(--ease-smooth) infinite;
  }
</style>
