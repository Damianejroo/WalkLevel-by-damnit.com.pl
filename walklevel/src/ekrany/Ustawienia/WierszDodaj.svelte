<script lang="ts">
  // „Dodaj…” (docs/wyglad.md 8.14): wysokość 50, ramka przerywana --control, tekst --text.
  import Ikona from '../../lib/ui/Ikona.svelte';

  interface Props {
    napis: string;
    onclick: () => void;
  }
  let { napis, onclick }: Props = $props();
</script>

<button type="button" class="dodaj" {onclick}>
  <Ikona nazwa="dodaj" />
  <span class="jedna-linia">{napis}</span>
</button>

<style>
  .dodaj {
    display: flex;
    align-items: center;
    gap: var(--sp-3);
    width: 100%;
    min-width: 0;
    height: 50px;
    padding: 0 16px;
    border: 1px dashed var(--control);
    border-radius: var(--r-md);
    color: var(--text);
    font-weight: var(--fw-semibold);
    transition: background-color var(--t-color) linear;
  }
  .dodaj:hover { background: var(--surface-2); }
</style>
