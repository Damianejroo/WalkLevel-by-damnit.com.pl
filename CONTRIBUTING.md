# Contributing to WalkLevel

English · [Polski](CONTRIBUTING.pl.md)

## Checks

Run from the repository root, fastest first. CI runs the same steps.

```powershell
npm ci
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
npm run check                     # text dictionary, svelte-check, tsc
npm run build
npm run test:ui                   # view test in Edge
node tests-ui/settings-collapse.ts
.\packaging\build.ps1 -Version X.Y.Z [-Iscc path\to\ISCC.exe]   # exe and installer
```

Generators: `npm run texts` (text dictionary from `docs/texts.md`),
`npm run server-definitions` (definitions file for the account server).
Scripts are TypeScript run by Node.js 24 directly, without a build step.

Project rules (layout, pinned versions, core/UI boundary, look) are in
[docs/architecture.md](docs/architecture.md).

## Standards

These apply to new code and new files. Existing Polish code is migrated module by module in
separate tasks, not as a side effect of other changes.

- New code, identifiers and comments in English. Comments only when they are really needed.
- kebab-case for new file and folder names, CSS classes and text keys. Inside code, follow the
  language: Rust `snake_case` / `CamelCase`, TypeScript `camelCase` / `PascalCase`.
- Language versions of a document are separate files with a suffix before the extension:
  `README.md` (English) and `README.pl.md` (Polish). In the repository
  root only `README`, `CONTRIBUTING`, `PRIVACY` and `LICENSE` are upper case.
- Dates in the UI and in documents: DD.MM.YYYY.
- User-facing texts come from `docs/texts.md` (`npm run texts` regenerates the dictionary).
- One change per task, no abstractions in advance, no dependency for a single function.

## Commits, pull requests and releases

Everything written for GitHub comes in English and Polish at once. Documents are two files
(see above). Commit messages, pull request descriptions and release notes are one text:
English first, Polish below.

Commit message:

```
Short title in English

Body in English.

Krótki tytuł po polsku

Treść po polsku.
```

A release is an annotated tag `vX.Y.Z`. The tag message becomes the release notes, so write
it in English and Polish:

```powershell
git tag -a v0.6.3 -F notes.txt
git push origin v0.6.3
```

GitHub Actions runs the checks, builds the exe and the installer, and publishes the release.
