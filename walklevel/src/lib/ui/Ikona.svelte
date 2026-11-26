<script lang="ts" module>
  // Kody Segoe Fluent Icons (fallback: Segoe MDL2 Assets - te same kody), docs/wyglad.md 12.
  export const IKONY = {
    ustawienia: '', wstecz: '', odswiez: '', bluetooth: '', dziennik: '',
    folder: '', folder_otworz: '', zglos: '', klawiatura: '', oko: '',
    chod: '', pobierz: '', wykres: '', osoby: '', pad: '', trening: '',
    w_dol: '', w_prawo: '', zaznacz: '', edytuj: '', dodaj: '', minus: '',
    plik: '', losuj: '', pole_zaznaczone: '', pole_puste: '', pole_czesciowo: '',
    konto: '', gwiazda: '', wyglad: '', dane: '', pomoc: '', bieznia: '',
    zamknij: '',
  } as const;
  export type NazwaIkony = keyof typeof IKONY;
</script>

<script lang="ts">
  interface Props {
    /** Nazwa z IKONY albo `znak` - pojedynczy znak fontu (np. ikona profilu z IconRef.glyph). */
    nazwa?: NazwaIkony;
    znak?: string;
    rozmiar?: number;
    /** Nazwa dla czytnika ekranu; bez niej ikona jest ozdobą (aria-hidden). */
    etykieta?: string;
  }
  let { nazwa, znak, rozmiar = 16, etykieta }: Props = $props();
  const tekst = $derived(znak ?? (nazwa ? IKONY[nazwa] : ''));
</script>

<span
  class="ikona"
  style:font-size="{rozmiar}px"
  role={etykieta ? 'img' : undefined}
  aria-label={etykieta}
  aria-hidden={etykieta ? undefined : 'true'}>{tekst}</span>

<style>
  .ikona {
    display: inline-grid;
    place-items: center;
    width: 1em;
    height: 1em;
    flex: none;
    font-family: var(--font-icon);
    font-style: normal;
    font-weight: normal;
    line-height: 1;
    user-select: none;
  }
</style>
