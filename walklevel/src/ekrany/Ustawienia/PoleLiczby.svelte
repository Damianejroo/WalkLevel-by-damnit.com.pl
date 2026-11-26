<script lang="ts">
  // Pole liczby km/h: tytuł + opis | − wartość + (docs/wyglad.md 8.6 „Wartość steppera”).
  // Klik ±0,1; przytrzymanie powtarza, po 6 powtórzeniach co 0,5. W wartość można kliknąć i
  // wpisać liczbę: strzałki ±0,1, PageUp/PageDown ±0,5, Enter zapisuje, Esc przywraca.
  // `stala` - zamiast liczby stały napis (np. „Taśma staje”), bez zmiany.
  import Ikona from '../../lib/ui/Ikona.svelte';
  import { t } from '../../lib/teksty';
  import { liczba } from '../../lib/format';

  interface Props {
    tytul: string;
    opis?: string;
    wartosc: number;
    min: number;
    max: number;
    stala?: string;
    onzmiana: (kmh: number) => void;
  }
  let { tytul, opis, wartosc, min, max, stala, onzmiana }: Props = $props();

  /** Wartość ustawiona tu, zanim rdzeń odeśle nową (przytrzymanie, wpisywanie). */
  let lokalna = $state<number | null>(null);
  let edycja = $state<string | null>(null);
  const biezaca = $derived(lokalna ?? wartosc);
  const pokazana = $derived(edycja ?? t('pole.kmh', { kmh: liczba(biezaca, 1) }));

  $effect(() => {
    void wartosc;
    lokalna = null;
  });

  /** Zwraca false, gdy wartość już jest na granicy (nic do zmiany). */
  function ustaw(v: number) {
    const n = Math.round(Math.min(max, Math.max(min, v)) * 10) / 10;
    if (n === biezaca) return false;
    lokalna = n;
    onzmiana(n);
    return true;
  }

  // --- przytrzymanie − / + ---
  // Puszczenie łapie też okno: przycisk na granicy staje się `disabled` i sam może nie dostać
  // pointerup - powtarzanie nie może wtedy chodzić dalej. Na granicy powtarzanie się kończy.
  let zegar: ReturnType<typeof setTimeout> | undefined;
  let powtorzenia = 0;
  function trzymaj(kierunek: 1 | -1) {
    pusc();
    powtorzenia = 0;
    if (!ustaw(biezaca + kierunek * 0.1)) return;
    const dalej = () => {
      powtorzenia++;
      if (!ustaw(biezaca + kierunek * (powtorzenia > 6 ? 0.5 : 0.1))) return pusc();
      zegar = setTimeout(dalej, 70);
    };
    zegar = setTimeout(dalej, 350);
    window.addEventListener('pointerup', pusc);
    window.addEventListener('pointercancel', pusc);
  }
  function pusc() {
    clearTimeout(zegar);
    window.removeEventListener('pointerup', pusc);
    window.removeEventListener('pointercancel', pusc);
  }
  $effect(() => pusc);

  // Klik z klawiatury (Enter/Spacja) nie ma pointerdown - detail === 0
  const klik = (kierunek: 1 | -1) => (e: MouseEvent) => {
    if (e.detail === 0) ustaw(biezaca + kierunek * 0.1);
  };

  function zatwierdz() {
    if (edycja === null) return;
    // puste pole nie zeruje wartości; 0 można wpisać, gdy dopuszcza je `min` (np. „Chód od”)
    const tekst = edycja.replace(',', '.').replace(/[^\d.]/g, '');
    const n = Number(tekst);
    edycja = null;
    if (tekst && Number.isFinite(n)) ustaw(n);
  }
  function klawisz(e: KeyboardEvent) {
    const krok = { ArrowUp: 0.1, ArrowRight: 0.1, ArrowDown: -0.1, ArrowLeft: -0.1, PageUp: 0.5, PageDown: -0.5 }[e.key];
    const pole = e.currentTarget as HTMLInputElement;
    if (krok) {
      e.preventDefault();
      edycja = null;
      ustaw(biezaca + krok);
      edycja = liczba(lokalna ?? biezaca, 1);
    } else if (e.key === 'Enter') {
      zatwierdz();
      pole.blur();
    } else if (e.key === 'Escape') {
      edycja = null;
      pole.blur();
    }
  }
</script>

<div class="wiersz">
  <div class="tekst">
    <span class="tytul dane">{tytul}</span>
    {#if opis}<span class="opis dane">{opis}</span>{/if}
  </div>
  <div class="kontrolka">
    <button
      type="button"
      class="pm"
      aria-label={t('pole.mniej', { nazwa: tytul })}
      disabled={!!stala || biezaca <= min}
      onpointerdown={() => trzymaj(-1)}
      onpointerup={pusc}
      onpointerleave={pusc}
      onpointercancel={pusc}
      onclick={klik(-1)}><Ikona nazwa="minus" /></button
    >
    {#if stala}
      <span class="wartosc stala jedna-linia" title={stala}>{stala}</span>
    {:else}
      <input
        class="wartosc liczba"
        value={pokazana}
        inputmode="decimal"
        aria-label={t('pole.wpisz', { nazwa: tytul })}
        title={t('pole.wpisz_podpowiedz')}
        onfocus={(e) => {
          edycja = liczba(biezaca, 1);
          const pole = e.currentTarget;
          requestAnimationFrame(() => pole.select());
        }}
        oninput={(e) => (edycja = e.currentTarget.value)}
        onblur={zatwierdz}
        onkeydown={klawisz}
      />
    {/if}
    <button
      type="button"
      class="pm"
      aria-label={t('pole.wiecej', { nazwa: tytul })}
      disabled={!!stala || biezaca >= max}
      onpointerdown={() => trzymaj(1)}
      onpointerup={pusc}
      onpointerleave={pusc}
      onpointercancel={pusc}
      onclick={klik(1)}><Ikona nazwa="dodaj" /></button
    >
  </div>
</div>

<style>
  .wiersz {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: var(--sp-2);
    align-items: center;
    min-height: var(--control-h);
  }
  .tekst { display: flex; flex-direction: column; min-width: 0; }
  .tytul { font-size: var(--fs-base); font-weight: var(--fw-semibold); line-height: var(--lh-tight); }
  .opis { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-tight); }
  .kontrolka { display: flex; align-items: center; gap: var(--sp-1); }
  .pm {
    display: grid;
    place-items: center;
    width: var(--control-h);
    height: var(--control-h);
    border: 1px solid transparent;
    border-radius: var(--r-md);
    color: var(--text);
    touch-action: none;
    transition:
      background-color var(--t-color) linear,
      border-color var(--t-color) linear;
  }
  .pm:hover:not(:disabled) { background: var(--surface-2); border-color: var(--control); }
  .pm:disabled { color: var(--text-2); opacity: 0.5; }
  .wartosc {
    display: grid;
    place-items: center;
    width: 96px;
    height: var(--control-h);
    padding: 0 4px;
    border: 1px solid transparent;
    border-radius: var(--r-md);
    background: transparent;
    color: var(--text);
    font-size: var(--fs-base);
    text-align: center;
    transition:
      background-color var(--t-color) linear,
      border-color var(--t-color) linear;
  }
  input.wartosc:hover { border-color: var(--line); }
  input.wartosc:focus { outline: none; border-color: var(--text); background: var(--surface-2); }
  .stala { display: block; line-height: var(--control-h); color: var(--text-2); font-size: var(--fs-sm); }
</style>
