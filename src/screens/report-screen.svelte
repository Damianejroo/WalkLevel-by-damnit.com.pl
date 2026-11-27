<script lang="ts">
  // Ekran: Zgłoś problem (trasa 'report'), docs/texts.md 18.3, kontrakt 6.11.
  // Jedyny akcent: „Wyślij zgłoszenie”. Za krótki opis - błąd tuż pod polem z liczbą brakujących
  // znaków, znika, gdy opis wystarcza. Stan wysyłki (report.busy/status) przychodzi z rdzenia;
  // błąd komendy (poza walidacją opisu) pokazujemy w tym samym miejscu na czerwono.
  import Strona from '../lib/ui/page.svelte';
  import Pole from '../lib/ui/text-field.svelte';
  import Segmentowy from '../lib/ui/segmented-control.svelte';
  import Karta from '../lib/ui/card.svelte';
  import Przycisk from '../lib/ui/button.svelte';
  import * as api from '../lib/api';
  import { t } from '../lib/texts';
  import { stan, type PropsEkranu } from '../lib/state.svelte';

  let _props: PropsEkranu = $props();

  let formularz: HTMLDivElement;
  let opis = $state('');
  let kontakt = $state('');
  let okres = $state(stan.statics.reportDefaultS);
  /** Po próbie wysłania za krótkiego opisu - błąd pod polem, liczony na żywo. */
  let pokazBlad = $state(false);
  /** Komunikat błędu ostatniej komendy (sieć, dysk, serwer). */
  let bladKomendy = $state('');
  /** Komenda w toku, zanim rdzeń zgłosi report.busy - drugie kliknięcie nie wysyła drugi raz. */
  let wysylam = $state(false);

  const minimum = $derived(stan.statics.reportMinChars);
  // characters as the core counts them (code points, so an emoji is one character)
  const dlugosc = $derived([...opis.trim()].length);
  /** Description error returned by the core (shown until the description changes). */
  let coreError = $state('');
  const bladOpisu = $derived(
    !pokazBlad
      ? ''
      : coreError ||
          (dlugosc >= minimum
            ? ''
            : dlugosc === 0
              ? t('zgloszenie.blad.pusty')
              : t('zgloszenie.blad.krotki', { liczba: minimum - dlugosc })),
  );
  const okresy = $derived(stan.statics.reportPeriods.map((p) => ({ wartosc: p.seconds, napis: p.label })));
  const serwer = $derived(stan.app.reportEndpoint);
  const zajety = $derived(stan.report.busy || wysylam);
  const status = $derived(bladKomendy || stan.report.status);

  function sprawdzOpis() {
    coreError = '';
    if (pokazBlad && dlugosc >= minimum) pokazBlad = false;
  }

  function pokazBladOpisu() {
    pokazBlad = true;
    formularz.querySelector('textarea')?.focus();
  }

  async function wykonajKomende(wysylka: boolean) {
    if (zajety) return;
    bladKomendy = '';
    wysylam = true;
    try {
      if (wysylka) {
        await api.reportSend(opis.trim(), kontakt.trim(), okres);
        // jak w 0.5.x: opis znika tylko po wysłaniu przez serwer; przy e-mailu zostaje
        if (serwer) opis = '';
      } else {
        await api.reportSave(opis.trim(), kontakt.trim(), okres);
      }
    } catch (e) {
      if (!api.jestBledemKomendy(e) || e.code === 'cancelled') return;
      if (e.code === 'invalid' && wysylka) {
        coreError = e.message;
        pokazBladOpisu();
      } else bladKomendy = e.message;
    } finally {
      wysylam = false;
    }
  }

  function wyslij() {
    if (dlugosc < minimum) return pokazBladOpisu();
    void wykonajKomende(true);
  }
</script>

<Strona tytul={t('zgloszenie.tytul')}>
  <div class="zgloszenie" bind:this={formularz}>
    <p class="wstep">{t('zgloszenie.wstep')}</p>

    <Pole
      typ="wielowierszowe"
      etykieta={t('zgloszenie.opis_etykieta')}
      podpowiedz={t('zgloszenie.opis_przyklad')}
      blad={bladOpisu}
      maxlength={5000}
      bind:wartosc={opis}
      oninput={sprawdzOpis}
    />

    <Pole
      etykieta={t('zgloszenie.kontakt_etykieta')}
      podpowiedz={t('zgloszenie.kontakt_przyklad')}
      autocomplete="off"
      maxlength={200}
      bind:wartosc={kontakt}
    />

    <div class="okres">
      <span class="etykieta" aria-hidden="true">{t('zgloszenie.okres_etykieta')}</span>
      <Segmentowy opcje={okresy} etykieta={t('zgloszenie.okres_dostepnosc')} bind:wartosc={okres} />
      <p class="podpowiedz">{t('zgloszenie.okres_opis')}</p>
    </div>

    <Karta tytul={t('zgloszenie.dolaczone_tytul')}>
      <ul class="lista">
        <li>{t('zgloszenie.dolaczone1')}</li>
        <li>{t('zgloszenie.dolaczone2')}</li>
        <li>{t('zgloszenie.dolaczone3')}</li>
      </ul>
      <p class="zdanie">{t('zgloszenie.niedolaczone')}</p>
      <p class="zdanie">{serwer ? t('zgloszenie.droga_ogolna') : t('zgloszenie.droga_email')}</p>
    </Karta>

    <div class="akcje">
      <Przycisk
        wariant="glowny"
        napis={serwer ? t('zgloszenie.wyslij') : t('zgloszenie.przygotuj_email')}
        disabled={zajety}
        onclick={wyslij}
      />
      <Przycisk
        napis={t('zgloszenie.zapisz')}
        title={t('zgloszenie.zapisz_podpowiedz')}
        disabled={zajety}
        onclick={() => wykonajKomende(false)}
      />
    </div>
    <p class="status" class:blad={!!bladKomendy} class:trwa={zajety} role="status" aria-live="polite">{status}</p>
  </div>
</Strona>

<style>
  .zgloszenie {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    min-width: 0;
  }
  .wstep { color: var(--text-2); font-size: var(--fs-md); }
  .okres { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
  .etykieta { font-size: var(--fs-md); font-weight: var(--fw-semibold); }
  .podpowiedz { font-size: var(--fs-sm); color: var(--text-2); }

  .lista {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding-inline-start: 1.25em;
    list-style: disc;
    font-size: var(--fs-md);
  }
  .lista li::marker { color: var(--text-2); }
  .zdanie { font-size: var(--fs-sm); color: var(--text-2); }

  .akcje {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2);
    min-width: 0;
  }
  .akcje > :global(*) { flex: 1 1 200px; }

  /* Miejsce na dwie linie stanu zarezerwowane - przyciski nad nim nie skaczą */
  .status {
    min-height: calc(2 * var(--lh-body) * var(--fs-md));
    margin-top: calc(-1 * var(--sp-2));
    font-size: var(--fs-md);
    color: var(--text);
  }
  .status.trwa { color: var(--text-2); }
  .status.blad { color: var(--danger); }
</style>
