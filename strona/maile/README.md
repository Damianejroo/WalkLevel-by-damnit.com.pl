# Maile WalkLevel: szablony do wdrożenia na serwerze

Źródło: `zbuduj.py` (jeden układ dla wszystkich maili). Po zmianie: `uv run python strona/maile/zbuduj.py`.
Podgląd z przykładowymi danymi: `podglad/*.html` (otwórz w przeglądarce).

Styl ustalony z HUB i Kabelkami: jasny list (ciemny wariant tylko przez `prefers-color-scheme`
w Apple Mail i Outlooku na macOS), jeden akapit, jeden przycisk jak w aplikacji, reszta zwykłymi
zdaniami. Bez pastylek, ramek z paskiem z boku i emoji. Jeden growy akcent: „Zadanie · Pierwszy
krok” w mailu potwierdzającym.

| Plik | Kiedy | Temat |
|---|---|---|
| `potwierdzenie.html` + `.txt` | link potwierdzający adres (po rejestracji i na żądanie) | Potwierdź adres e-mail w WalkLevel |
| `nowe-haslo.html` + `.txt` | „Nie pamiętam hasła” | Twój link do nowego hasła w WalkLevel |
| `zgloszenie.html` + `.txt` | „Zgłoś problem” z aplikacji (do autora) | [WalkLevel] Zgłoszenie {{NUMER}} · wersja {{WERSJA}} |

Każdy mail jako multipart/alternative: część tekstowa z `.txt` (bez linii „Temat:”), część HTML z `.html`.

## Maile konta (PocketBase)

Pola PocketBase, bez zmian w nazwach: `{APP_URL}`, `{TOKEN}`. Linki prowadzą do tras serwera:

- potwierdzenie: `{APP_URL}/api/bieznia/potwierdz-email?token={TOKEN}` (ważny 3 dni, wpisane w treść),
- nowe hasło: `{APP_URL}/api/bieznia/nowe-haslo?token={TOKEN}` (30 minut, jednorazowy, wpisane w treść).

Jeśli zmieni się ważność tokenu, trzeba zmienić też zdanie „Link działa przez …” w `zbuduj.py`.

## Zgłoszenie (hak wysyłki)

Pola w podwójnych klamrach; wartości od użytkownika escapować jako HTML (w `.txt` zwykły tekst):

- `{{NUMER}}`: identyfikator zgłoszenia (ten, który wraca do aplikacji jako `id`)
- `{{WERSJA}}`: pole `wersja`
- `{{KONTAKT}}`: pole `kontakt` albo „nie podano”
- `{{OKRES}}`: okres logów, np. „ostatnie 6 godzin” (z pola formularza `okres` - liczba godzin, wysyła je aplikacja od 0.5.0; bez pola „nie podano”)
- `{{ROZMIAR}}`: rozmiar paczki (poniżej 1 KB w bajtach - już poprawione przez HUB)
- `{{CZAS}}`: czas przyjęcia, czas polski, „25.11.2026, 22:41”
- `{{OPIS}}`: opis, escapowany; nowe linie zostają (`white-space: pre-wrap`)

## Do ustawienia na serwerze

- Obrazki: `logo.png` (96×96) i `odznaka.png` (96×96) pod `https://bieznia.damnit-live.pl/maile/`,
  publicznie, z długim cache. Bez obrazków maile dalej są czytelne (puste `alt`, nazwa jest tekstem).
- Nazwa nadawcy: „WalkLevel · damnit” (ustawiona przez HUB 26.11.2026, adres `no-reply@damnit-live.pl`
  bez zmian; celowo bez domeny w nazwie - inna domena niż adres to sygnał dla filtrów antyspamowych).
- Do decyzji Damiana: nagłówek „Odpowiedz do: kontakt@damnit.com.pl”.
