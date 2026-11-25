# Polityka prywatności aplikacji WalkLevel (dawniej Bieżnia)

Administratorem danych jestem ja, Damian Skrzypiec - prowadzę damnit.com.pl prywatnie.
We wszystkich sprawach dotyczących danych pisz na kontakt@damnit.com.pl.

## W skrócie

WalkLevel działa na Twoim komputerze. Nie ma reklam ani analityki. Dane z bieżni, ustawienia,
historia kroków, poziom, osiągnięcia i logi zostają u Ciebie. Konto online jest dobrowolne: bez niego nic nie
wychodzi z komputera poza sprawdzaniem aktualizacji. Do mnie trafia coś tylko wtedy, gdy
założysz konto online albo sam wyślesz zgłoszenie problemu.

## Co zostaje na Twoim komputerze

W folderze `%LOCALAPPDATA%\Bieznia`:

- osoby i profile gier, ustawienia aplikacji (progi prędkości, motyw, zapamiętana bieżnia -
  jej nazwa i adres Bluetooth),
- historia kroków: sesje chodzenia (czas, kroki, dystans, model bieżni),
- przy koncie online: pseudonim i e-mail konta oraz to, które sesje już wysłano; sam token
  logowania - w Menedżerze poświadczeń Windows (tylko przy „Zapamiętaj logowanie”),
- dziennik zdarzeń: start aplikacji, błędy, przebieg połączenia z bieżnią,
- szczegółowe logi Bluetooth, jeśli są włączone (Ustawienia > Dziennik i zgłoszenia):
  surowa rozmowa z bieżnią, w tym prędkość i liczba kroków,
- raport diagnostyczny, jeśli go zapiszesz: lista urządzeń Bluetooth w pobliżu (nazwy
  i adresy) i rozmowa z bieżnią,
- nagrania przejść, jeśli włączysz nagrywanie.

Aplikacja nie zapisuje wciskanych klawiszy (sprawdza tylko klawisze sterowania), haseł ani
niczego z gier. Wszystko usuniesz, kasując ten folder (token konta usuniesz, wylogowując się).

## Kiedy aplikacja łączy się z internetem

**Sprawdzanie i pobieranie aktualizacji.** Przy starcie i po kliknięciu Aktualizuj
aplikacja pyta serwis GitHub o najnowszą wersję, a po kliknięciu Aktualizuj pobiera
z GitHuba instalator. GitHub widzi wtedy adres IP Twojego komputera i wersję aplikacji.
Ja nie dostaję z tego żadnych danych. Cel: dostarczanie poprawek. Podstawa: mój prawnie
uzasadniony interes - utrzymanie działającej i bezpiecznej aplikacji (art. 6 ust. 1 lit. f
RODO).

**Profile modeli bieżni.** Po kliknięciu „Sprawdź nowe modele” (Ustawienia > Obsługiwane
bieżnie) aplikacja pobiera z GitHuba listę i pliki profili bieżni (same dane: zakresy
prędkości, nazwy modeli). GitHub widzi wtedy adres IP Twojego komputera. Podstawa jak przy
aktualizacjach.

**Konto online (opcjonalne).** Gdy założysz konto albo się zalogujesz, aplikacja łączy się
z serwerem bieznia.damnit-live.pl i wysyła tam dane konta oraz sesje chodzenia osoby
przypiętej do konta. Co dokładnie, po co, kto to widzi i jak długo jest przechowywane -
opisuje [Regulamin i prywatność konta online](https://bieznia.damnit-live.pl/bieznia).
Bez konta aplikacja nie łączy się z tym serwerem.

**Zgłoszenie problemu.** Gdy klikniesz Wyślij zgłoszenie (albo wyślesz mi e-mail
z przygotowaną paczką), dostaję: Twój opis, kontakt (jeśli go podasz), logi aplikacji
i Bluetooth, ostatni raport diagnostyczny, ustawienia aplikacji, wersję aplikacji i Windows.
Okno zgłoszenia pokazuje to przed wysłaniem. Z logów i ustawień aplikacja usuwa ścieżki
z nazwą użytkownika Windows i adresy innych urządzeń Bluetooth niż wybrana bieżnia.
Zgłoszenie trafia przez serwer bieznia.damnit-live.pl (OVHcloud) prosto na moją skrzynkę
kontakt@damnit.com.pl; serwer nie zachowuje treści ani paczki, tylko numer zgłoszenia,
czas, wersję aplikacji i rozmiar przez 30 dni (ochrona przed nadużyciami). Możesz też
zapisać paczkę na dysk i wysłać ją sam. Cel: znalezienie i naprawienie błędu.
Podstawa: mój prawnie uzasadniony interes - pomoc w rozwiązaniu zgłoszonego problemu
(art. 6 ust. 1 lit. f RODO). Zgłoszenie przechowuję do czasu rozwiązania problemu, a potem
je usuwam.

## Komu przekazuję dane

- **GitHub, Inc.** - obsługuje sprawdzanie i pobieranie aktualizacji oraz profili bieżni,
  zgodnie ze swoją
  [polityką prywatności](https://docs.github.com/site-policy/privacy-policies/github-general-privacy-statement).
- Konto online: serwer w OVHcloud (Polska) i zaszyfrowane kopie zapasowe - szczegóły
  w [regulaminie konta](https://bieznia.damnit-live.pl/bieznia).

Zgłoszeń nie przekazuję nikomu.

## Twoje prawa

Możesz poprosić o dostęp do swoich danych, ich poprawienie, usunięcie, ograniczenie albo
przeniesienie. Możesz też sprzeciwić się temu, co robię na podstawie uzasadnionego
interesu. Wystarczy napisać na kontakt@damnit.com.pl.

Jeśli uznasz, że przetwarzam Twoje dane niezgodnie z prawem, możesz złożyć skargę do
Prezesa Urzędu Ochrony Danych Osobowych (ul. Stawki 2, 00-193 Warszawa).

## Zmiany

Jeśli aplikacja zacznie wysyłać jakiekolwiek inne dane albo korzystać z narzędzia obcej
firmy, opiszę to tutaj, zanim to włączę.

Obowiązuje od: 2026-11-25 (dodane: konto online, profile modeli bieżni)
