<script lang="ts">
  // Przełącznik (docs/wyglad.md 8.2): neutralny (włączony = --ink), cały wiersz to <label>.
  interface Props {
    wlaczony?: boolean;
    /** Tytuł wiersza ustawienia (bez niego - sam przełącznik z `etykieta`). */
    tytul?: string;
    opis?: string;
    /** Nazwa dostępna, gdy nie ma tytułu. */
    etykieta?: string;
    disabled?: boolean;
    /** `zKlawiatury` - Enter albo spacja (kliknięcie bez wskaźnika, `detail === 0`). */
    onzmiana?: (wlaczony: boolean, zKlawiatury: boolean) => void;
    /**
     * Po kliknięciu myszą fokus nie zostaje na przełączniku: klawisze, które program do pada
     * wysyła do okna WalkLevel (Enter, spacja), nie przełączą go wtedy przypadkiem.
     */
    bezFokusuPoKliknieciu?: boolean;
  }
  let { wlaczony = $bindable(false), tytul, opis, etykieta, disabled = false, onzmiana, bezFokusuPoKliknieciu = false }: Props =
    $props();
  const id = $props.id();

  function przelacz(e: MouseEvent) {
    const zKlawiatury = e.detail === 0;
    wlaczony = !wlaczony;
    onzmiana?.(wlaczony, zKlawiatury);
    if (bezFokusuPoKliknieciu && !zKlawiatury) (e.currentTarget as HTMLElement).blur();
  }
</script>

{#snippet przelacznik()}
  <button
    type="button"
    role="switch"
    class="przelacznik"
    aria-checked={wlaczony}
    aria-label={tytul ? undefined : etykieta}
    aria-describedby={opis ? `${id}-opis` : undefined}
    id="{id}-przelacznik"
    {disabled}
    onclick={przelacz}
  >
    <span class="tor"><span class="galka"></span></span>
  </button>
{/snippet}

{#if tytul}
  <label class="wiersz" class:wylaczony={disabled} for="{id}-przelacznik">
    <span class="tekst">
      <span class="tytul dane">{tytul}</span>
      {#if opis}<span class="opis" id="{id}-opis">{opis}</span>{/if}
    </span>
    {@render przelacznik()}
  </label>
{:else}
  {@render przelacznik()}
{/if}

<style>
  .wiersz {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: var(--sp-3);
    align-items: start;
    cursor: pointer;
  }
  .wylaczony { cursor: default; color: var(--text-2); }
  .tekst { display: flex; flex-direction: column; gap: 2px; min-width: 0; padding-block: 10px; }
  .tytul { font-size: var(--fs-base); line-height: var(--lh-tight); }
  .opis { font-size: var(--fs-sm); color: var(--text-2); line-height: var(--lh-body); }

  .przelacznik {
    display: grid;
    place-items: center;
    width: 56px;
    height: var(--control-h);
    flex: none;
    border-radius: var(--r-md);
  }
  .tor {
    position: relative;
    width: 52px;
    height: 30px;
    border: 2px solid var(--control);
    border-radius: var(--r-round);
    background: var(--bg);
    transition:
      background-color var(--t-color) linear,
      border-color var(--t-color) linear;
  }
  .galka {
    position: absolute;
    top: 3px;
    left: 5px;               /* x = 7 od krawędzi toru (z ramką 2 px) */
    width: 20px;
    height: 20px;
    border-radius: var(--r-round);
    background: var(--text-2);
    transition:
      transform var(--t-switch) var(--ease-enter),
      background-color var(--t-color) linear;
  }
  [aria-checked='true'] .tor { background: var(--ink); border-color: var(--ink); }
  [aria-checked='true'] .galka { background: var(--on-ink); transform: translateX(22px); }
  .przelacznik:disabled .tor { border-color: var(--line); }
  .przelacznik:disabled .galka { opacity: 0.5; }
</style>
