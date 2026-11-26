<script lang="ts">
  // Pole tekstowe (docs/wyglad.md 8.6): wysokość 44, błąd tuż pod polem, hasło z przyciskiem
  // „pokaż”, wariant wielowierszowy (min. 120) i dziennik (krój o stałej szerokości).
  import Ikona from './Ikona.svelte';

  interface Props {
    wartosc?: string;
    /** Etykieta nad polem (widoczna); bez niej podaj `etykietaDostepna`. */
    etykieta?: string;
    etykietaDostepna?: string;
    podpowiedz?: string;
    /** Komunikat błędu pod polem (pusty - brak błędu). */
    blad?: string;
    opis?: string;
    typ?: 'text' | 'email' | 'password' | 'search' | 'wielowierszowe';
    mono?: boolean;
    disabled?: boolean;
    readonly?: boolean;
    maxlength?: number;
    autocomplete?: HTMLInputElement['autocomplete'];
    oninput?: (wartosc: string) => void;
    onblur?: () => void;
    onenter?: () => void;
    /** Nazwa przycisku „pokaż hasło” (typ password). Brak klucza w docs/teksty.md - podaje ekran. */
    etykietaPokaz?: string;
  }
  let {
    wartosc = $bindable(''), etykieta, etykietaDostepna, podpowiedz, blad, opis, typ = 'text', mono = false,
    disabled = false, readonly = false, maxlength, autocomplete, oninput, onblur, onenter, etykietaPokaz,
  }: Props = $props();
  const id = $props.id();
  let widoczneHaslo = $state(false);
  const opisId = $derived([blad ? `${id}-blad` : '', opis ? `${id}-opis` : ''].filter(Boolean).join(' ') || undefined);
</script>

<div class="pole" class:z-bledem={!!blad}>
  {#if etykieta}<label class="etykieta" for={id}>{etykieta}</label>{/if}
  {#if typ === 'wielowierszowe'}
    <textarea
      {id}
      class="wejscie wielowierszowe"
      class:mono
      bind:value={wartosc}
      placeholder={podpowiedz}
      aria-label={etykieta ? undefined : etykietaDostepna}
      aria-invalid={blad ? true : undefined}
      aria-describedby={opisId}
      {disabled}
      {readonly}
      {maxlength}
      oninput={() => oninput?.(wartosc)}
      {onblur}
    ></textarea>
  {:else}
    <div class="ramka">
      <input
        {id}
        class="wejscie"
        class:mono
        type={typ === 'password' && widoczneHaslo ? 'text' : typ}
        bind:value={wartosc}
        placeholder={podpowiedz}
        aria-label={etykieta ? undefined : etykietaDostepna}
        aria-invalid={blad ? true : undefined}
        aria-describedby={opisId}
        {disabled}
        {readonly}
        {maxlength}
        {autocomplete}
        spellcheck={typ === 'text' ? undefined : false}
        oninput={() => oninput?.(wartosc)}
        {onblur}
        onkeydown={(e) => e.key === 'Enter' && onenter?.()}
      />
      {#if typ === 'password'}
        <button
          type="button"
          class="pokaz"
          aria-pressed={widoczneHaslo}
          aria-label={etykietaPokaz}
          title={etykietaPokaz}
          onclick={() => (widoczneHaslo = !widoczneHaslo)}
        >
          <Ikona nazwa="oko" />
        </button>
      {/if}
    </div>
  {/if}
  {#if blad}<p class="blad" id="{id}-blad" role="alert">{blad}</p>{/if}
  {#if opis}<p class="opis" id="{id}-opis">{opis}</p>{/if}
</div>

<style>
  .pole { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
  .etykieta { font-size: var(--fs-md); font-weight: var(--fw-semibold); }
  .ramka { position: relative; display: flex; min-width: 0; }
  .wejscie {
    width: 100%;
    min-width: 0;
    height: var(--control-h);
    padding: 0 12px;
    border: 1px solid var(--control);
    border-radius: var(--r-md);
    background: var(--bg);
    color: var(--text);
    font-size: var(--fs-base);
    transition: border-color var(--t-color) linear;
  }
  .wejscie::placeholder { color: var(--text-2); opacity: 1; }
  .wejscie:focus { outline: none; border-color: var(--text); box-shadow: 0 0 0 1px var(--text); }
  .wejscie:disabled { color: var(--text-2); border-color: var(--line); }
  input[type='password'],
  .ramka:has(.pokaz) .wejscie { padding-inline-end: var(--control-h); }
  .wielowierszowe {
    height: auto;
    min-height: 120px;
    padding: 8px;
    font-size: 0.9375rem;
    line-height: var(--lh-body);
    resize: vertical;
  }
  .mono { font-family: var(--font-mono); font-size: var(--fs-sm); }
  .z-bledem .wejscie { border-color: var(--danger); }
  .z-bledem .wejscie:focus { box-shadow: 0 0 0 1px var(--danger); }
  .pokaz {
    position: absolute;
    inset-block: 0;
    inset-inline-end: 0;
    display: grid;
    place-items: center;
    width: var(--control-h);
    color: var(--text-2);
    border-radius: var(--r-md);
  }
  .pokaz[aria-pressed='true'] { color: var(--text); }
  .blad { font-size: var(--fs-sm); color: var(--danger); }
  .opis { font-size: var(--fs-sm); color: var(--text-2); }
</style>
