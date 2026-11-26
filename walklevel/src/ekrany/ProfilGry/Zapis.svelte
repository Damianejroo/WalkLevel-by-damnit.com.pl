<script lang="ts">
  // Stan zapisu pod formularzem, który zapisuje się sam: „Zmiany zapisują się same”, po każdym
  // zapisie na 2 s „Zapisano”. Jedna linia o stałej wysokości.
  import Ikona from '../../lib/ui/Ikona.svelte';
  import { t } from '../../lib/teksty';

  interface Props {
    /** Rośnie przy każdym udanym zapisie. */
    licznik: number;
  }
  let { licznik }: Props = $props();
  let zapisano = $state(false);

  $effect(() => {
    if (licznik === 0) return;
    zapisano = true;
    const zegar = setTimeout(() => (zapisano = false), 2000);
    return () => clearTimeout(zegar);
  });
</script>

<p class="zapis" role="status">
  <span class="jedna-linia">{zapisano ? t('app.zapis.zapisano') : t('app.zapis.auto')}</span>
  <span class="znak" class:widoczny={zapisano}><Ikona nazwa="zaznacz" rozmiar={12} /></span>
</p>

<style>
  .zapis {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    padding-inline: 15px;      /* w osi tekstu kart (ramka 1 + odstęp 14) */
    font-size: var(--fs-sm);
    line-height: var(--lh-body);
    color: var(--text-2);
  }
  .znak { opacity: 0; transition: opacity var(--t-color) linear; }
  .widoczny { opacity: 1; }
</style>
