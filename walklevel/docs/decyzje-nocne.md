# Decyzje podjęte w nocy 25/26.11.2026 (do przejrzenia przez Damiana)

Każda decyzja jest odwracalna. Przy każdej jest powód.

1. **Akordeon Ustawień: kategorie otwierają się niezależnie** (bez automatycznego zwijania innych i bez
   przewijania). Powód: zgłoszenie „przycisk akordeonu nie powinien uciekać spod kursora” - przy
   zasadzie „najwyżej jedna otwarta” zwijanie kategorii nad klikniętą zawsze przesuwa nagłówek.
   Wariant z jedną otwartą da się przywrócić (opisany w docs/wyglad.md).
2. **Akcent tylko na jednym elemencie ekranu**, stany wyboru (przełącznik, segment, kafelek trybu,
   wybrany wiersz) neutralne; w Ustawieniach akcentu brak (docs/wyglad.md).
3. **Szlak v2** (docs/definicje-v2.json, docs/analiza-szlak.md): +21 przystanków, gęsto na początku
   (najdłuższy odstęp do 500 km to 45 km), poprawione pozycje etapów Tolkiena i Westeros według
   map, poprawiony Kurier carski (5 523 km). Id istniejących osiągnięć bez zmian - nic zdobytego nie
   znika. Do decyzji: Pokémon GO (gra mobilna z prawdziwym chodzeniem) jako motyw, Kerbal 5 razy,
   Tolkien 20 przystanków, Skyrim zostaje 5,7 km (skala mapy w grze). Serwer liczy poziomy z v1 -
   v2 musi wejść na serwerze razem z aplikacją (HUB).
4. **Dystans i czas ruchu liczone niezależnie od ustawień gry** (dziś zależą od progu „Chód od” -
   przy 2,6 km/h dystans wychodził 27% za mały). Dystans = prędkość × czas, pilnowany licznikiem
   bieżni (±10 m). Liczniki kroków odporne na reset, przepełnienie i pojedyncze błędne odczyty
   (docs/analiza-dystans.md).
5. **Tryb próbny zamiast „Wyjście: Podgląd / Klawiatura”**: przełącznik „Klawisze w grze:
   Włączone / Tryb próbny” z jasną podpowiedzią (docs/teksty.md).
6. **Kopia danych**: plik dalej z rozszerzeniem .bieznia (zgodność ze starszymi kopiami i z 0.5.x).
7. **Synchronizacja sesji z serwera** według kontraktu HUB (docs/synchro.md), trasa wdrożona na
   produkcji za zgodą Damiana w sesji HUB.
