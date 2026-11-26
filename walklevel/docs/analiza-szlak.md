# Analiza: odległości szlaku i osiągnięć (definicje v1 → v2)

Stan na: WalkLevel 0.5.2, plik `bieznia/poziomy/definicje.json` (wersja 1, 61 osiągnięć,
43 przystanki szlaku). Wynik: `walklevel/docs/definicje-v2.json` (wersja 2, 82 osiągnięcia,
61 przystanków). Plik v1 nie był zmieniany.

**Aktualizacja 26.11.2026:** szlak przebudowany pod kątem różnorodności uniwersów (najwyżej 2
przystanki z jednego świata): 59 przystanków, 80 osiągnięć. Opis, research i lista zmienionych id
w sekcji 9. Sekcje 1–7 opisują pierwszą wersję v2.

## 1. Najważniejsze wnioski

1. **Większość dystansów „ze świata” się zgadza** (Erangel, Nether, Cytadela, Gwiazdy Śmierci,
   Carthag–Arrakin, Halo, Minmus, Mun, Mur, Rivendell, Mordor, Hoth, Dysk, Arrakis wg Encyklopedii
   Diuny). Skyrim 5,7 km jest poprawny **dla skali mapy w grze**. Tekst sugerował jednak całą
   prowincję, więc doprecyzowałem go. Progu nie zmieniałem.
2. **Źle ustawione są etapy wypraw liczone od startu** (Władca Pierścieni i Droga Królewska).
   Porównałem je z rozpiską Eowyn Challenge (atlas Fonstad, 1 779 mil = 2 865 km) i z fanowskimi
   pomiarami map Westeros. Caradhras, Wrota Morii, Rauros, Czarna Brama i Schody Cirith Ungol
   były przesunięte o 60–300 km. Fosa Cailin była prawie dwa razy za daleko (1 150 zamiast
   ok. 580 km). Bliźniaki i Harrenhal w ogóle nie leżą przy Drodze Królewskiej.
3. **Kurier carski: pomylone jednostki.** U Verne’a jest 5 200 **wiorst**, czyli ok. 5 523 km,
   a nie 5 200 km.
4. **Zagęszczenie:** doszło 21 przystanków, w tym 10 do 500 km. Przeskok 44,7 → 160 km to teraz
   siedem kroków, najdłuższy ma 45 km. Do Mordoru żaden odstęp nie przekracza 145 km (w v1 było
   150 km, ale tylko dzięki złym pozycjom etapów).
5. **Żadne id nie zniknęło.** Przesunięte przystanki mają stare id. Dwa id dostały nowy motyw,
   bo stary był nie do obrony (sekcja 4). Pozostałe pola pliku (`poziomy_xp`, `rangi`, `zasady`,
   osiągnięcia spoza szlaku) są identyczne z v1, co sprawdziłem skryptem.

Progi rzadkości z zasad projektu, przy 5 km dziennie: pospolite ≤ 70 km (14 dni),
rzadkie ≤ 455 km (91 dni), epickie ≤ 1 825 km (365 dni), heroiczne ≤ 4 560 km (912 dni),
dalej legendarne. Spełnia je i v1, i v2. Rzadkość na szlaku tylko rośnie z dystansem.

## 2. Weryfikacja dystansów v1

Oznaczenia: **OK**, **≈** (szacunek fanowski albo zaokrąglenie, zostaje), **tekst** (liczba
dobra, poprawiony opis), **BŁĄD** (zła wartość albo motyw).

| id | km v1 | Motyw | Ocena | Poprawna wartość, uzasadnienie | v2 |
|---|---|---|---|---|---|
| km1 | 1 | Tutorial | OK | Bez twierdzenia liczbowego. | bez zmian |
| km2 | 2 | Bariera w Gothicu | ≈ | Brak oficjalnych wymiarów Kolonii. Dolina jest mała, więc rząd wielkości 2 km jest wiarygodny. Nie znalazłem źródła, które by to potwierdzało albo podważało. | bez zmian |
| km57 | 5,7 | Skyrim z zachodu na wschód | tekst | Mapa w grze ma ok. 37 km². Siatka wysokości ma 119 × 94 komórek po ok. 58 m, czyli ok. 6,8 × 5,4 km razem z niedostępnymi brzegami. Dostępna część z zachodu na wschód to ok. 5,5–6 km. W świecie gry prowincja jest wielokrotnie większa, ale kanonicznej liczby nie ma. Uwaga Damiana jest słuszna wobec tekstu („cała prowincja”), nie wobec liczby. | tekst: „mapa Skyrima w grze”, km bez zmian |
| km113 | 11,3 | Erangel po przekątnej | OK | 8 × 8 km, przekątna 11,31 km. | bez zmian |
| km24 | 24 | Nether | OK | Skala 1:8: 3 000 bloków w Netherze = 24 000 bloków (m) w Overworldzie. | bez zmian |
| km30 | 30 | Zona ze Stalkera | OK | Zona jest wzorowana na strefie wykluczenia wokół ChNPP o promieniu 30 km. Kordon leży na jej południu. | drobne doprecyzowanie tekstu |
| km447 | 44,7 | Cytadela | OK | Kodeks Mass Effect: 44,7 km długości przy otwartych ramionach. | bez zmian |
| km160 | 160 | Gwiazda Śmierci | OK | Powszechnie przyjmowane 160 km. Starsze materiały Legend podawały 120 km. | bez zmian |
| km200 | 200 | Carthag–Arrakin | OK | Ok. 200 km. Arrakin leży na południowy zachód od Carthagu, za Połamanymi Ziemiami. | bez zmian |
| km318 | 318 | Halo w poprzek | OK | Halopedia: Installation 04 ma 10 000 km średnicy i 318 km szerokości. | bez zmian |
| km377 | 377 | Dookoła Minmusa | OK | Promień 60 km, obwód 377 km. | bez zmian |
| km483 | 483 | Mur | OK | 100 lig = 300 mil = 482,8 km (GRRM). | bez zmian |
| km628 | 628 | Równik drugiej Gwiazdy Śmierci | OK | 200 km średnicy, obwód 628,3 km. | bez zmian |
| km737 | 737 | Do Rivendell | OK | 458 mil (Eowyn Challenge / Fonstad) = 737 km. | bez zmian |
| km880 | 880 | Przełęcz Caradhrasu | **BŁĄD** | 880 km to dopiero ok. 145 km za Rivendell, a Drużyna szła dwa tygodnie do Hollin (45 staj w linii prostej). Tabela Eowyn: Rivendell → Moria to 550 km (Wrota ok. 1 285). Po dniach marszu (14 do Hollin, ok. 2 dalej, z 19) przełęcz wypada na **ok. 1 190 km**. | km 1 190 |
| km950 | 950 | Boston → Pittsburgh | ≈ | Droga ok. 920 km, przez Lincoln i z objazdami trochę więcej. | bez zmian |
| km1019 | 1 019 | Wrota Morii | **BŁĄD** | Tabela Eowyn: ok. **1 285 km** (Rivendell 735 + 550). | km 1 285 |
| km1150 | 1 150 | Fosa Cailin | **BŁĄD** | Pomiary map: Winterfell → Cerwyn 100 mil, Cerwyn → Fosa Cailin 260 mil, razem 360 mil = **ok. 580 km**. | km 580 |
| km1257 | 1 257 | Dookoła Muna | OK | Promień 200 km, obwód 1 256,6 km. | bez zmian |
| km1400 | 1 400 | Mroczna Puszcza | ≈ | Bilbo: Bag End → Erebor 900–967 mil (1 450–1 556 km). Puszcza zajmuje ostatnią trzecią część drogi, a tekst nie podaje liczby. | bez zmian |
| km1530 | 1 530 | Pod Samotną Górę | OK | 951 mil, w przedziale 900–967 mil. | bez zmian |
| km1670 | 1 670 | Bliźniaki | **BŁĄD** | Bliźniaki stoją nad Zielonymi Widłami, na zachód od Drogi Królewskiej, więc nie są jej etapem. Nie ma też pomiaru odległości od Winterfell. | nowy motyw, km 1 609 (sekcja 4) |
| km1800 | 1 800 | Wodogrzmoty Rauros | **BŁĄD** | Lórien → Rauros to 389 mil spływu. Tabela Eowyn: Amon Hen (przy Rauros) **ok. 2 105 km**. Na 1 800 km Drużyna jest dopiero przy Brunatnych Polach. | km 2 105, rzadkość heroiczne |
| km1940 | 1 940 | Rubinowy Bród | **BŁĄD** | Pomiary map: Fosa Cailin → Harroway (przy Brodzie) 700 mil, razem z Winterfell 1 060 mil = **ok. 1 706 km**. | km 1 706, rzadkość epickie |
| km2080 | 2 080 | Emyn Muil | **BŁĄD (tekst)** | Frodo i Sam błądzą po Emyn Muil dopiero za Rauros (2 105+). Na 2 080 km Drużyna płynie przez Emyn Muil i mija Argonath. | nazwa „Argonath”, tekst o Drużynie, km bez zmian |
| km2220 | 2 220 | Harrenhal | **BŁĄD** | Harrenhal leży nad Okiem Boga, z dala od Drogi Królewskiej, i nie ma pomiaru. | nowy motyw na tym samym km (sekcja 4) |
| km2300 | 2 300 | Martwe Bagna | ≈ | Tabela Eowyn: **2 360 km**. | km 2 360 |
| km2414 | 2 414 | Królewska Przystań | tekst | 1 500 mil to najczęstszy fanowski szacunek (zakres 1 450–2 000). Zdanie „Cała Droga Królewska” jest błędne, bo droga biegnie od Muru do Końca Burzy (prawie 2 000 mil). | tekst poprawiony, km bez zmian |
| km2565 | 2 565 | Czarna Brama | **BŁĄD** | Bagna (2 360) 1–2 marca, Morannon 5 marca, Minas Morgul (2 640) 10 marca. Po dniach marszu wychodzi **ok. 2 480 km**. | km 2 480 |
| km2715 | 2 715 | Schody Cirith Ungol | **BŁĄD** | Schody zaczynają się tuż za Minas Morgul (2 640), a przełęcz jest na 2 675. Daje to **ok. 2 650 km**. | km 2 650 |
| km2865 | 2 865 | Z buta do Mordoru | OK | 1 779 mil (Eowyn / Fonstad). | bez zmian |
| km3060 | 3 060 | Tam i z powrotem | ≈ | Ok. 1 900 mil w obie strony. Droga powrotna była nieco inna, więc to szacunek. | bez zmian |
| km3235 | 3 235 | Do krateru Schiaparelli | ≈ | W powieści ok. 3 200 km. 3 235 km podają streszczenia. | bez zmian |
| km3490 | 3 490 | Szlak do Oregonu | tekst | Historyczny szlak ma ok. 2 170 mil (3 492 km). Gra z 1985 r. ma własną, krótszą skalę (ok. 2 000 mil), a tekst mówił o „szlaku z gry”. | tekst: historyczny szlak, po którym prowadzi gra |
| km4500 | 4 500 | Death Stranding | ≈ | USA od wybrzeża do wybrzeża to ok. 4 500 km. Mapa w grze jest mocno skrócona, ale fabularnie to cały kontynent. | bez zmian |
| km4800 | 4 800 | Z Muru do Dorne | ≈ | Ok. 3 000 mil to wartość wyliczana ze skali Muru (300 mil). GRRM porównywał też Westeros do Ameryki Płd. | tekst dopowiada, skąd skala |
| km5200 | 5 200 | Kurier carski | **BŁĄD** | U Verne’a jest **5 200 wiorst** (fr. Wikipedia: 5 523 km). Przekład Kingstona podaje 3 400 mil, czyli ok. 5 470 km. | km 5 523, tekst o wiorstach |
| km6128 | 6 128 | Do serca Arrakis | OK | Encyklopedia Diuny (niekanoniczna): promień 6 128 km. Tekst to zaznacza. | bez zmian |
| km7200 | 7 200 | Przez Hoth | OK | Średnica 7 200 km (Wookieepedia). | bez zmian |
| km8047 | 8 047 | Od Osi do Krawędzi | OK | Dysk ma ok. 10 000 mil średnicy. Promień 5 000 mil = 8 047 km. | bez zmian |

Osiągnięcia spoza szlaku, które podają liczby: Siedem tysięcy stopni (Wysoki Hrothgar, 7 000
stopni), Trzy dni do upadku (Majora’s Mask), Noc hordy (domyślnie co 7. noc w 7 Days to Die),
Pełna pora roku (28 dni w Stardew Valley), W 80 dni dookoła świata i Rok na wyspie (Animal
Crossing). Wszystkie się zgadzają i zostają bez zmian. Rangi poza zakresem zadania przejrzałem
tylko pobieżnie, bez błędów: „Skyrim ma kilka kilometrów wszerz” pasuje do skali gry,
„Frodo: prawie 3 000 km” pasuje do 2 865 km.

## 3. Nowy szlak (v2)

Pogrubione wiersze to nowe przystanki. „Przesunięty” oznacza stare id z nowym km.
Odstęp liczę od poprzedniego przystanku.

| km | id | Rzadkość | Nazwa | Uniwersum | Odstęp | Status |
|---|---|---|---|---|---|---|
| 1 | km1 | pospolite | Tutorial ukończony | - | 1 | bez zmian |
| 2 | km2 | pospolite | Za Barierą | Gothic | 1 | bez zmian |
| 5,7 | km57 | pospolite | Skyrim na piechotę | TES V: Skyrim | 3,7 | tekst |
| 11,3 | km113 | pospolite | Erangel po przekątnej | PUBG | 5,6 | bez zmian |
| 24 | km24 | pospolite | Skrót przez Nether | Minecraft | 12,7 | bez zmian |
| 30 | km30 | pospolite | Przez Zonę | S.T.A.L.K.E.R. | 6 | tekst |
| 44,7 | km447 | pospolite | Cytadela od końca do końca | Mass Effect | 14,7 | bez zmian |
| **53** | **km53** | pospolite | **Przełęcz Chilkoot** | „Zew krwi” (London) | 8,3 | nowy |
| **63** | **km63** | pospolite | **Krawędź Valheim** | Valheim | 10 | nowy |
| **70** | **km70** | pospolite | **Cuszima wzdłuż** | Ghost of Tsushima | 7 | nowy |
| **80,5** | **km805** | rzadkie | **Słoniem do Allahabadu** | „W 80 dni dookoła świata” | 10,5 | nowy |
| **100** | **km100** | rzadkie | **Srebrny Jogger** | Pokémon GO | 19,5 | nowy |
| **115** | **km115** | rzadkie | **Prom w Bucklebury** | Władca Pierścieni | 15 | nowy |
| 160 | km160 | rzadkie | Gwiazda Śmierci na wylot | Gwiezdne wojny | 45 | bez zmian |
| 200 | km200 | rzadkie | Z Carthag do Arrakin | Diuna | 40 | bez zmian |
| **215** | **km215** | rzadkie | **Pod Rozbrykanym Kucykiem** | Władca Pierścieni | 15 | nowy |
| **246** | **km246** | rzadkie | **Z Aten do Sparty** | „Dzieje” Herodota | 31 | nowy |
| **276** | **km276** | rzadkie | **Dookoła Pol** | Kerbal Space Program | 30 | nowy |
| 318 | km318 | rzadkie | Halo w poprzek | Halo | 42 | bez zmian |
| 377 | km377 | rzadkie | Dookoła Minmusa | Kerbal Space Program | 59 | bez zmian |
| **385** | **km385** | rzadkie | **Wichrowy Czub** | Władca Pierścieni | 8 | nowy |
| **408** | **km408** | rzadkie | **Dookoła Bop** | Kerbal Space Program | 23 | nowy |
| 483 | km483 | epickie | Patrol na Murze | Pieśń Lodu i Ognia | 75 | bez zmian |
| 580 | km1150 | epickie | Fosa Cailin | Pieśń Lodu i Ognia | 97 | przesunięty (1 150 → 580) |
| 628 | km628 | epickie | Równik drugiej Gwiazdy Śmierci | Gwiezdne wojny | 48 | bez zmian |
| 737 | km737 | epickie | Do Rivendell | Władca Pierścieni | 109 | bez zmian |
| **817** | **km817** | epickie | **Dookoła Ike** | Kerbal Space Program | 80 | nowy |
| 950 | km950 | epickie | Z Bostonu do Pittsburgha | The Last of Us | 133 | bez zmian |
| **1 000** | **km1000** | epickie | **Złoty Jogger** | Pokémon GO | 50 | nowy |
| **1 046** | **km1046** | epickie | **Z Winterfell na Mur** | Pieśń Lodu i Ognia | 46 | nowy |
| 1 190 | km880 | epickie | Przełęcz Caradhrasu | Władca Pierścieni | 144 | przesunięty (880 → 1 190) |
| 1 257 | km1257 | epickie | Dookoła Muna | Kerbal Space Program | 67 | bez zmian |
| 1 285 | km1019 | epickie | Wrota Morii | Władca Pierścieni | 28 | przesunięty (1 019 → 1 285) |
| 1 400 | km1400 | epickie | Mroczna Puszcza | Hobbit | 115 | bez zmian |
| **1 480** | **km1480** | epickie | **Lothlórien** | Władca Pierścieni | 80 | nowy |
| 1 530 | km1530 | epickie | Pod Samotną Górę | Hobbit | 50 | bez zmian |
| 1 609 | km1670 | epickie | Ściana Pierścienia | „Pierścień” (Niven) | 79 | nowy motyw i km (było: Bliźniaki 1 670) |
| 1 706 | km1940 | epickie | Rubinowy Bród | Pieśń Lodu i Ognia | 97 | przesunięty (1 940 → 1 706), heroiczne → epickie |
| **1 790** | **km1790** | epickie | **Brunatne Pola** | Władca Pierścieni | 84 | nowy |
| **1 935** | **km1935** | heroiczne | **Progi Sarn Gebir** | Władca Pierścieni | 145 | nowy |
| 2 080 | km2080 | heroiczne | Argonath (było: Emyn Muil) | Władca Pierścieni | 145 | nazwa i tekst |
| 2 105 | km1800 | heroiczne | Wodogrzmoty Rauros | Władca Pierścieni | 25 | przesunięty (1 800 → 2 105), epickie → heroiczne |
| 2 220 | km2220 | heroiczne | Zejście z Emyn Muil | Władca Pierścieni | 115 | nowy motyw (było: Harrenhal) |
| 2 360 | km2300 | heroiczne | Martwe Bagna | Władca Pierścieni | 140 | przesunięty (2 300 → 2 360) |
| 2 414 | km2414 | heroiczne | Królewska Przystań | Pieśń Lodu i Ognia | 54 | tekst |
| 2 480 | km2565 | heroiczne | Czarna Brama | Władca Pierścieni | 66 | przesunięty (2 565 → 2 480) |
| **2 540** | **km2540** | heroiczne | **Ithilien** | Władca Pierścieni | 60 | nowy |
| 2 650 | km2715 | heroiczne | Schody Cirith Ungol | Władca Pierścieni | 110 | przesunięty (2 715 → 2 650) |
| **2 762** | **km2762** | heroiczne | **Omsk** | „Michał Strogow” (Verne) | 112 | nowy |
| 2 865 | km2865 | heroiczne | Z buta do Mordoru | Władca Pierścieni | 103 | bez zmian |
| 3 060 | km3060 | heroiczne | Tam i z powrotem | Hobbit | 195 | bez zmian |
| 3 235 | km3235 | heroiczne | Do krateru Schiaparelli | „Marsjanin” | 175 | bez zmian |
| 3 490 | km3490 | heroiczne | Szlak do Oregonu | The Oregon Trail | 255 | tekst |
| **4 000** | **km4000** | heroiczne | **Valles Marineris** | trylogia marsjańska (Robinson) | 510 | nowy |
| 4 500 | km4500 | heroiczne | Przesyłka dostarczona | Death Stranding | 500 | bez zmian |
| 4 800 | km4800 | legendarne | Z Muru do Dorne | Pieśń Lodu i Ognia | 300 | tekst |
| 5 523 | km5200 | legendarne | Kurier carski | „Michał Strogow” (Verne) | 723 | przesunięty (5 200 → 5 523), tekst |
| 6 128 | km6128 | legendarne | Do serca Arrakis | Diuna | 605 | bez zmian |
| 7 200 | km7200 | legendarne | Przez Hoth | Gwiezdne wojny | 1 072 | bez zmian |
| 8 047 | km8047 | legendarne | Od Osi do Krawędzi | Świat Dysku | 847 | bez zmian |
| **10 000** | **km10000** | legendarne | **Platynowy Jogger** | Pokémon GO | 1 953 | nowy |

Odstępy: do 100 km najwyżej 19,5 km, do 500 km najwyżej 75 km (ostatni odcinek przed Murem),
do Mordoru najwyżej 145 km. W v1 był przeskok 44,7 → 160 km (115 km, 23 dni przy 5 km
dziennie). Teraz najdłuższa przerwa do 500 km trwa 15 dni, a do 160 km najwyżej 9 dni.
Jeśli 75 km przed Murem i 59 km przed Minmusem to wciąż za długo, kandydaci na dodatkowe
przystanki są w sekcji 7.

### 3.1. Uzasadnienie nowych przystanków

| id | Twierdzenie | Źródło, wyliczenie |
|---|---|---|
| km53 | Szlak Chilkoot z Dyea do jeziora Bennett ma 53 km; Buck idzie nim ku Klondike | NPS, Wikipedia: 33 mile (53 km). W powieści Londona psi zaprzęg przechodzi Dyea Canyon, Sheep Camp i Chilcoot Divide. |
| km63 | Świat Valheim to dysk o promieniu 10 km, obwód ok. 63 km | Domyślny promień świata 10 000 m (strefa krawędzi do 10 500 m). 2π · 10 = 62,8 km, próg zaokrąglony do 63 („prawie 63 km”). |
| km70 | Cuszima ma ok. 70 km z północy na południe | Wikipedia: ok. 70 × 15 km. Strona turystyczna Nagasaki podaje 82 km, więc 70 to dolna, bezpieczna wartość. Tekst mówi wprost „w rzeczywistości”, bo mapa gry jest pomniejszona. |
| km805 | 50 mil bez torów między Kholby a Allahabadem, Fogg kupuje słonia | Rozdz. XI powieści. 50 mil = 80,5 km. |
| km100 / km1000 / km10000 | Medal Jogger w Pokémon GO: 10 / 100 / 1 000 / 10 000 km | Serebii, StrategyWiki, iMore. Motyw z gry o chodzeniu, idealny do tej aplikacji. |
| km115 | Prom w Bucklebury | Rozpiska etapów Eowyn Challenge: Bucklebury 115 km od Bag End. |
| km215 | Bree | Ta sama rozpiska: 215 km (inne zestawienia: 120–135 mil). |
| km246 | Filippides z Aten do Sparty w dwa dni, Spartathlon 246 km | Herodot, Dzieje VI.105–106. Trasa współczesnego Spartathlonu ma 246 km. |
| km276 / km408 / km817 | Pol r = 44 km, Bop r = 65 km, Ike r = 130 km | Dane ciał w Kerbal Space Program. Obwody: 276,5 / 408,4 / 816,8 km. |
| km385 | Wichrowy Czub | Rozpiska Eowyn: Bree 215 + 170 = 385 km. |
| km1046 | Winterfell → Czarny Zamek Drogą Królewską ok. 650 mil | Fanowskie pomiary map (ta sama tabela co Fosa Cailin). 650 mil = 1 046 km. |
| km1480 | Lothlórien | Rozpiska Eowyn: 1 480 km. Inne zestawienie: Rivendell → Lórien 462 mile, razem 920 mil = 1 481 km. |
| km1609 (id km1670) | Ściany Pierścienia mają ok. 1 000 mil wysokości | Niven (powieść i esej o megastrukturach z 1974 r.). 1 000 mil = 1 609 km. |
| km1790 / km1935 / km2080 | Brunatne Pola, Sarn Gebir, Argonath | Spływ Lórien (1 480) → Amon Hen (2 105) trwał ok. 9,5 dnia (16–25 lutego). Brunatne Pola przypadają na 3.–5. dzień, Sarn Gebir na 7.–8., Argonath na ostatni. Pozycje liniowo wg dni. To szacunek z dokładnością do kilkudziesięciu km. |
| km2220 | Zejście z Emyn Muil | Frodo i Sam: przeprawa 26 lutego, błądzenie 27–29 lutego, zejście 29 lutego, Bagna (2 360) od 1 marca. |
| km2540 | Ithilien | Frodo i Sam w Ithilien 7–8 marca, między Czarną Bramą (5 marca) a Morgul (10 marca). |
| km2762 | Omsk w połowie drogi Strogowa | Verne: Omsk leży „mniej więcej w połowie drogi”. 5 523 / 2 = 2 762 km. |
| km4000 | Valles Marineris ma ok. 4 000 km | Wymiar rzeczywisty (NASA). Kanion jest jednym z głównych miejsc trylogii Robinsona. |

Wszystkie nowe teksty: bez zwracania się do gracza (więc bez rodzaju i bez „X, a ty Y”), bez
wykrzykników, bez zdradzania fabuły (np. Gollum nie pada ani razu). Polskie nazwy według
przekładu Skibniewskiej: Wichrowy Czub, Pod Rozbrykanym Kucykiem, Brunatne Pola,
Srebrna Żyła. Znaki odznak są tylko z zestawu, który rysuje `bieznia/gui/odznaki.py`
(nowego rysowania nie trzeba).

## 4. Zasady zmian i co się dzieje ze zdobytymi

- **Id nigdy nie znika.** Sprawdziłem skryptem: zbiór id v2 zawiera wszystkie id v1.
- **Motyw dobry, km zły → zmiana km, id zostaje.** Dotyczy km880, km1019, km1150, km1800,
  km1940, km2300, km2565, km2715, km5200. Id przestaje odpowiadać liczbie km. To tylko
  identyfikator, nikt go nie widzi. Nowe id są tak dobrane, żeby nie kolidowały ze starymi.
  Dlatego Brunatne Pola mają `km1790`, a nie `km1800`, a Sarn Gebir `km1935`, a nie `km1940`.
- **Motyw nie do obrony → nowy motyw pod tym samym id.**
  - `km1670` (Bliźniaki) → Ściana Pierścienia, km 1 609. Zamki Freyów nie leżą na Drodze
    Królewskiej i nie ma żadnego pomiaru odległości.
  - `km2220` (Harrenhal) → Zejście z Emyn Muil, km bez zmian. Ten sam powód.
  - `km2080` zostaje przy Emyn Muil, ale z perspektywy Drużyny na rzece, i ma nazwę
    „Argonath”.
- **Zmiany rzadkości:** `km1800` epickie → heroiczne (+700 XP premii), `km1940` heroiczne →
  epickie (−700 XP). Rzadkość musi wynikać z km.
- **Zdobyte wcześniej:** `evaluate(..., earned_before=...)` nie odbiera zdobyczy. Ktoś, kto
  w v1 zdobył np. `km1800` na 1 800 km, zachowa je, choć w v2 próg to 2 105 km. Szlak pokaże
  je wtedy jako zdobyte „przed czasem”. W praktyce to nikogo nie dotyczy: najniższy
  przesunięty próg v1 to 880 km, a nikt jeszcze tyle nie przeszedł. Przy `km1150` jest
  odwrotnie: próg spada z 1 150 do 580 km, więc osoby między 580 a 1 150 km dostaną je od razu
  po aktualizacji. To zamierzone.
- **Zmiana progów = nowa `wersja`** (zasada z pola `opis`). Stąd `"wersja": 2`.

## 5. Wpływ na XP, poziomy i kod

Premie za nowe przystanki (`xp_za_rzadkosc` bez zmian):

| Do km | Premia v1 | Premia v2 | Różnica | Ile to km |
|---|---|---|---|---|
| 100 | 140 | 400 | +260 | 2,6 km |
| 500 | 1 040 | 1 900 | +860 | 8,6 km |
| 1 000 | 3 040 | 4 900 | +1 860 | 18,6 km |
| 2 865 | 16 140 | 23 100 | +6 960 | 69,6 km |
| 8 047 | 33 440 | 41 600 | +8 160 | 81,6 km |

To najwyżej ułamek poziomu na początku i 1–2 poziomy przy 3 000+ km. Test „poziom 100 po ok.
5 latach przy 5 km dziennie” liczy same km, więc się nie zmienia. `poziomy_xp` i `rangi`
zostają bez zmian.

Do zrobienia w 0.6.0 (poza tym plikiem):

- `walklevel/docs/kontrakt.md` mówi o „61 osiągnięciach, wersja 1”. W v2 jest **82**
  osiągnięcia, w tym 61 przystanków szlaku.
- Testy rdzenia w Ruście, przeniesione z `tests/test_poziomy.py`: `km57` nadal wypada na 6 km,
  `km113` na 12 km, odstępy do 2 865 km ≤ 150 (dziś test dopuszcza 152), rzadkość rośnie
  z km. Warto dodać test, że id v2 ⊇ id v1, i test, że km mieści się w progach rzadkości
  z sekcji 1.
- Serwer kont liczy to samo z tego samego pliku. Przy wydaniu v2 musi dostać ten sam plik
  w tym samym momencie co aplikacja (nic nie wrzucamy teraz).
- Więcej przystanków to więcej popupów na początku: do 100 km wypada 12 zamiast 7. Przy
  jednej sesji z kilkoma progami popupy powinny iść w kolejce, nie na raz.
- Propozycje z `docs/teksty.md` sekcja 28 („Tobą”, „Twoje” wielką literą, nowy warunek
  serii) **nie są wprowadzone** w `definicje-v2.json`, żeby nie mieszać dwóch decyzji. Można
  je nałożyć na v2 bez zmiany progów. Nowe teksty szlaku nie zwracają się do gracza, więc ich
  to nie dotyczy.

## 6. Pytania do Damiana

1. **Kerbal Space Program pojawia się 5 razy** (Minmus, Mun + nowe Pol, Bop, Ike). Wybrałem
   te księżyce, bo mają dokładne, pewne wymiary. Jeśli to za dużo, Bop (408) i Pol (276) można
   wymienić na inne motywy. Wtedy odstępy przed Murem i przed Halo wydłużą się do ok. 75 km.
2. **Władca Pierścieni jest kręgosłupem szlaku** (droga do Mordoru). Ma teraz 17 przystanków
   (plus 3 z Hobbita), w tym 7 nowych i 1 z nowym motywem pod starym id. Rozpisanie drogi Froda etap po etapie daje ciągłość, ale kosztem
   różnorodności. Alternatywa: zostawić 4–5 kluczowych etapów i resztę wymienić na inne
   uniwersa.
3. **Westeros opiera się na fanowskich pomiarach map** (Fosa Cailin, Rubinowy Bród,
   Winterfell → Mur, Królewska Przystań). GRRM podał tylko długość Muru, a mapy są
   niespójne. Czy taki szacunek wystarczy, czy wolisz Westeros tylko z twardymi liczbami
   (Mur, Z Muru do Dorne)?
4. **Pokémon GO** to aplikacja chodzona po prawdziwym świecie, a nie fikcyjne uniwersum, ale
   gra o chodzeniu. Pasuje do zasady „motywy z gier”?
5. Skyrim: zostawiam 5,7 km z dopiskiem „w skali gry”. Jeśli wolisz wartość z całej siatki
   mapy, to 6,8 km. Wymaga to zmiany testu `km57`.

## 7. Kandydaci zapasowi (nie weszli do v2)

- Rendezvous with Rama (Clarke): cylinder 50 km długości, 20 km średnicy, obwód 62,8 km.
  Koliduje z Valheim.
- Pierwsza Gwiazda Śmierci: równik π · 160 = 503 km. Za blisko Muru i powtarza motyw.
- Shire wszerz: 40 staj = 120 mil = 193 km (Prolog Władcy Pierścieni). Wypełniłby lukę
  160 → 200, ale jest kolejnym Tolkienem.
- Trzej Myśliwi: 45 staj w niecałe cztery dni = 217 km. Koliduje z Bree.
- Kerbin: obwód 3 770 km. Gilly: 82 km.
- Perska Droga Królewska u Herodota: 450 parasangów, ok. 2 500 km. Ładnie zgrywa się z Drogą
  Królewską Westeros, ale przelicznik parasanga jest niepewny.
- Odrzucone z braku pewnej liczby: punkty orientacyjne gry The Oregon Trail (1985) (jedno
  nieoficjalne źródło), Baldur’s Gate → Waterdeep (500–750 mil zależnie od źródła), droga
  Watneya do Pathfindera, Bliźniaki i Harrenhal od Winterfell.

## 8. Źródła

- Eowyn Challenge / atlas K. W. Fonstad: rozpiska etapów 2 865 km:
  https://lord-of-the-steps.ru/en/blog/frodos-route; podział na odcinki (458 / 462 / 389 / 470
  mil): https://dangermouse.net/blog/?p=838; Tolkien Gateway (Hollin, Moria 40+ mil):
  https://tolkiengateway.net/wiki/Hollin, https://tolkiengateway.net/wiki/West-gate
- Bilbo, Bag End → Erebor 900–967 mil: https://www.glyphweb.com/arda/q/questoferebor.html,
  https://footprintsonthecamino.blogspot.com/2014/06/the-road-goes-ever-on-and-on.html
- Westeros, pomiary map: https://winteriscoming.net/2015/11/10/game-of-thrones-fan-tabulates-distances-between-places-in-westeros;
  Droga Królewska: https://awoiaf.westeros.org/index.php/The_Kingsroad;
  Winterfell → KL: https://screenrant.com/game-thrones-winterfell-kings-landing-distance-travel-time/;
  3 000 mil: https://awoiaf.westeros.org/index.php/Climate, https://gizmodo.com/mapping-the-seven-kingdoms-5689619
- Skyrim, rozmiar mapy: https://forums.nexusmods.com/topic/669051-distances-in-skyrim,
  https://gamerant.com/every-elder-scrolls-game-ranked-map-size/
- Halo: https://www.halopedia.org/Installation_04
- Gwiazda Śmierci: https://en.wikipedia.org/wiki/Death_Star
- Carthag–Arrakin: https://dune.fandom.com/wiki/Arrakeen, https://tj-rpg.github.io/dune/directory/Locations/Carthag
- Arrakis, promień wg Encyklopedii Diuny: https://fictionhorizon.com/dune-heres-how-big-arrakis-is-compared-to-earth-and-other-planets-of-our-solar-system/
- Michał Strogow: https://standardebooks.org/ebooks/jules-verne/michael-strogoff/w-h-g-kingston/text/chapter-1-4
  (3 400 mil), https://wikimonde.com/article/Michel_Strogoff (5 200 wiorst = 5 523 km)
- W 80 dni (Kholby–Allahabad, 50 mil): https://standardebooks.org/ebooks/jules-verne/around-the-world-in-eighty-days/george-makepeace-towle/text/chapter-11
- Marsjanin: https://the-martian.fandom.com/wiki/Schiaparelli_crater, https://www.space.com/30400-the-martian-how-to-stay-alive-on-mars-infographic.html
- Szlak Oregoński: https://www.nps.gov/oreg/planyourvisit/upload/National-Park-Service-Oregon-Trail-Map-508.pdf,
  https://wikipedia.com/wiki/The_Oregon_Trail_(1985_video_game)
- Chilkoot: https://www.nps.gov/klgo/learn/historyculture/historic-chilkoot-trail.htm
- Valheim: https://www.pcgamer.com/valheim-player-discovers-whats-beyond-the-edge-of-its-enormous-map/,
  https://thunderstore.io/c/valheim/p/JereKuusela/Expand_World/v/1.0.0/
- Cuszima: https://en.wikipedia.org/wiki/Tsushima_Island, https://www.discover-nagasaki.com/en/featured-topics/about-tsushima
- Pokémon GO, medal Jogger: https://serebii.net/pokemongo/medals.shtml, https://strategywiki.org/wiki/Pokémon_GO/Medals
- Pierścień (Niven), ściany 1 000 mil: https://larryniven.fandom.com/wiki/Ringworld
- Rama: https://en.wikipedia.org/wiki/Rendezvous_with_Rama
- Kerbal Space Program (promienie ciał): dane z gry (wiki KSP)

## 9. Przebudowa szlaku 26.11.2026 (różnorodność uniwersów)

Uwaga Damiana: za dużo Kerbala („Z Kerbala 1, góra 2 rzeczy”). Zasada zastosowana do każdego
uniwersum: **najwyżej 2 przystanki z jednego świata**. Kont poza kontem Damiana na serwerze jeszcze
nie ma, więc id przystanków można było zmieniać i usuwać (sekcja 9.4). Sekcje 3 i 6 opisują stan
sprzed przebudowy.

### 9.1. Research uniwersów 26.11

Wiarygodność: **wysoka** (tekst źródłowy, dane z gry, oficjalny opis, encyklopedia), **średnia**
(źródła podają różne liczby albo jest tylko przewodnik), **niska** (pomiar albo wyliczenie
społeczności). Status: **wszedł**, **zapas** (dobra liczba, ale koliduje z innym przystankiem)
albo powód odrzucenia.

| Kandydat | Liczba | Źródło | Wiarygodność | Status |
|---|---|---|---|---|
| Gothic, bariera | kula 2 km średnicy | opis gry na Steam: https://store.steampowered.com/app/65540/Gothic_1/ | wysoka (wydawca) | wszedł (dotąd bez źródła) |
| **DayZ** (propozycja Damiana), Chernarus | 225 km² (15 × 15 km), przekątna 21,2 km | https://en.wikipedia.org/wiki/DayZ_(video_game) | wysoka (przekątna wyliczona) | wszedł |
| „Oliver Twist” (Dickens) | 70 mil do Londynu, tydzień pieszo | rozdz. VIII: https://standardebooks.org/ebooks/charles-dickens/oliver-twist/text/chapter-8 | wysoka | wszedł (112,7 km) |
| Space Engineers, planeta ziemiopodobna | 120 km średnicy, równik 377 km | https://spaceengineers.fandom.com/wiki/Planets, https://www.moddb.com/news/update-01108-planets | wysoka | wszedł |
| Togo, sztafeta do Nome 1925 | 261 mil (420 km), najdłuższy etap | https://www.smithsonianmag.com/history/this-heroic-dog-raced-across-the-frozen-alaskan-wilderness-to-deliver-life-saving-medicine-but-his-contributions-were-long-overlooked-180985905/, https://en.wikipedia.org/wiki/Togo_(dog) | średnia (261–264 mile) | wszedł |
| Via Appia („Quo vadis”) | 132 + 230 mil, ok. 583 km | https://www.britannica.com/topic/Appian-Way | średnia (inne źródło: 563 km) | wszedł |
| Szlak św. Olafa, Oslo → Trondheim | ok. 640 km | https://www.visitnorway.com/listings/saint-olav-ways-to-trondheim-seven-route-suggestions/22461/ | średnia (632–643 km) | wszedł |
| „Marsjanin”, Ares 3 → Pathfinder | ok. 800 km | https://apod.nasa.gov/apod/ap150516.html, https://hirise.lpl.arizona.edu/ESP_041277_2115 | wysoka | wszedł |
| „The Unlikely Pilgrimage of Harold Fry” (Joyce) | 627 mil (1 009 km) | https://www.spectator.co.uk/article/the-unlikely-pilgrimage-of-harold-fry-by-rachel-joyce-review | wysoka (dla powieści; BFI o filmie: 450 mil) | wszedł |
| Sztafeta z surowicą do Nome (Balto) | 674 mile (1 085 km) w 5,5 dnia | https://en.wikipedia.org/wiki/1925_serum_run_to_Nome | wysoka | wszedł |
| Pielgrzymka 88 świątyń Shikoku | ok. 1 200 km | https://en.wikivoyage.org/wiki/88_Temple_Pilgrimage, https://en.wikipedia.org/wiki/Shikoku_Pilgrimage | średnia (oficjalna strona turystyczna: 1 400 km) | wszedł, tekst: „najczęściej podawane” |
| Land’s End → John o’Groats | 874 mile (1 407 km) drogami | https://en.wikipedia.org/wiki/Land%27s_End_to_John_o%27_Groats | wysoka | wszedł |
| Star Citizen: Calliope, Daymar | r = 240 i 295 km, równik 1 508 i 1 854 km | https://starcitizen.tools/Calliope, https://starcitizen.tools/Daymar | wysoka (dane z gry) | weszły oba |
| Star Citizen: Lyria, Yela, Cellin | r = 223, 313, 260 km | https://starcitizen.tools/Lyria, https://starcitizen.tools/Yela, https://starcitizen.tools/index.php?title=Cellin | wysoka | odrzucone: limit 2 na uniwersum |
| Szlak przyrodniczy Tōkai | 1 697 km | https://www.kankyo1.metro.tokyo.lg.jp/naturepark/english/know/park/introduction/toritu/meiji/toukaishizenhodou.html | wysoka (władze Tokio) | wszedł |
| „Dzika droga” (Strayed), Pacific Crest Trail | 1 100 mil (1 770 km) | https://en.wikipedia.org/wiki/Wild_(memoir), https://www.npr.org/2012/03/18/148858058/a-wild-solitary-journey-on-the-pacific-crest-trail | wysoka | wszedł |
| Via Francigena | Sigeric ok. 1 700 km, dzisiejszy szlak 1 900–2 000 km | https://en.wikipedia.org/wiki/Via_Francigena, https://caminoways.com/how-long-is-the-via-francigena, https://pilgrimstorome.org.uk/planning/walking-the-via-francigena/ | średnia | wszedł, tekst: „według przewodników około 2 000 km” |
| Szlak Mormonów | ok. 1 300 mil (2 092 km) | https://home.nps.gov/mopi/espanol/index.htm, https://en.wikipedia.org/wiki/Mormon_Trail | wysoka (FAQ NPS: 1 400 mil) | wszedł |
| Dniepr („Ogniem i mieczem”) | 2 201 km | https://en.wikipedia.org/wiki/Dnieper | wysoka (inne źródła: 2 145–2 290 km) | wszedł |
| Wielka Rafa Koralowa („Gdzie jest Nemo”) | ponad 2 300 km | https://en.wikipedia.org/wiki/Great_Barrier_Reef | wysoka | wszedł |
| „Rabbit-Proof Fence” (Noyce) | 1 500 mil przez 9 tygodni | https://en.wikipedia.org/wiki/Rabbit-Proof_Fence | wysoka | wszedł (2 414 km) |
| Droga Królewska Persów (Herodot V.52–53) | 450 parasangów, 90 dni, ok. 2 500 km | https://livius.org/articles/concept/royal-road | średnia (Wikipedia: 2 699 km, Britannica: ponad 2 400 km) | wszedł |
| „Tracks”, wyprawa Robyn Davidson | ok. 2 700 km | https://en.wikipedia.org/wiki/Tracks_(2013_film) | wysoka | wszedł |
| Pony Express | ok. 1 900 mil (3 058 km) w 10 dni | https://www.nps.gov/poex/learn/historyculture/index.htm | średnia (1 800–1 966 mil) | wszedł |
| Route 66 („Grona gniewu”) | 2 448 mil (3 940 km) | https://en.wikipedia.org/wiki/U.S._Route_66, https://www.legendsofamerica.com/66-facts/ | wysoka | wszedł |
| „Zew krwi”, zaprzęg Bucka | 2 500 mil w niecałe 5 miesięcy | rozdz. V: https://etc.usf.edu/lit2go/151/the-call-of-the-wild/2664/chapter-5-the-toil-of-trace-and-trail/ | wysoka | wszedł (4 023 km) |
| Rawicz, „The Long Walk” | ok. 6 500 km (4 000 mil) | https://en.wikipedia.org/wiki/S%C5%82awomir_Rawicz | liczba z książki, sama relacja podważana | wszedł, tekst to zaznacza |
| „Podróże Guliwera”, Brobdingnag | ok. 6 000 mil (9 656 km) | rozdz. II.4: https://standardebooks.org/ebooks/jonathan-swift/gullivers-travels/text/chapter-2-4 | wysoka | wszedł |
| **Dziedzictwo Hogwartu** (propozycja Damiana) | mapa ok. 7,65 km² (3,7 × 1,4 km + 1,9 × 1,3 km) | https://www.powerpyx.com/hogwarts-legacy-full-world-map-revealed/ | niska (pomiar poradnika, twórcy nic nie podali) | odrzucone: to tylko powierzchnia, a długość ok. 5,6 km wypada na Skyrim |
| **Kingdom Come: Deliverance II** (propozycja Damiana) | „dwa razy większa niż KCD1”, bez km | https://gamepressure.com/kingdom-come-deliverance-2/how-big-is-the-open-world/z2117a8, https://dexerto.com/gaming/kingdom-come-deliverance-2-map-size-3025981 | niska (Warhorse nie podaje wymiarów) | odrzucone: brak sprawdzalnej liczby |
| Atak Tytanów, mury | Maria → Rose 100 km, Rose → Sina 130 km, Sina r = 250 km | wyłącznie strony fanowskie, przewodnika nie udało się potwierdzić | niska | odrzucone |
| Starkiller (Gwiezdne wojny) | 660 km średnicy (Star Wars: Super Graphic) | cytowania na forach | średnia | odrzucone: limit 2 (Gwiazda Śmierci, Hoth) |
| Laputa (Swift) | 7 837 jardów (7,2 km) | rozdz. III.3: https://standardebooks.org/ebooks/jonathan-swift/gullivers-travels/text/chapter-3-3 | wysoka | zapas (za gęsto przy Skyrimie) |
| Nordschleife (Gran Turismo, Forza) | 20,832 km | https://en.wikipedia.org/wiki/List_of_Nordschleife_lap_times_(racing) | wysoka | zapas (koliduje z DayZ) |
| Most Ramy (Ramajana) | 48 km | https://en.wikipedia.org/wiki/Adam%27s_Bridge | wysoka | zapas (między Cytadelą a Chilkoot) |
| Mur Hadriana | 80 mil rzymskich, 73 mile (117 km) | https://en.wikipedia.org/wiki/Hadrian%27s_Wall | wysoka | zapas (koliduje z Oliverem) |
| „Jądro ciemności”, marsz Marlowa | 200 mil (322 km) | https://www.sparknotes.com/nofear/lit/heart-of-darkness/part-1/page_13 | wysoka | zapas (koliduje z Halo) |
| Indianapolis 500 | 500 mil (805 km) | https://en.wikipedia.org/wiki/Indianapolis_500 | wysoka | zapas (koliduje z Pathfinderem) |
| Camino Francés („The Way”) | ok. 780–800 km | https://en.wikipedia.org/wiki/The_Way_(2010_film) | średnia | zapas |
| Test Drive Unlimited (Oahu) / TDU 2 | 1 000 mil dróg / ponad 3 000 km | https://www.gamesradar.com/test-drive-unlimited-13, https://en.wikipedia.org/wiki/Test_Drive_Unlimited_2 | średnia (Autoblog: 1 500 mil dla TDU 2) | zapas (Oahu koliduje z Pierścieniem) |
| The Crew | ok. 10 000 km dróg | https://whatculture.com/gaming/10-biggest-open-world-maps-in-video-games-2 | średnia | zapas (koliduje z Platynowym Joggerem) |
| Fuel | 14 400 km² (Guinness) | https://www.guinnessworldrecords.com/world-records/110300-largest-playable-area-in-a-racing-videogame | wysoka | odrzucone: sama powierzchnia |
| Bashō, „Ścieżkami Północy” | ok. 2 400 km w ok. 150 dni | https://en.wikipedia.org/wiki/Oku_no_Hosomichi | wysoka | zapas (koliduje z 2 414 km) |
| Harrer, „Siedem lat w Tybecie”; Kipling, „Kim” | ponad 1 500 mil; 1 500 mil Grand Trunk Road | https://www.smithsonianmag.com/history/the-strange-journey-of-heinrich-harrer-52700169/, https://en.wikipedia.org/wiki/Grand_Trunk_Road | średnia | zapas (ta sama pozycja 2 414 km) |
| Tōkaidō (Hiroshige) | 492 albo 514 km | https://www.nippon.com/en/japan-topics/g02415/, https://www.britannica.com/topic/Tokaido | średnia | odrzucone: źródła się różnią, tuż za Murem |
| Główny Szlak Beskidzki / Sudecki | 496 / 443 km | https://en.wikipedia.org/wiki/Main_Beskid_Trail, https://en.wikipedia.org/wiki/Main_Sudetes_Trail | wysoka | zapas (bez motywu z gry lub książki) |
| Iditarod | 975 albo 998 mil zależnie od trasy | https://en.wikipedia.org/wiki/Iditarod_Trail_Sled_Dog_Race | średnia | zapas |
| King, „Wielki Marsz” | brak łącznej liczby w źródłach | https://en.wikipedia.org/wiki/The_Long_Walk | - | odrzucone |
| Metro Exodus | brak łącznej liczby | https://www.gamerevolution.com/review/495517-metro-exodus-review-pc | - | odrzucone |
| Need for Speed: The Run | ok. 3 000 mil | https://www.pcgamer.com/need-for-speed-the-run-trailer-races-3000-miles-for-25-million/ | średnia | zapas |
| Bryson, „A Walk in the Woods” | 870 mil | jedna recenzja | niska | odrzucone |
| Ksenofont, „Anabaza” | 34 255 stadiów | https://www.gutenberg.org/files/1170/1170-0.txt | średnia (niepewna długość stadionu) | odrzucone |

### 9.2. Stara → nowa pozycja

Bez zmian zostały: km1, km57, km113, km24, km30, km447, km53, km63, km70, km805, km100, km160,
km200, km246, km318, km483, km737, km950, km1257, km2762, km2865, km3235, km3490, km4500,
km4800, km6128, km7200, km8047, km10000. Ściana Pierścienia i Kurier carski mają tę samą treść
pod nowymi id.

| Było (km, id, motyw, uniwersum) | Jest (km, id, motyw, uniwersum) | Źródło |
|---|---|---|
| 2, km2, Za Barierą (bez źródła) | 2, km2, ten sam motyw, tekst według opisu gry | Steam |
| - | 21,2, km212, Chernarus z rogu w róg, DayZ | Wikipedia |
| 115, km115, Prom w Bucklebury, Władca Pierścieni | 112,7, km1127, Siedemdziesiąt mil do Londynu, „Oliver Twist” | Dickens, rozdz. VIII |
| 215, km215, Pod Rozbrykanym Kucykiem, Władca Pierścieni | usunięty (odstęp 200 → 246 to 46 km) | - |
| 276, km276, Dookoła Pol, KSP | usunięty (odstęp 246 → 318 to 72 km) | - |
| 377, km377, Dookoła Minmusa, KSP | 377, km377, Równik w Space Engineers | wiki Space Engineers |
| 385, km385, Wichrowy Czub; 408, km408, Dookoła Bop, KSP | 420, km420, Najdłuższy etap Togo, sztafeta do Nome 1925 | Smithsonian |
| 580, km1150, Fosa Cailin, Westeros (pomiar map) | 583, km583, Via Appia do Brindisi, „Quo vadis” | Britannica |
| 628, km628, Równik drugiej Gwiazdy Śmierci | 640, km640, Szlak św. Olafa | Visit Norway |
| 817, km817, Dookoła Ike, KSP | 800, km800, Po Pathfindera, „Marsjanin” | NASA |
| 1 000, km1000, Złoty Jogger, Pokémon GO | 1 009, km1009, Pielgrzymka Harolda Fry | The Spectator |
| 1 046, km1046, Z Winterfell na Mur (pomiar map) | 1 085, km1085, Surowica dla Nome (Balto) | Wikipedia |
| 1 190, km880, Przełęcz Caradhrasu | 1 200, km1200, 88 świątyń Shikoku | Wikivoyage |
| 1 285, km1019, Wrota Morii; 1 400, km1400, Mroczna Puszcza | 1 407, km1407, Land’s End – John o’Groats | Wikipedia |
| 1 480, km1480, Lothlórien; 1 530, km1530, Pod Samotną Górę | 1 508, km1508, Równik Calliope, Star Citizen | starcitizen.tools |
| 1 609, km1670, Ściana Pierścienia | 1 609, km1609, treść bez zmian | Niven |
| 1 706, km1940, Rubinowy Bród (pomiar map) | 1 697, km1697, Szlak Tōkai | władze Tokio |
| 1 790, km1790, Brunatne Pola | 1 770, km1770, Dzika droga (Strayed) | Wikipedia, NPR |
| 1 935, km1935, Progi Sarn Gebir | 1 854, km1854, Równik Daymara, Star Citizen | starcitizen.tools |
| 2 080, km2080, Argonath | 2 000, km2000, Via Francigena | przewodniki (≈) |
| 2 105, km1800, Wodogrzmoty Rauros | 2 092, km2092, Szlak Mormonów | NPS |
| 2 220, km2220, Zejście z Emyn Muil | 2 201, km2201, Wzdłuż Dniepru, „Ogniem i mieczem” | Wikipedia |
| 2 360, km2300, Martwe Bagna | 2 300, km2300, Wielka Rafa Koralowa, „Gdzie jest Nemo” | Wikipedia |
| 2 414, km2414, Królewska Przystań (pomiar map) | 2 414, km2414, Wzdłuż płotu przeciw królikom, „Rabbit-Proof Fence” | Wikipedia |
| 2 480, km2565, Czarna Brama; 2 540, km2540, Ithilien | 2 500, km2500, Droga Królewska Persów, Herodot | Livius |
| 2 650, km2715, Schody Cirith Ungol | 2 700, km2700, Z wielbłądami do oceanu, „Tracks” | Wikipedia |
| 3 060, km3060, Tam i z powrotem, Hobbit | 3 058, km3058, Pony Express | NPS |
| 4 000, km4000, Valles Marineris, trylogia marsjańska | 3 940, km3940, Route 66, „Grona gniewu”; 4 023, km4023, Dwa i pół tysiąca mil, „Zew krwi” | Wikipedia; London, rozdz. V |
| 5 523, km5200, Kurier carski | 5 523, km5523, treść bez zmian | Verne |
| - | 6 500, km6500, Z łagru do Indii, Rawicz | Wikipedia |
| - | 9 656, km9656, Brobdingnag wzdłuż, „Podróże Guliwera” | Swift, rozdz. II.4 |

Wynik: 59 przystanków (było 61), 80 osiągnięć (było 82). Osiągnięcia spoza szlaku bez zmian.

### 9.3. Uniwersa po zmianie

Po 2 przystanki: Pokémon GO (100, 10 000), Gwiezdne wojny (160, 7 200), Diuna (200, 6 128),
Herodot (246, 2 500), Pieśń Lodu i Ognia (483, 4 800: same liczby od GRRM, bez pomiarów map),
Śródziemie (737, 2 865), „Marsjanin” (800, 3 235), Star Citizen (1 508, 1 854), sztafeta do Nome
1925 (420, 1 085), „Zew krwi” (53, 4 023), „Michał Strogow” (2 762, 5 523). Kerbal Space Program:
1 (Mun, 1 257 km). Pozostałe po 1. Przed zmianą: Tolkien 20, Westeros 6, KSP 5, Gwiezdne wojny 3,
Pokémon GO 3.

Odstępy: do 500 km najwyżej 72 km, do 2 500 km najwyżej 150 km (przed 950 i 1 407 km). Jedna
dziura 200 km: 2 500 → 2 700 km. Wcześniej wypełniały ją etapy Froda (Czarna Brama, Ithilien,
Cirith Ungol), a żaden kandydat ze źródłem nie wypada na 2 550–2 650 km. Testy pilnują 150 km do
2 500 km i 200 km do 2 865 km.

### 9.4. Zmienione i usunięte id (dla HUB)

- Usunięte: km115, km215, km276, km385, km408, km628, km817, km880, km1000, km1019, km1046,
  km1150, km1400, km1480, km1530, km1790, km1800, km1935, km1940, km2080, km2220, km2540,
  km2565, km2715, km3060, km4000.
- Przemianowane (ta sama treść i próg): km1670 → km1609, km5200 → km5523.
- Nowy motyw pod starym id, z tym samym progiem co w v1: km377, km2414, km2300 (w v2 sprzed
  przebudowy km2300 miało 2 360 km, w v1 2 300 km).
- Nowe: km212, km1127, km420, km583, km640, km800, km1009, km1085, km1200, km1407, km1508,
  km1697, km1770, km1854, km2000, km2092, km2201, km2500, km2700, km3058, km3940, km4023,
  km6500, km9656.

Żadne nowe id nie było wcześniej użyte z innym progiem, więc zdobycze zapisane w `historia.db`
nie zaliczają przystanku „przed czasem” (pilnuje tego test `v2_changes_only_the_trail`). Zapisane
id, których już nie ma, `evaluate` pomija bez błędu: pętla idzie po definicjach, a nie po zapisanych
zdobyczach. Nowe przystanki liczą się z dystansu (fixtura `v2_zdobyte_w_v1`).

Plik dla serwera: `walklevel/docs/definicje-v2-serwer.json`, generowany skryptem
`walklevel/tools/definicje_serwer.py` z `docs/definicje-v2.json`. Test
`server_file_matches_app_definitions` pilnuje, że id, typy, progi, rzadkości, `poziomy_xp`
i liczbowe `zasady` są te same co w aplikacji. Typy w pliku: dystans, sesja_km, dzien_kroki, seria,
gra_pierwsza, gry_km, gra_km, trening, start_miedzy, znajomi, polecenia (bez nowych).
