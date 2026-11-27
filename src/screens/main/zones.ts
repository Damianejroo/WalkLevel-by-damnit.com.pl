// Strefy wskaźnika prędkości, progi i żetony dla trybu z ustawień profilu (render_zones z 0.5.x,
// docs/design.md 8.14, docs/texts.md 6.1). Czysta funkcja - liczona tylko przy zmianie ustawień.
import type { Settings } from '../../lib/contract';
import { liczba } from '../../lib/format';
import { t, tl } from '../../lib/texts';

/** Kolor strefy: tokeny --zone-stop / --zone-walk / --zone-run. */
export type KolorStrefy = 'stop' | 'walk' | 'run';

export interface Strefa { od: number; do: number; kolor: KolorStrefy }
export interface Prog { v: number; tekst: string; strefa: number }
export interface Zeton { nazwa: string; wartosc: string; kolor: KolorStrefy }

export interface UkladStref {
  strefy: Strefa[];
  progi: Prog[];
  /** Górna krawędź skali wskaźnika (km/h). */
  vmax: number;
  /** Żetony pod wskaźnikiem (w trybie Trening puste - jest pasek poziomów). */
  zetony: Zeton[];
}

const kmh = (v: number) => liczba(v, 1);

/** Strefy między kolejnymi prędkościami: granice w połowie odległości. */
function przedzialy(predkosci: number[], kolory: KolorStrefy[], vmax: number): Strefa[] {
  const ciecia = [0, ...predkosci.slice(1).map((v, i) => (predkosci[i] + v) / 2), vmax];
  return kolory.map((kolor, i) => ({ od: ciecia[i], do: ciecia[i + 1], kolor }));
}

function zeton(tekst: string, kolor: KolorStrefy): Zeton {
  const [nazwa, wartosc = ''] = tekst.split(' / ');
  return { nazwa, wartosc, kolor };
}

export function ukladStref(s: Settings, maxSpeed: number): UkladStref {
  const klawisze = { klawisz_chodu: s.walkKey.name, klawisz_biegu: s.runKey.name };

  if (s.direction === 'treadmill') {
    const lo = s.standBelow;
    const hi = s.runOn;
    return {
      strefy: [
        { od: 0, do: lo, kolor: 'stop' },
        { od: lo, do: hi, kolor: 'walk' },
        { od: hi, do: maxSpeed, kolor: 'run' },
      ],
      progi: [{ v: lo, tekst: kmh(lo), strefa: 1 }, { v: hi, tekst: kmh(hi), strefa: 2 }],
      vmax: maxSpeed,
      zetony: [
        zeton(t('glowny.strefy.stoisz', { kmh: kmh(lo) }), 'stop'),
        zeton(t('glowny.strefy.chod', { ...klawisze, kmh: kmh(lo) }), 'walk'),
        zeton(t('glowny.strefy.bieg', { ...klawisze, kmh: kmh(hi) }), 'run'),
      ],
    };
  }

  if (s.direction === 'keyboard') {
    // „Taśma staje”: bez klawisza 0 km/h i żeton „stop” (jak pad z puszczoną gałką)
    const stoi = s.keyIdle === 'stop';
    const p = [stoi ? 0 : s.idleSpeed, s.walkSpeed, s.runSpeed];
    const vmax = Math.max(3.5, Math.max(...p) * 1.4);
    const bez = zeton(t('glowny.strefy.bez_klawisza', { kmh: kmh(p[0]) }), 'stop');
    return {
      strefy: przedzialy(p, ['stop', 'walk', 'run'], vmax),
      progi: p.flatMap((v, i) => (v > 0 ? [{ v, tekst: kmh(v), strefa: i }] : [])),
      vmax,
      zetony: [
        stoi ? { ...bez, wartosc: t('glowny.strefy.stop') } : bez,
        zeton(t('glowny.strefy.klawisz', { ...klawisze, kmh: kmh(p[1]) }), 'walk'),
        zeton(t('glowny.strefy.klawisz_bieg', { ...klawisze, kmh: kmh(p[2]) }), 'run'),
      ],
    };
  }

  if (s.direction === 'training') {
    const p = [...s.trainingLevels];
    const vmax = Math.max(3, Math.max(...p) * 1.25);
    return {
      strefy: przedzialy(p, ['walk', 'walk', 'run', 'run'], vmax),
      progi: p.map((v, i) => ({ v, tekst: kmh(v), strefa: i })),
      vmax,
      zetony: [],
    };
  }

  // Pad: stopień 0 to gałka puszczona (stop / 0 km/h / prędkość stopnia 1)
  const kroki = s.padSteps;
  const bezRuchu = s.padIdle !== 'slow';
  const p = kroki.map(([, v], i) => (i === 0 && bezRuchu ? 0 : v));
  const vmax = Math.max(3, Math.max(...p) * 1.25);
  const n = kroki.length;
  const kolory: KolorStrefy[] = kroki.map((_, i) => (i === 0 ? 'stop' : i === n - 1 ? 'run' : 'walk'));
  const krotko = n > 4; // 5 żetonów: bez jednostki (jest pod liczbą na wskaźniku)
  return {
    strefy: przedzialy(p, kolory, vmax),
    progi: p.flatMap((v, i) => (v > 0 ? [{ v, tekst: kmh(v), strefa: i }] : [])),
    vmax,
    zetony: kroki.map(([at, v], i) => {
      const predkosc = i === 0 && s.padIdle === 'zero' ? 0 : v;
      const [nazwa, wartosc] = i === 0
        ? tl('glowny.strefy.puszczona', { kmh: kmh(predkosc) })
        : tl('glowny.strefy.stopien', { procent: at, kmh: kmh(predkosc) });
      return {
        nazwa: i === 0 && krotko ? t('glowny.strefy.puszczona_krotko') : nazwa,
        wartosc: i === 0 && s.padIdle === 'stop' ? t('glowny.strefy.stop') : krotko ? kmh(predkosc) : wartosc,
        kolor: kolory[i],
      };
    }),
  };
}
