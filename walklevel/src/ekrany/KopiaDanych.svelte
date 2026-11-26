<script lang="ts">
  // Ekran: Kopia danych - zapis (trasa 'export'), docs/teksty.md 17.1, kontrakt 6.10.
  // Wybór osób i profili (pole osoby „częściowo”, gdy zaznaczona część profili), co zapisać,
  // zakres wyników, podsumowanie liczone przez rdzeń na żywo i zapis pliku (okno systemowe).
  // Akcent: tylko przycisk „Zapisz plik kopii…”.
  import { untrack } from 'svelte';
  import Strona from '../lib/ui/Strona.svelte';
  import Karta from '../lib/ui/Karta.svelte';
  import Przycisk from '../lib/ui/Przycisk.svelte';
  import Segmentowy from '../lib/ui/Segmentowy.svelte';
  import PoleWyboru, { type Zaznaczenie } from './Kopia/PoleWyboru.svelte';
  import Awatar from './Kopia/Awatar.svelte';
  import IkonaProfilu from './Kopia/IkonaProfilu.svelte';
  import * as api from '../lib/api';
  import { t, tl } from '../lib/teksty';
  import { stan, wroc, wykonaj, type PropsEkranu } from '../lib/stan.svelte';
  import type { ExportChoice, ExportRange, ExportSummary, Person } from '../lib/typy';

  let { aktywny }: PropsEkranu = $props();

  let wybrane = $state<Record<string, string[]>>({});
  let ustawienia = $state(true);
  let wyniki = $state(true);
  let logi = $state(false);
  let zakres = $state<ExportRange>('all');
  let podsumowanie = $state<ExportSummary | null>(null);
  let zapisuje = $state(false);

  const osoby = $derived(stan.people?.people ?? []);

  // Każde wejście na ekran zaczyna od pełnego wyboru (jak w 0.5.x).
  let byl = false;
  $effect(() => {
    const teraz = aktywny;
    untrack(() => {
      if (teraz && !byl) {
        wybrane = Object.fromEntries(osoby.map((o) => [o.id, o.profiles.map((p) => p.id)]));
        ustawienia = true;
        wyniki = true;
        logi = false;
        zakres = 'all';
      }
    });
    byl = teraz;
  });

  const wybor = $derived<ExportChoice>({
    people: Object.fromEntries(
      osoby.map((o) => [o.id, o.profiles.map((p) => p.id).filter((id) => wybrane[o.id]?.includes(id))]),
    ),
    settings: ustawienia,
    results: wyniki,
    logs: logi,
    range: zakres,
  });
  const ileOsob = $derived(Object.values(wybor.people).filter((l) => l.length > 0).length);
  const blad = $derived(
    ileOsob === 0 ? t('kopia.blad.brak_osoby') : !(ustawienia || wyniki || logi) ? t('kopia.blad.brak_tresci') : null,
  );

  // Podsumowanie z rdzenia przy każdej zmianie wyboru; starsza odpowiedź nie nadpisze nowszej.
  let numer = 0;
  $effect(() => {
    if (!aktywny || blad) return;
    const c = $state.snapshot(wybor);
    const n = ++numer;
    api.backupExportSummary(c).then(
      (s) => { if (n === numer) podsumowanie = s; },
      () => { if (n === numer) podsumowanie = null; },
    );
  });

  const tekstStopki = $derived(
    blad ??
      (podsumowanie
        ? t('kopia.eksport.podsumowanie', { liczba: podsumowanie.people, profile: podsumowanie.profiles, sesje: podsumowanie.sessions })
        : ''),
  );

  function zaznaczenieOsoby(o: Person): Zaznaczenie {
    const ile = wybor.people[o.id]?.length ?? 0;
    return ile === 0 ? 'nie' : ile === o.profiles.length ? 'tak' : 'czesciowo';
  }
  function przelaczOsobe(o: Person, wlacz: boolean) {
    wybrane[o.id] = wlacz ? o.profiles.map((p) => p.id) : [];
  }
  function przelaczProfil(o: Person, id: string, wlacz: boolean) {
    const obecne = (wybrane[o.id] ?? []).filter((x) => x !== id);
    wybrane[o.id] = wlacz ? [...obecne, id] : obecne;
  }

  async function zapisz() {
    if (blad || zapisuje) return;
    zapisuje = true;
    const wynik = await wykonaj(api.backupExport($state.snapshot(wybor)));
    zapisuje = false;
    if (wynik) wroc(); // null - okno zapisu zamknięte bez wyboru, zostajemy
  }

  const [ustawieniaTytul, ustawieniaOpis] = tl('kopia.eksport.ustawienia');
  const [wynikiTytul, wynikiOpis] = tl('kopia.eksport.wyniki');
  const [logiTytul, logiOpis] = tl('kopia.eksport.logi');
  const zakresy: { wartosc: ExportRange; napis: string }[] = [
    { wartosc: 'all', napis: t('kopia.eksport.zakres.wszystko') },
    { wartosc: 'year', napis: t('kopia.eksport.zakres.rok') },
    { wartosc: 'month', napis: t('kopia.eksport.zakres.30') },
  ];
</script>

<Strona tytul={t('kopia.eksport.tytul')}>
  <Karta tytul={t('kopia.eksport.osoby')}>
    <div class="lista">
      {#each osoby as o (o.id)}
        <div class="osoba">
          <PoleWyboru
            zaznaczenie={zaznaczenieOsoby(o)}
            tytul={o.name}
            opis={t('kopia.eksport.profile_wybrane', { liczba: wybor.people[o.id]?.length ?? 0, razem: o.profiles.length })}
            onzmiana={(w) => przelaczOsobe(o, w)}
          >
            {#snippet poczatek()}<Awatar nazwa={o.name} kolor={o.color} />{/snippet}
          </PoleWyboru>
          {#each o.profiles as p (p.id)}
            <PoleWyboru
              wciecie
              zaznaczenie={wybor.people[o.id]?.includes(p.id) ? 'tak' : 'nie'}
              tytul={p.name}
              onzmiana={(w) => przelaczProfil(o, p.id, w)}
            >
              {#snippet poczatek()}<IkonaProfilu ikona={p.icon} />{/snippet}
            </PoleWyboru>
          {/each}
        </div>
      {/each}
    </div>
  </Karta>

  <Karta tytul={t('kopia.eksport.co')}>
    <div class="lista">
      <PoleWyboru zaznaczenie={ustawienia ? 'tak' : 'nie'} tytul={ustawieniaTytul} opis={ustawieniaOpis} onzmiana={(w) => (ustawienia = w)} />
      <PoleWyboru zaznaczenie={wyniki ? 'tak' : 'nie'} tytul={wynikiTytul} opis={wynikiOpis} onzmiana={(w) => (wyniki = w)} />
      <PoleWyboru zaznaczenie={logi ? 'tak' : 'nie'} tytul={logiTytul} opis={logiOpis} onzmiana={(w) => (logi = w)} />
    </div>
  </Karta>

  <Karta tytul={t('kopia.eksport.zakres')}>
    <div class:wylaczony={!wyniki}>
      <Segmentowy opcje={zakresy} bind:wartosc={zakres} etykieta={t('kopia.eksport.zakres')} disabled={!wyniki} />
    </div>
  </Karta>

  <div class="stopka">
    <p class="podsumowanie" class:blad={!!blad} aria-live="polite">{tekstStopki}</p>
    <span class="akcja">
      <Przycisk wariant="glowny" szeroki napis={t('kopia.eksport.zapisz')} disabled={!!blad || zapisuje} onclick={zapisz} />
    </span>
  </div>
  <p class="uwaga">{t('kopia.eksport.opis_pliku')}</p>
</Strona>

<style>
  /* wiersze z polami mają własny odstęp wewnętrzny - wyrównanie do krawędzi tytułu karty */
  .lista { display: flex; flex-direction: column; margin-inline: calc(-1 * var(--sp-2)); }
  /* zakres bez znaczenia, gdy wyniki nie są zapisywane */
  .wylaczony { opacity: 0.6; }
  .osoba + .osoba { margin-top: var(--sp-1); padding-top: var(--sp-1); border-top: 1px solid var(--line); }

  .stopka {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--sp-2) var(--sp-4);
    margin-top: var(--sp-1);
  }
  /* tekst zabiera wolne miejsce w rzędzie; po zawinięciu przycisk ma całą szerokość */
  .podsumowanie {
    flex: 999 1 220px;
    min-width: 0;
    min-height: calc(var(--fs-md) * var(--lh-body));
    font-size: var(--fs-md);
    color: var(--text-2);
  }
  .podsumowanie.blad { color: var(--text); font-weight: var(--fw-semibold); }
  .akcja { display: flex; flex: 1 0 auto; min-width: 0; }
  .uwaga { font-size: var(--fs-sm); color: var(--text-2); }
</style>
