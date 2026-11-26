<script lang="ts">
  // Ustawienia jednego trybu sterowania (docs/teksty.md 12.1-12.4) na karcie „Sterowanie”.
  // Pokazuje tryb wybrany w przełączniku karty - nie musi być aktywny (ustawienia na później).
  import Segmentowy from '../../lib/ui/Segmentowy.svelte';
  import Przelacznik from '../../lib/ui/Przelacznik.svelte';
  import PoleLiczby from './PoleLiczby.svelte';
  import PrzyciskKlawisza from './PrzyciskKlawisza.svelte';
  import OsStopni from './OsStopni.svelte';
  import Rzad from './Rzad.svelte';
  import { t, tl } from '../../lib/teksty';
  import { liczba } from '../../lib/format';
  import { stan, zywo, wykonaj } from '../../lib/stan.svelte';
  import * as api from '../../lib/api';
  import type { Direction, KeyIdle, OutputKind, PadIdle, SettingKey, Settings } from '../../lib/typy';

  let { tryb }: { tryb: Direction } = $props();

  const s = $derived(stan.settings);
  const caps = $derived(stan.device.caps);
  const kmh = (x: number) => liczba(x, 1);

  // --- Chodzenie ---
  const [etykietaWyjscia, napisyWyjscia] = (() => {
    const tekst = t('ustawienia.chodzenie.klawisze_w_grze');
    const i = tekst.indexOf(': ');
    return [tekst.slice(0, i), tekst.slice(i + 2).split(' / ')];
  })();
  const opcjeWyjscia: { wartosc: OutputKind; napis: string }[] = [
    { wartosc: 'keys', napis: napisyWyjscia[0] },
    { wartosc: 'dry', napis: napisyWyjscia[1] },
  ];

  // --- Klawisze: puszczony klawisz i przytrzymanie przed przyspieszeniem (0.6.1) ---
  const opcjePuszczonego: { wartosc: KeyIdle; napis: string }[] = [
    { wartosc: 'slow', napis: t('ustawienia.klawisze.puszczony.wolno') },
    { wartosc: 'stop', napis: t('ustawienia.klawisze.puszczony.stop') },
  ];
  const opcjePrzytrzymania = [0, 1, 2, 3].map((sekundy) => ({
    wartosc: sekundy,
    napis: sekundy === 0 ? t('ustawienia.klawisze.przytrzymanie.od_razu') : t('ustawienia.klawisze.przytrzymanie.sekundy', { sekundy }),
  }));
  /** Opis pola „Bez klawisza” przy „Taśma staje”: z rozruchem, gdy bieżnia rusza co najmniej 2 s. */
  const opisStoi = $derived(
    caps.startDelayS >= 2
      ? t('ustawienia.klawisze.bez_stoi_rozruch', { sekundy: Math.round(caps.startDelayS) })
      : t('ustawienia.klawisze.bez_stoi'),
  );

  // --- Pad ---
  const opcjeLiczby = ([2, 3, 4, 5] as const).map((n) => ({ wartosc: n, napis: String(n) }));
  const opcjeGalki: { wartosc: PadIdle; napis: string }[] = [
    { wartosc: 'stop', napis: t('ustawienia.pad.puszczona.stop') },
    { wartosc: 'slow', napis: t('ustawienia.pad.puszczona.wolno') },
    { wartosc: 'zero', napis: t('ustawienia.pad.puszczona.zero') },
  ];
  const pad = $derived(zywo.telemetria?.pad ?? null);
  const stanPada = $derived(pad?.name ? pad.statusText : t('ustawienia.pad.brak'));
  /** Napis gałki puszczonej, gdy taśma nie jedzie tempem pierwszego stopnia. */
  const puszczona = $derived(
    s.padIdle === 'stop' ? t('ustawienia.pad.puszczona.stop') : s.padIdle === 'zero' ? t('pole.kmh', { kmh: kmh(0) }) : undefined,
  );
  const predkosci = $derived(s.padSteps.map(([, v], i) => (i === 0 && puszczona ? puszczona : kmh(v))));
  const [padMin, padMax] = $derived(stan.statics.padSpeedRange);
  const [trenMin, trenMax] = $derived(stan.statics.trainingRange);

  const zapisz = <K extends SettingKey>(klucz: K, v: Settings[K]) => wykonaj(api.settingsSet(klucz, v));
</script>

{#if tryb === 'treadmill' || tryb === 'keyboard'}
  {@const klawisze = tryb === 'keyboard'}
  <Rzad>
    <PrzyciskKlawisza
      rola="walk"
      nazwaRoli={t(klawisze ? 'ustawienia.klawisze.klawisz_chodu' : 'ustawienia.chodzenie.klawisz_chodu')}
      klawisz={s.walkKey.label || s.walkKey.name}
    />
    <PrzyciskKlawisza
      rola="run"
      nazwaRoli={t(klawisze ? 'ustawienia.klawisze.klawisz_biegu' : 'ustawienia.chodzenie.klawisz_biegu')}
      klawisz={s.runKey.label || s.runKey.name}
    />
  </Rzad>
{/if}

{#if tryb === 'treadmill'}
  <div class="grupa">
    <span class="etykieta">{etykietaWyjscia}</span>
    <Segmentowy opcje={opcjeWyjscia} wartosc={s.output} etykieta={etykietaWyjscia} onzmiana={(v) => zapisz('output', v)} />
    <p class="opis">{t('ustawienia.chodzenie.klawisze_w_grze_opis')}</p>
  </div>
  <PoleLiczby
    tytul={t('ustawienia.chodzenie.chod_od')}
    wartosc={s.standBelow}
    min={0}
    max={stan.statics.maxSpeed}
    onzmiana={(v) => wykonaj(api.thresholdsSet('walk', v))}
  />
  <PoleLiczby
    tytul={t('ustawienia.chodzenie.bieg_od')}
    wartosc={s.runOn}
    min={0}
    max={stan.statics.maxSpeed}
    onzmiana={(v) => wykonaj(api.thresholdsSet('run', v))}
  />
  <p class="opis">{t('ustawienia.chodzenie.histereza', { kmh: kmh(stan.statics.hysteresis), klawisz_biegu: s.runKey.name })}</p>
  <Przelacznik
    tytul={t('ustawienia.chodzenie.szybkie_stop')}
    opis={t('ustawienia.chodzenie.szybkie_stop_opis')}
    wlaczony={s.quickStop}
    onzmiana={(v) => zapisz('quickStop', v)}
  />
{:else if tryb === 'keyboard'}
  <p class="opis">{t('ustawienia.klawisze.opis')}</p>
  {@const bez = tl('ustawienia.klawisze.bez')}
  {@const chod = tl('ustawienia.klawisze.chod', { klawisz_chodu: s.walkKey.name })}
  {@const bieg = tl('ustawienia.klawisze.bieg', { klawisz_chodu: s.walkKey.name, klawisz_biegu: s.runKey.name })}
  {@const stoi = s.keyIdle === 'stop'}
  <PoleLiczby tytul={chod[0]} opis={chod[1]} wartosc={s.walkSpeed} min={caps.speedMin} max={caps.speedMax} onzmiana={(v) => zapisz('walkSpeed', v)} />
  <PoleLiczby tytul={bieg[0]} opis={bieg[1]} wartosc={s.runSpeed} min={caps.speedMin} max={caps.speedMax} onzmiana={(v) => zapisz('runSpeed', v)} />
  <div class="grupa">
    <span class="etykieta">{t('ustawienia.klawisze.puszczony')}</span>
    <Segmentowy opcje={opcjePuszczonego} wartosc={s.keyIdle} etykieta={t('ustawienia.klawisze.puszczony')} onzmiana={(v) => zapisz('keyIdle', v)} />
  </div>
  <!-- przy „Taśma staje” to samo pole z napisem zamiast liczby - bez zmiany wysokości układu -->
  <PoleLiczby
    tytul={bez[0]}
    opis={stoi ? opisStoi : bez[1]}
    wartosc={s.idleSpeed}
    min={caps.speedMin}
    max={caps.speedMax}
    stala={stoi ? t('ustawienia.klawisze.puszczony.stop') : undefined}
    onzmiana={(v) => zapisz('idleSpeed', v)}
  />
  <div class="grupa">
    <span class="etykieta">{t('ustawienia.klawisze.przytrzymanie')}</span>
    <Segmentowy opcje={opcjePrzytrzymania} wartosc={s.keyHoldS} etykieta={t('ustawienia.klawisze.przytrzymanie')} onzmiana={(v) => zapisz('keyHoldS', v)} />
    <p class="opis">{t('ustawienia.klawisze.przytrzymanie_opis')}</p>
  </div>
{:else if tryb === 'training'}
  <p class="opis">
    {t('ustawienia.trening.opis', {
      klawisz_wlaczania: s.toggleKey.label,
      klawisz_szybciej: stan.settings.fasterKey.label,
      klawisz_wolniej: stan.settings.slowerKey.label,
    })}
  </p>
  {#each s.trainingLevels as v, i (i)}
    <PoleLiczby
      tytul={t('ustawienia.trening.poziom', { poziom: i + 1 })}
      wartosc={v}
      min={trenMin}
      max={trenMax}
      onzmiana={(x) => wykonaj(api.trainingSpeedSet(i as 0 | 1 | 2 | 3, x))}
    />
  {/each}
{:else}
  <p class="opis stan-pada jedna-linia" title={stanPada}>{stanPada}</p>
  <p class="opis">{t('ustawienia.pad.drabinka_opis')}</p>
  <OsStopni
    kroki={s.padSteps}
    {predkosci}
    galka={pad?.name ? pad.deflection : null}
    minOdstep={stan.statics.padMinGap}
    etykieta={t('ustawienia.pad.drabinka_dostepnosc')}
    opisKroku={(i, procent) => tl('ustawienia.pad.stopien', { stopien: i, procent }).join(', ')}
    onprog={(i, at) => wykonaj(api.padStepSet(i, at, null))}
  />
  <div class="liczba-stopni">
    <span class="etykieta">{t('ustawienia.pad.liczba')}</span>
    <div class="segmenty">
      <Segmentowy
        opcje={opcjeLiczby}
        wartosc={s.padSteps.length as 2 | 3 | 4 | 5}
        etykieta={t('ustawienia.pad.liczba')}
        onzmiana={(n) => wykonaj(api.padStepsCount(n))}
      />
    </div>
  </div>
  <div class="grupa">
    <span class="etykieta">{t('ustawienia.pad.puszczona')}</span>
    <Segmentowy opcje={opcjeGalki} wartosc={s.padIdle} etykieta={t('ustawienia.pad.puszczona')} onzmiana={(v) => zapisz('padIdle', v)} />
    {#if caps.startDelayS >= 2 && s.padIdle === 'stop'}
      <p class="opis">{t('ustawienia.pad.odliczanie', { sekundy: Math.round(caps.startDelayS) })}</p>
    {/if}
  </div>
  {#each s.padSteps as [at, v], i (i)}
    {@const nazwa = i === 0 ? [t('ustawienia.pad.stopien_puszczona')] : tl('ustawienia.pad.stopien', { stopien: i, procent: at })}
    <PoleLiczby
      tytul={nazwa[0]}
      opis={nazwa[1]}
      wartosc={v}
      min={padMin}
      max={padMax}
      stala={i === 0 ? puszczona : undefined}
      onzmiana={(x) => wykonaj(api.padStepSet(i, null, x))}
    />
  {/each}
{/if}

<style>
  .opis { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-body); }
  .grupa { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
  .etykieta { font-size: var(--fs-base); line-height: var(--lh-tight); }
  /* „Liczba stopni”: napis i przełącznik obok siebie, w wąskiej karcie przełącznik schodzi niżej */
  .liczba-stopni {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2) var(--sp-3);
    min-width: 0;
  }
  .liczba-stopni .etykieta { flex: 1 1 120px; }
  .segmenty { flex: 1 1 200px; min-width: 0; }
</style>
