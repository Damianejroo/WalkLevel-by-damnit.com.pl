# WalkLevel 0.6.0 – system wyglądu

Dokument opisuje wygląd nowej wersji (Tauri 2 + Svelte 5 + Rust, webview WebView2). Przenosi
tożsamość wizualną z wersji 0.5.x (PySide6: `bieznia/gui/theme.py`, `widgets.py`,
`komponenty.py`, `odznaki.py`, `popupy.py`, `okno.py`, `ekrany/*`) i od razu poprawia uwagi
Damiana (a–i). Wszystkie liczby poniżej to piksele logiczne CSS (przy 100% skali Windows
1 px = 1 px ekranu, przy 150% WebView2 sam skaluje) – tak samo jak piksele logiczne w Qt.

Spis:

1. Zasady w skrócie
2. Tokeny (CSS custom properties): motywy, akcenty profili, rzadkość, popup
3. Akcent: jeden element na widok
4. Typografia
5. Odstępy, wymiary, promienie
6. Ruch: czasy, krzywe, co wolno animować, architektura ekranów
7. Szerokość okna: min/max, zawijanie
8. Specyfikacja komponentów
9. Ekrany – co zmienia się względem 0.5.x
10. Zakazane wzorce i co je zastępuje
11. Lista kontrolna „nic nie wystaje” i test automatyczny
12. Mapowanie ze starego kodu

---

## 1. Zasady w skrócie

1. **Kolory, odstępy, promienie, czasy – tylko z tokenów.** Żadnych kolorów wpisanych w
   komponent (w 0.5.x kilka komponentów miało własne `setStyleSheet` z kolorami – koniec z tym).
2. **Spokojna, neutralna baza** (ciepłe szarości z 0.5.x), kolor tylko tam, gdzie niesie
   znaczenie: jeden akcent na widok (sekcja 3) i „kolory danych” (strefy wskaźnika, kolory
   profili na wykresie, kolory rzadkości na odznakach).
3. **Najwęższy układ zawsze się mieści.** Nic nie wystaje z prawej przy minimalnej szerokości
   okna, także z dużym tekstem; rzędy zawijają się zamiast rozpychać (sekcja 7).
4. **Stałe ekrany sterowane stanem.** Ekrany są montowane raz i aktualizowane przez stan
   (runes Svelte 5); nic nie jest przebudowywane przy każdej zmianie. Animacje w CSS tylko na
   `transform` i `opacity` (sekcja 6).
5. **Zmiana stanu nie zmienia układu.** Przełączenie trybu, zmiana napisu na przycisku, nowa
   liczba – wysokości i szerokości zostają (stałe sloty, cyfry tabelaryczne, szerokość
   przycisku = najdłuższy z jego napisów).
6. **Bez wzorców „typowych dla AI”**: bez pastylek wszędzie, bez kolorowych pasków z lewej,
   bez gradientów, bez emoji, bez poświat (sekcja 10).
7. **Dostępność jak w 0.5.x**: kontrast tekstu 7:1, tekstu pomocniczego 4,5:1, ramki kontrolki
   3:1; pole trafienia min. 44×44; obwódka fokusu tylko z klawiatury (`:focus-visible`);
   „Ogranicz ruch” z Windows wyłącza animacje (`prefers-reduced-motion`).

---

## 2. Tokeny

Proponowany plik: `walklevel/src/lib/styl/tokeny.css` (jedyne źródło wartości), obok
`baza.css` (reset, typografia, przewijanie) i komponenty w `src/lib/ui/*.svelte`.

### 2.1 Wspólne (niezależne od motywu)

```css
:root {
  color-scheme: light dark;

  /* kroje */
  --font-ui: "Segoe UI Variable Text", "Segoe UI Variable", "Segoe UI", system-ui, sans-serif;
  --font-num: "Bahnschrift", "Segoe UI Variable Display", "Segoe UI", sans-serif;
  --font-icon: "Segoe Fluent Icons", "Segoe MDL2 Assets";
  --font-mono: "Cascadia Mono", "Consolas", monospace;

  /* „Duży tekst” w Ustawieniach: 1.15 (jak w 0.5.x skaluje się tylko pismo, nie odstępy) */
  --text-scale: 1;

  /* typografia (rem; 1rem = 16px * --text-scale) */
  --fs-cap: 0.6875rem;   /* 11 – podpisy HUD wersalikami, oś wykresu, kicker popupu */
  --fs-xs: 0.75rem;      /* 12 – tytuł kafelka statystyki, drobne etykiety */
  --fs-sm: 0.8125rem;    /* 13 – podpowiedzi, druga linia wiersza */
  --fs-md: 0.875rem;     /* 14 – tekst wyciszony, segmenty */
  --fs-base: 1rem;       /* 16 – treść, przyciski */
  --fs-lg: 1.125rem;     /* 18 – nazwa rangi, tytuł w karcie poziomu */
  --fs-xl: 1.25rem;      /* 20 – tytuł strony */
  --fs-num: 1.375rem;    /* 22 – wartość kafelka statystyki (Bahnschrift) */
  --fs-num-lg: 1.5rem;   /* 24 – suma km na szlaku */
  --fs-gauge: 2.75rem;   /* 44 – prędkość na wskaźniku */

  --lh-tight: 1.25;
  --lh-body: 1.4;
  --lh-caps: 1.3;        /* zapas na kreski nad Ó, Ś, Ż w Bahnschrift */

  --fw-regular: 400;
  --fw-semibold: 600;
  --fw-bold: 700;

  /* odstępy */
  --sp-1: 4px;
  --sp-2: 8px;
  --sp-3: 12px;          /* GAP z 0.5.x: między kartami */
  --sp-4: 16px;          /* PAD z 0.5.x: margines strony */
  --sp-5: 24px;
  --sp-6: 32px;
  --card-pad: 12px 14px 14px;

  /* wymiary */
  --control-h: 44px;     /* przycisk, pole, przełącznik (pole trafienia) */
  --segment-h: 36px;     /* segment w przełączniku segmentowanym */
  --icon-btn: 44px;
  --row-h: 56px;         /* slot żetonów stref / paska Treningu, wiersz listy */
  --header-h: 64px;      /* nagłówek kategorii Ustawień */
  --pagebar-h: 60px;     /* pasek tytułu strony (tytuł + podpis) */
  --focus-w: 2px;
  --focus-offset: 2px;

  /* promienie */
  --r-xs: 3px;           /* pasek XP, słupki wykresu */
  --r-sm: 6px;           /* klawisz (KeyCap), podpowiedź */
  --r-seg: 7px;          /* segment wewnątrz segmentowanego (10 - 3) */
  --r-md: 10px;          /* karta, przycisk, pole, kafelek (RADIUS z 0.5.x) */
  --r-lg: 12px;          /* lista rozwijana (popover) */
  --r-round: 999px;      /* TYLKO: tor przełącznika, awatar, kropka stanu, próbka koloru */

  /* szerokość */
  --window-min-w: 420px; /* minWidth okna Tauri */
  --content-max: 680px;  /* maksymalna szerokość kolumny treści */
  --scrollbar-w: 8px;

  /* ruch (sekcja 6) */
  /* 26.11: skrócone o ok. 40% i krzywe z szybkim startem - poprzednie „pływały” */
  --t-press: 60ms;
  --t-color: 90ms;
  --t-switch: 110ms;
  --t-theme: 120ms;
  --t-out: 100ms;
  --t-in: 140ms;
  --t-slide: 140ms;
  --t-morph: 160ms;
  --t-gauge: 260ms;
  --ease-enter: cubic-bezier(0.25, 1, 0.5, 1);    /* wejście: od razu rusza, krótki dojazd */
  --ease-exit: cubic-bezier(0.5, 0, 0.75, 0);     /* wyjście: przyspiesza i znika */
  --ease-smooth: cubic-bezier(0.25, 1, 0.5, 1);   /* zmiana na miejscu: jak wejście */

  /* rzadkość odznak – wypełnienie (oba motywy) */
  --rarity-pospolite: #9aa5b1;
  --rarity-rzadkie: #4aa3ff;
  --rarity-epickie: #b26bff;
  --rarity-heroiczne: #ff5470;
  --rarity-legendarne: #ffa133;
  --on-rarity: #15171d;  /* znak/liczba na odznace: 5,5–8,9:1 na każdym kolorze */

  /* popup nad grą – ciemny w każdym motywie (ma być czytelny nad każdą grą) */
  --popup-bg: #15171d;
  --popup-text: #f2f3f5;     /* 16,1:1 */
  --popup-text-2: #a9afbb;   /* 8,1:1 */
  --popup-line: rgb(255 255 255 / 0.10);
}

html { font-size: calc(100% * var(--text-scale)); }
```

### 2.2 Motyw ciemny

Wartości 1:1 z `theme.DARK`; nowe tokeny pochodne opisane w komentarzach.

```css
:root[data-theme="dark"] {
  color-scheme: dark;
  --bg: #151514;
  --bg-window: rgb(21 21 20 / 0.82);   /* nad materiałem Mica (backdrop_alpha 0.82) */
  --surface: #1e1e1c;
  --surface-2: #2a2a27;                /* najechanie, tło ikony kategorii */
  --surface-3: #33322e;                /* wciśnięte (= line) */
  --text: #ecebe6;                     /* 15,3:1 do bg */
  --text-2: #a5a29a;                   /* 7,2:1 do bg */
  --line: #33322e;                     /* tylko ozdoby i podziały, nigdy ramka kontrolki */
  --control: #7a786f;                  /* ramka kontrolki, 4,1:1 do bg */
  --danger: #f08a84;
  --ink: #ecebe6;                      /* „kontrast”: wypełnienie neutralne (przełącznik ON, toast) */
  --on-ink: #151514;
  --focus: #ecebe6;                    /* obwódka fokusu – neutralna */
  --sel-bg: #2a2a27;                   /* zaznaczony segment / kafelek trybu */
  --sel-border: #7a786f;
  --shadow-pop: 0 8px 24px rgb(0 0 0 / 0.40);

  /* akcent i bieg – domyślnie Mięta i bursztyn; profil podmienia --accent-d / --run-d */
  --accent: var(--accent-d, #8fc2b0);
  --on-accent: #0f1f1a;
  --run: var(--run-d, #e6b46e);

  /* tekst w kolorze rzadkości (napis „EPICKIE”) – na ciemnym = wypełnienie */
  --rarity-pospolite-text: #9aa5b1;
  --rarity-rzadkie-text: #4aa3ff;
  --rarity-epickie-text: #b26bff;
  --rarity-heroiczne-text: #ff5470;
  --rarity-legendarne-text: #ffa133;
}
```

### 2.3 Motyw jasny

```css
:root[data-theme="light"] {
  color-scheme: light;
  --bg: #fbfaf7;
  --bg-window: rgb(251 250 247 / 0.86);
  --surface: #ffffff;
  --surface-2: #f0eee8;
  --surface-3: #e4e1d8;
  --text: #1b1b19;                     /* 16,5:1 */
  --text-2: #5c5951;                   /* 6,7:1 */
  --line: #e4e1d8;
  --control: #8a877e;                  /* 3,4:1 */
  --danger: #a4282a;
  --ink: #1b1b19;
  --on-ink: #fbfaf7;
  --focus: #1b1b19;
  --sel-bg: #ffffff;                   /* na torze --bg: odróżnia go ramka --sel-border */
  --sel-border: #8a877e;
  --shadow-pop: 0 8px 24px rgb(0 0 0 / 0.14);

  --accent: var(--accent-l, #2f5d50);
  --on-accent: #ffffff;
  --run: var(--run-l, #8a5410);

  /* wypełnienia rzadkości mają na jasnym tle 1,9–3,1:1 – napisy dostają ciemniejsze odcienie
     (te same tony, >= 4,6:1 do #fbfaf7 i #ffffff) */
  --rarity-pospolite-text: #657383;
  --rarity-rzadkie-text: #006ee0;
  --rarity-epickie-text: #9738ff;
  --rarity-heroiczne-text: #e50026;
  --rarity-legendarne-text: #ad5d00;
}
```

Bez Mica (Windows 10 albo efekt niedostępny) klasa `no-mica` na `<html>`:
`--bg-window: var(--bg)`. Motyw „Jak Windows” rozwiązuje kod (`matchMedia("(prefers-color-scheme: dark)")`
z nasłuchem zmian) i ustawia `data-theme`; ten sam motyw idzie do paska tytułu okna
(`getCurrentWindow().setTheme()`), jak `apply_backdrop` w 0.5.x.

### 2.4 Akcenty profili (paleta 20 kolorów)

Dane z `bieznia/paleta.py`: każdy kolor ma odcień pod ciemny motyw (`d`, też „próbka” do
awatarów, palety i wykresów) i pod jasny (`l`). Tekst na akcencie: `#0f1f1a` w ciemnym,
`#ffffff` w jasnym. Profile w odcieniach 10–70° (Koral, Pomarańcz, Morela, Banan) zlewałyby się
z bursztynem biegu – bieg dostaje wtedy Malinę.

| Nazwa | hue | ciemny (`--accent-d`, próbka) | jasny (`--accent-l`) | bieg → Malina |
|---|---|---|---|---|
| Czerwień | 0 | `#fa3838` | `#e70606` | |
| Koral | 10 | `#f34523` | `#dc2e0c` | tak |
| Pomarańcz | 25 | `#f96b06` | `#bf5305` | tak |
| Morela | 32 | `#df8620` | `#a56318` | tak |
| Banan | 52 | `#f9d806` | `#847303` | tak |
| Limonka | 80 | `#a4ec13` | `#587f0a` | |
| Zieleń | 118 | `#26df20` | `#178613` | |
| Szmaragd | 145 | `#19e66f` | `#0f8540` | |
| **Mięta** (domyślny) | 158 | `#8fc2b0` | `#2f5d50` | |
| Morski | 174 | `#13ecd6` | `#0b8276` | |
| Niebo | 196 | `#0db5f2` | `#097ba5` | |
| Błękit | 212 | `#2384f3` | `#0c6fe1` | |
| Chaber | 226 | `#5c7ff2` | `#1346ec` | |
| Indygo | 242 | `#7c78f0` | `#2019e6` | |
| Fiolet | 258 | `#956df3` | `#5413ec` | |
| Lawenda | 270 | `#a16dd5` | `#7f39c6` | |
| Orchidea | 292 | `#cf46e5` | `#c01fd8` | |
| Fuksja | 312 | `#f312c6` | `#d30bab` | |
| Róż | 330 | `#f42f91` | `#df0c75` | |
| Malina | 345 | `#f0426e` | `#e21246` | |

Malina jako bieg: `--run-d: #f0426e`, `--run-l: #e21246`.

Paleta żyje w jednym pliku danych (`src/lib/styl/paleta.ts`, ta sama lista co w Rust, jeśli
rdzeń jej potrzebuje do koloru z ikony). Zastosowanie koloru profilu:

```ts
export function ustawAkcent(nazwa: string) {
  const k = kolor(nazwa);               // nieznana nazwa -> Mięta
  const s = document.documentElement.style;
  if (k.name === "Mięta") {
    for (const v of ["--accent-d", "--accent-l", "--run-d", "--run-l"]) s.removeProperty(v);
    return;
  }
  s.setProperty("--accent-d", k.dark);
  s.setProperty("--accent-l", k.light);
  if (k.hue >= 10 && k.hue <= 70) {     // żółcie i pomarańcze: bieg w Malinie
    s.setProperty("--run-d", "#f0426e");
    s.setProperty("--run-l", "#e21246");
  } else {
    s.removeProperty("--run-d");
    s.removeProperty("--run-l");
  }
}
```

Zmiana koloru/motywu to tylko podmiana zmiennych – bez przebudowy widoków (w 0.5.x
`_restyle` przechodził po wszystkich widżetach i przebudowywał Ustawienia).

### 2.5 Tokeny semantyczne (aliasy – używane przez komponenty)

```css
:root[data-theme] {
  --zone-stop: var(--text-2);
  --zone-walk: var(--accent);
  --zone-run: var(--run);
  --status-ok: var(--text);       /* połączono, dane płyną */
  --status-busy: var(--text-2);   /* szukam / łączę (pulsuje) */
  --status-warn: var(--run);      /* ponawiam, czekam na dane */
  --status-error: var(--danger);  /* Bluetooth wyłączony, nie bieżnia */
}
```

### 2.6 Testy tokenów

Progi z `tests/test_theme.py` przenosimy do testu (vitest), który czyta `tokeny.css`:
`--text`/`--bg` >= 7:1, `--text-2`/`--bg` i `--text-2`/`--surface-2` >= 4,5:1,
`--control`/`--bg` i `--control`/`--surface` >= 3:1, każdy akcent palety do tła motywu i do
`--on-accent` >= 4,5:1 (jak w `paleta.py`), napisy rzadkości do tła >= 4,5:1,
`--on-rarity` do każdego wypełnienia rzadkości >= 4,5:1.

---

## 3. Akcent: jeden element na widok (uwaga a)

W 0.5.x akcent był na: wypełnionych kafelkach trybu, podświetleniu każdego segmentowanego,
każdym włączonym przełączniku, ramce wybranego wiersza, ikonach 7 kategorii Ustawień, ramce
otwartej kategorii, linkach, „Zapisano”, wierszu „Dodaj…”, pasku i podpisie poziomu,
gradiencie karty poziomu, szyn szlaku, kropce stanu, obwódce fokusu. To za dużo.

**Reguła:** na ekranie jest najwyżej **jeden** element w kolorze akcentu – ten, który mówi
najwięcej o stanie albo jest główną akcją. Może nie być żadnego. Wszystkie stany wyboru
(zaznaczony segment, wybrany tryb, włączony przełącznik, wybrany wiersz, zaznaczone pole)
są **neutralne**: kolor tekstu (`--ink`), ramka `--sel-border`, tło `--sel-bg`, pogrubienie.

Nie liczą się do limitu „kolory danych”: kolory stref na wskaźniku poza strefą chodu, kolory
profili na wykresie i awatarach, kolory rzadkości na odznakach (tylko odznaka i jej napis –
nigdy tło karty).

| Ekran | Jedyny element z akcentem | Co traci akcent |
|---|---|---|
| Główny | łuk wskaźnika w strefie chodu (`--zone-walk`); gdy bieżnia rozłączona i wskaźnik szary – przycisk „Szukaj bieżni” (`primary`) | kafelki trybu, przełącznik „Sterowanie w grze”, podpis i pasek poziomu, gradient karty poziomu, kropka stanu |
| Ustawienia | brak (paleta pokazuje próbki kolorów – to dane) | ikony kategorii, ramka otwartej kategorii, przełączniki, segmenty, wiersze profili/osób, „Dodaj osobę”, linki w „O aplikacji”, „Zgłoś problem…” (staje się zwykłym przyciskiem) |
| Konto online | jedna główna akcja zakładki: „Akceptuję” (gdy jest karta regulaminu), inaczej „Zaproś” w Znajomych; w Rankingu brak | własny wiersz w rankingu (wyróżnienie tłem), link „ja”, kod zaproszenia |
| Osiągnięcia | brak (odznaki niosą kolory rzadkości) | przycisk „Twój szlak” |
| Szlak | znacznik „TU JESTEŚ” (kropka na szynie + podpis) | przebyte odcinki szyny (teraz `--text`), gradient i pasek karty |
| Historia | brak (słupki w kolorach profili) | segmenty zakresu |
| Zgłoś problem | „Wyślij zgłoszenie” | – |
| Kreator osoby, profil gry | przycisk przejścia dalej/zapisu | wybór ikony (teraz ramka `--ink`) |
| Popup nad grą | kolor rzadkości odznaki i kickera | gradient, pasek z lewej |
| Przycisk na ekranie (bańka) | wypełnienie, gdy sterowanie włączone (chód = akcent, bieg = `--run`) | – |

Obwódka fokusu jest neutralna (`--focus` = kolor tekstu), więc nigdy nie dubluje akcentu.

---

## 4. Typografia

### 4.1 Kroje

- **Segoe UI Variable Text** (Windows 11; fallback Segoe UI na Windows 10) – cały interfejs.
- **Bahnschrift** (w Windows od 10 1709) – tożsamość „gamingowa”: wszystkie zmieniające się
  liczby (prędkość, kroki, km, XP, poziom, miejsce w rankingu, km szlaku, wartości kafelków),
  liczby na odznakach, podpisy HUD wersalikami (POZIOM 12, EPICKIE, TU JESTEŚ, OSIĄGNIĘCIE ·
  EPICKIE), tytuł popupu. Nigdy treść zdań.
- **Segoe Fluent Icons** (fallback Segoe MDL2 Assets) – ikony; czcionka systemowa, nie
  dołączamy jej do instalatora.
- **Cascadia Mono / Consolas** – tylko Dziennik.

Wszędzie, gdzie liczba się zmienia: `font-variant-numeric: tabular-nums;` (odpowiednik
`tabular()` z 0.5.x) i rezerwa szerokości w `ch`, żeby nowa cyfra nie przesuwała sąsiadów.

### 4.2 Skala

| Rola | Krój | Rozmiar | Waga | Interlinia | Uwagi |
|---|---|---|---|---|---|
| Tytuł strony (h1) | UI | `--fs-xl` 20 | 600 | 1.25 | jedna linia, wielokropek |
| Podpis tytułu (np. pseudonim) | UI | `--fs-sm` 13 | 400, `--text-2` | 1.25 | jedna linia, wielokropek |
| Tytuł sekcji w karcie | UI | `--fs-base` 16 | 700 | 1.25 | bez paska z lewej |
| Nazwa rangi / tytuł karty poziomu | UI | `--fs-lg` 18 | 700 | 1.25 | |
| Treść, przycisk, wiersz | UI | `--fs-base` 16 | 400 / 600 (przycisk) | 1.4 | |
| Tekst wyciszony (`muted`), segment | UI | `--fs-md` 14 | 400 / 600 (segment) | 1.4 | |
| Podpowiedź, druga linia wiersza | UI | `--fs-sm` 13 | 400, `--text-2` | 1.4 | w 0.5.x 12 px – w WebView 13 px czyta się lepiej |
| Etykieta kafelka, oś wykresu | UI | `--fs-xs` 12 / `--fs-cap` 11 | 400 | 1.3 | |
| Podpis HUD wersalikami | Bahnschrift | 11–13 | 700 | `--lh-caps` | `text-transform: uppercase; letter-spacing: 0.12em` |
| Wartość kafelka | Bahnschrift | `--fs-num` 22 (płynnie 18–24) | 700 | 1.1 | tabular, jedna linia |
| Suma km na szlaku | Bahnschrift | `--fs-num-lg` 24 | 700 | 1.1 | |
| Prędkość na wskaźniku | Bahnschrift | `--fs-gauge` 44 | 700 | 1 | tabular |
| Tytuł popupu nad grą | Bahnschrift | 17 | 700 | 1.2 | |

Polskie znaki w wersalikach Bahnschrift: kreski nad Ó/Ś/Ż wychodzą ponad wysokość wiersza
(w Qt trzeba było dodawać margines – `caps()`/`ACCENTS_ABOVE`). W CSS: `line-height: var(--lh-caps)`
i przy `overflow: hidden` (wielokropek) `padding-block-start: 0.12em`.

Liczby po polsku: przecinek dziesiętny (`2,5`), tysiące rozdzielone **niełamliwą wąską spacją**
U+202F (`12 345`), jednostka po niełamliwej spacji U+00A0 (`2,5 km/h`) – liczba nigdy nie
łamie się w pół.

---

## 5. Odstępy, wymiary, promienie

- Margines strony `--sp-4` (16); odstęp między kartami `--sp-3` (12); wewnątrz karty
  `--card-pad` (12/14/14), odstęp między elementami karty 8.
- Wysokość kontrolki `--control-h` 44 (przycisk, pole, przełącznik, segmentowany).
- Wysokości wierszy: wiersz wyboru (osoba/profil) 62, urządzenie 60, „Dodaj…” 50, nagłówek
  kategorii 64, slot stref/Treningu 56, kafelek statystyki 64, wiersz rankingu min. 52,
  wiersz szlaku min. 52.
- Promienie: karta, przycisk, pole, kafelek 10; segment 7; lista rozwijana 12; klawisz 6;
  pasek XP i słupki 3. Pełne zaokrąglenie tylko: tor przełącznika, awatar, kropka stanu,
  próbka koloru. Żadnych „pastylek” na etykietach i wartościach.
- Cienie: tylko warstwy nad treścią (lista rozwijana, toast) – `--shadow-pop`. Karty płaskie,
  oddzielone ramką `--line`.

---

## 6. Ruch (uwaga g)

### 6.1 Czasy i krzywe

| Zdarzenie | Czas | Krzywa | Właściwości |
|---|---|---|---|
| Najechanie, zmiana koloru | `--t-color` 90 | liniowa | `background-color`, `color`, `border-color` (tylko kolor – nie układ) |
| Wciśnięcie | `--t-press` 60 | liniowa | kolor tła |
| Gałka przełącznika | `--t-switch` 110 | `--ease-enter` | `transform: translateX` |
| Zmiana motywu | 0 | – | natychmiast, cały ekran naraz (na klatkę `bez-przejsc`); przenikanie zrzutów szło przez szarą klatkę i zamrażało obraz |
| Wejście ekranu | 100 | `--ease-enter` | `opacity` 0,5→1, `transform: translateX(4px→0)` (wstecz: -4px) |
| Wyjście ekranu | 0 | – | poprzedni ekran znika od razu (przenikanie dwóch ekranów wyglądało jak lag) |
| Podświetlenie segmentowanego | `--t-slide` 140 | `--ease-smooth` | `transform: translate` wskaźnika |
| Rozwinięcie kategorii / zmiana karty trybu | `--t-morph` 160 | `--ease-smooth` | treść: `opacity` + `translateY(-4px→0)`; elementy pod spodem: FLIP `translateY` |
| Strzałka kategorii | `--t-switch` 110 | `--ease-smooth` | `transform: rotate(0→90deg)` |
| Wskaźnik prędkości | `--t-gauge` 260 | `--ease-enter` | rysowanie w SVG/canvas (wyjątek, 6.2) |
| Pasek XP po zmianie | `--t-gauge` 260 | `--ease-enter` | `transform: scaleX` |
| Popup nad grą | wejście 140 / wyjście 100, widoczny 6000 | enter / exit | `opacity`, `translateY(12px→0)` |
| Toast | wejście 140 / wyjście 100, widoczny 4000 | enter / exit | `opacity`, `translateY(8px→0)` |
| Potwierdzenie (akcja niebezpieczna) | 4000 | – | napis „Na pewno?” na przycisku, bez animacji |
| Kropka „szukam” | 1000, pętla | `--ease-smooth` | `opacity` 1↔0.35 |

`@media (prefers-reduced-motion: reduce)` – wszystkie czasy 0 ms, pętle zatrzymane, zmiany
od razu (WebView2 bierze to z Windows: Ułatwienia dostępu > Efekty animacji – tak jak
`system_reduces_motion()` w 0.5.x).

### 6.2 Co wolno animować

- W CSS (`transition`, `@keyframes`, Web Animations) **wyłącznie `transform` i `opacity`**.
  Zakazane animacje: `height`, `width`, `top/left`, `margin`, `padding`, `grid-template-rows`,
  `max-height`, `box-shadow`, `filter`, `backdrop-filter`.
- Kolory przy najechaniu (120 ms) są dozwolone – zmieniają tylko kolor, bez układu.
- Wyjątek: rysowanie wewnątrz elementu o **stałym rozmiarze** (wskaźnik prędkości w SVG albo
  canvas, interpolacja w `requestAnimationFrame`) – nie wywołuje przeliczenia układu.
- Zmiana wysokości (rozwinięcie kategorii, inna karta trybu) dzieje się od razu w układzie;
  płynność daje technika **FLIP**: zmierz położenia elementów pod zmianą, zmień DOM, ustaw
  `transform: translateY(stare - nowe)`, odtwórz do 0. To samo, co robił `Cover.morph` na
  zrzutach, ale bez zrzutów.

### 6.3 Stałe ekrany sterowane stanem

- Każdy ekran (Główny, Ustawienia, Historia, Osiągnięcia, Szlak, Konto online, …) to komponent
  zamontowany **raz**; przełączanie stron zmienia tylko, który jest widoczny (wszystkie w tej
  samej komórce siatki `grid-area: 1 / 1`, nieaktywne z `inert` i `visibility: hidden` po
  zakończeniu przejścia). Bez `replace_page`, bez „odcisków stanu” do decydowania o
  przebudowie – Svelte aktualizuje tylko to, co się zmieniło.
- Każda strona ma własny kontener przewijania – powrót na stronę zachowuje miejsce.
- Dane na żywo (4×/s: prędkość, kroki, stan połączenia) zmieniają tylko tekst i `transform`;
  pola liczbowe mają zarezerwowaną szerokość, więc nie ma przeliczeń układu wokół nich.
- Listy (`{#each … (id)}`) z kluczami; długie listy (osiągnięcia, szlak) z
  `content-visibility: auto; contain-intrinsic-size: auto 72px;`.
- Karty z `contain: layout paint;`. Bez `backdrop-filter` (Mica robi system).
- Zmiana motywu: podmiana `data-theme` w `document.startViewTransition()` (160 ms przenikania
  zrzutów – `opacity`), przy ograniczonym ruchu bez przejścia.

---

## 7. Szerokość okna (uwagi b, h)

### 7.1 Liczby

- Okno Tauri: `minWidth: 420`, `minHeight: 640`, domyślnie 740 × 890 (jak `DEFAULT_W/H`).
- Kolumna treści: `width: min(100%, var(--content-max))` = **maks. 680 px**, wyśrodkowana;
  pasek tytułu strony w tej samej kolumnie. Tło (Mica) na całe okno. Szersze okno nie
  rozciąga kart, wierszy ani wykresu.
- Kontener przewijania z `scrollbar-gutter: stable` (miejsce na pasek 8 px zawsze
  zarezerwowane – treść nie skacze w bok, jak w `CalmScrollBar`).
- Najwęższa treść: 420 − 2 × 16 − 8 = **380 px**; z dużym tekstem (×1,15) pismo zajmuje tyle,
  co przy ~330 px. **Każdy komponent musi działać przy 320 px szerokości swojego kontenera**
  (wnętrze karty przy minimalnym oknie: 380 − 28 = 352 px).

### 7.2 Zasady zawijania

1. Każde dziecko rzędu flex ma `min-width: 0`; tekst w wierszu albo zawija się
   (`overflow-wrap: anywhere` dla nazw z danych: pseudonimy, nazwy profili, gier, urządzeń),
   albo jest w jednej linii z wielokropkiem i pełną treścią w `title`.
2. Rzędy przycisków: `display: flex; flex-wrap: wrap; gap: 8px;` dzieci `flex: 1 1 160px`
   (odpowiednik `ResponsiveRow` – zawijanie zamiast wychodzenia poza okno).
3. Siatki: `repeat(auto-fit, minmax(X, 1fr))` z `X <= 150px` albo zapytania o kontener.
4. Komponenty reagują na **szerokość kontenera** (`container-type: inline-size` +
   `@container`), nie okna – ta sama karta działa w kolumnie 680 i 352 px.
5. Stałe szerokości tylko dla elementów mniejszych niż 120 px (ikony, odznaki, wartość
   steppera 96, kolumna km szlaku 64, miejsce w rankingu 32). Żadnych `width: 220px` na
   segmentowanym (w 0.5.x „Liczba skoków”) ani `200px` na wartości nagłówka.
6. Przycisk, którego napis się zmienia, ma szerokość najdłuższego napisu dzięki siatce: wszystkie
   warianty w jednej komórce (`grid-area: 1 / 1`), widoczny tylko bieżący, reszta
   `visibility: hidden` (odpowiednik `fix_width`). Szerokość nie skacze i nie zależy od JS.
7. `overflow-x: clip` na kolumnie to tylko siatka bezpieczeństwa, nigdy poprawka – test (11.2)
   wykrywa wystawanie także wtedy, gdy jest przycięte.
8. Duży tekst (×1,15) testujemy przy minimalnej szerokości jak zwykły.

### 7.3 Progi komponentów (szerokość kontenera)

| Komponent | Szeroki | Wąski |
|---|---|---|
| Kafelki statystyk (4) | 4 w rzędzie przy >= 4 × 110 + 3 × 8 | 2 × 2 (wysokość kafelka bez zmian) |
| Kafelki trybu (4) | 4 w rzędzie przy >= 440 | 2 × 2 |
| Pasek Treningu | `repeat(4, minmax(52px, 1fr)) minmax(72px, auto)` – mieści się w 320 | to samo |
| Żetony stref (3–5) | równe kolumny | od 5 żetonów: drugi wiersz napisu ukryty jednostką (jak w 0.5.x) |
| Segmentowany (n opcji) | n kolumn, gdy każda >= najdłuższy napis + 20 | 4 opcje: 2 × 2; 3 opcje: 3 × 1 pionowo; 2 opcje zawsze w rzędzie |
| Nagłówek kategorii | ikona + tytuł/opis + wartość + strzałka | < 440: wartość ukryta |
| Wiersz ustawienia (tekst + kontrolka) | obok siebie | kontrolka szersza niż 50% (segmentowany) – pod tekstem w pełnej szerokości |
| Wiersz rankingu | 4 kolumny | to samo (statystyki w dwóch liniach) |
| Pasek rzadkości (5) | 5 kolumn | `repeat(auto-fit, minmax(60px, 1fr))` – zawija na 3 + 2 |
| Paleta 20 kolorów | `repeat(auto-fill, 32px)`, przerwa 6 | zawija na tyle rzędów, ile trzeba |
| Siatka ikon profilu | `repeat(auto-fill, 48px)` | zawija |
| Legenda wykresu | flex-wrap | flex-wrap |

---

## 8. Specyfikacja komponentów

Każdy komponent: anatomia, wymiary, stany, tokeny, zachowanie w wąskim kontenerze, dostępność.

### 8.1 Przycisk (`Przycisk.svelte`)

- Wysokość 44, `padding: 0 16px`, promień 10, `--fs-base`/600, napis w jednej linii
  (`white-space: nowrap`, wielokropek przy braku miejsca, pełny w `title`), min. szerokość 44.
- Warianty:
  - **zwykły** (domyślny): tło `--surface`, ramka 1 px `--control` (w 0.5.x 2 px – 1 px
    spełnia 3:1 i jest lżejszy), tekst `--text`; najechanie `--surface-2`; wciśnięcie
    `--surface-3`.
  - **główny** (`primary`): tło i ramka `--accent`, tekst `--on-accent`; najechanie
    `color-mix(in srgb, var(--accent) 85%, var(--text))`. **Najwyżej jeden na ekranie.**
  - **ikona**: 44 × 44, przezroczysty, bez ramki, glif `--font-icon` 16 px `--text`;
    najechanie tło `--surface-2` i ramka `--control`. Zawsze `aria-label` + `title`.
  - **link**: bez tła i ramki, `--text`, podkreślenie przy najechaniu i fokusie
    (w 0.5.x akcent – teraz neutralny).
  - **stop**: tekst i ramka `--danger`, 700; najechanie tło `--danger`, tekst `--bg`.
  - **potwierdz** (akcja niebezpieczna, dawniej `HoldButton`): jak stop. Pierwsze kliknięcie
    zamienia napis na „Na pewno? Kliknij jeszcze raz” (tło `--danger`, tekst `--bg`), drugie
    w ciągu 4 s wykonuje akcję; po 4 s albo utracie fokusu wraca. Szerokość = dłuższy z napisów.
    26.11: zastąpiło przytrzymanie 1 s, którego nic nie zapowiadało („usuwanie nie ma
    potwierdzenia”).
- Wyłączony: tekst `--text-2`, ramka `--line`, `cursor: default`.
- Fokus: `outline: 2px solid var(--focus); outline-offset: 2px;` tylko `:focus-visible`.

### 8.2 Przełącznik (`Przelacznik.svelte`)

- Pole trafienia 56 × 44; tor 52 × 30 (promień pełny), gałka 20 px na `x = 7 → 29`
  (`translateX(22px)`, 160 ms `--ease-enter`).
- Wyłączony: tor `--bg`, ramka 2 px `--control`, gałka `--text-2`.
- Włączony: tor i ramka `--ink`, gałka `--on-ink` (**neutralnie**, bez akcentu).
- `role="switch"`, `aria-checked`; nazwa = tytuł wiersza ustawienia. Kliknięcie w tekst
  wiersza też przełącza (cały wiersz to `<label>`).
- Wiersz ustawienia: `grid-template-columns: minmax(0, 1fr) auto; gap: 12px;` – tytuł 16,
  podpowiedź 13 `--text-2` (może się zawijać; wiersz rośnie, przełącznik wyrównany do góry
  pierwszej linii).

### 8.3 Segmentowany (`Segmenty.svelte`)

- Tor: wysokość 44, tło `--bg`, ramka 1 px `--control`, promień 10, `padding: 3px`;
  `display: grid; grid-template-columns: repeat(n, minmax(0, 1fr)); gap: 2px;` – segmenty
  **równej szerokości**.
- Segment: wysokość 36, promień 7, `--fs-md`/600, `--text-2`, w jednej linii; najechanie
  (niezaznaczony) `--text`.
- Wskaźnik zaznaczenia: jeden element `position: absolute` o rozmiarze jednej komórki, tło
  `--sel-bg`, ramka 1 px `--sel-border`, promień 7; zaznaczony napis `--text`. Przesunięcie
  `transform: translate(calc(i * (100% + 2px)), calc(r * (100% + 2px)))` w 260 ms
  `--ease-smooth`. **Bez akcentu** (w 0.5.x podświetlenie w kolorze akcentu).
- Wąski kontener (7.3): przy 4 opcjach siatka 2 × 2, przy 3 – kolumna; komórki dalej równe, więc
  wskaźnik przesuwa się po obu osiach. Wysokość nie zależy od wybranej wartości.
- Oznaczenie „aktywny tryb” w segmentach trybów (Ustawienia > Sterowanie): kropka 6 px
  `--text` przed nazwą + `title` „aktywny tryb” (zamiast „Klawisze ✓”).
- `role="radiogroup"`, segment `role="radio"` + `aria-checked`; strzałki zmieniają wybór,
  Tab wchodzi tylko na zaznaczony segment.

### 8.4 Karta (`Karta.svelte`)

- Tło `--surface`, ramka 1 px `--line`, promień 10, `--card-pad`, odstęp między dziećmi 8.
- Tytuł sekcji (opcjonalny): 16/700 `--text`, jedna linia z wielokropkiem; dynamiczna część
  (np. „Damian” w „Profile gier · Damian”) jako podpis 13 `--text-2` w drugiej linii albo z
  prawej (`margin-inline-start: auto`, ustępuje miejsca – wielokropek). **Bez paska z lewej**
  (`section` w 0.5.x miał `border-left: 3px solid accent`).
- Karta klikalna (np. karta poziomu, „mój profil”): cała karta to `<button>`/`<a>`;
  najechanie tło `--surface-2` i ramka `--control`; strzałka `E76C` 12 px `--text-2` z prawej.
- Bez gradientów i bez kolorowych teł (`Panel` z 0.5.x znika – patrz 10).

### 8.5 Nagłówek akordeonu – kategoria Ustawień (`Kategoria.svelte`, uwagi c, e)

Anatomia (`display: grid; grid-template-columns: 40px minmax(0, 1fr) auto 16px; gap: 14px;
align-items: center;`, `padding: 0 14px`, **stała wysokość 64**):

1. Ikona: pole 40 × 40, tło `--surface-2`, promień 10, glif 20 px `--text-2`
   (w 0.5.x akcent – teraz neutralny). Glify: Konto i osoby `E77B`, Gry i sterowanie `E7FC`,
   Bieżnia `E805`, Poziomy i powiadomienia `E734`, Wygląd i okno `E771`, Dane i kopia `E74E`,
   Pomoc i diagnostyka `E9D9`.
2. Tekst: tytuł 16/600 **w jednej linii** z wielokropkiem; pod nim opis 13 `--text-2`
   **w jednej linii** z wielokropkiem; pełny opis w `title`. Nic się nie łamie, więc tytuł
   nigdy nie nachodzi na opis, a wysokość nagłówka jest stała.
3. Wartość (np. „Damian · konto Damianejro”, „WalkingPad X21”): 13 `--text-2`, jedna linia,
   `max-width: 40%`, `flex-shrink` większy niż tekstu – **ustępuje miejsca pierwsza**
   (skraca się wielokropkiem przed tytułem); w kontenerze < 440 px ukryta. Wartość
   aktualizuje się na żywo, bez zmiany wysokości.
4. Strzałka `E76C` 12 px `--text-2`; otwarta: `transform: rotate(90deg)` (160 ms) – zamiast
   podmiany glifu.

Stany: najechanie tło `--surface-2`; otwarta: ramka 1 px `--control` (neutralnie, bez akcentu);
fokus z klawiatury: obwódka `--focus`. `<button aria-expanded aria-controls>`.

Zachowanie (uwaga e – kliknięty nagłówek nie ucieka spod kursora):

- **Kategorie otwierają się i zamykają niezależnie** (można mieć kilka otwartych). Kliknięcie
  zmienia tylko to, co jest **pod** klikniętym nagłówkiem – sam nagłówek i wszystko nad nim
  stoją w miejscu. W 0.5.x otwarcie kategorii B zwijało A nad nią, więc B uciekała w górę,
  a potem strona przewijała się automatycznie.
- **Bez automatycznego przewijania** po rozwinięciu.
- Rozwinięcie: treść pojawia się od razu w układzie, animuje `opacity 0→1` +
  `translateY(-4px→0)` (220 ms `--ease-enter`); nagłówki pod nią zjeżdżają FLIP-em
  (`translateY`, 220 ms). Zwinięcie: treść znika od razu, nagłówki pod nią podjeżdżają FLIP-em
  (180 ms `--ease-exit`).
- Otwarte kategorie zapamiętane w stanie aplikacji (po powrocie na Ustawienia te same).
- Gdyby jednak miała zostać zasada „najwyżej jedna otwarta”: przy otwieraniu B zwijamy A tylko,
  jeśli A jest **pod** B; jeśli A jest nad B – zwijamy A i w tej samej klatce korygujemy
  `scrollTop` o różnicę położenia B (pomiar `getBoundingClientRect` przed i po); gdy korekta
  nie jest możliwa (za mało przewinięcia nad A) – A zostaje otwarta.

### 8.6 Pole tekstowe (`Pole.svelte`)

- Wysokość 44, tło `--bg`, ramka 1 px `--control`, promień 10, `padding: 0 12px`, 16 px.
  Podpowiedź (placeholder) `--text-2`.
- Fokus: ramka `--text` + `box-shadow: 0 0 0 1px var(--text)` (grubsza ramka bez zmiany
  rozmiaru).
- Błąd: ramka `--danger`, komunikat 13 `--danger` **tuż pod polem** (`aria-describedby`,
  `aria-invalid`) – jak przy krótkim zgłoszeniu w 0.5.2.
- Hasło: przycisk ikony „pokaż” (`E7B3`) wewnątrz pola z prawej, 44 × 44.
- Pole wielowierszowe: min. wysokość 120, `padding: 8px`, 15 px (zwykłe pismo); Dziennik:
  `--font-mono` 13 px.
- Wartość steppera (− 2,5 km/h +): szer. 96, Bahnschrift 16/700 tabular, wyśrodkowana,
  przezroczysta ramka; najechanie ramka `--line`; fokus ramka `--text`, tło `--surface-2`;
  strzałki ±0,1, PageUp/PageDown ±0,5, Esc przywraca, przytrzymanie −/+ przyspiesza
  (po 6 powtórzeniach krok 0,5 – logika z `Stepper`).

### 8.7 Kafelek statystyki (`Kafelek.svelte`)

- Karta (tło `--surface`, ramka `--line`, promień 10), `padding: 10px 12px`, **stała wysokość
  64**, `container-type: inline-size`.
- Tytuł: 12 `--text-2` („Kroki”, „Kroki/min”, „Dystans”, „Czas ruchu”, „Średnio”).
- Wartość: Bahnschrift 700 tabular, `font-size: clamp(1.125rem, 9cqi, 1.5rem)`, jedna linia,
  wielokropek jako ostateczność (pełna w `title`) – „12 h 05 min” nigdy nie wychodzi poza
  kafelek. Brak danych: „–” (półpauza).
- Siatka 4 / 2 × 2 (7.3). `aria-label` = tytuł, wartość czytana jako treść.

### 8.8 Odznaka rzadkości – heksagon (`Odznaka.svelte`)

- SVG `viewBox="-1 -1 2 2"`, rozmiary: 34 (ranking, pasek rzadkości, znajomi), 40, 44 (szlak),
  58 (wiersz osiągnięcia), 62 (karta poziomu), 64 (popup), 70 (profil osoby).
- Heksagon „ostrzem w górę” (wierzchołki co 60° od −90°), promień 0,80 pola.
- **Zdobyta**: wypełnienie **płaskie** `--rarity-*`; obrys 0,05 jasnej wersji koloru
  (`color-mix(in srgb, var(--rarity-x) 65%, white)`; w jasnym motywie obrys
  `--rarity-*-text`, żeby odznaka nie zlewała się z białą kartą); wewnętrzny heksagon
  0,82 promienia obrys 1 px `rgb(255 255 255 / 0.28)` (ozdoba z 0.5.x). Znak albo liczba
  w `--on-rarity` (w 0.5.x biały na gradiencie – na płaskim legendarnym biały miał 2:1,
  ciemny ma 8,9:1). **Bez gradientu, bez poświaty, bez cienia pod znakiem.**
- **Zablokowana**: wypełnienie `--surface-2`, obrys `--control`, znak `--text-2` z kryciem 0,6;
  postęp: obrys po obwodzie w `--rarity-*` (grubość max(2 px, 5% rozmiaru), zaokrąglone
  końce) – `pathLength="1"` + `stroke-dasharray: p 1`, start w górnym wierzchołku, zgodnie z
  zegarem.
- Liczba (poziom): Bahnschrift 700, wysokość 0,95 promienia dla 1–2 cyfr, 0,75 dla 3.
- Znaki: proste kształty z `odznaki.glyph()` przeniesione 1:1 na ścieżki SVG w układzie
  −1..1 (stopy, kopula, helm, miecz, dycha, wieniecNN, licznik, luki, mur, lisc, gora, wulkan,
  pudlo, wglab, kostka, biegacz, glob, plomien, pad, portale, dom, ksiezyc, slonce, druzyna,
  fala, portal, brama, drzwi, celownik, cytadela, gwiazda, dysk, pierscien, planeta, wieza,
  drzewo, kolo, czerw, radiacja, korona). Elementy „wycięte” (`QColor(0,0,0,110)`) jako
  `fill-opacity` koloru `--on-rarity`, wulkan z lawą `#ff6a2a` – wyjątek w danych.
- Poziom → rzadkość (`rank_rarity`): < 10 pospolite, < 25 rzadkie, < 50 epickie, < 80
  heroiczne, od 80 legendarne.
- Napis rzadkości obok odznaki: Bahnschrift wersaliki 11–13 / 700, `--rarity-*-text`.
- `role="img"`, `aria-label` np. „Epickie, zdobyte” / „Rzadkie, 40% postępu” /
  „Poziom 12, ranga …”.

### 8.9 Pasek XP / postępu (`PasekXp.svelte`)

- Wysokość 12 (karta poziomu, profil) albo 8 (wiersz osiągnięcia, popup); tor `--bg`, ramka
  1 px `--control`, promień 3.
- Wypełnienie płaskie: na karcie poziomu **`--ink`** (neutralnie – akcent zostaje dla
  wskaźnika); w wierszu osiągnięcia i na profilu kolor rzadkości; w popupie awansu kolor
  rzadkości rangi. Rysowane jako element pełnej szerokości z `transform: scaleX(v)`,
  `transform-origin: left` – zmiana wartości 420 ms `--ease-enter`.
- Podziałka: domyślnie 10 odcinków – przerwy 2 px w kolorze toru jako nakładka (SVG albo
  powtarzające się elementy 2 px; nie gradient); w wierszach osiągnięć bez podziałki.
- **Bez połysku, bez świecącego końca, bez gradientu** (w 0.5.x wszystkie trzy).
- Obok paska (wiersz osiągnięcia): podpis Bahnschrift 12 `--text-2` „12,3 / 42 KM”, jedna
  linia, nie kurczy się (pasek się kurczy).
- `role="progressbar"`, `aria-valuemin/max/now`, `aria-valuetext` („1 250 z 2 000 XP”).

### 8.10 Wiersz szlaku (`WierszSzlaku.svelte`)

- `display: grid; grid-template-columns: 64px 26px minmax(0, 1fr); gap: 10px;` min. wysokość
  52, odstęp między wierszami 4.
- Kolumna km: Bahnschrift 14 tabular, do prawej, `--text` (przebyte) albo `--text-2`
  (przed Tobą); format „1 234 km” z niełamliwymi spacjami (64 px mieści „9 999 km”; w 0.5.x
  84 px).
- Szyna (SVG pełnej wysokości wiersza): linia 3 px w górnej i dolnej połowie; przebyta
  `--text` (w 0.5.x akcent), przyszła `--control`. Węzeł: przebyty – heksagon 14 px
  wypełniony kolorem rzadkości; przyszły – heksagon `--surface-2` z obrysem `--control`;
  „tu jesteś” – kropka 10 px `--accent` z pierścieniem 2 px `--accent` w odległości 3 px
  (**bez warstw poświaty**).
- Panel: tło `--surface`, ramka `--line`, promień 10, `padding: 6px 12px`, siatka
  `44px minmax(0, 1fr)`: odznaka 44 + tekst. Tekst: nazwa 15/700 (do 2 linii, potem
  wielokropek), **pod nazwą** napis rzadkości Bahnschrift 11 `--rarity-*-text` (w 0.5.x z
  prawej obok nazwy – wypychał), opis 13 `--text-2` zawijany (kursywą dla przebytych).
- Wiersz „TU JESTEŚ”: panel tło `--surface-2`, bez kolorowego paska; „TU JESTEŚ” Bahnschrift
  12 wersaliki `--accent` (jedyny akcent ekranu), pod nim zdanie „następny przystanek za
  12,5 km · ok. 3 dni przy 5 km dziennie” 13 `--text-2` **zawijane** (w 0.5.x wersaliki w
  jednej linii o stałej wysokości 44 – wystawały z prawej).
- Karta na górze szlaku: suma Bahnschrift 24 „123,4 km” + opis „przebyte łącznie · 12 z 40
  przystanków” 13 `--text-2`; flex-wrap.
- Po otwarciu „Tu jesteś” w 1/3 wysokości widoku (`scrollTop` ustawiony przed pierwszym
  malowaniem – bez przewijania na oczach).

### 8.11 Wiersz rankingu (`WierszRankingu.svelte`)

- `display: grid; grid-template-columns: 32px 34px minmax(0, 1fr) auto; gap: 10px;
  align-items: center;` min. wysokość 52, między wierszami linia 1 px `--line`.
- Miejsce: Bahnschrift 16/700 tabular, do prawej, „1.”; pierwsze trzy bez kolorów i medali.
- Odznaka poziomu 34 (8.8).
- Osoba: pseudonim jako link (16/600 `--text`, jedna linia, wielokropek, `title`), pod nim
  ranga Bahnschrift 11 wersaliki `--rarity-*-text`.
- Statystyki z prawej, **dwie linie**, do prawej: „12 345” Bahnschrift 16 tabular + „kroków”
  13 `--text-2`; druga linia „12,3 km” 13 `--text-2`. W rankingu poziomów: „1 234 km” +
  „łącznie”. Kolumna max. ~100 px – w 0.5.x „12 345 kroków · 12,3 km” w jednej linii
  wypychało wiersz poza okno.
- Własny wiersz: tło `--surface-2`, promień 8, dopisek „(Ty)” 13 `--text-2` po pseudonimie
  (w 0.5.x link w akcencie).
- Wiersz klikalny tylko przez link (profil), cały wiersz nie jest przyciskiem.

### 8.12 Toast – komunikat w oknie (`Toast.svelte`)

- `position: fixed`, wyśrodkowany w kolumnie treści, 20 px od dołu okna; szerokość
  `min(420px, kolumna − 32px)`; tło `--ink`, tekst `--on-ink` 15/600, promień 10,
  `padding: 10px 14px`, `--shadow-pop`, tekst zawijany, wyśrodkowany.
- Widoczny 4000 ms; kliknięcie zamyka; nowy zastępuje stary (bez kolejki).
- Wejście `opacity` + `translateY(8px→0)` 220 ms, wyjście 180 ms.
- `role="status"`, `aria-live="polite"`; treść też w logu (jak `toast()` w 0.5.x).

### 8.13 Popup nad grą (`popup.html` – osobne okno)

- Osobne okno Tauri: przezroczyste, bez ramki, zawsze na wierzchu, bez fokusu
  (`focusable: false`, WS_EX_NOACTIVATE), przepuszcza kliknięcia (`setIgnoreCursorEvents(true)`),
  bez paska zadań. **Jedno okno na cały stos** (380 × 3 × 96 + 2 × 10), ustawione w prawym
  dolnym rogu obszaru roboczego ekranu z oknem na pierwszym planie, margines 16 – zamiast
  osobnego okna na każdy popup.
- Karta popupu 380 × 96: tło `--popup-bg` (nieprzezroczyste), ramka 1 px `--popup-line`,
  promień 8, `padding: 8px 16px 8px 14px`, siatka `64px minmax(0,1fr)`, odstęp 12.
  **Bez gradientu i bez paska z lewej** (w 0.5.x oba).
- Treść: odznaka 64 (8.8; znak albo liczba poziomu); kicker Bahnschrift 11 wersaliki
  `letter-spacing: 0.15em` w kolorze rzadkości (na `--popup-bg` 5,5–8,9:1), np.
  „OSIĄGNIĘCIE · EPICKIE”, „AWANS · POZIOM 12”; tytuł Bahnschrift 17/700 `--popup-text`,
  jedna linia z wielokropkiem; opcjonalny pasek 8 px (postęp do następnego poziomu); linia
  12 `--popup-text-2` („+250 XP · Przejdź 42 km”), jedna linia z wielokropkiem.
- Stos: najwyżej 3 widoczne, najnowszy na dole, starsze wyżej (odstęp 10), reszta w kolejce;
  każdy 6000 ms. Wejście `opacity 0→1` + `translateY(12px→0)` 220 ms, wyjście `opacity` 180 ms,
  pozostałe karty dosuwają się `translateY` 220 ms.
- Więcej niż 4 nowe naraz: najrzadsze + zbiorczy „+N kolejnych” (rzadkie, znak „korona”).
  Pierwsze liczenie: jedno podsumowanie „Z TWOJEJ HISTORII”.
- Dźwięk (opcja): `osiagniecie.wav` / `awans.wav`, nigdy systemowy „ding”.
- Gra w wyłącznym pełnym ekranie może przykryć okno – zdobycz i tak jest na liście.

### 8.14 Pozostałe komponenty tożsamości

**Wskaźnik prędkości** (SVG, stała wysokość 186, min. szerokość 280): łuk 240° od 210°,
promień 78, środek w y = 124. Tor 14 px `--surface-2` (końce płaskie). Pasma stref na
zewnątrz (r + 14, grubość 5), nieaktywne z kryciem 0,32, przerwa 1,5° między pasmami; kolory
`--zone-stop/walk/run` (Trening: poziomy 1–2 chód, 3–4 bieg; Pad: puszczona stop, środkowe
chód, ostatni bieg). Łuk wartości 14 px, końce zaokrąglone, kolor aktywnej strefy. Kreski
progów r+20…r+27, etykiety progów 12 px na r + 40 (aktywna pogrubiona w kolorze strefy).
Środek: Bahnschrift 44/700 tabular „2,5”, pod spodem „km/h taśmy” 13 `--text-2`. Bez
połączenia: liczba `--text-2`, bez łuku wartości. Wartość dochodzi do celu w 420 ms
(`requestAnimationFrame`, wyjątek z 6.2). `role="img"` + opis „2,5 km/h, chodzisz”.

**Podpis stanu pod wskaźnikiem**: 15/700, wyśrodkowany, jedna linia z wielokropkiem (stała
wysokość 22).

**Slot stref / Treningu** – stała wysokość 56 w każdym trybie (uwaga f):
- Żetony stref (Klawisze, Pad, Chodzenie): siatka równych kolumn, każda komórka: tło
  `--surface`, ramka 1 px `--line`, promień 10; dwie linie: nazwa 13/600 i wartość 13
  tabular. Aktywna strefa: kwadrat 8 px w kolorze strefy przed nazwą, nazwa i wartość
  `--text` 700, ramka `--control`; nieaktywne `--text-2`. **Bez wypełniania kolorem**
  (w 0.5.x aktywny żeton wypełniony kolorem strefy). Tylko informacja (bez fokusu i kursora).
- Pasek Treningu: 4 przyciski poziomów (dwie linie: „1” Bahnschrift 16/700 i „2,5 km/h” 13
  tabular) + Stop (8.1 wariant stop, min. 72); aktywny poziom: tło `--sel-bg`, ramka 2 px
  `--ink`, `aria-pressed` (w 0.5.x wypełniony akcentem).

**Kafelki trybu** (Klawisze `E765`, Pad `E7FC`, Chodzenie `E805`, Trening `E945`): wysokość 78,
ikona 20 px, nazwa 15/700, opis 11 `--text-2` w jednej linii z wielokropkiem; wybrany: tło
`--sel-bg`, ramka 2 px `--ink`; niewybrany: `--surface`, ramka 1 px `--control`.
`role="radiogroup"`. 4 / 2 × 2 (7.3).

**Karta stanu połączenia**: stała wysokość 62; kropka 10 px (`--status-*`, „szukam” pulsuje
`opacity`); tytuł 16/600 i podtytuł 14 `--text-2` – po jednej linii z wielokropkiem; przycisk
akcji o szerokości najdłuższego z napisów („Szukaj bieżni”, „Anuluj”, „Rozłącz”,
„Spróbuj ponownie”, „Wyłącz symulator” – technika 7.2 pkt 6); `primary` tylko gdy bieżnia
nie jest połączona.

**Karta poziomu (ekran główny)**: karta klikalna (8.4) bez gradientu: odznaka 62 z liczbą
poziomu | kolumna: „POZIOM 12” Bahnschrift 12 wersaliki `--text-2` + XP z prawej
„1 250 / 2 000 XP” Bahnschrift 12 `--text-2` (flex-wrap), ranga 18/700, pasek XP 12
(`--ink`), podpowiedź 13 `--text-2` „Jeszcze 3,2 km do poziomu 13” | trofeum: odznaka 40
„korona” + „12/40” Bahnschrift 13.

**Klawisz (KeyCap)**: ramka 1 px `--control`, dolna 3 px, promień 6, `padding: 1px 8px`, 700,
klikalny (zmiana klawisza). **Przycisk z klawiszem** (KeyButton): przycisk zwykły z napisem
„Chód: W”, szerokość najdłuższej z próbek („…: naciśnij klawisz…”, „…: Strzałka w prawo”,
„…: Ctrl+Shift+F10”), Esc anuluje.

**Wiersz wyboru (osoba, profil)**: wysokość 62, `grid: 20px 38px minmax(0,1fr) 44px`:
pierścień wyboru 20 px (ramka 2 px `--control`; wybrany: ramka 6 px `--ink`), obrazek 38,
nazwa 16/600 jednoliniowo z wielokropkiem + opis 13 `--text-2` zawijany, ołówek `E70F`.
Wybrany wiersz: ramka 2 px `--ink` (w 0.5.x akcent). **„Dodaj…”**: wysokość 50, ramka
przerywana 1 px `--control`, ikona `E710` i tekst `--text` (w 0.5.x akcent).

**Lista rozwijana (popover profilu/osoby)**: tło `--surface-2`, ramka 1 px `--control`,
promień 12, `padding: 6px`, `--shadow-pop`; szerokość `clamp(280px, 100%, 360px)`, pod
wyzwalaczem, przycięta do kolumny z marginesem 16. Wiersze 42 (50 z podpisem), najechanie
`color-mix(in srgb, var(--text) 10%, transparent)`, aktywny pogrubiony + `E73E` w `--text`.
Linki „Zarządzaj…” w `--text`. Wejście `opacity` + `translateY(-4px→0)` 160 ms; Esc i klik
obok zamykają; fokus wraca na wyzwalacz bez obwódki.

**Paleta kolorów profilu**: próbki 32 px (pełne koło, kolor `d`), `repeat(auto-fill, 32px)`,
przerwa 6; wybrana: pierścień 3 px `--text` + `E73E` w `#0f1f1a`; opis pod spodem 13
`--text-2` („Koral · dobrany z ikony gry”). Klawiatura: strzałki po siatce.

**Siatka ikon profilu**: 48 × 48, glif 20 px; wybrana: ramka 2 px w kolorze profilu (to
podgląd profilu – dane), tło `--surface-2`.

**Oś skoków pada (StepLadder)**: wysokość 96; odcinki 14 px promień 4 w kolorach stref
(krycie 0,4 → 1); kółka progów 9 px (`--surface` + obrys `--text`; zaznaczony 3 px
`--focus`), etykiety „35%” 11 `--text-2` nad osią, prędkości 11 `--text` pod; trójkąt gałki
`--text`. Przeciąganie myszą, strzałki ±1%, Tab – następny próg. Szerokość = kontener.

**Pasek udziału (ShareBar)**: 6 px, tor `--surface-2`, wypełnienie kolor profilu, promień 3.

### 8.15 Wykres godzinowy / dzienny – „Kroki w ciągu dnia” (`WykresKrokow.svelte`)

- SVG o szerokości kontenera (pomiar `ResizeObserver`; przerysowanie tylko przy zmianie
  szerokości lub danych), **stała wysokość 190**; pola: lewa oś 46, dół 22, góra 8.
- Oś Y: okrągła podziałka 1/2/5 × 10ⁿ, najwyżej 5 przedziałów, szczyt min. 100
  (`axis_step`); linie siatki 1 px `--line`; etykiety 11 px `--text-2` do prawej: „0”,
  „500”, „1 tys.”, „1,5 tys.”.
- Słupki: ułożone z części (profile albo osoby), największa część na dole; kolor = próbka
  koloru profilu/osoby (`d`, dane – nie akcent); szerokość `min(30, slot × 0,6)`; każda część
  promień 2, przerwa 1,5 px między częściami (widać tło karty).
- Etykiety X:
  - **godzinowy (Dziś)**: 24 sloty; co 3 godziny „6:00”, a gdy `3 × slot < 34 px` samo „6”;
    bieżąca godzina „teraz” w `--text`, etykiety ±1 godzinę od „teraz” pominięte (nie
    nachodzą na siebie). Przy 352 px karty slot ≈ 10 px – działa.
  - **dzienny**: do 7 dni skróty „pn wt śr cz pt sb nd”, do 14 „d.m”, dalej co n/7; dziś
    „dziś” w `--text`; etykieta pomijana, jeśli nie mieści się w `slot + 20`.
- Najechanie na słupek: `title` „13:00 – 1 234 kroków” (bez własnych dymków).
- Pierwsze pokazanie: słupki rosną `transform: scaleY(0→1)` od osi, 280 ms (raz, nie przy
  każdej aktualizacji); przy ograniczonym ruchu bez animacji.
- Legenda pod wykresem: `flex-wrap`, odstęp 6/10; kropka 10 px w kolorze + nazwa 13
  `--text-2` (max. 14 znaków + „…”, pełna w `title`); najwyżej 6 pozycji (w 0.5.x legenda w
  jednym rzędzie bez zawijania – przy 6 profilach wystawała).
- Brak danych: tekst 14 `--text-2` „Brak kroków w tym okresie. Wejdź na bieżnię – wyniki
  pojawią się tu same.”
- Dostępność: `role="img"`, `aria-label` z podsumowaniem („Dziś 5 432 kroki, najwięcej o
  18:00”) + ukryta tabela z wartościami dla czytników ekranu.

---

## 9. Ekrany – co zmienia się względem 0.5.x

### 9.1 Pasek tytułu strony (uwaga d)

`display: grid; grid-template-columns: 44px minmax(0, 1fr) auto; gap: 8px;` wysokość 60,
`padding: 12px 16px 8px`, w kolumnie treści (max. 680).

- Tytuł h1 20/600 **w jednej linii** z wielokropkiem; **dynamiczna część jako podpis**
  13 `--text-2` w drugiej linii (też z wielokropkiem). Nigdy tytuł złożony z „·”:
  - „Konto online · Damianejro” → **Konto online** / podpis „Damianejro”,
  - „Osiągnięcia · 12 z 40” → **Osiągnięcia** / „12 z 40 zdobytych”,
  - „Profile gier · Damian” (tytuł karty) → **Profile gier** / „Damian”,
  - „Sterowanie · Profil” (tytuł karty) → **Sterowanie** / nazwa profilu.
- Pasek nie przewija się z treścią; jego wysokość jest stała (z podpisem i bez).

### 9.2 Główny

- Pasek: „WalkLevel” (nigdy nie skracany – `min-width: max-content`) + podpis „wersja 0.6.0”
  / „Damian · wersja 0.6.0” | awatar osoby (70 × 44, gdy > 1 osoba) | ikony 44: Aktualizuj
  (tylko gdy jest aktualizacja albo trwa sprawdzanie), Znajomi i ranking (z kontem), Historia,
  Ustawienia. „Zgłoś problem” przechodzi do Ustawień > Pomoc i diagnostyka (było tam też w
  0.5.x). Przy 380 px: 95 + 70 + 4 × 44 + odstępy = ok. 360 – mieści się.
- Kolejność: karta stanu (62) → karta poziomu → karta wskaźnika (wskaźnik 186 + podpis 22 +
  **slot 56**) → kafelki statystyk → karta sterowania (profil + klawisz + przełącznik, kafelki
  trybu, podpowiedź).
- **Trening nie zmienia wysokości (uwaga f)**: żetony stref i pasek Treningu zajmują ten sam
  slot 56; podpis pod wskaźnikiem jednoliniowy; podpowiedź trybu (`output_hint`) ma
  `min-height` dwóch linii i `line-clamp: 2` (pełna treść w `title`); podtytuł karty
  sterowania jednoliniowy. Przełączenie dowolnego trybu nie przesuwa niczego pod kartą.
- **Trening się mieści (uwaga b)**: pasek poziomów z 7.3 (320 px), kafelki trybu 2 × 2 poniżej
  440 px.

### 9.3 Ustawienia (uwagi b, c, e)

- Kategorie jak w 0.5.x (7, ta sama kolejność i opisy), nagłówki z 8.5, niezależne otwieranie.
- Wewnątrz: karty z tytułem bez paska; wiersze ustawień 8.2; segmentowane w pełnej szerokości.
- „Liczba skoków” (pad): wiersz `flex-wrap` – napis i segmentowany (bez stałego 220 px).
- „Aktywny jest tryb …” + „Włącz Pad”: flex-wrap, przycisk przechodzi pod tekst.
- Paleta: zawijająca siatka (w 0.5.x 10 kolumn × 38 px = 374 px – wystawała w wąskim oknie).
- „Zgłoś problem…” zwykły przycisk; linki w „O aplikacji” w `--text`.

### 9.4 Konto online (uwagi b, d)

- Tytuł „Konto online” + podpis z pseudonimem; zakładki: segmentowany 4 (Znajomi, Ranking,
  Osoby, Konto) – 2 × 2 tylko gdyby nie mieściły się w rzędzie.
- Ranking: segmentowany zakresu (Tydzień, Miesiąc, Od początku, Poziomy) – 2 × 2 w wąskim
  kontenerze; wiersze 8.11.
- Zaproszenia: nazwa (1fr, wielokropek) + „Odrzuć”/„Przyjmij” – `flex-wrap`, przyciski
  przechodzą pod nazwę.
- Podział na gry: wiersz `grid: minmax(0,1fr) auto` – nazwa gry z wielokropkiem, statystyki w
  dwóch liniach do prawej (jak w rankingu), zamiast jednej długiej linii.
- Kod zaproszenia: Bahnschrift 18 `--text`, zaznaczalny (bez akcentu).
- Karta poziomu na profilu: odznaka 70 + kolumna, bez gradientu.

### 9.5 Osiągnięcia i szlak

- Pasek rzadkości: 5 komórek w pionie (odznaka 34 z liczbą zdobytych, nazwa Bahnschrift 10–11
  w kolorze rzadkości, „3 z 8” 11 `--text-2`), zawijanie z 7.3.
- Nagłówki grup („Ostatnio zdobyte”, „Najbliżej”, „Pozostałe”): 16/700 bez paska z lewej.
- Wiersz osiągnięcia: karta neutralna (bez gradientu i paska), odznaka 58 | nazwa 16/700
  (zawija się) i **pod nią** napis rzadkości (w 0.5.x z prawej obok nazwy) | opis kursywą,
  warunek, pasek postępu + „12,3 / 42 KM”, stopka „zdobyte wczoraj · +250 XP” Bahnschrift
  12 `--text-2`.
- Szlak: 8.10.

### 9.6 Historia

- Segmentowane „Moja/Wszyscy” i „Dziś/7 dni/30 dni/Wszystko” – neutralne.
- Kafelki 8.7, wykres 8.15, udział profili (wiersze z paskiem udziału), ostatnie sesje
  (wiersz 56: ikona 32, „Dziś 18:20 · Profil” 16/600 jednoliniowo, „5 432 kroków · 42 min ·
  3,1 km/h” 13 `--text-2` zawijane, przełącznik „online” z prawej).

### 9.7 Okno

- Natywny pasek tytułu Windows (przyciąganie, Snap, skróty) w kolorze motywu; Mica pod
  oknem (`windowEffects: mica`, okno przezroczyste, `body { background: var(--bg-window) }`).
- Przycisk na ekranie (bańka 56 × 56, osobne okno bez fokusu): koło, wyłączone `--surface` +
  ramka `--control`, włączone wypełnienie akcentem (chód) albo `--run` (bieg), ikona `E805`
  22 px i prędkość Bahnschrift 12 w kolorze `--on-accent`.

---

## 10. Zakazane wzorce (uwaga i) i co je zastępuje

| W 0.5.x | Dlaczego źle | W 0.6.0 |
|---|---|---|
| `QLabel#section { border-left: 3px solid accent }` | kolorowy pasek z lewej przy nagłówku | sam tytuł 16/700 |
| `Panel(stripe=True)` – pasek 4 px z lewej na kartach osiągnięć i „Tu jesteś” | jw. | karta neutralna; kolor niesie odznaka |
| Pasek 4 px z lewej w popupie nad grą | jw. | brak; kicker w kolorze rzadkości |
| Gradienty: `Panel` (tło w kolorze), `Hex` (liniowy + poświata 6 warstw), `XPBar` (gradient, połysk, świecący koniec), popup (gradient od koloru), „Tu jesteś” (poświata) | gradienty i poświaty | płaskie wypełnienia, jedna ozdobna linia wewnątrz heksagonu |
| Żetony stref wypełnione kolorem, kafelki trybu wypełnione akcentem, poziomy Treningu wypełnione akcentem | „pastylki” i plamy koloru | neutralne zaznaczenie (ramka `--ink`, `--sel-bg`), mały kwadrat koloru strefy |
| „Klawisze ✓”, „✓ to aktywny tryb” | znaki/emoji w interfejsie | kropka 6 px + tekst „aktywny tryb”; ikony tylko z Segoe Fluent |
| Akcent w podświetleniu każdego segmentowanego, w przełącznikach, wierszach, linkach, ikonach kategorii | za dużo akcentu (a) | sekcja 3 |
| Kolorowe tła kart (karta poziomu, profil osoby w kolorze rzadkości) | plamy koloru | karty neutralne |
| Wersaliki z rozstrzeleniem wszędzie | „HUD” jako dekoracja | wersaliki Bahnschrift tylko dla: POZIOM, nazwa rzadkości, TU JESTEŚ, kicker popupu, jednostka postępu |

Ogólnie: bez emoji w tekstach i przyciskach; bez zaokrąglonych „pastylek” na etykietach,
stanach i wartościach; bez cieni na kartach; bez rozmyć; bez ikon w kółkach z kolorowym tłem.

---

## 11. Lista kontrolna „nic nie wystaje”

### 11.1 Ręcznie (przed wydaniem, na każdym ekranie)

Szerokości okna: **420** (minimum), 480, 560, 740 (domyślna), 1200 (szerokie – kolumna max.
680). Każda w motywie jasnym i ciemnym, z tekstem 100% i **115% (Duży tekst)**, przy skali
Windows 100% i 150%.

Dane najgorszego przypadku (atrapa w trybie deweloperskim):

- pseudonim 24 znaki bez spacji (`Damianejro_bardzo_dlugi1`), nazwa profilu 32 znaki,
  nazwa gry 40 znaków, urządzenie „WalkingPad X21 (KS-ST-X21C)”,
- 4 osoby, 6 profili (legenda wykresu), 5 skoków pada, 40 osiągnięć,
- ranking z „12 345 678 kroków” i „1 234,5 km”, poziom 100,
- Trening z poziomami „12,5 km/h”, czas „12 h 05 min”, kroki „123 456”,
- karta „Zaktualizowaliśmy regulamin konta”, 3 zaproszenia przychodzące,
- szlak z „następny przystanek za 1 234,5 km · ok. 247 dni przy 5 km dziennie”.

Ekrany do przejścia:

- [ ] Główny – każdy z 4 trybów (Klawisze, Pad, Chodzenie, **Trening**); przełączanie trybów
      **nie zmienia wysokości** niczego (porównaj położenie kafelków statystyk przed i po).
- [ ] Główny – wszystkie stany połączenia (napis przycisku zmienia się bez zmiany szerokości).
- [ ] Ustawienia – każda kategoria rozwinięta; nagłówki: tytuł i opis po jednej linii,
      wartość skraca się pierwsza; kliknięty nagłówek zostaje pod kursorem.
- [ ] Ustawienia > Gry i sterowanie – każdy tryb w segmentowanym (Pad z 5 skokami, „Liczba
      skoków”, „Gałka puszczona”).
- [ ] Ustawienia > Wygląd – paleta 20 kolorów, segmentowany motywu.
- [ ] Konto online – 4 zakładki, ranking we wszystkich 4 zakresach, profil osoby, zakładka
      Konto (segmentowane widoczności).
- [ ] Osiągnięcia – pasek rzadkości, wiersze z postępem.
- [ ] **Szlak** – wiersz „Tu jesteś” z najdłuższym zdaniem, km 4-cyfrowe.
- [ ] Historia – 4 zakresy, wykres godzinowy i 30-dniowy, legenda 6 pozycji, sesje.
- [ ] Szczegóły sesji, Zgłoś problem, Dziennik, Eksport/Import, Obsługiwane bieżnie,
      Kreator osoby, Edycja profilu gry, Logowanie/Rejestracja.
- [ ] Listy rozwijane profilu i osoby przy prawej krawędzi okna 420.
- [ ] Toast z długim tekstem przy 420.
- [ ] Popup nad grą z najdłuższą nazwą osiągnięcia (wielokropek, nic poza 380 × 96).

Dla każdego: brak poziomego paska przewijania, żaden element nie dotyka prawej krawędzi
kolumny, żaden tekst nie nachodzi na inny, tytuły stron w jednej linii (+ podpis), przyciski
bez uciętych napisów (wielokropek tylko tam, gdzie zaplanowany i z `title`).

### 11.2 Automatycznie

Funkcja w kodzie deweloperskim (`src/lib/dev/wystaje.ts`), wołana przez test e2e (Playwright
na serwerze Vite z atrapą rdzenia – bez uruchamiania bieżni) i dostępna w konsoli:

```ts
/** Elementy wychodzące poza kolumnę treści albo z poziomym przepełnieniem. */
export function wystaje(kolumna: HTMLElement): string[] {
  const granica = kolumna.getBoundingClientRect().right + 0.5;
  const wyniki: string[] = [];
  for (const el of kolumna.querySelectorAll<HTMLElement>("*")) {
    const r = el.getBoundingClientRect();
    if (r.width === 0 || getComputedStyle(el).visibility === "hidden") continue;
    if (r.right > granica) wyniki.push(`poza kolumną: ${opis(el)} (${Math.round(r.right - granica)} px)`);
    const s = getComputedStyle(el);
    const ucina = s.overflowX !== "visible" && s.textOverflow !== "ellipsis";
    if (ucina && el.scrollWidth > el.clientWidth + 1) wyniki.push(`przepełnienie: ${opis(el)}`);
  }
  return wyniki;
}
const opis = (el: HTMLElement) =>
  `${el.tagName.toLowerCase()}${el.className ? "." + String(el.className).split(" ")[0] : ""}` +
  ` „${(el.textContent ?? "").trim().slice(0, 30)}”`;
```

Test e2e: dla każdego ekranu × szerokości {420, 480, 740} × tekst {1, 1.15} × motyw –
`expect(wystaje(kolumna)).toEqual([])`, plus `document.documentElement.scrollWidth <=
innerWidth`. Dodatkowo test stałej wysokości: zapisz `getBoundingClientRect().top` kafelków
statystyk, przełącz kolejno 4 tryby, porównaj (różnica 0). Test akordeonu: zapisz położenie
klikniętego nagłówka, kliknij, porównaj (różnica 0). W trybie deweloperskim nakładka
obrysowuje na czerwono elementy zwrócone przez `wystaje()`.

---

## 12. Mapowanie ze starego kodu

| 0.5.x (PySide6) | 0.6.0 |
|---|---|
| `theme.Theme`, `DARK`, `LIGHT` | `tokeny.css` – `[data-theme="dark"/"light"]` |
| `theme.with_color`, `paleta.KOLORY`, `clashes_with_run` | `paleta.ts` + `ustawAkcent()` (`--accent-d/-l`, `--run-d/-l`) |
| `theme.qss(scale)`, `large_text` | `--text-scale: 1 / 1.15` |
| `RADIUS 10, CONTROL_H 44, GAP 12, PAD 16, FOCUS_W/OFFSET 2` | `--r-md`, `--control-h`, `--sp-3`, `--sp-4`, `--focus-*` |
| `T_COLOR, T_PRESS, T_PANEL_IN/OUT, T_MORPH, T_SLIDE, T_THEME`, `EASE_*` | `--t-*`, `--ease-enter/-exit/-smooth` |
| `apply_backdrop` (Mica, ciemny pasek tytułu) | `windowEffects: mica` + `setTheme`, `--bg-window` |
| `system_reduces_motion`, `Motion.enabled` | `prefers-reduced-motion` |
| `FocusRing` | `:focus-visible` z `--focus` |
| `Switch` | `Przelacznik.svelte` (8.2) |
| `Segmented` | `Segmenty.svelte` (8.3) |
| `HoldButton` | `Przycisk` wariant potwierdz (8.1) |
| `StatusDot` | kropka w karcie stanu (8.14) |
| `StatTile`, `TileGrid` | `Kafelek.svelte` + siatka (8.7, 7.3) |
| `ElidedLabel` | `text-overflow: ellipsis` + `title` |
| `ResponsiveRow`, `FlowLayout` | `flex-wrap` (7.2) |
| `fix_width`, `KeyButton.sizeHint` | siatka z nałożonymi napisami (7.2 pkt 6) |
| `FloatingToggle` | okno bańki (9.7) |
| `ZoneGauge`, `ZoneChips`, `TrainingBar`, `ModeTiles` | 8.14 |
| `StepLadder`, `Stepper`, `PalettePicker`, `IconGrid`, `ChoiceRow`, `AddRow`, `CheckRow`, `Popup` (lista), `SaveNote` | 8.6, 8.14 (`SaveNote`: „Zapisano” w `--text-2` z `E73E`) |
| `odznaki.Hex`, `glyph`, `RARITY_COLORS`, `rank_rarity` | `Odznaka.svelte` + ścieżki SVG (8.8) |
| `odznaki.XPBar` | `PasekXp.svelte` (8.9) |
| `odznaki.Panel` | karta neutralna (8.4) – bez gradientu i paska |
| `caps`, `big_num`, `num_font`, `NUM` | klasy `.hud` i `.liczba` z Bahnschrift (4) |
| `popupy.Popup`, `PopupStack` | jedno okno stosu popupów (8.13) |
| `okno.Cover`, `Pages.go`, `morph_card`, `quietly`, `RowCompare`, `settle_layout` | niepotrzebne: stałe ekrany + `opacity/transform` + FLIP (6) |
| `CalmScrollBar` | `scrollbar-gutter: stable` + cienki pasek 8 px (`::-webkit-scrollbar`, kciuk `--control`, promień 4) |
| `historia.DayBars`, `ShareBar` | `WykresKrokow.svelte` (8.15), pasek udziału (8.14) |
| `poziomy.Rail`, `trail_row` | `WierszSzlaku.svelte` (8.10) |
| `konto.online_ranking` (wiersze) | `WierszRankingu.svelte` (8.11) |
| `App.toast` | `Toast.svelte` (8.12) |
| `okno.header`, `sticky_page`, `framed_page` | pasek tytułu strony z podpisem (9.1) |
| `Window.setMinimumSize(420, 640)`, `DEFAULT_W/H 740×890` | `minWidth/minHeight` Tauri, `--content-max: 680px` |

Ikony Segoe Fluent (kody z 0.5.x): ustawienia `E713`, wstecz `E72B`, odśwież `E72C`,
Bluetooth `E702`, dziennik `E81C`, folder `E838`/`E8B7`, zgłoś `EBE8`, klawiatura `E765`,
oko `E7B3`, chód `E805`, pobierz `E896`, wykres `E9D2`, osoby `E716`, pad `E7FC`,
trening `E945`, strzałka w dół `E70D`, w prawo `E76C`, zaznacz `E73E`, edytuj `E70F`,
dodaj `E710`, minus `E738`, plik `E8A5`, losuj `E8B1`, pole wyboru `E73A`/`E739`/`E73C`
(zaznaczone/puste/częściowo). Ikony profilu: Pad `E7FC`, Tarcza `EA18`, Ogień `ECAD`,
Gwiazda `E734`, Auto `E804`, Samolot `E709`, Świat `E774`, Flaga `E7C1`, Robot `E99A`,
Klocek `EA86`, Iskry `F4A5`, Serce `EB51`.
