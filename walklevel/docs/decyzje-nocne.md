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

   **Aktualizacja 26.11 (przebudowa szlaku, docs/analiza-szlak.md sekcja 9):**
   - Rozstrzygnięte: najwyżej 2 przystanki z jednego uniwersum (uwaga Damiana o Kerbalu
     zastosowana do wszystkich). Kerbal 1 (Mun), Tolkien 2 (potem zmienione, patrz niżej),
     Westeros 2 (Mur, Z Muru do Dorne, czyli bez fanowskich pomiarów map), Pokémon GO 2 (Srebrny i Platynowy
     Jogger), Gwiezdne wojny 2. W miejsce reszty weszło 26 nowych przystanków z 50+ sprawdzonych
     kandydatów (research w sekcji 9.1).
   - Z propozycji Damiana: DayZ wszedł (Chernarus 225 km², 21,2 km z rogu w róg). Dziedzictwo
     Hogwartu i Kingdom Come: Deliverance II odrzucone: twórcy nie podają wymiarów, są tylko
     pomiary poradników (Hogwart ok. 7,65 km²) i „dwa razy większa niż KCD1”.
   - Gothic dostał źródło: „kula o średnicy dwóch kilometrów” to opis wydawcy na Steam.
   - Id przystanków zmienione i usunięte (konto Damiana to jedyne na serwerze). Lista dla HUB
     w sekcji 10.4. Plik dla serwera: docs/definicje-v2-serwer.json (tools/definicje_serwer.py,
     tylko istniejące typy). Na serwer nic nie wysłano.
   - **Frodo przywrócony decyzją Damiana** („Tak, przywróć Froda”, analiza-szlak.md sekcja 10):
     Śródziemie zwolnione z limitu, 15 etapów od Bucklebury do Mordoru. Inne uniwersa stoją
     między etapami, nadal najwyżej 2 na uniwersum (Kerbal 1). Z nowych pozycji wypadło 7, które
     wypadały kilka-kilkanaście km od etapu Froda (Oliver Twist, Space Engineers, Shikoku,
     Calliope, Szlak Mormonów, Dniepr, Droga Królewska Persów). DayZ zostaje. 65 przystanków,
     86 osiągnięć. Dziura 2 500 → 2 700 km zniknęła; najdłuższy odstęp do Mordoru to 150 km,
     testy znów pilnują 150 km do 2 865 km. Lista id względem v1 dla HUB w sekcji 10.4, plik dla
     serwera wygenerowany ponownie.
   - Do decyzji: (a) w środku szlaku zostało kilka prawdziwych tras (Land’s End – John o’Groats,
     Tōkai, Via Francigena, szlak św. Olafa), bo gier z dystansami 1 000–2 500 km prawie nie ma;
     (b) Pokémon GO i Skyrim 5,7 km jak wyżej.
4. **Dystans i czas ruchu liczone niezależnie od ustawień gry** (dziś zależą od progu „Chód od” -
   przy 2,6 km/h dystans wychodził 27% za mały). Dystans = prędkość × czas, pilnowany licznikiem
   bieżni (±10 m). Liczniki kroków odporne na reset, przepełnienie i pojedyncze błędne odczyty
   (docs/analiza-dystans.md).
5. **Tryb próbny zamiast „Wyjście: Podgląd / Klawiatura”**: przełącznik „Klawisze w grze:
   Włączone / Tryb próbny” z jasną podpowiedzią (docs/teksty.md).
6. **Kopia danych**: plik dalej z rozszerzeniem .bieznia (zgodność ze starszymi kopiami i z 0.5.x).
7. **Synchronizacja sesji z serwera** według kontraktu HUB (docs/synchro.md), trasa wdrożona na
   produkcji za zgodą Damiana w sesji HUB.
