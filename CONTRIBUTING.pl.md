# Jak współtworzyć WalkLevel

[English](CONTRIBUTING.md) · Polski

## Bramki

Uruchamiane z katalogu głównego repozytorium, od najszybszej. CI robi to samo.

```powershell
npm ci
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
npm run check                     # słownik tekstów, svelte-check, tsc
npm run build
npm run test:ui                   # test widoków w Edge
node tests-ui/settings-collapse.ts
.\packaging\build.ps1 -Version X.Y.Z [-Iscc ścieżka\do\ISCC.exe]   # exe i instalator
```

Generatory: `npm run texts` (pakiety językowe z `docs/texts.md`, zob. `docs/i18n.pl.md`),
`npm run server-definitions` (plik definicji dla serwera kont). Skrypty są
w TypeScripcie i uruchamia je bezpośrednio Node.js 24, bez kompilacji.

Zasady projektu (układ, przypięte wersje, granica rdzeń - interfejs, wygląd) są w
[docs/architecture.pl.md](docs/architecture.pl.md).

## Standardy

Dotyczą nowego kodu i nowych plików. Istniejący kod po polsku migrujemy moduł po module
w osobnych zadaniach, nie przy okazji innych zmian.

- Nowy kod, identyfikatory i komentarze po angielsku. Komentarze tylko wtedy, gdy naprawdę są
  potrzebne.
- kebab-case w nazwach nowych plików i folderów, w klasach CSS i w kluczach tekstów. W kodzie
  konwencje języka: Rust `snake_case` / `CamelCase`, TypeScript `camelCase` / `PascalCase`.
- Wersje językowe dokumentu to osobne pliki z przyrostkiem przed rozszerzeniem: `README.md`
  (angielski) i `README.pl.md` (polski). W katalogu głównym wielkimi
  literami są tylko `README`, `CONTRIBUTING`, `PRIVACY` i `LICENSE`.
- Data w interfejsie i w dokumentach: DD.MM.RRRR.
- Teksty dla użytkownika pochodzą z `docs/texts.md`, tłumaczenia z `locales/` (`npm run texts`
  odtwarza pakiety, `docs/i18n.pl.md` opisuje tłumaczenie).
- Jedna zmiana na zadanie, bez abstrakcji na zapas, bez zależności dla jednej funkcji.

## Commity, pull requesty i wydania

Wszystko, co trafia na GitHuba, jest naraz po angielsku i po polsku. Dokumenty to dwa pliki
(wyżej). Komunikat commita, opis pull requesta i opis wydania to jeden tekst: najpierw
angielski, pod nim polski.

Komunikat commita:

```
Short title in English

Body in English.

Krótki tytuł po polsku

Treść po polsku.
```

Wydanie to tag z opisem `vX.Y.Z`. Treść tagu staje się opisem wydania, więc piszemy ją po
angielsku i po polsku:

```powershell
git tag -a v0.6.3 -F opis.txt
git push origin v0.6.3
```

GitHub Actions uruchamia bramki, buduje exe i instalator, podpisuje instalator kluczem
właściciela, potem publikuje wydanie. Bez sekretów do podpisu wydanie się zatrzymuje; zobacz
[docs/release-signing.pl.md](docs/release-signing.pl.md).
