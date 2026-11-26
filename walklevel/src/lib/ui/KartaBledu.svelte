<script lang="ts">
  // Komunikat błędu w miejscu treści (docs/wyglad.md 8.17): karta neutralna jak 8.4 (bez paska
  // z lewej, bez pastylki), ikona ostrzeżenia w --danger, tytuł („Nie udało się wczytać
  // znajomych”), jedno zdanie: co się stało i co zrobić, przycisk „Spróbuj ponownie” i - gdy
  // aplikacja ponawia sama - „Kolejna próba za 10 s”. `zwarta` - w panelu znajomych (bez ramki).
  import Ikona from './Ikona.svelte';
  import Przycisk from './Przycisk.svelte';
  import { t } from '../teksty';

  interface Props {
    tytul: string;
    opis: string;
    /** Bez - bez przycisku. */
    onponow?: () => void;
    /** Sekundy do kolejnej próby, którą aplikacja zrobi sama (brak - nie ponawia). */
    ponowiZa?: number | null;
    zajete?: boolean;
    zwarta?: boolean;
  }
  let { tytul, opis, onponow, ponowiZa = null, zajete = false, zwarta = false }: Props = $props();
</script>

<section class="blad" class:zwarta role="alert">
  <span class="ikona"><Ikona nazwa="ostrzezenie" rozmiar={20} /></span>
  <div class="tresc">
    <h2 class="tytul dane">{tytul}</h2>
    <p class="opis dane">{opis}</p>
    {#if onponow || ponowiZa !== null}
      <div class="akcje">
        {#if onponow}<Przycisk napis={t('online.ponow')} disabled={zajete} onclick={onponow} />{/if}
        {#if ponowiZa !== null}
          <span class="ponowi" aria-live="off">{t('online.ponowi', { sekundy: Math.max(1, Math.ceil(ponowiZa)) })}</span>
        {/if}
      </div>
    {/if}
  </div>
</section>

<style>
  .blad {
    display: grid;
    grid-template-columns: 20px minmax(0, 1fr);
    gap: var(--sp-3);
    align-items: start;
    padding: 14px 16px;
    border: 1px solid var(--line);
    border-radius: var(--r-lg);
    background: var(--surface);
  }
  .zwarta { padding: 10px 12px; border-radius: var(--r-md); }
  .ikona { padding-top: 1px; color: var(--danger); }
  .tresc { display: grid; gap: 4px; min-width: 0; }
  .tytul { font-size: var(--fs-base); font-weight: var(--fw-bold); line-height: var(--lh-tight); overflow-wrap: anywhere; }
  .opis { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-body); overflow-wrap: anywhere; }
  .akcje { display: flex; flex-wrap: wrap; align-items: center; gap: var(--sp-2) var(--sp-3); margin-top: 6px; }
  .akcje > :global(button) { flex: 0 1 auto; }
  .ponowi { font-size: var(--fs-sm); color: var(--text-2); }
</style>
