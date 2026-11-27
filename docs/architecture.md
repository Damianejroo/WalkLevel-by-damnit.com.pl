# WalkLevel architecture

English · [Polski](architecture.pl.md)

WalkLevel is a Windows app: walking on a treadmill controls a character in a game, with levels,
a distance trail and an optional online account. Since 0.6.0 it is built with Tauri 2, Svelte 5
and Rust, and the whole project lives in the repository root. User-facing texts are in English and
Polish, more languages come as data packs ([docs/i18n.md](i18n.md)); Polish has full diacritics.
The tagline stays: „Chodzisz w realu, idziesz w grze”.

## Project layout

| Path | Contents |
| --- | --- |
| `crates/core` | `walklevel-core`: all logic without UI (treadmill drivers, control, history, people and profiles, levels, online account, backups, reports, updates). It does not depend on Tauri. Core tests live here. |
| `src-tauri` | Glue only: commands, events, windows. No business logic. |
| `src` | Svelte 5 UI. One window with fixed screens driven by state; screens are mounted once and then only hidden, not rebuilt. |
| `docs` | `contract.md` (types and commands), `design.md` (design tokens and components), `texts.md` (the single source of keys and Polish texts), `i18n.md` (languages and translations), `sync.md` (sync with the account server), `release-signing.md` (signed releases), `analysis-*.md` (analyses); `docs/readme/` holds the README screenshots. |
| `locales` | Language packs: `languages.json` (the list of languages), `pl/` generated from `docs/texts.md`, other languages translated by hand ([docs/i18n.md](i18n.md)). |
| `tests-ui` | View test and the language pack generator. |
| `packaging` | Icon, installer images and the Inno Setup script. |
| `tools` | Helper scripts (definitions file for the account server). |

## Pinned versions and pitfalls

Do not mix these with older APIs; many examples online are written for earlier versions.

- **Tauri 2.** `@tauri-apps/api` 2.x, `invoke` is imported from `@tauri-apps/api/core`.
  `@tauri-apps/api/tauri` and `allowlist` belong to Tauri 1 and do not work here.
  App commands are registered in `src-tauri/build.rs` (`AppManifest::commands`), so a window can
  call only the commands granted in its file in `src-tauri/capabilities/` (`main`, `popup`,
  `bubble`). A new command needs an entry in `build.rs` and `allow-<command>` in the capabilities
  of the windows that use it. Core APIs and plugins need entries there too. Grant them narrowly.
- **Svelte 5, runes only:** `$state`, `$derived`, `$effect`, `$props`, event attributes such as
  `onclick={...}`, snippets with `{#snippet}` / `{@render}`. Not allowed: `export let`, `$:`,
  `on:click`, `<slot>`, `createEventDispatcher`.
- **Scripts** (`tests-ui/`, `tools/`) are TypeScript run directly by
  Node.js 24, without a build step. That allows only erasable syntax (no `enum`, no `namespace`)
  and imports with the `.ts` extension. Types are checked by `tsc -p tsconfig.node.json` as part
  of `npm run check`.
- **Rust 2024.** reqwest 0.13 with the `rustls` feature (not `rustls-tls`, which is the 0.12
  name). `reqwest::blocking` must never run inside a tokio runtime; call it from
  `spawn_blocking` or a separate thread.

## Core and UI boundary

- `src/lib/api.ts` is the only place in the UI that calls `invoke` or `listen`. It exposes typed
  functions that match `docs/contract.md`. Components never call `invoke` directly. In a plain
  browser (`npm run dev`) the same functions are served by a mock core, `src/dev/mock.ts`.
- State: at start the UI asks for the full state (`app_snapshot`), then the core sends changes as
  `wl:stan` events with state slices. The UI keeps them in a store (`src/lib/state.svelte.ts`) and
  only renders them. Treadmill telemetry is a separate event, `wl:telemetria` (10-20 Hz; the
  control loop runs every 50 ms). Other events: `wl:komunikat` (toasts), `wl:log`,
  `wl:nawigacja`, `wl:popupy`, `wl:panel-okno`.
- Command errors: the error type implements `Serialize` and carries a message in the current language for the
  user.
- Async commands: no `MutexGuard` held across `.await`; arguments are owned (`String`), not
  borrowed.

## Look

The full specification is in [docs/design.md](design.md). In short:

- Tokens are CSS custom properties: light and dark theme plus the profile accent colour. The
  accent goes on one most important element of a view only. No pills on everything, no coloured
  bars on the left, no gradients, no emoji.
- Nothing may stick out of the window at the minimum width (420 px): rows wrap and text is
  truncated with an ellipsis. Content has a maximum width.
- Animations use only `transform` and `opacity`. Switching the control mode does not change the
  layout height. A clicked accordion header stays under the cursor.

## Checks

Run from the repository root, fastest first. CI runs the same steps.

1. `cargo clippy --workspace --all-targets --locked -- -D warnings`
2. `cargo test --workspace --locked`
3. `npm run check` (text dictionary, svelte-check, tsc)
4. `npm run build`, then `npm run test:ui` (Edge; CI adds `--no-screenshots`) and
   `node tests-ui/settings-collapse.ts`
5. before a release: `packaging/build.ps1 -Version X.Y.Z [-Iscc path\ISCC.exe]` (exe and
   installer)

Only after these, `npx tauri dev`.

## Compatibility with 0.5.x

0.6.x updates 0.5.x installations in place, so these must not change:

- Data folder `%LOCALAPPDATA%\Bieznia` with `dane/osoby.json`, `dane/konto.json`,
  `dane/historia.db` (schema 5) and `poziomy/definicje.json`.
- The `Bieznia/konto` entry in Windows Credential Manager.
- For development and tests, the `WALKLEVEL_DANE` environment variable points to another data
  folder, so real data stays untouched.
- The Inno Setup installer keeps the same AppId and folder `{localappdata}\Programs\Bieznia`.
  Since 0.6.4 the program is `WalkLevel.exe` and the installer `WalkLevel-Setup-X.Y.Z.exe`; the
  installer removes the old `Bieznia.exe` and repoints the shortcuts. The updater looks for
  `WalkLevel-Setup-…` first, then `Bieznia-Setup-…`, and the 0.6.4 release also carries a copy
  under the old name for 0.6.3 (`LEGACY_INSTALLER_COPY` in `release.yml`).
- The User-Agent header stays `Bieznia/<version>` (the account server may recognise it).
- Since 0.6.4 the updater installs only an installer signed with the owner's key
  ([docs/release-signing.md](release-signing.md)).

## No bloat

One change per task. No abstractions in advance, no dependency for a single function, and dead
code gets removed. Before adding a crate or an npm package, check whether the standard library
is enough.

## Code standards

These apply to new code and new files. Existing Polish code is migrated module by module in
separate tasks, not as a side effect of another change. Until a module is migrated, do not mix
styles in it: a fix in a Polish module stays in Polish.

- New code, identifiers and comments in English. Comments only when they are really needed:
  they explain why, not what.
- kebab-case for new file and folder names, CSS classes and text keys. Inside code, follow the
  language: Rust `snake_case` / `CamelCase`, TypeScript `camelCase` / `PascalCase`.
- Documents come in language versions as separate files, with the language suffix before the
  extension: `README.md` + `README.pl.md`, `CONTRIBUTING.md` + `CONTRIBUTING.pl.md`,
  `PRIVACY.md` + `PRIVACY.pl.md`, `docs/architecture.md` + `docs/architecture.pl.md`.
  In the repository root, apart from these files and `LICENSE`, names are kebab-case.
- Dates in the UI and in documents: DD.MM.YYYY.
- Commit messages, pull request descriptions and release notes are one text in English and
  Polish: title and body in English, a blank line, then the same in Polish. A release is an
  annotated tag `vX.Y.Z` whose message is the release notes (English first, Polish below).
  Details in [CONTRIBUTING.md](../CONTRIBUTING.md).
