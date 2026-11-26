// Liczenie list osiągnięć i podpisów dla ekranów Osiągnięcia i Szlak (kontrakt 6.7: listy i
// szlak liczy UI z `statics.definitions` i wycinka `levels`).
import type { Achievement, AchievementProgress } from '../../lib/typy';
import { liczba } from '../../lib/format';
import { t } from '../../lib/teksty';

/** Osiągnięcie z definicji razem z postępem osoby. */
export interface Pozycja {
  a: Achievement;
  p: AchievementProgress;
  zdobyte: boolean;
}

/** Km jak w 0.5.x: od 100 bez części dziesiętnej („1 257”), niżej z jedną („44,7”, „2”). */
export function kmKrotko(km: number): string {
  if (km >= 100) return liczba(km, 0);
  const s = liczba(km, 1);
  return s.endsWith(',0') ? s.slice(0, -2) : s;
}

/** Podpis paska postępu („12,3 / 42 km”, „15 600 / 20 000 kroków”); pusty - bez paska. */
export function podpisPostepu(a: Achievement, p: AchievementProgress): string {
  switch (a.kind) {
    case 'dystans':
    case 'sesja_km':
    case 'gra_km':
      return t('poziomy.postep.km', { liczba: kmKrotko(p.current), razem: kmKrotko(p.target) });
    case 'dzien_kroki':
      return t('poziomy.postep.kroki', { liczba: Math.floor(p.current), razem: Math.round(p.target) });
    case 'seria':
      return t('poziomy.postep.dni', { liczba: Math.floor(p.current), razem: Math.round(p.target) });
    case 'gry_km':
      return t('poziomy.postep.gry', { liczba: Math.floor(p.current), razem: Math.round(p.target) });
    default:
      return '';
  }
}

/** „dziś”, „wczoraj”, „3 dni temu”, powyżej 30 dni data (dni kalendarzowe, czas lokalny). */
export function kiedy(utcS: number, teraz = new Date()): string {
  const dzien = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  const data = new Date(utcS * 1000);
  const dni = Math.round((dzien(teraz) - dzien(data)) / 86_400_000);
  if (dni <= 0) return t('osiagniecia.dzis');
  if (dni === 1) return t('osiagniecia.wczoraj');
  if (dni <= 30) return t('poziomy.osiagniecie.dni_temu', { dni });
  return data.toLocaleDateString('pl-PL', { day: 'numeric', month: '2-digit', year: 'numeric' });
}
