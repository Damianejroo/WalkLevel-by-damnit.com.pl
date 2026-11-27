// Rejestr ekranów: trasa -> komponent z src/screens/. Nowy ekran = nowy plik + wpis tutaj
// (+ wpis w tests-ui/screens.ts, żeby test „nic nie wystaje” go sprawdzał).
import type { Component } from 'svelte';
import type { PropsEkranu, Trasa } from '../lib/state.svelte';
import type { Klucz } from '../lib/texts';

import Glowny from './main-screen.svelte';
import Ustawienia from './settings-screen.svelte';
import Historia from './history-screen.svelte';
import Osiagniecia from './achievements-screen.svelte';
import Szlak from './trail-screen.svelte';
import KontoOnline from './online-screen.svelte';
import ProfilOsoby from './user-profile-screen.svelte';
import ZglosProblem from './report-screen.svelte';
import Dziennik from './log-screen.svelte';
import KopiaDanych from './backup-export-screen.svelte';
import WczytanieKopii from './backup-import-screen.svelte';
import ObslugiwaneBieznie from './models-screen.svelte';
import Kreator from './wizard-screen.svelte';
import WyborBiezni from './devices-screen.svelte';
import ProfilGry from './game-profile-screen.svelte';
import EdycjaOsoby from './person-edit-screen.svelte';
import Sesja from './session-screen.svelte';
import Logowanie from './sign-in-screen.svelte';
import Wzornik from '../dev/component-gallery.svelte';

export interface Ekran {
  komponent: Component<PropsEkranu>;
  /** Nazwa ekranu (klucz z docs/texts.md) - lista ekranów w podglądzie. */
  nazwa: Klucz;
}

export const EKRANY: Record<Trasa, Ekran> = {
  main: { komponent: Glowny, nazwa: 'app.nazwa' },
  settings: { komponent: Ustawienia, nazwa: 'ustawienia.tytul' },
  history: { komponent: Historia, nazwa: 'historia.tytul' },
  achievements: { komponent: Osiagniecia, nazwa: 'poziomy.tytul' },
  trail: { komponent: Szlak, nazwa: 'szlak.tytul' },
  online: { komponent: KontoOnline, nazwa: 'online.tytul_ogolny' },
  onlinePerson: { komponent: ProfilOsoby, nazwa: 'profil_online.tytul' },
  report: { komponent: ZglosProblem, nazwa: 'zgloszenie.tytul' },
  log: { komponent: Dziennik, nazwa: 'dziennik.tytul' },
  export: { komponent: KopiaDanych, nazwa: 'kopia.eksport.tytul' },
  import: { komponent: WczytanieKopii, nazwa: 'kopia.import.tytul' },
  models: { komponent: ObslugiwaneBieznie, nazwa: 'modele.tytul' },
  wizard: { komponent: Kreator, nazwa: 'kreator.imie.tytul' },
  devices: { komponent: WyborBiezni, nazwa: 'bieznia.lista.tytul' },
  profile: { komponent: ProfilGry, nazwa: 'profil.tytul_edycja' },
  profileNew: { komponent: ProfilGry, nazwa: 'profil.tytul_nowy' },
  person: { komponent: EdycjaOsoby, nazwa: 'osoba.edycja.tytul' },
  session: { komponent: Sesja, nazwa: 'sesja.tytul' },
  account: { komponent: Logowanie, nazwa: 'konto.logowanie.tytul' },
  wzornik: { komponent: Wzornik, nazwa: 'app.nazwa' },
};
