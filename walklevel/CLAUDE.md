# WalkLevel 0.6.0 - zasady projektu (Tauri 2 + Svelte 5 + Rust)

Aplikacja Windows: chodzenie po bieżni steruje postacią w grze; poziomy, szlak, konto online.
Hasło: „Chodzisz w realu, idziesz w grze” (bez zmian). Teksty i komentarze po polsku, pełne znaki.

## Układ

- `crates/core` (walklevel-core) - cała logika bez interfejsu, NIE zależy od tauri. Testy tutaj.
- `src-tauri` - tylko klej: komendy, zdarzenia, okna. Bez logiki biznesowej.
- `src/` - interfejs Svelte 5. Jedno okno, stałe ekrany sterowane stanem, bez przebudowy ekranów.
- `docs/` - kontrakt.md (typy i komendy), wyglad.md (tokeny i komponenty), teksty.md (jedyne źródło
  tekstów), synchro.md (synchronizacja z serwerem), analiza-*.md.

## Wersje (przypięte, nie mieszać ze starszymi)

- Tauri 2 (`@tauri-apps/api` 2.x, import `invoke` z `@tauri-apps/api/core`; NIE `@tauri-apps/api/tauri`,
  NIE `allowlist` - to Tauri v1). Własne komendy są dozwolone domyślnie; API rdzenia i wtyczki
  wymagają wpisów w `src-tauri/capabilities/` dla etykiety okna - nadawać wąsko.
- Svelte 5 tylko z runes: `$state`, `$derived`, `$effect`, `$props`, zdarzenia `onclick={...}`,
  snippety `{#snippet}`/`{@render}`. ZAKAZANE: `export let`, `$:`, `on:click`, `<slot>`,
  `createEventDispatcher`.
- Rust 2024; reqwest 0.13 (feature `rustls`, nie `rustls-tls`); `reqwest::blocking` NIGDY wewnątrz
  runtime'u tokio (tylko w `spawn_blocking` albo osobnym wątku).

## Granica rdzeń - interfejs

- Jedyne miejsce z `invoke`/`listen` w UI: `src/lib/api.ts` (typowane funkcje zgodne z docs/kontrakt.md).
  Komponenty nie wołają `invoke` bezpośrednio.
- Stan: rdzeń wysyła pełny stan przy starcie i zmiany zdarzeniami; UI trzyma go w store
  (`src/lib/stan.svelte.ts`) i tylko renderuje. Telemetria bieżni osobnym zdarzeniem (10-20 Hz).
- Błędy komend: typ błędu implementuje `Serialize` i niesie komunikat po polsku dla użytkownika.
- W komendach async: bez `MutexGuard` trzymanego przez `.await`; argumenty posiadane (`String`), nie pożyczone.

## Wygląd

- Tokeny z docs/wyglad.md (CSS custom properties, jasny/ciemny + akcent profilu). Akcent tylko na
  jednym najważniejszym elemencie widoku. Bez pastylek „na wszystko”, kolorowych pasków z lewej,
  gradientów, emoji.
- Nic nie wystaje z okna przy minimalnej szerokości (420 px): rzędy się zawijają, tekst skraca się
  z wielokropkiem. Maksymalna szerokość treści. Animacje tylko `transform`/`opacity`; zmiana trybu
  nie zmienia wysokości układu; kliknięty nagłówek akordeonu zostaje pod kursorem.

## Bramki (od najszybszej)

`cargo check` -> `cargo clippy --all-targets -- -D warnings` -> `cargo test -p walklevel-core` ->
`npm run check` -> `npm run build` -> dopiero potem `tauri dev`/`tauri build`.
Wspólny katalog kompilacji dla agentów: `CARGO_TARGET_DIR=D:\Dev\bieznia-olek\walklevel\target-shared`.

## Zgodność z 0.5.x (nie łamać)

Dane w `%LOCALAPPDATA%\Bieznia` (dev/testy: zmienna `WALKLEVEL_DANE`): `dane/osoby.json`,
`dane/konto.json`, `dane/historia.db` (schemat 5), poświadczenie `Bieznia/konto` w Menedżerze
poświadczeń, `poziomy/definicje.json`. Instalator Inno Setup z tym samym AppId, exe `Bieznia.exe`.

## Bez bloatu

Jedna zmiana na zadanie, bez abstrakcji na zapas, bez zależności dla jednej funkcji, martwy kod
usuwać. Przed dodaniem crate'a/paczki npm sprawdzić, czy nie wystarczy biblioteka standardowa.
