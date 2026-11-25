"""Szablony maili WalkLevel (HTML + tekst) z jednego układu.

    uv run python strona/maile/zbuduj.py

Powstają:
- potwierdzenie.html / .txt - link potwierdzający adres e-mail konta,
- nowe-haslo.html / .txt - link do ustawienia nowego hasła,
- zgloszenie.html / .txt - zgłoszenie problemu z aplikacji (do autora),
- podglad/*.html - te same maile z przykładowymi danymi (do obejrzenia w przeglądarce).

Styl (rady HUB i Kabelków): mniej, nie więcej. Jasny list (ciemne maile Gmail i Outlook
odwracają po swojemu), ciemny wariant tylko dla klientów z prefers-color-scheme. Jeden
akapit, jeden przycisk jak w aplikacji, reszta zwykłymi zdaniami drobnym drukiem; bez
pastylek, ramek z paskiem z boku i dekoracji. Jeden growy akcent na mail: w potwierdzeniu
adresu „zadanie” jak popup z aplikacji.

Pola: maile konta używają pól PocketBase ({APP_URL}, {TOKEN}); zgłoszenie - pól z haka
wysyłki ({{NAZWA}}, lista w README.md). HTML: tabele, style w linii, bez skryptów.
"""

from __future__ import annotations

import base64
import html
from pathlib import Path

HERE = Path(__file__).resolve().parent
ASSETS = "https://bieznia.damnit-live.pl/maile"
LOGO_URL = f"{ASSETS}/logo.png"
BADGE_URL = f"{ASSETS}/odznaka.png"
SITE = "https://bieznia.damnit-live.pl/bieznia"
CONTACT = "kontakt@damnit.com.pl"
SLOGAN = "Chodzisz w realu, idziesz w grze"  # hasło aplikacji (też do promocji)
VERIFY_LINK = "{APP_URL}/api/bieznia/potwierdz-email?token={TOKEN}"
RESET_LINK = "{APP_URL}/api/bieznia/nowe-haslo?token={TOKEN}"

# jasny list (domyślny) i ciemny wariant w barwach aplikacji (bieznia/gui/theme.py)
OUTER, LETTER, LINE = "#f3f2ee", "#ffffff", "#e4e1d8"
TEXT, TEXT_2, TEXT_3 = "#1b1b19", "#5c5951", "#8a877e"
ACCENT, ON_ACCENT, LINK = "#8fc2b0", "#0f1f1a", "#2f7a63"
POPUP, POPUP_TEXT, POPUP_TEXT_2, COMMON = "#1b1b19", "#ecebe6", "#a5a29a", "#9aa5b1"
DARK_CSS = """
  @media (prefers-color-scheme: dark) {
    .outer { background: #0e0e0d !important; }
    .letter { background: #1b1b19 !important; }
    .t1 { color: #ecebe6 !important; }
    .t2 { color: #a5a29a !important; }
    .t3 { color: #77756e !important; }
    .ln { color: #8fc2b0 !important; }
    .rule { background: #2e2d2a !important; }
    .popup { background: #262624 !important; }
  }"""
FONT = "'Segoe UI', Helvetica, Arial, sans-serif"
MONO = "Consolas, 'Courier New', monospace"


def esc(text: str) -> str:
    return html.escape(text, quote=True)


def title(text: str) -> str:
    return (
        f'<h1 class="t1" style="margin:0 0 16px;font-family:{FONT};font-size:24px;'
        f'line-height:30px;font-weight:700;color:{TEXT};">{esc(text)}</h1>'
    )


def para(text: str) -> str:
    return (
        f'<p class="t1" style="margin:0 0 18px;font-family:{FONT};font-size:16px;'
        f'line-height:26px;color:{TEXT};">{esc(text)}</p>'
    )


def small(text: str, *, raw: bool = False, gap: int = 8) -> str:
    body = text if raw else esc(text)
    return (
        f'<p class="t2" style="margin:0 0 {gap}px;font-family:{FONT};font-size:14px;'
        f'line-height:22px;color:{TEXT_2};">{body}</p>'
    )


def button(text: str, href: str) -> str:
    """Jak przycisk główny w aplikacji: mięta, ciemny napis, zaokrąglenie 8 px."""
    return f"""
<table role="presentation" cellpadding="0" cellspacing="0" border="0" style="margin:4px 0 16px;">
  <tr><td bgcolor="{ACCENT}" style="border-radius:8px;background:{ACCENT};">
    <a href="{href}" target="_blank"
       style="display:inline-block;padding:12px 26px;font-family:{FONT};font-size:16px;
       font-weight:600;color:{ON_ACCENT};text-decoration:none;border-radius:8px;">{esc(text)}</a>
  </td></tr>
</table>"""


def fallback(href: str) -> str:
    return small("Przycisk nie działa? Skopiuj ten adres do przeglądarki:", gap=4) + (
        f'<p style="margin:0 0 8px;font-family:{MONO};font-size:12px;line-height:18px;'
        f'word-break:break-all;"><a class="ln" href="{href}" target="_blank" '
        f'style="color:{LINK};text-decoration:none;">{href}</a></p>'
    )


def rule() -> str:
    return (
        '<table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0"'
        ' style="margin:24px 0 20px;"><tr><td class="rule" height="1"'
        f' bgcolor="{LINE}" style="background:{LINE};font-size:0;line-height:0;">&nbsp;</td>'
        "</tr></table>"
    )


def quest(kicker: str, name: str, line: str) -> str:
    """Jedyny growy akcent: ciemny „popup” jak w aplikacji, z odznaką pospolitą."""
    return f"""
<table role="presentation" class="popup" cellpadding="0" cellspacing="0" border="0"
       bgcolor="{POPUP}" style="margin:0 0 26px;background:{POPUP};border-radius:10px;">
  <tr>
    <td style="padding:12px 0 12px 14px;" valign="middle">
      <img src="{BADGE_URL}" width="44" height="44" alt=""
           style="display:block;border:0;"></td>
    <td style="padding:12px 20px 12px 12px;" valign="middle">
      <div style="font-family:Bahnschrift,{FONT};font-size:11px;line-height:14px;
           letter-spacing:1.5px;color:{POPUP_TEXT_2};">{esc(kicker.upper())}</div>
      <div style="font-family:{FONT};font-size:17px;line-height:22px;font-weight:700;
           color:{POPUP_TEXT};">{esc(name)}</div>
      <div style="font-family:{FONT};font-size:12px;line-height:16px;color:{COMMON};">
           {esc(line)}</div>
    </td>
  </tr>
</table>"""


def footer(lines: list[str]) -> str:
    return "".join(
        f'<p class="t3" style="margin:0 0 6px;font-family:{FONT};font-size:12px;'
        f'line-height:18px;color:{TEXT_3};">{line}</p>'
        for line in lines
    )


def layout(page_title: str, preheader: str, body: str, foot: str) -> str:
    """Szkielet: jasne tło, biały list 560 px wyrównany do lewej (logo, treść), drobna stopka
    pod listem. Bez ramek i cieni."""
    return f"""<!DOCTYPE html>
<html lang="pl" xmlns="http://www.w3.org/1999/xhtml">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="color-scheme" content="light dark">
<meta name="supported-color-schemes" content="light dark">
<title>{esc(page_title)}</title>
<style>
  @media (max-width: 600px) {{
    .wrap {{ width: 100% !important; }}
    .pad {{ padding-left: 24px !important; padding-right: 24px !important; }}
  }}{DARK_CSS}
</style>
</head>
<body class="outer" style="margin:0;padding:0;background:{OUTER};">
<div style="display:none;max-height:0;overflow:hidden;opacity:0;">
  {esc(preheader)}&#8199;&#65279;&#847;&#8199;&#65279;&#847;&#8199;&#65279;&#847;</div>
<table role="presentation" class="outer" width="100%" cellpadding="0" cellspacing="0"
       border="0" bgcolor="{OUTER}" style="background:{OUTER};">
  <tr><td align="center" style="padding:36px 12px 28px;">
    <table role="presentation" class="wrap letter" width="560" cellpadding="0" cellspacing="0"
           border="0" bgcolor="{LETTER}"
           style="width:560px;max-width:560px;background:{LETTER};border-radius:12px;">
      <tr><td class="pad" align="left" style="padding:28px 40px 0;">
        <table role="presentation" cellpadding="0" cellspacing="0" border="0">
          <tr>
            <td valign="middle"><img src="{LOGO_URL}" width="36" height="36" alt=""
                style="display:block;border:0;border-radius:7px;"></td>
            <td valign="middle" style="padding-left:10px;">
              <div class="t1" style="font-family:{FONT};font-size:17px;line-height:20px;
                   font-weight:700;color:{TEXT};">WalkLevel</div>
              <div class="t3" style="font-family:Bahnschrift,{FONT};font-size:11px;
                   line-height:15px;letter-spacing:1.5px;color:{TEXT_3};">{esc(SLOGAN.upper())}</div>
            </td>
          </tr>
        </table>
      </td></tr>
      <tr><td class="pad" align="left" style="padding:28px 40px 32px;">{body}</td></tr>
    </table>
    <table role="presentation" class="wrap" width="560" cellpadding="0" cellspacing="0"
           border="0" style="width:560px;max-width:560px;">
      <tr><td class="pad" align="left" style="padding:18px 40px 0;">{foot}</td></tr>
    </table>
  </td></tr>
</table>
</body>
</html>
"""


WHY = "Dostajesz tę wiadomość, bo ten adres podano przy koncie w aplikacji WalkLevel."


def user_footer() -> str:
    a = f'style="color:{TEXT_3};text-decoration:underline;"'
    return footer(
        [
            esc(WHY),
            f'Pytania: <a class="t3" href="mailto:{CONTACT}" {a}>{CONTACT}</a>'
            f' &nbsp;·&nbsp; <a class="t3" href="{SITE}" {a}>Regulamin i prywatność</a>',
            "WalkLevel · damnit.com.pl",
        ]
    )


def user_text_footer() -> str:
    return f"""--
{WHY}
Pytania: {CONTACT} · Regulamin i prywatność: {SITE}
WalkLevel · damnit.com.pl
"""


# --- maile -------------------------------------------------------------------------


def verification() -> tuple[str, str, str]:
    subject = "Potwierdź adres e-mail w WalkLevel"
    intro = (
        "Ten adres podano przy koncie online w aplikacji WalkLevel. Zostało jedno kliknięcie."
        " Potem odzyskasz przez niego konto, gdyby hasło kiedyś wypadło z głowy."
    )
    body = (
        quest("Zadanie", "Pierwszy krok", "Potwierdź adres e-mail · jedno kliknięcie")
        + title("Potwierdź adres e-mail")
        + para(intro)
        + button("Potwierdzam adres", VERIFY_LINK)
        + small("Link działa przez 3 dni.")
        + small(
            "Jeśli to nie Ty, zignoruj tę wiadomość. Bez kliknięcia adres nie zostanie"
            " potwierdzony."
        )
        + rule()
        + fallback(VERIFY_LINK)
        + small("Do zobaczenia na bieżni.", gap=0)
    )
    text = f"""Potwierdź adres e-mail

{intro}

Potwierdzam adres: {VERIFY_LINK}

Link działa przez 3 dni.
Jeśli to nie Ty, zignoruj tę wiadomość. Bez kliknięcia adres nie zostanie potwierdzony.

Do zobaczenia na bieżni.

{user_text_footer()}"""
    page = layout(subject, "Zostało jedno kliknięcie.", body, user_footer())
    return subject, page, text


def new_password() -> tuple[str, str, str]:
    subject = "Twój link do nowego hasła w WalkLevel"
    intro = (
        "Ktoś, pewnie Ty, poprosił o nowe hasło do konta WalkLevel powiązanego z tym"
        " adresem. Poziom, osiągnięcia i historia zostają tam, gdzie były."
    )
    body = (
        title("Ustaw nowe hasło")
        + para(intro)
        + button("Ustaw nowe hasło", RESET_LINK)
        + small(
            "Link działa przez 30 minut i tylko raz. Po zmianie hasła inne urządzenia"
            " zostaną wylogowane."
        )
        + small("Jeśli to nie Ty, nic nie rób. Hasło zostaje stare, a link sam wygaśnie.")
        + rule()
        + fallback(RESET_LINK)
    )
    text = f"""Ustaw nowe hasło

{intro}

Ustaw nowe hasło: {RESET_LINK}

Link działa przez 30 minut i tylko raz. Po zmianie hasła inne urządzenia zostaną wylogowane.
Jeśli to nie Ty, nic nie rób. Hasło zostaje stare, a link sam wygaśnie.

{user_text_footer()}"""
    page = layout(subject, "Link działa przez 30 minut.", body, user_footer())
    return subject, page, text


def report() -> tuple[str, str, str]:
    """Do autora: czytelność zamiast wizytówki - fakty w tabelce, opis, ostrzeżenie."""
    subject = "[WalkLevel] Zgłoszenie {{NUMER}} · wersja {{WERSJA}}"

    def row(name: str, value: str) -> str:
        return f"""
<tr>
  <td class="t2" width="110" valign="top" style="padding:7px 0;font-family:{FONT};
      font-size:14px;color:{TEXT_2};">{esc(name)}</td>
  <td class="t1" valign="top" style="padding:7px 0;font-family:{FONT};font-size:15px;
      color:{TEXT};">{value}</td>
</tr>"""

    facts = (
        '<table role="presentation" width="100%" cellpadding="0" cellspacing="0" border="0"'
        ' style="margin:0 0 8px;">'
        + row("Numer", f'<span style="font-family:{MONO};">{{{{NUMER}}}}</span>')
        + row("Wersja", "{{WERSJA}}")
        + row("Kontakt", "{{KONTAKT}}")
        + row("Logi", "{{OKRES}}")
        + row("Paczka", "{{ROZMIAR}}, w załączniku")
        + row("Wysłano", "{{CZAS}}")
        + "</table>"
    )
    body = (
        title("Nowe zgłoszenie")
        + facts
        + rule()
        + f'<p class="t1" style="margin:0 0 20px;font-family:{FONT};font-size:16px;'
        f'line-height:26px;color:{TEXT};white-space:pre-wrap;">{{{{OPIS}}}}</p>'
        + small(
            "Opis i paczka pochodzą od użytkownika, otwieraj załącznik ostrożnie. Serwer nie"
            " przechowuje ani treści zgłoszenia, ani paczki.",
            gap=0,
        )
    )
    foot = footer(["Wysłane z formularza „Zgłoś problem” w aplikacji WalkLevel."])
    text = """Nowe zgłoszenie z aplikacji WalkLevel

Numer:    {{NUMER}}
Wersja:   {{WERSJA}}
Kontakt:  {{KONTAKT}}
Logi:     {{OKRES}}
Paczka:   {{ROZMIAR}}, w załączniku
Wysłano:  {{CZAS}}

{{OPIS}}

--
Opis i paczka pochodzą od użytkownika, otwieraj załącznik ostrożnie.
Serwer nie przechowuje ani treści zgłoszenia, ani paczki.
"""
    page = layout("Zgłoszenie {{NUMER}}", "Nowe zgłoszenie z aplikacji WalkLevel.", body, foot)
    return subject, page, text


SAMPLE = {
    "{APP_URL}": "https://bieznia.damnit-live.pl",
    "{TOKEN}": "przykladowy-token-1234567890",
    "{{NUMER}}": "chtcy4tg3vng59y",
    "{{WERSJA}}": "0.5.0",
    "{{KONTAKT}}": "nie podano",
    "{{OKRES}}": "ostatnie 6 godzin",
    "{{ROZMIAR}}": "48 KB",
    "{{CZAS}}": "25.11.2026, 22:41",
    "{{OPIS}}": "Bieżnia rozłącza się po 2 minutach.\nPo ponownym połączeniu działa dalej.",
}


def data_uri(name: str) -> str:
    return "data:image/png;base64," + base64.b64encode((HERE / name).read_bytes()).decode()


def fill(text: str) -> str:
    """Podgląd bez serwera: przykładowe wartości i obrazki osadzone w pliku."""
    for key, value in SAMPLE.items():
        text = text.replace(key, esc(value))
    text = text.replace(LOGO_URL, data_uri("logo.png"))
    return text.replace(BADGE_URL, data_uri("odznaka.png"))


if __name__ == "__main__":
    (HERE / "podglad").mkdir(exist_ok=True)
    for name, build in (
        ("potwierdzenie", verification),
        ("nowe-haslo", new_password),
        ("zgloszenie", report),
    ):
        subject, page, text = build()
        # LF niezależnie od systemu - sumy kontrolne zgadzają się po przesłaniu treści
        (HERE / f"{name}.html").write_text(page, encoding="utf-8", newline="\n")
        (HERE / f"{name}.txt").write_text(
            f"Temat: {subject}\n\n{text}", encoding="utf-8", newline="\n"
        )
        (HERE / "podglad" / f"{name}.html").write_text(fill(page), encoding="utf-8")
        # podgląd tego, co widzi czytelnik z ciemnym motywem (klient z prefers-color-scheme)
        dark = fill(page).replace("@media (prefers-color-scheme: dark) {", "@media all {")
        (HERE / "podglad" / f"{name}-ciemny.html").write_text(dark, encoding="utf-8")
        kb = len(page.encode()) / 1024
        print(f"{name}: {subject} ({kb:.1f} KB)")
