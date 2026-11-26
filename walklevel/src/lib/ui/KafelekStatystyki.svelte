<script lang="ts">
  // Kafelek statystyki (docs/wyglad.md 8.7): stała wysokość 64, wartość Bahnschrift skalowana do
  // szerokości kafelka, wielokropek jako ostateczność. Brak danych: „–”.
  interface Props {
    tytul: string;
    wartosc: string | null;
    /** Pełna nazwa dla czytnika ekranu (np. „Kroki na minutę” przy tytule „Kroki/min”). */
    etykieta?: string;
  }
  let { tytul, wartosc, etykieta }: Props = $props();
  const tekst = $derived(wartosc ?? '–');
</script>

<div class="kafelek" role="group" aria-label={etykieta ?? tytul}>
  <span class="tytul jedna-linia" aria-hidden="true">{tytul}</span>
  <span class="wartosc liczba jedna-linia" title={tekst}>{tekst}</span>
</div>

<style>
  .kafelek {
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 2px;
    min-width: 0;
    height: 64px;
    padding: 10px 12px;
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: var(--surface);
    container-type: inline-size;
  }
  .tytul { font-size: var(--fs-xs); color: var(--text-2); line-height: var(--lh-tight); }
  .wartosc {
    font-size: clamp(1.125rem, 9cqi, 1.5rem);
    line-height: 1.1;
    padding-block-start: 0.06em;
  }
</style>
