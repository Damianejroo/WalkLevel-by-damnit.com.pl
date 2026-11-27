<script lang="ts">
  // Przycisk przypisywania klawisza (docs/design.md 8.14 „Przycisk z klawiszem”): „Klawisz chodu: W”.
  // Klik -> „…: naciśnij klawisz…”, następny klawisz trafia do rdzenia (key_set), Esc, klik obok
  // albo drugi klik anuluje. `kombinacje` (włączanie, szybciej, wolniej): trzymane Ctrl/Alt/Shift czekają na
  // klawisz właściwy. Szerokość = najdłuższy z napisów, więc przycisk nie skacze.
  import Przycisk from '../../lib/ui/button.svelte';
  import { t } from '../../lib/texts';
  import { wykonaj } from '../../lib/state.svelte';
  import * as api from '../../lib/api';
  import type { KeyCapture, KeyRole } from '../../lib/contract';

  interface Props {
    rola: KeyRole;
    nazwaRoli: string;
    klawisz: string;
    kombinacje?: boolean;
  }
  let { rola, nazwaRoli, klawisz, kombinacje = false }: Props = $props();

  const MODYFIKATORY: Record<string, string> = {
    ControlLeft: 'Ctrl', ControlRight: 'Ctrl', AltLeft: 'Alt', AltRight: 'Alt', ShiftLeft: 'Shift', ShiftRight: 'Shift',
  };

  let czeka = $state(false);
  let wcisniete = $state('');
  let korzen: HTMLElement;

  const napis = $derived(
    !czeka
      ? t('klawisz.przycisk', { rola: nazwaRoli, klawisz })
      : wcisniete
        ? t('klawisz.czekam_kombinacja', { rola: nazwaRoli, wcisniete })
        : t('klawisz.czekam', { rola: nazwaRoli }),
  );
  const napisy = $derived([
    ...new Set([
      napis,
      t('klawisz.przycisk', { rola: nazwaRoli, klawisz }),
      t('klawisz.czekam', { rola: nazwaRoli }),
      t('klawisz.przycisk', { rola: nazwaRoli, klawisz: kombinacje ? 'Ctrl+Shift+F10' : 'Shift' }),
    ]),
  ]);

  function trzymane(e: KeyboardEvent) {
    return [e.ctrlKey && 'Ctrl', e.altKey && 'Alt', e.shiftKey && 'Shift'].filter(Boolean).join('+');
  }

  function zakoncz(anuluj: boolean) {
    if (!czeka) return;
    czeka = false;
    wcisniete = '';
    if (anuluj) void wykonaj(api.keyCapture(null));
  }

  function zacznij() {
    if (czeka) return zakoncz(true);
    czeka = true;
    wcisniete = '';
    void wykonaj(api.keyCapture(rola));
  }

  $effect(() => {
    if (!czeka) return;
    const wcisnij = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.repeat) return;
      if (e.code === 'Escape') return zakoncz(true);
      if (kombinacje && (e.code in MODYFIKATORY || e.key === 'Meta')) {
        wcisniete = trzymane(e);
        return;
      }
      const k: KeyCapture = {
        code: e.code, key: e.key, ctrl: e.ctrlKey, alt: e.altKey, shift: e.shiftKey, meta: e.metaKey, location: e.location,
      };
      zakoncz(false);
      void wykonaj(api.keySet(rola, k));
    };
    const pusc = (e: KeyboardEvent) => {
      e.preventDefault();
      if (kombinacje) wcisniete = trzymane(e);
    };
    const obok = (e: PointerEvent) => {
      if (!korzen.contains(e.target as Node)) zakoncz(true);
    };
    const utrata = () => zakoncz(true);
    window.addEventListener('keydown', wcisnij, true);
    window.addEventListener('keyup', pusc, true);
    window.addEventListener('pointerdown', obok, true);
    window.addEventListener('blur', utrata);
    return () => {
      window.removeEventListener('keydown', wcisnij, true);
      window.removeEventListener('keyup', pusc, true);
      window.removeEventListener('pointerdown', obok, true);
      window.removeEventListener('blur', utrata);
    };
  });

  // Zwinięcie kategorii w trakcie przypisywania: rdzeń wraca do zwykłego skrótu
  $effect(() => () => zakoncz(true));
</script>

<div class="klawisz" bind:this={korzen}>
  <Przycisk
    szeroki
    {napis}
    {napisy}
    etykieta={czeka ? napis : t('klawisz.dostepnosc', { rola: nazwaRoli, klawisz })}
    title={t('klawisz.podpowiedz')}
    onclick={zacznij}
  />
</div>

<style>
  .klawisz { min-width: 0; }
</style>
