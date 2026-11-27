<script lang="ts">
  // Toast - komunikat na dole okna (docs/design.md 8.12): --ink, 4 s (liczy state.svelte.ts),
  // klik chowa, nowy zastępuje stary. Wejście opacity + translateY 220 ms, wyjście 180 ms.
  import { fade } from 'svelte/transition';
  import type { Toast } from '../contract';

  interface Props {
    toast: (Toast & { id: number }) | null;
    onzamknij: () => void;
  }
  let { toast, onzamknij }: Props = $props();
</script>

<div class="warstwa" role="status" aria-live="polite">
  {#if toast}
    {#key toast.id}
      <button type="button" class="toast" class:blad={toast.kind === 'error'} onclick={onzamknij} out:fade={{ duration: 100 }}>
        {toast.text}
      </button>
    {/key}
  {/if}
</div>

<style>
  .warstwa {
    position: fixed;
    /* w kolumnie ekranów, obok panelu znajomych (App.svelte ustawia --panel-lewo/-prawo) */
    left: var(--panel-lewo, 0px);
    right: var(--panel-prawo, 0px);
    bottom: 20px;
    z-index: 20;
    display: grid;
    justify-items: center;
    padding: 0 var(--sp-4);
    pointer-events: none;
  }
  .warstwa > :global(*) { grid-area: 1 / 1; }
  .toast {
    width: min(420px, 100%, calc(var(--content-max) - 32px));
    padding: 10px 14px;
    border-radius: var(--r-md);
    background: var(--ink);
    color: var(--on-ink);
    box-shadow: var(--shadow-pop);
    font-size: 0.9375rem;
    font-weight: var(--fw-semibold);
    line-height: var(--lh-body);
    text-align: center;
    overflow-wrap: anywhere;
    pointer-events: auto;
    animation: wl-wjazd-w-gore var(--t-in) var(--ease-enter);
  }
  .blad { outline: 2px solid var(--danger); outline-offset: -2px; }
</style>
