# Podpis wydań

[English](release-signing.md) · Polski

Od 0.6.4 przycisk **Aktualizuj** instaluje tylko instalator podpisany kluczem właściciela.
Klucz publiczny jest wbudowany w aplikację (`SIGNING_PUBLIC_KEY` w `crates/core/src/updates.rs`),
klucz prywatny jest tylko u właściciela i w sekretach Actions repozytorium. Podpis to plik
[minisign](https://jedisct1.github.io/minisign/) obok instalatora w wydaniu:
`WalkLevel-Setup-X.Y.Z.exe.sig`.

Klucze i podpisy robi `tauri signer`, który jest w projekcie (`@tauri-apps/cli` w
`package.json`), więc nie trzeba niczego doinstalowywać. Aplikacja sprawdza je biblioteką
[`minisign-verify`](https://crates.io/crates/minisign-verify) (bez zależności; tej samej używa
aktualizator Tauri).

## Jednorazowa konfiguracja (właściciel)

Komendy w PowerShellu w folderze repozytorium, po `npm ci`.

1. **Wygeneruj parę kluczy.** Trzymaj ją poza repozytorium:

   ```powershell
   New-Item -ItemType Directory -Force "$env:USERPROFILE\.walklevel" | Out-Null
   npx tauri signer generate -w "$env:USERPROFILE\.walklevel\walklevel-signing.key"
   ```

   Komenda dwa razy pyta o hasło. Wybierz długie i najpierw zapisz je w menedżerze haseł.
   Powstaje `walklevel-signing.key` (klucz prywatny zaszyfrowany hasłem) i
   `walklevel-signing.key.pub` (klucz publiczny).

2. **Wpisz klucz publiczny do aplikacji.** Skopiuj jedyną linię pliku `.pub`:

   ```powershell
   Get-Content "$env:USERPROFILE\.walklevel\walklevel-signing.key.pub" | Set-Clipboard
   ```

   W `crates/core/src/updates.rs` zastąp tekst stałej `SIGNING_PUBLIC_KEY`
   (`"PLACEHOLDER: …"`) tą linią w cudzysłowie. Potem sprawdź i zrób commit:

   ```powershell
   cargo test -p walklevel-core --lib updates
   ```

   Test `built_in_key_is_placeholder_or_valid` nie przejdzie, jeśli linia nie jest kluczem
   publicznym minisign. Dopóki stoi placeholder, kompilacja wydania (`cargo build --release`,
   `packaging/build.ps1`) kończy się błędem, tak samo workflow wydania.

3. **Dodaj klucz prywatny i jego hasło jako sekrety Actions** (potrzebne `gh auth login`):

   ```powershell
   Get-Content -Raw "$env:USERPROFILE\.walklevel\walklevel-signing.key" | gh secret set WALKLEVEL_SIGNING_KEY --repo damnit-damian/WalkLevel-by-damnit.com.pl
   gh secret set WALKLEVEL_SIGNING_KEY_PASSWORD --repo damnit-damian/WalkLevel-by-damnit.com.pl
   ```

   Druga komenda pyta o wartość: wklej hasło. Bez `gh`: na GitHubie otwórz repozytorium,
   **Settings > Secrets and variables > Actions > New repository secret** i dodaj oba sekrety
   pod tymi nazwami (wartość `WALKLEVEL_SIGNING_KEY` to cała zawartość pliku `.key`).

4. **Przechowaj bezpieczną kopię klucza prywatnego.** Plik `.key` (jako załącznik albo jego
   tekst jako bezpieczna notatka) i hasło trzymaj w menedżerze haseł, dodatkowo możesz mieć
   kopię na zaszyfrowanym pendrivie trzymanym offline. Nigdy nie wkładaj klucza do
   repozytorium, do synchronizowanego folderu bez szyfrowania, do maila ani na czat. Kto ma
   klucz i hasło, może wydać aktualizację, którą przyjmie każdy zainstalowany WalkLevel.

## Co robi workflow wydania

`.github/workflows/release.yml`, zadanie `budowa`:

1. Od razu się zatrzymuje, gdy brakuje `WALKLEVEL_SIGNING_KEY` albo
   `WALKLEVEL_SIGNING_KEY_PASSWORD` albo gdy `SIGNING_PUBLIC_KEY` to wciąż placeholder.
2. Buduje exe i `WalkLevel-Setup-X.Y.Z.exe` (`packaging/build.ps1`).
3. Podpisuje go: `npx tauri signer sign --app-version X.Y.Z` zapisuje
   `WalkLevel-Setup-X.Y.Z.exe.sig`. Wersja trafia do zaufanego komentarza podpisu
   (`version:X.Y.Z`).
4. Sprawdza nowy podpis kluczem wbudowanym w aplikację (pomijany test w `updates.rs`,
   uruchamiany tylko tutaj). Jeśli sekret i klucz publiczny nie są jedną parą, wydanie się
   zatrzymuje, zamiast wypuścić aktualizację, której nikt nie zainstaluje.
5. Tylko w 0.6.4 (`LEGACY_INSTALLER_COPY`): kopiuje instalator, `.sha256` i `.sig` pod starą
   nazwę `Bieznia-Setup-X.Y.Z.exe` dla przycisku Aktualizuj w 0.6.3.
6. Dodaje poświadczenie pochodzenia i tworzy szkic wydania z instalatorem, `.sha256` i `.sig`;
   publikacja następuje po przejściu testów.

## Jak aplikacja sprawdza aktualizację

1. Instalator musi pochodzić z
   `https://github.com/damnit-damian/WalkLevel-by-damnit.com.pl/releases/download/<tag>/<plik>`
   (przekierowanie tylko na serwery pobierania GitHuba).
2. Najpierw pobiera `<instalator>.sig`. Brak `.sig` w wydaniu: nic nie jest pobierane, a
   aplikacja pokazuje `aktualizacja.powod.signature-missing`.
3. Pobiera instalator i sprawdza SHA-256 podane przez GitHub.
4. Sprawdza podpis minisign (Ed25519 nad skrótem BLAKE2b pliku) kluczem `SIGNING_PUBLIC_KEY`
   i to, że zaufany komentarz podaje wersję wydania. Uszkodzony podpis, inny klucz, inny plik
   albo inna wersja: plik jest usuwany, a aplikacja pokazuje
   `aktualizacja.powod.signature-invalid`.
5. Dopiero wtedy uruchamia instalator.

## Podpis ręczny

Do lokalnego testu samodzielnie zbudowanego instalatora:

```powershell
npx tauri signer sign -f "$env:USERPROFILE\.walklevel\walklevel-signing.key" --app-version X.Y.Z dist-installer\WalkLevel-Setup-X.Y.Z.exe
```

## Nowy klucz

Gdy klucz zginie, zainstalowanych aplikacji nie da się już zaktualizować przyciskiem:
użytkownicy muszą pobrać kolejny instalator ręcznie. Gdy klucz wycieknie, od razu zrób nową
parę, wpisz nowy klucz publiczny do aplikacji, to wydanie podpisz ostatni raz **starym**
kluczem (zainstalowane aplikacje ufają tylko jemu), potem podmień oba sekrety. Każde kolejne
wydanie jest podpisane nowym kluczem.

## Testy

`crates/core/tests/fixtures/updates/signatures.json` zawiera WYŁĄCZNIE TESTOWE klucze i podpisy
zrobione `tauri signer` do testów jednostkowych (dobry podpis, uszkodzony, brak podpisu, podpis
innego pliku, innej wersji i innym kluczem). Ich kluczy prywatnych nie zachowano, a aplikacja
im nie ufa.
