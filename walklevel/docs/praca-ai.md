# Praca z asystentem AI nad WalkLevel (Tauri 2 + Svelte 5 + Rust)

Stan wiedzy: listopad 2026. Dokument opisuje praktykę, nie obietnice producentów. Tam, gdzie
źródłem jest blog lub projekt społecznościowy, a nie oficjalna dokumentacja, jest to zaznaczone.
Numery wersji odnoszą się do tego repozytorium (Tauri 2.12, Svelte 5.57, Rust edition 2024 /
1.90, reqwest 0.13, btleplug 0.13, windows 0.62).

## 1. Najkrócej

- Asystenci AI (Claude Code, Copilot, Cursor) piszą Rusta i TypeScript dobrze, ale **Svelte 5 i
  Tauri 2 są w danych treningowych reprezentowane słabiej niż ich poprzednie wersje**. Typowy błąd
  to nie zły algorytm, tylko poprawny kod dla *starej* wersji API.
- Ten stos ma jedną dużą przewagę nad Pythonem/Qt: **większość takich pomyłek wychodzi przy
  kompilacji** (`cargo check`, `svelte-check`, `tsc`), a nie u użytkownika. Kompilator działa jak
  recenzent, który nigdy się nie męczy, pod warunkiem że agent faktycznie go uruchamia po każdej
  zmianie.
- Koszt: trzy warstwy (Rust, TypeScript, Svelte) plus granica IPC plus pliki konfiguracji
  (`tauri.conf.json`, `capabilities/`). Agent musi trzymać w głowie więcej miejsc, w których
  „to samo” musi się zgadzać.
- Najwięcej daje: wąski, czysty rdzeń w Ruście bez zależności od Tauri (już jest:
  `crates/core`), cienka warstwa komend, krótki `CLAUDE.md` z listą zakazanych wzorców i
  poleceń weryfikujących, oficjalne narzędzia Svelte dla LLM (MCP + autofixer) oraz przypięte
  wersje.

## 2. Typowe pułapki dla AI

### 2.1 Svelte 4 zamiast Svelte 5 (runes)

Najczęstszy i najlepiej udokumentowany problem. Większość treści o Svelte w sieci powstała przed
Svelte 5, więc modele domyślnie wracają do starej składni, a najgorsze są *mieszanki*: runes w
jednym miejscu, `$:` w drugim. Społeczność zbudowała nawet benchmarki (SvelteBench, svelte-evals)
mierzące właśnie zgodność z runes.

| Stary wzorzec (Svelte 4) | Svelte 5 |
|---|---|
| `let x = 0` jako stan reaktywny | `let x = $state(0)` |
| `$: y = x * 2` | `const y = $derived(x * 2)` (lub `$derived.by(() => ...)`) |
| `$: { efekt uboczny }` | `$effect(() => { ... })`, ale tylko gdy naprawdę trzeba |
| `export let prop` | `let { prop } = $props()` |
| `on:click={f}` | `onclick={f}` |
| `createEventDispatcher` | funkcje przekazywane jako propsy (callback props) |
| `<slot />`, `slot="x"` | `{#snippet x()}` + `{@render x()}` |
| `new App({ target })` | `mount(App, { target })` |
| store `writable` do stanu współdzielonego | moduł `*.svelte.ts` z `$state` (store dalej działa, ale to nie jest domyślna droga) |

Subtelniejsze pułapki, których kompilator nie złapie:

- **Nadużywanie `$effect`.** Dokumentacja Svelte nazywa `$effect` „wyjściem awaryjnym” i odradza
  używanie go do synchronizowania jednego stanu z drugim; do tego służy `$derived`. Dwa efekty
  aktualizujące się nawzajem to typowy przepis na pętlę. Modele chętnie piszą `$effect`, bo
  przypomina `$:` i `useEffect` z Reacta.
- **Destrukturyzacja stanu** (`let { a } = obiektState`) gubi reaktywność.
- **Runes poza plikami `.svelte` / `.svelte.ts`** nie działają (zwykły `.ts` ich nie przetworzy).

### 2.2 Tauri v1 zamiast Tauri v2

Tauri 2 zmienił sporo nazw i całą koncepcję uprawnień. Typowe pomyłki:

- Import `invoke` z `@tauri-apps/api/tauri` (v1) zamiast `@tauri-apps/api/core` (v2).
- API okna: w v2 jest `getCurrentWindow()` z `@tauri-apps/api/window` oraz
  `getCurrentWebviewWindow()` z `@tauri-apps/api/webviewWindow` (nazwy z wersji 2.1+; wcześniej
  `getCurrent`). Modele mieszają wszystkie trzy warianty.
- Moduły takie jak `fs`, `dialog`, `shell`, `http`, `notification`, `updater` **nie są już w
  `@tauri-apps/api`**, tylko w osobnych wtyczkach. Każda wtyczka wymaga trzech rzeczy naraz:
  crate w `Cargo.toml`, pakiet npm i rejestracji `.plugin(...)` w `Builder`, a do tego wpisu w
  `capabilities/`. Agent często zrobi dwie z czterech.
- `allowlist` w `tauri.conf.json` **nie istnieje w v2**. Jeden z pakietów skilli dla Claude
  wprost zauważa, że model często generuje konfigurację w stylu v1 (źródło społecznościowe).
- W Ruście: metody `emit`/`listen` wymagają zaimportowania traitów (`use tauri::Emitter;`,
  `use tauri::Manager;`). Błąd „no method named `emit`” to najczęściej brak importu, a nie
  zła nazwa; agent bywa skłonny „poprawiać” to przepisywaniem kodu na v1.
- Struktura `tauri.conf.json` v2 (`build.devUrl`, `build.frontendDist`, `app.windows`, ...) różni
  się od v1 (`devPath`, `distDir`, `tauri.windows`). Schemat `$schema` w pliku pomaga: edytor i
  agent dostają błąd od razu.

### 2.3 System uprawnień (capabilities)

Ważne rozróżnienie, które modele często mylą:

- **Własne komendy aplikacji** (`#[tauri::command]` zarejestrowane w `invoke_handler`) są według
  oficjalnej dokumentacji **dozwolone domyślnie dla wszystkich okien**. Można je ograniczyć przez
  `AppManifest::commands` w `build.rs`.
- **Wtyczki i API rdzenia** wymagają jawnych uprawnień w `src-tauri/capabilities/*.json`
  (np. `core:window:allow-set-title`, `dialog:default`). Brak wpisu daje w konsoli błąd typu
  „... not allowed”, a z perspektywy UI wywołanie „po prostu nic nie robi”.
- Uprawnienia przypisuje się do **etykiety okna** (`"windows": ["main"]`), nie tytułu. Nowe
  okno o innej etykiecie nie dostaje niczego, dopóki go nie dopiszesz.
- Okno należące do kilku capabilities dostaje sumę uprawnień. Gdy `tauri.conf.json` zacznie
  wymieniać capabilities jawnie, pozostałe pliki z katalogu przestają być ładowane.
- Typowe błędy AI to dwa przeciwne skrajności: brak uprawnienia (funkcja nie działa) albo
  „naprawa” przez nadanie szerokiego zestawu (`"windows": ["*"]`, wszystkie `default` wszystkich
  wtyczek). Druga jest gorsza, bo nic jej nie zgłosi.
- Komenda, której nie ma w `generate_handler![...]`, też „milczy”. To kolejne miejsce, które musi
  się zgadzać.

### 2.4 Biblioteki Rusta, które zmieniły API

Model zna najczęściej wersję sprzed roku lub dwóch. W tym repo konkretnie:

- **reqwest 0.13** (grudzień 2025): domyślny TLS to rustls, feature `rustls-tls` nazywa się teraz
  `rustls`, dostawca kryptografii domyślnie aws-lc zamiast ring, `RequestBuilder::query()` i
  `form()` są za feature'ami (`query`, `form`) wyłączonymi domyślnie, usunięto stare
  przestarzałe metody. Model wytrenowany na 0.11/0.12 napisze `features = ["rustls-tls"]` i
  `.form(...)` bez feature'a. Repo już ma poprawne `rustls` i `query`.
- **keyring**: wersja 3 nie ma domyślnych feature'ów (trzeba podać `windows-native` itd., inaczej
  dostaje się magazyn „mock”, który nic nie zapisuje), wersja 4 rozbiła magazyny na osobne crate'y.
  To klasyczny przypadek, gdzie kod kompiluje się, a nie działa. WalkLevel używa bezpośrednio
  `windows` (`Win32_Security_Credentials`), więc problem dotyczy tylko sytuacji, gdyby agent
  zaproponował „przejście na keyring”.
- **windows** (crate Microsoftu): nazwy feature'ów i ścieżki modułów zmieniają się między
  wersjami 0.5x/0.6x; brak feature'a daje błąd „unresolved import”, który agent próbuje leczyć
  zmianą kodu zamiast dopisaniem feature'a.
- **btleplug**, **rusqlite**, **zip**, **toml**: każde ma historię zmian sygnatur. Ogólna zasada:
  prawdziwe API jest na dysku, w `~/.cargo/registry/src/.../<crate>-<wersja>/`. Polecenie dla
  agenta „sprawdź sygnaturę w źródle crate'a w rejestrze / na docs.rs dla dokładnie tej wersji”
  działa lepiej niż ponowne zgadywanie.
- Badania nad halucynacjami pakietów (2025-2026) pokazują, że modele wymyślają też nieistniejące
  crate'y. W Ruście jest to mniej groźne niż w npm/PyPI, bo crate musi być jawnie dopisany do
  `Cargo.toml`, ale nowa zależność zawsze wymaga sprawdzenia, czy istnieje i kto ją utrzymuje.
- **Edition 2024** (repo jej używa): zmiany w regułach przechwytywania w `impl Trait`, `unsafe
  extern`, zarezerwowane `gen`. Kod w stylu 2021 zwykle daje czytelny błąd kompilacji, więc to
  niewielki problem.

### 2.5 Async w Ruście i w Tauri

- **`reqwest::blocking` wewnątrz runtime'u tokio panikuje** (blokujący klient tworzy własny
  runtime). Komendy `async` w Tauri działają w runtime'ie tokio. Repo ma włączony feature
  `blocking`, więc warto ustalić zasadę: klient blokujący tylko w wątkach spoza runtime'u albo w
  `spawn_blocking`, w komendach async wyłącznie klient asynchroniczny.
- **`std::sync::MutexGuard` trzymany przez `.await`** sprawia, że future nie jest `Send`, a Tauri
  wymaga `Send` dla komend async. Błąd kompilacji jest długi i mylący; modele „naprawiają” go,
  zamieniając wszystko na `tokio::sync::Mutex`, co nie zawsze jest dobre. Zasada: zwolnij blokadę
  przed `.await` (zakres `{ }`), `tokio::sync::Mutex` tylko gdy trzeba trzymać blokadę przez
  `.await`.
- **Pożyczone argumenty w komendach async** (`&str`, `State<'_, T>`) wymagają, by komenda zwracała
  `Result`; inaczej trzeba używać typów posiadanych (`String`). To z oficjalnej dokumentacji.
- **Błędy zwracane z komend muszą implementować `serde::Serialize`**. Zalecany wzorzec: własny enum
  błędów z `thiserror` i ręcznym `Serialize` (repo ma już `thiserror`).
- **Nazwy argumentów:** Rust `snake_case` przychodzi z JS jako `camelCase` (`invoke_message` to
  `invokeMessage`). Literówka w kluczu `invoke` daje błąd dopiero w działaniu, nie przy
  kompilacji. To najsłabszy punkt całego stosu: granica IPC jest typowana tylko „na słowo”.
- `tokio::spawn` wywołany poza kontekstem runtime'u (np. w niektórych hookach) panikuje;
  bezpieczniej `tauri::async_runtime::spawn`.
- Blokujące operacje (BLE, SQLite, system plików) w komendzie synchronicznej wykonują się w
  wątku głównym i zamrażają okno. Komendy niesynchroniczne domyślnie idą na wątek główny, chyba
  że są `async` albo `#[tauri::command(async)]`.

### 2.6 Kompilacja i czas iteracji

- Pierwsza kompilacja Tauri trwa minuty, kolejne kompilacje przyrostowe zwykle kilka do
  kilkudziesięciu sekund. Dla agenta to dużo, bo pętla „zmień, sprawdź” powtarza się dziesiątki
  razy na zadanie.
- `cargo check` jest wyraźnie szybszy od `cargo build` i wystarcza jako pętla wewnętrzna.
  `tauri dev` i pełny build zostaw na koniec zadania.
- **rust-analyzer i `tauri dev` potrafią sobie nawzajem unieważniać cache**, jeśli dzielą katalog
  `target` z innymi flagami. Popularna poprawka: osobny katalog dla rust-analyzera
  (`"rust-analyzer.cargo.targetDir": "target/analyzer"` w ustawieniach VS Code). Jeden autor
  opisuje spadek z około minuty do kilkunastu sekund (źródło społecznościowe).
- Nie czyść `target` bez potrzeby (`cargo clean` kasuje stan przyrostowy). Naprzemienne
  `cargo check` i `cargo build` też dziś powoduje część rekompilacji.
- Testy rdzenia (`cargo test -p walklevel-core`) nie muszą kompilować Tauri ani webview. To
  najszybsza pętla i główny powód, żeby logika żyła w `crates/core`.
- Profil `release` w repo (`lto = true`, `codegen-units = 1`, `opt-level = "s"`) jest dobry dla
  małego exe, ale wolny w kompilacji: agent nie powinien go używać do sprawdzania zmian.

## 3. Porównanie z Pythonem/Qt (dotychczasowa aplikacja)

| | Python + Qt (PySide6/PyQt) | Tauri 2 + Svelte 5 + Rust |
|---|---|---|
| Ilość danych treningowych | Bardzo duża, AI pisze płynnie | Rust i TS duża, Svelte 5 i Tauri 2 mała |
| Typowa pomyłka AI | PyQt5 vs PyQt6 vs PySide6: `exec_()` vs `exec()`, krótkie enumy (`Qt.AlignCenter`) vs pełne (`Qt.AlignmentFlag.AlignCenter`), różne sygnały/sloty | Svelte 4 zamiast 5, Tauri v1 zamiast v2, stare API crate'ów |
| Kiedy pomyłka wychodzi | Często dopiero przy uruchomieniu danej ścieżki kodu (mypy/ruff pomagają częściowo) | Większość przy `cargo check` / `svelte-check` / `tsc` |
| Wielowątkowość | Łatwo o dostęp do widżetów spoza wątku GUI; błędy losowe | Kompilator wymusza `Send`/`Sync`; za to długie, trudne komunikaty błędów |
| Czas iteracji | Natychmiastowy (bez kompilacji) | Sekundy do minut na zmianę w Ruście; Vite HMR dla UI natychmiast |
| Liczba miejsc do zgrania | Jeden język, jeden proces | Rust + TS + Svelte + IPC + capabilities + `tauri.conf.json` |
| Pakowanie | PyInstaller: duży exe, częste fałszywe alarmy antywirusów, kruche zależności | Mały exe/instalator, ale osobny łańcuch narzędzi (MSVC, WebView2) |
| Refaktoryzacja przez AI | Ryzykowna bez dobrych testów: zmiana nazwy, której nie widać aż do uruchomienia | Bezpieczniejsza: kompilator wskaże każde miejsce do poprawy |

Wniosek: w Pythonie AI szybciej produkuje kod, w Ruście szybciej dowiadujesz się, że kod jest zły.
Przy długim utrzymaniu przez jedną osobę liczy się bardziej to drugie, ale tylko jeśli granica
IPC i reguły Svelte są pilnowane, bo tam kompilator Rusta nie sięga.

Dla równowagi: kontrolowane badanie METR (2025, doświadczeni programiści, duże dojrzałe repozytoria,
narzędzia z początku 2025) wykazało *spowolnienie* o 19% przy użyciu AI, mimo że uczestnicy
sądzili, że przyspieszyli. To nie jest wyrok, ale argument, żeby mierzyć efekt (czy zadanie
naprawdę się skończyło, czy testy przechodzą), a nie polegać na wrażeniu.

## 4. Co pomaga

### 4.1 Kompilator i narzędzia sprawdzające jako recenzent

- Badania nad „kompilatorem w pętli” (RustAssistant 2023: około 74% skuteczności w naprawie
  rzeczywistych błędów kompilacji Rusta; badanie z 2026 na 699 zadaniach: duży spadek błędów
  składni i niezdefiniowanych odwołań, gdy model dostaje wynik kompilatora) potwierdzają to, co
  widać w praktyce: model z dostępem do `cargo check` jest dużo lepszy niż bez niego.
- Te same badania ostrzegają, że komunikaty kompilatora potrafią model zmylić, a agent może
  „naprawiać” w kółko. Zasada: jeśli ten sam błąd wraca po 2-3 próbach, zatrzymaj się i przeczytaj
  dokumentację/źródło crate'a zamiast zgadywać dalej.
- Zestaw bramek dla tego repo (od najszybszej):
  1. `cargo check --workspace`
  2. `cargo clippy --workspace -- -D warnings`
  3. `cargo test -p walklevel-core`
  4. `npm run check` (svelte-check + tsc)
  5. `npm run tauri dev` tylko do ręcznego sprawdzenia UI

### 4.2 Typowana granica IPC

Najwięcej „cichych” błędów powstaje na `invoke('nazwa', { klucz })`. Dwie drogi:

- **tauri-specta** (lub TauRPC): generuje z Rusta plik TypeScript z typowanymi funkcjami komend i
  zdarzeń. Literówka w nazwie lub kluczu staje się błędem `tsc`. Wersja dla Tauri 2 to linia
  2.x (długo w stanie RC), więc trzeba sprawdzić stan przed przypięciem.
- Minimum bez dodatkowej zależności: **jeden plik `src/lib/api.ts`**, w którym jest każde
  `invoke`, z ręcznie napisanymi typami zgodnymi z `serde` w Ruście. Reszta UI importuje tylko z
  niego. Agent ma wtedy jedno miejsce do sprawdzenia.

### 4.3 Dokumentacja dla LLM

- **Svelte**: oficjalne `llms.txt` w kilku rozmiarach (`/llms-small.txt`, `/llms-medium.txt`,
  `/llms-full.txt`, osobno dla samego Svelte: `/docs/svelte/llms.txt`), oficjalny serwer MCP
  `@sveltejs/mcp` (narzędzia `list-sections`, `get-documentation`, `svelte-autofixer`),
  skille i wtyczki dla Claude Code, Cursor i Copilot CLI. Konfiguracja automatyczna:
  `npx sv add ai-tools`. `svelte-autofixer` to statyczna analiza wyłapująca właśnie wzorce
  Svelte 4 i inne częste błędy AI; działa też z CLI bez MCP
  (`npx @sveltejs/mcp svelte-autofixer plik.svelte`).
- **Tauri**: `https://v2.tauri.app/llms.txt` (indeks), `llms-small.txt` (skrót) i
  `llms-full.txt` (całość). Oficjalnego serwera MCP z dokumentacją nie znalazłem; są
  społecznościowe. Istnieją też społecznościowe wtyczki MCP do sterowania działającą aplikacją
  Tauri (zrzuty ekranu webview, kliknięcia, wykonywanie JS), np. `@hypothesi/tauri-mcp-server`;
  przed użyciem trzeba sprawdzić, czy są utrzymywane, i nie dodawać ich do buildu produkcyjnego.
- Praktyka: nie wklejaj całego `llms-full.txt` do `CLAUDE.md` (megabajt tekstu w każdej sesji).
  Wpisz link i polecenie „przy pracy nad X najpierw pobierz Y”, albo podłącz MCP, który pobiera
  tylko potrzebne sekcje.

### 4.4 Przypinanie wersji

- `Cargo.lock` i `package-lock.json` w repo (są). Agent instaluje zależności z lockfile'a
  (`npm ci`, `cargo build --locked`), a nie „najnowsze”.
- W `CLAUDE.md` krótka tabela: biblioteka, wersja, „uwaga” (np. „reqwest 0.13: feature `rustls`,
  `query`; nie `rustls-tls`”). To kilka linii, które oszczędzają godziny.
- Aktualizacje zależności jako osobne, małe zadania z własnym przejściem bramek, nigdy „przy
  okazji”.

### 4.5 Wąskie moduły i testy

- Agent działa najlepiej, gdy zadanie mieści się w jednym module, a moduł da się przetestować
  bez reszty. `crates/core` z modułami `treadmill`, `history`, `levels`, `mapping`, ... to dobry
  podział; ważne, żeby `core` **nigdy nie zależał od `tauri`**.
- Testy w `core` piszą się łatwo i szybko kompilują. Dla BLE: trait nad urządzeniem i atrapa w
  testach, prawdziwy `btleplug` tylko w implementacji produkcyjnej.
- Pliki powyżej kilkuset linii utrudniają agentowi pracę (więcej kontekstu, więcej przypadkowych
  zmian). Lepiej rozdzielić niż rozbudowywać.

## 5. Rekomendacje dla tego repo

### 5.1 Układ projektu

```
walklevel/
  CLAUDE.md                 # krótki: stos, wersje, bramki, zakazy, granice (< 200 linii)
  .claude/rules/
    svelte.md               # paths: src/**/*.svelte, src/**/*.svelte.ts
    tauri.md                # paths: src-tauri/**
    core.md                 # paths: crates/core/**
  docs/
    praca-ai.md             # ten plik
    decyzje.md              # krótkie zapisy decyzji (data, decyzja, powód) - zamiast długich opisów
  crates/core/              # cała logika; zero zależności od tauri; testy jednostkowe
  src-tauri/
    src/commands/           # cienkie adaptery: parsuj argumenty, wywołaj core, zmapuj błąd
    capabilities/           # minimalne uprawnienia, każde dopisane świadomie
  src/
    lib/api.ts              # JEDYNE miejsce z invoke/listen (lub wygenerowane bindingi)
    lib/stan/*.svelte.ts    # stan współdzielony na runes
    lib/components/         # małe komponenty, jeden plik = jedna rzecz
```

### 5.2 Zawartość CLAUDE.md (szkielet)

Oficjalna dokumentacja Claude Code zaleca poniżej 200 linii na plik `CLAUDE.md`: dłuższy zużywa
kontekst i jest gorzej przestrzegany. Importy `@plik` nie zmniejszają kosztu (ładują się razem).
Reguły potrzebne tylko w części kodu idą do `.claude/rules/*.md` z polem `paths:` we
frontmatterze; ładują się tylko przy pracy z pasującymi plikami.

Proponowane sekcje (każda kilka linii):

1. **Stos i wersje** z jednolinijkowymi uwagami (Tauri 2.12, Svelte 5 runes, Rust 2024 / 1.90,
   reqwest 0.13, btleplug 0.13, windows 0.62).
2. **Bramki „gotowe”**: lista poleceń z punktu 4.1; zadanie nie jest skończone, dopóki nie
   przechodzą.
3. **Zakazane wzorce** (lista do szybkiego `grep`):
   - Svelte: `export let`, `$:`, `on:`, `createEventDispatcher`, `<slot`; `$effect` do
     przepisywania stanu.
   - Tauri: `@tauri-apps/api/tauri`, `allowlist`, `invoke` poza `src/lib/api.ts`.
   - Rust: `reqwest::blocking` w kodzie async, `.unwrap()` w kodzie produkcyjnym komend,
     `MutexGuard` przez `.await`, zależność `tauri` w `crates/core`.
4. **Granice architektury**: co wolno importować skąd.
5. **Zasady zależności**: nowa zależność tylko z uzasadnieniem w opisie zmiany; najpierw
   sprawdzić std i istniejące crate'y; sprawdzić wersję na crates.io/docs.rs.
6. **Uprawnienia**: każdy nowy wpis w `capabilities/` z komentarzem po co; nigdy `"*"` w
   `windows` bez decyzji.
7. **Gdzie szukać prawdy**: linki do `llms.txt` Tauri i Svelte, polecenie „sygnatury crate'ów
   sprawdzaj w źródle w rejestrze cargo / na docs.rs dla wersji z Cargo.lock”.
8. **Zasady procesu** już ustalone w projekcie (makiety przed większymi zmianami UI, wydanie
   tylko za zgodą, bez instalowania niczego na komputerze użytkownika).

Jeśli kiedyś dojdzie Copilot lub Cursor: wspólne reguły w `AGENTS.md`, a `CLAUDE.md` importuje
go (`@AGENTS.md`) plus dopisuje rzeczy specyficzne dla Claude. Copilot czyta
`.github/copilot-instructions.md` i `AGENTS.md`, Cursor `.cursor/rules/*.mdc` i `AGENTS.md`.
Jedno źródło prawdy, zero kopiowania.

### 5.3 Automatyczne pilnowanie zamiast proszenia

- Hook Claude Code (np. `PostToolUse` po edycji `.svelte`) uruchamiający `svelte-autofixer` lub
  `svelte-check` na zmienionym pliku, i prosty skrypt `grep` na zakazane wzorce. To, co da się
  sprawdzić automatem, nie powinno zależeć od tego, czy model pamięta zasadę.
- `clippy` z `-D warnings` w tej samej roli dla Rusta.
- Hooki i ustawienia to zmiana konfiguracji: wprowadzać świadomie, jako osobny krok.

### 5.4 Przeciw bloatowi

- Jedna zmiana = jedno zadanie. Agent nie „porządkuje przy okazji” niezwiązanych plików.
- Usuwaj martwy kod od razu (kompilator Rusta i `svelte-check` pokażą nieużywane elementy;
  `cargo machete` znajdzie nieużywane zależności).
- Żadnych abstrakcji „na zapas”: trait dopiero przy drugiej implementacji (wyjątek: atrapa BLE w
  testach).
- `CLAUDE.md` przeglądać co kilka wydań: usuwać zasady, które już pilnuje automat lub które
  przestały być prawdziwe. Przestarzała instrukcja szkodzi bardziej niż brak instrukcji.
- Dokumentacja krótka: `decyzje.md` z jednym akapitem na decyzję zamiast rozbudowanych opisów,
  które się dezaktualizują.

## 6. Czego nie udało się potwierdzić

- Brak twardych danych porównujących skuteczność asystentów AI dla Tauri vs Python/Qt; porównanie
  w punkcie 3 to synteza znanych typów błędów i rozsądku, nie wynik badania.
- Aktualnych rankingów SvelteBench (który model najlepiej pisze Svelte 5) nie udało się pobrać.
- Konkretne „halucynacje” API reqwest/keyring przez modele to obserwacja praktyczna oparta na
  zmianach w changelogach, nie zmierzone zjawisko.
- Stan utrzymania społecznościowych serwerów MCP dla Tauri nie został sprawdzony.

## Źródła

Oficjalne:

- Svelte, dokumentacja dla LLM: https://svelte.dev/docs/llms oraz https://svelte.dev/llms.txt
- Svelte, narzędzia AI (MCP, skille, subagenci): https://svelte.dev/docs/ai/overview
- Svelte, zalecane instrukcje dla agenta: https://svelte.dev/docs/ai/instructions
- Svelte, CLI MCP / autofixer: https://svelte.dev/docs/ai/cli
- Svelte, `$effect` i kiedy go nie używać: https://svelte.dev/docs/svelte/$effect
- Svelte, przewodnik migracji do v5: https://svelte.dev/docs/svelte/v5-migration-guide
- Tauri, llms.txt: https://v2.tauri.app/llms.txt , https://v2.tauri.app/llms-small.txt
- Tauri, migracja z v1: https://v2.tauri.app/start/migrate/from-tauri-1/
- Tauri, capabilities: https://v2.tauri.app/security/capabilities/
- Tauri, wywoływanie Rusta z frontendu (async, błędy, nazwy argumentów): https://v2.tauri.app/develop/calling-rust/
- Tauri, historia wersji `@tauri-apps/api`: https://v2.tauri.app/release/@tauri-apps/api/all-versions/
- tauri-build `AppManifest`: https://docs.rs/tauri-build/latest/tauri_build/struct.AppManifest.html
- reqwest 0.13 (blog autora): https://seanmonstar.com/blog/reqwest-v013-rustls-default/
- keyring 3.x, feature'y: https://docs.rs/crate/keyring/3.6.3/features
- Claude Code, pamięć i CLAUDE.md (limit 200 linii, importy, `.claude/rules`, AGENTS.md): https://code.claude.com/docs/en/memory
- GitHub Copilot i AGENTS.md: https://github.blog/changelog/2025-08-28-copilot-coding-agent-now-supports-agents-md-custom-instructions
- VS Code / Copilot, pliki instrukcji: https://code.visualstudio.com/docs/copilot/customization/custom-instructions
- PyQt6, różnice względem PyQt5: https://www.riverbankcomputing.com/static/Docs/PyQt6/pyqt5_differences.html

Badania:

- METR, wpływ AI na doświadczonych programistów (2025): https://metr.org/blog/2025-07-10-early-2025-ai-experienced-os-dev-study/
- RustAssistant, naprawa błędów kompilacji Rusta przez LLM: https://arxiv.org/abs/2308.05177
- Kompilator w pętli agenta: https://arxiv.org/abs/2601.12146
- Halucynacje crate'ów w kodzie Rusta: https://www.alphaxiv.org/abs/2606.08444
- Halucynacje pakietów (ogólnie): https://arxiv.org/abs/2501.19012
- Rust-SWE-bench, agenci na repozytoriach Rusta: https://www.alphaxiv.org/abs/2602.22764

Społecznościowe (mniej pewne, ale praktyczne):

- Problem Svelte 5 a modele AI (dyskusja r/sveltejs): https://www.reddit.com/r/sveltejs/comments/1kap87m/the_problem_with_svelte_and_ai_models/
- SvelteBench (dziś w sveltejs/ai): https://github.com/sveltejs/ai
- Autofixer na 134 komponentach (relacja z użycia): https://chudi.dev/blog/sveltekit-mcp-server-claude-code
- Tauri 2: capabilities, wtyczki, migracja (przegląd błędów): https://fixdevs.com/blog/tauri-2-not-working/
- Skill dla Claude o uprawnieniach Tauri 2: https://skills.sh/openaec-foundation/tauri-2-claude-skill-package/tauri-syntax-permissions
- Szybszy `tauri dev` (osobny targetDir dla rust-analyzera): https://dev.to/ahonn/how-to-make-your-tauri-dev-faster-2en1
- Szybsza kompilacja Rusta: https://corrode.dev/blog/tips-for-faster-rust-compile-times/
- tauri-specta: https://github.com/specta-rs/tauri-specta
- TauRPC: https://docs.rs/crate/taurpc/latest
- Serwer MCP do sterowania aplikacją Tauri: https://github.com/hypothesi/mcp-server-tauri
- PyQt5 vs PyQt6: https://www.pythonguis.com/faq/pyqt5-vs-pyqt6/
- Długość CLAUDE.md w praktyce: https://dev.to/iggredible/your-claudemd-is-probably-too-long-17bi
