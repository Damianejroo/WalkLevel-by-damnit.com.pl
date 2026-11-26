<script lang="ts">
  // Karta stanu połączenia z bieżnią (docs/wyglad.md 8.14, docs/teksty.md 3): stała wysokość 62,
  // kropka stanu (neutralna, „szukam” pulsuje), tytuł i podtytuł po jednej linii, przycisk akcji
  // o szerokości najdłuższego z możliwych napisów - zmiana stanu nie przesuwa niczego.
  // Teksty składa rdzeń (device.status, kontrakt 9.1) z kluczy glowny.status.*.
  import Przycisk from '../../lib/ui/Przycisk.svelte';
  import * as api from '../../lib/api';
  import { stan, wykonaj } from '../../lib/stan.svelte';
  import { t } from '../../lib/teksty';

  const status = $derived(stan.device.status);
  const NAPISY = [
    ...new Set([
      t('glowny.status.brak.akcja'),
      t('glowny.status.rozlaczona.akcja'),
      t('glowny.status.skanowanie.akcja'),
      t('glowny.status.polaczono.akcja'),
      t('glowny.status.ponawiam.akcja'),
      t('glowny.status.bt_wylaczony.akcja'),
      t('glowny.status.nieobslugiwane.akcja'),
      t('glowny.status.symulator.akcja'),
    ]),
  ];
  const kropka = $derived(
    status.tone === 'danger' ? 'blad' : status.tone === 'run' ? 'uwaga' : status.busy ? 'trwa' : status.tone === 'muted' ? 'brak' : 'ok',
  );
  // napis spoza listy (np. inny tekst z rdzenia) też musi się zmieścić
  const napisy = $derived(NAPISY.includes(status.actionLabel) ? NAPISY : [...NAPISY, status.actionLabel]);
</script>

<section class="kontener"><div class="stan" aria-live="polite">
  <span class="kropka {kropka}" class:pulsuje={status.busy} aria-hidden="true"></span>
  <span class="teksty">
    <span class="tytul jedna-linia" title={status.title}>{status.title}</span>
    <span class="podtytul jedna-linia" title={status.sub}>{status.sub}</span>
  </span>
  <Przycisk
    wariant={status.primary ? 'glowny' : 'zwykly'}
    napis={status.actionLabel}
    {napisy}
    onclick={() => wykonaj(api.deviceStatusAction())}
  />
</div></section>

<style>
  .kontener {
    container-type: inline-size;
    min-width: 0;
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: var(--surface);
    contain: layout paint;
  }
  .stan {
    display: grid;
    grid-template-columns: 10px minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
    height: 60px;
    padding: 0 8px 0 14px;
  }
  /* wąsko: tytuł i podtytuł na całą szerokość, przycisk pod nimi - wysokość zależy tylko od
     szerokości okna, nie od stanu */
  @container (max-width: 459px) {
    .stan {
      grid-template-columns: 10px minmax(0, 1fr);
      row-gap: 8px;
      height: auto;
      padding: 10px 12px 12px;
    }
    .stan > :global(.przycisk) { grid-column: 1 / -1; width: 100%; }
  }
  .kropka {
    width: 10px;
    height: 10px;
    border-radius: var(--r-round);
    background: var(--status-ok);
  }
  .trwa,
  .brak { background: var(--status-busy); }
  .uwaga { background: var(--status-warn); }
  .blad { background: var(--status-error); }
  .pulsuje { animation: wl-puls 1000ms var(--ease-smooth) infinite; }
  .teksty { display: flex; flex-direction: column; min-width: 0; }
  .tytul { font-size: var(--fs-base); font-weight: var(--fw-semibold); line-height: var(--lh-tight); }
  .podtytul { font-size: var(--fs-md); color: var(--text-2); line-height: var(--lh-tight); }
</style>
