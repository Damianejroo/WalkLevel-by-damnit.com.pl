<script lang="ts">
  // Ekran: Obsługiwane bieżnie (trasa 'models'), docs/teksty.md 14.1. Lista profili bieżni
  // pogrupowana po rodzaju (KingSmith, FTMS), pobieranie nowych i dodawanie z pliku.
  // Dane: stan.models (przy każdym wejściu api.modelsRefresh). Akcent: „Sprawdź nowe modele”.
  import Strona from '../lib/ui/Strona.svelte';
  import Karta from '../lib/ui/Karta.svelte';
  import Przycisk from '../lib/ui/Przycisk.svelte';
  import { t } from '../lib/teksty';
  import { liczba } from '../lib/format';
  import * as api from '../lib/api';
  import { stan, wykonaj, type PropsEkranu } from '../lib/stan.svelte';
  import type { ModelItem } from '../lib/typy';

  let { aktywny }: PropsEkranu = $props();

  $effect(() => {
    if (aktywny) void wykonaj(api.modelsRefresh());
  });

  interface Grupa {
    rodzaj: string;
    nazwa: string;
    modele: ModelItem[];
  }

  // Rodzaje po nazwie; w rodzaju konkretne modele przed profilami całej marki
  const grupy = $derived.by(() => {
    const wg = new Map<string, Grupa>();
    for (const m of stan.models.items) {
      const g = wg.get(m.driver) ?? { rodzaj: m.driver, nazwa: m.driverName, modele: [] };
      g.modele.push(m);
      wg.set(m.driver, g);
    }
    const lista = [...wg.values()].sort((a, b) => a.nazwa.localeCompare(b.nazwa, 'pl'));
    for (const g of lista) g.modele.sort((a, b) => Number(a.generic) - Number(b.generic) || a.name.localeCompare(b.name, 'pl'));
    return lista;
  });

  const kmh = (x: number) => liczba(x, 1);

  function szczegoly(m: ModelItem): string {
    const czesci: string[] = [];
    if (m.generic) czesci.push(t('modele.marka_reszta'));
    czesci.push(t('modele.zakres', { min_kmh: kmh(m.caps.speedMin), max_kmh: kmh(m.caps.speedMax) }));
    if (m.caps.startDelayS >= 1) czesci.push(t('modele.rozruch', { sekundy: Math.round(m.caps.startDelayS) }));
    if (m.match.length) czesci.push(t('modele.nazwy', { lista: m.match.slice(0, 3).map((n) => `${n}…`).join(', ') }));
    czesci.push(m.source === 'wbudowany' ? t('modele.wbudowany') : t('modele.dodatkowy'));
    return czesci.join(' · ');
  }
</script>

<Strona tytul={t('modele.tytul')}>
  <p class="opis">{t('modele.opis')}</p>

  <div class="akcje">
    <span class="akcja">
      <Przycisk napis={t('modele.dodaj')} title={t('modele.dodaj_podpowiedz')} szeroki onclick={() => void wykonaj(api.modelsAddFile())} />
    </span>
    <span class="akcja">
      <Przycisk
        wariant="glowny"
        napis={t('modele.sprawdz')}
        szeroki
        disabled={stan.models.busy}
        onclick={() => void wykonaj(api.modelsFetch())}
      />
    </span>
  </div>
  {#if stan.models.status}<p class="status" role="status">{stan.models.status}</p>{/if}

  {#each grupy as g (g.rodzaj)}
    <Karta tytul={g.nazwa}>
      <ul class="lista">
        {#each g.modele as m (m.id)}
          <li class="model">
            <span class="tekst">
              <span class="nazwa dane">{m.mine ? t('modele.twoja', { model: m.name }) : m.name}</span>
              <span class="szczegoly dane">{szczegoly(m)}</span>
            </span>
            {#if m.removable}
              <Przycisk
                napis={t('modele.usun')}
                etykieta={t('modele.usun_dostepnosc', { model: m.name })}
                onclick={() => void wykonaj(api.modelsRemove(m.id))}
              />
            {/if}
          </li>
        {/each}
      </ul>
    </Karta>
  {/each}
</Strona>

<style>
  .opis {
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .akcje {
    display: flex;
    flex-wrap: wrap;
    gap: var(--sp-2);
  }
  /* podstawa = szerokość napisu: oba w rzędzie, gdy się mieszczą, inaczej jeden pod drugim */
  .akcja { display: flex; flex: 1 1 auto; min-width: 0; }
  /* komunikat pod przyciskami - zmienia tylko to, co niżej; kliknięty przycisk stoi w miejscu */
  .status {
    margin-top: calc(var(--sp-1) * -1);
    font-size: var(--fs-sm);
    color: var(--text-2);
  }
  .lista { display: flex; flex-direction: column; }
  .model {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    min-width: 0;
    padding-block: 10px;
  }
  .model + .model { border-top: 1px solid var(--line); }
  .model:last-child { padding-bottom: 0; }
  .tekst { display: flex; flex-direction: column; gap: 2px; flex: 1; min-width: 0; }
  .nazwa {
    font-size: var(--fs-base);
    font-weight: var(--fw-semibold);
    line-height: var(--lh-tight);
  }
  .szczegoly {
    font-size: var(--fs-sm);
    color: var(--text-2);
    line-height: var(--lh-body);
  }
</style>
