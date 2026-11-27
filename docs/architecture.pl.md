# Architektura WalkLevel

[English](architecture.md) · Polski

WalkLevel to aplikacja na Windows: chodzenie po bieżni steruje postacią w grze, do tego poziomy,
szlak dystansu i opcjonalne konto online. Od 0.6.0 powstaje w Tauri 2, Svelte 5 i Ruście, a cały
projekt jest w katalogu głównym repozytorium. Teksty dla użytkownika są po angielsku i po polsku,
kolejne języki dochodzą jako pakiety danych ([docs/i18n.pl.md](i18n.pl.md)); polski z pełnymi
znakami. Hasło bez zmian: „Chodzisz w realu, idziesz w grze”.

## Układ projektu

| Ścieżka | Zawartość |
| --- | --- |
| `crates/core` | `walklevel-core`: cała logika bez interfejsu (sterowniki bieżni, sterowanie, historia, osoby i profile, poziomy, konto online, kopie, zgłoszenia, aktualizacje). Nie zależy od Tauri. Tu są testy rdzenia. |
| `src-tauri` | Tylko klej: komendy, zdarzenia, okna. Bez logiki biznesowej. |
| `src` | Interfejs w Svelte 5. Jedno okno, stałe ekrany sterowane stanem; ekrany montują się raz, potem są tylko chowane, bez przebudowy. |
| `docs` | `contract.md` (typy i komendy), `design.md` (tokeny i komponenty), `texts.md` (jedyne źródło kluczy i polskich tekstów), `i18n.md` (języki i tłumaczenia), `sync.md` (synchronizacja z serwerem kont), `release-signing.pl.md` (podpis wydań), `analysis-*.md` (analizy); w `docs/readme/` są zrzuty do README. |
| `locales` | Pakiety językowe: `languages.json` (lista języków), `pl/` generowany z `docs/texts.md`, pozostałe języki tłumaczone ręcznie ([docs/i18n.pl.md](i18n.pl.md)). |
| `tests-ui` | Test widoków i generator pakietów językowych. |
| `packaging` | Ikona, grafiki instalatora i skrypt Inno Setup. |
| `tools` | Skrypty pomocnicze (plik definicji dla serwera kont). |

## Przypięte wersje i pułapki

Nie mieszać ze starszymi API; wiele przykładów w sieci dotyczy wcześniejszych wersji.

- **Tauri 2.** `@tauri-apps/api` 2.x, `invoke` importowany z `@tauri-apps/api/core`.
  `@tauri-apps/api/tauri` i `allowlist` to Tauri 1 i tu nie działają.
  Komendy aplikacji są zarejestrowane w `src-tauri/build.rs` (`AppManifest::commands`), więc okno
  może wołać tylko komendy nadane mu w jego pliku w `src-tauri/capabilities/` (`main`, `popup`,
  `bubble`). Nowa komenda wymaga wpisu w `build.rs` i `allow-<komenda>` w uprawnieniach okien,
  które jej używają. API rdzenia Tauri i wtyczki też wymagają tam wpisów. Uprawnienia nadawać
  wąsko.
- **Svelte 5, tylko runes:** `$state`, `$derived`, `$effect`, `$props`, zdarzenia jako atrybuty,
  np. `onclick={...}`, snippety `{#snippet}` / `{@render}`. Niedozwolone: `export let`, `$:`,
  `on:click`, `<slot>`, `createEventDispatcher`.
- **Skrypty** (`tests-ui/`, `tools/`) są w TypeScripcie i uruchamia je
  bezpośrednio Node.js 24, bez kompilacji. Dlatego tylko składnia usuwalna (bez `enum`
  i `namespace`) i importy z rozszerzeniem `.ts`. Typy sprawdza `tsc -p tsconfig.node.json`
  w ramach `npm run check`.
- **Rust 2024.** reqwest 0.13 z funkcją `rustls` (nie `rustls-tls`, to nazwa z 0.12).
  `reqwest::blocking` nigdy wewnątrz runtime'u tokio; wołać go w `spawn_blocking` albo
  w osobnym wątku.

## Granica rdzeń - interfejs

- `src/lib/api.ts` to jedyne miejsce w interfejsie z `invoke` i `listen`. Udostępnia typowane
  funkcje zgodne z `docs/contract.md`. Komponenty nie wołają `invoke` bezpośrednio. W zwykłej
  przeglądarce (`npm run dev`) te same funkcje obsługuje atrapa rdzenia, `src/dev/mock.ts`.
- Stan: przy starcie interfejs pobiera pełny stan (`app_snapshot`), potem rdzeń wysyła zmiany
  zdarzeniami `wl:stan` z wycinkami stanu. Interfejs trzyma je w store
  (`src/lib/state.svelte.ts`) i tylko renderuje. Telemetria bieżni idzie osobnym zdarzeniem,
  `wl:telemetria` (10-20 Hz; pętla sterowania działa co 50 ms). Pozostałe zdarzenia:
  `wl:komunikat` (komunikaty), `wl:log`, `wl:nawigacja`, `wl:popupy`, `wl:panel-okno`.
- Błędy komend: typ błędu implementuje `Serialize` i niesie komunikat w bieżącym języku dla
  użytkownika.
- Komendy async: bez `MutexGuard` trzymanego przez `.await`; argumenty posiadane (`String`),
  nie pożyczone.

## Wygląd

Pełna specyfikacja jest w [docs/design.md](design.md). W skrócie:

- Tokeny to CSS custom properties: motyw jasny i ciemny oraz kolor akcentu profilu. Akcent
  tylko na jednym, najważniejszym elemencie widoku. Bez pastylek „na wszystko”, kolorowych
  pasków z lewej, gradientów i emoji.
- Nic nie wystaje z okna przy minimalnej szerokości (420 px): rzędy się zawijają, tekst skraca
  się z wielokropkiem. Treść ma maksymalną szerokość.
- Animacje tylko na `transform` i `opacity`. Zmiana trybu sterowania nie zmienia wysokości
  układu. Kliknięty nagłówek akordeonu zostaje pod kursorem.

## Bramki

Z katalogu głównego, od najszybszej. CI uruchamia te same kroki.

1. `cargo clippy --workspace --all-targets --locked -- -D warnings`
2. `cargo test --workspace --locked`
3. `npm run check` (słownik tekstów, svelte-check, tsc)
4. `npm run build`, potem `npm run test:ui` (Edge; w CI z `--no-screenshots`)
   i `node tests-ui/settings-collapse.ts`
5. przed wydaniem: `packaging/build.ps1 -Version X.Y.Z [-Iscc ścieżka\ISCC.exe]` (exe
   i instalator)

Dopiero potem `npx tauri dev`.

## Zgodność z 0.5.x

0.6.x aktualizuje instalacje 0.5.x w miejscu, więc tego nie wolno zmieniać:

- Folder danych `%LOCALAPPDATA%\Bieznia` z plikami `dane/osoby.json`, `dane/konto.json`,
  `dane/historia.db` (schemat 5) i `poziomy/definicje.json`.
- Wpis `Bieznia/konto` w Menedżerze poświadczeń Windows.
- W pracy nad kodem i w testach zmienna środowiskowa `WALKLEVEL_DANE` wskazuje inny folder
  danych, żeby nie ruszać prawdziwych.
- Instalator Inno Setup ma ten sam AppId i folder `{localappdata}\Programs\Bieznia`. Od 0.6.4
  program to `WalkLevel.exe`, a instalator `WalkLevel-Setup-X.Y.Z.exe`; instalator usuwa stary
  `Bieznia.exe` i przepina skróty. Aktualizator szuka najpierw `WalkLevel-Setup-…`, potem
  `Bieznia-Setup-…`, a wydanie 0.6.4 ma też kopię pod starą nazwą dla 0.6.3
  (`LEGACY_INSTALLER_COPY` w `release.yml`).
- Nagłówek User-Agent zostaje `Bieznia/<wersja>` (serwer kont może go rozpoznawać).
- Od 0.6.4 aktualizator instaluje tylko instalator podpisany kluczem właściciela
  ([docs/release-signing.pl.md](release-signing.pl.md)).

## Bez bloatu

Jedna zmiana na zadanie. Bez abstrakcji na zapas, bez zależności dla jednej funkcji, martwy kod
usuwamy. Przed dodaniem crate'a albo paczki npm sprawdzić, czy nie wystarczy biblioteka
standardowa.

## Standardy kodu

Dotyczą nowego kodu i nowych plików. Istniejący kod po polsku migrujemy moduł po module
w osobnych zadaniach, nie przy okazji innej zmiany. Do czasu migracji modułu nie mieszamy w nim
stylów: poprawka w polskim module zostaje po polsku.

- Nowy kod, identyfikatory i komentarze po angielsku. Komentarze tylko wtedy, gdy naprawdę są
  potrzebne: mówią dlaczego, nie co.
- kebab-case w nazwach nowych plików i folderów, w klasach CSS i w kluczach tekstów. W kodzie
  konwencje języka: Rust `snake_case` / `CamelCase`, TypeScript `camelCase` / `PascalCase`.
- Dokumenty mają wersje językowe w osobnych plikach, z przyrostkiem języka przed rozszerzeniem:
  `README.md` + `README.pl.md`, `CONTRIBUTING.md` + `CONTRIBUTING.pl.md`, `PRIVACY.md` +
  `PRIVACY.pl.md`, `docs/architecture.md` + `docs/architecture.pl.md`.
  W katalogu głównym poza tymi plikami i `LICENSE` obowiązuje kebab-case.
- Data w interfejsie i w dokumentach: DD.MM.RRRR.
- Komunikat commita, opis pull requesta i opis wydania to jeden tekst po angielsku i po polsku:
  tytuł i treść po angielsku, pusta linia, potem to samo po polsku. Wydanie to tag z adnotacją
  `vX.Y.Z`, którego treść jest opisem wydania (najpierw angielski, pod nim polski). Szczegóły
  w [CONTRIBUTING.pl.md](../CONTRIBUTING.pl.md).
