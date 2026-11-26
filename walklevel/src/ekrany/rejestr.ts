// Rejestr ekranów: trasa -> komponent z src/ekrany/. Nowy ekran = nowy plik + wpis tutaj
// (+ wpis w tests-ui/ekrany.mjs, żeby test „nic nie wystaje” go sprawdzał).
import type { Component } from 'svelte';
import type { PropsEkranu, Trasa } from '../lib/stan.svelte';
import type { Klucz } from '../lib/teksty';

import Glowny from './Glowny.svelte';
import Ustawienia from './Ustawienia.svelte';
import Historia from './Historia.svelte';
import Osiagniecia from './Osiagniecia.svelte';
import Szlak from './Szlak.svelte';
import KontoOnline from './KontoOnline.svelte';
import ProfilOsoby from './ProfilOsoby.svelte';
import ZglosProblem from './ZglosProblem.svelte';
import Dziennik from './Dziennik.svelte';
import KopiaDanych from './KopiaDanych.svelte';
import WczytanieKopii from './WczytanieKopii.svelte';
import ObslugiwaneBieznie from './ObslugiwaneBieznie.svelte';
import Kreator from './Kreator.svelte';
import WyborBiezni from './WyborBiezni.svelte';
import ProfilGry from './ProfilGry.svelte';
import EdycjaOsoby from './EdycjaOsoby.svelte';
import Sesja from './Sesja.svelte';
import Logowanie from './Logowanie.svelte';
import Wzornik from './Wzornik.svelte';

export interface Ekran {
  komponent: Component<PropsEkranu>;
  /** Nazwa ekranu (klucz z docs/teksty.md) - lista ekranów w podglądzie. */
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
