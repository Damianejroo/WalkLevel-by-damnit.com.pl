"""Ustawienia: osoby, profile gier, sterowanie tylko dla trybu profilu, wygląd, bieżnia, kopia,
zwinięte Zaawansowane. Karta sterowania przebudowuje się przy zmianie trybu i profilu."""

from __future__ import annotations

import contextlib
import json
import logging
import time
from pathlib import Path
from typing import TYPE_CHECKING, Any

from PySide6.QtCore import Qt, QUrl
from PySide6.QtGui import QDesktopServices
from PySide6.QtWidgets import QHBoxLayout, QPushButton, QVBoxLayout, QWidget

from bieznia import HOMEPAGE, REPOSITORY, __author__, kopia
from bieznia import pad as gamepad
from bieznia.ble import kingsmith
from bieznia.gui import komponenty as k
from bieznia.gui.widgets import (
    HoldButton,
    KeyButton,
    ResponsiveRow,
    Segmented,
    Switch,
    button,
    card,
    fmt_num,
    label,
)
from bieznia.mapping import MappingConfig
from bieznia.paths import FROZEN

if TYPE_CHECKING:
    from bieznia.gui.app import App

log = logging.getLogger("bieznia")
HYSTERESIS = 0.6
# zmieniają się w tle, a w Ustawieniach albo ich nie widać, albo odświeżają się na żywo
NOT_IN_SETTINGS_VIEW = frozenset(
    {"last_version", "window_geometry", "floating_pos", "device_address", "device_name"}
)
MODE_NAMES = {"keyboard": "Klawisze", "gamepad": "Pad", "treadmill": "Chodzenie"}


def section(title: str) -> tuple[QWidget, QVBoxLayout]:
    c = card()
    lay = QVBoxLayout(c)
    lay.setContentsMargins(14, 12, 14, 14)
    lay.setSpacing(8)
    lay.addWidget(label(title, "section"))
    return c, lay


def clear(lay: Any) -> None:
    while (item := lay.takeAt(0)) is not None:
        if (w := item.widget()) is not None:
            w.hide()
            w.deleteLater()
        elif (sub := item.layout()) is not None:
            clear(sub)


class SettingsScreen:
    belt_target: float | None
    settings_view: str  # tryb, którego ustawienia widać na karcie (nie musi być aktywny)
    settings_built: str  # stan, z którego zbudowano widok Ustawień

    def open_settings(self: App) -> None:
        self.close_popups()
        self.settings_view = self.settings["direction"]
        self.refresh_settings()
        self.pages.go(self.SETTINGS)

    def settings_stamp(self: App) -> str:
        """Wszystko, co widać w Ustawieniach - gdy się nie zmieniło, widok nie jest budowany od
        nowa (wejście bez opóźnienia). Kroki „dziś” odświeżają się co minutę."""
        store = self.store
        return json.dumps(
            [
                {k: v for k, v in self.settings.items() if k not in NOT_IN_SETTINGS_VIEW},
                [
                    [p.id, p.name, p.color, p.active_profile]
                    + [
                        [
                            q.id,
                            q.name,
                            q.color,
                            q.glyph,
                            q.icon_file,
                            self.profile_summary(q.settings),
                        ]
                        for q in p.profiles
                    ]
                    for p in store.people
                ]
                if store
                else None,
                store.active_person if store else None,
                self.t.name,
                self.t.accent,
                self.advanced_open,
                self.settings_view,
                str(kopia.latest_backup()),
                int(time.time() // 60),
            ],
            sort_keys=True,
            default=str,
        )

    def refresh_settings(self: App) -> None:
        if getattr(self, "settings_built", "") != self.settings_stamp():
            self.rebuild_settings()

    def rebuild_settings(self: App) -> None:
        self.replace_page(self.SETTINGS, self.build_settings())
        self.settings_built = self.settings_stamp()

    def build_settings(self: App) -> QWidget:
        t = self.t
        page, lay = self.sticky_page("Ustawienia")
        self.profile_rows: dict[str, k.ChoiceRow] = {}
        store = self.store

        if store is not None:
            c, cl = section("Osoby")
            cl.addWidget(
                label("Każda osoba ma swoje profile gier, ustawienia i historię kroków.", "small")
            )
            for p in store.people:
                today = self.history_today(p.id) if self.history else 0
                row = k.ChoiceRow(
                    t,
                    k.avatar_pixmap(t, 38, p.name, p.color),
                    38,
                    p.name,
                    f"{len(p.profiles)} prof. gier · dziś {fmt_num(today, 0)} kroków",
                    p.id == store.person.id,
                    t.accent,
                )
                row.clicked.connect(lambda _=False, pid=p.id: self.choose_person(pid))
                row.edit.connect(lambda pid=p.id: self.open_person_edit(pid))
                cl.addWidget(row)
            add = k.AddRow(t, "Dodaj osobę")
            add.clicked.connect(self.open_wizard_add)
            cl.addWidget(add)
            lay.addWidget(c)

            c, cl = section(f"Profile gier · {store.person.name}")
            cl.addWidget(
                label("Ustawienia sterowania i kolor zapisują się w zaznaczonym profilu.", "small")
            )
            for pr in store.person.profiles:
                row = k.ChoiceRow(
                    t,
                    self.profile_pixmap(pr, 38),
                    38,
                    pr.name,
                    self.profile_summary(pr.settings),
                    pr.id == store.profile.id,
                    t.accent,
                )
                row.clicked.connect(lambda _=False, pid=pr.id: self.choose_profile(pid))
                row.edit.connect(lambda pid=pr.id: self.open_profile_edit(pid))
                self.profile_rows[pr.id] = row
                cl.addWidget(row)
            add = k.AddRow(t, "Nowy profil")
            add.clicked.connect(self.open_profile_new)
            cl.addWidget(add)
            lay.addWidget(c)

        title = "Sterowanie"
        if store is not None:
            title += f" · {store.profile.name}"
        c, cl = section(title)
        # przełącznik trybów stoi nad przebudowywaną częścią - podświetlenie płynnie jedzie
        self.mode_view = Segmented(list(MODE_NAMES.items()), self.settings_view, "Ustawienia trybu")
        self.mode_view.changed.connect(self.show_mode_settings)
        cl.addWidget(self.mode_view)
        self.control_body = k.SmoothBox()  # zmiana trybu: karta płynnie rośnie albo maleje
        self.control_box = self.control_body.lay
        cl.addWidget(self.control_body)
        lay.addWidget(c)
        self.build_control_settings()

        c, wl = section("Wygląd i okno")
        if store is not None:
            wl.addWidget(label(f"Kolor · zapisuje się w profilu {store.profile.name}"))
            self.palette = k.PalettePicker(t, store.profile.color, store.profile.icon_color)
            self.palette.picked.connect(self.on_profile_color)
            wl.addWidget(self.palette)
        theme_seg = Segmented(
            [("system", "Jak Windows"), ("light", "Jasny"), ("dark", "Ciemny")],
            self.settings["theme"],
            "Motyw",
        )
        theme_seg.changed.connect(lambda v: self.on_look("theme", v))
        wl.addWidget(theme_seg)
        for key, title_, hint, handler in (
            (
                "large_text",
                "Duży tekst",
                "Wszystkie napisy większe o 15%",
                lambda v: self.on_look("large_text", v),
            ),
            (
                "always_on_top",
                "Zawsze na wierzchu",
                "Okno nie chowa się pod grą",
                self.on_always_on_top,
            ),
            (
                "floating_button",
                "Przycisk na ekranie",
                "Klik włącza i wyłącza sterowanie, prawy przycisk otwiera okno",
                self.on_floating_button,
            ),
            (
                "show_person_switch",
                "Zmiana osoby na pasku",
                "Awatar z listą osób (przy jednej osobie i tak go nie ma)",
                lambda v: self.on_switch_visibility("show_person_switch", v),
            ),
            (
                "show_profile_switch",
                "Zmiana profilu na karcie sterowania",
                "Lista profili po kliknięciu nazwy (przy jednym profilu i tak jej nie ma)",
                lambda v: self.on_switch_visibility("show_profile_switch", v),
            ),
        ):
            sw = Switch(t, title_, bool(self.settings.get(key, False)))
            sw.toggled.connect(handler)
            self.setting_row(wl, title_, hint, sw)
        lay.addWidget(c)

        c, bl = section("Bieżnia")
        self.device_label = label("", "muted")
        bl.addWidget(self.device_label)
        self.toggle_btn = KeyButton("Start/stop", self.toggle_key, combos=True)
        self.toggle_btn.captured.connect(lambda key: self.on_key("toggle", key))
        bl.addWidget(label("Klawisz włączania sterowania"))
        bl.addWidget(
            label(
                "Kliknij i naciśnij klawisz albo kombinację, np. Ctrl+Shift+F8. Działa też w grze."
                " Bez skrótów Windows (Win, Alt+Tab, Alt+F4) i nakładek (Shift+Tab, Alt+Z).",
                "small",
            )
        )
        bl.addWidget(self.toggle_btn)
        self.belt_switch = Switch(t, "Start i stop bieżni", self.settings["belt_control"])
        self.belt_switch.toggled.connect(self.on_belt_control)
        self.setting_row(
            bl,
            "Klawisz uruchamia i zatrzymuje taśmę",
            "Wyłącz, jeśli wolisz startować tylko pilotem",
            self.belt_switch,
        )
        self.forget_btn = HoldButton("Zapomnij bieżnię", t)
        self.forget_btn.confirmed.connect(self.forget_device)
        bl.addWidget(self.forget_btn)
        lay.addWidget(c)

        if store is not None:
            c, kl = section("Kopia danych")
            kl.addWidget(
                label(
                    "Przenieś osoby, profile gier i wyniki na inny komputer albo zrób kopię.",
                    "small",
                )
            )
            export_btn = button("Eksportuj…")
            export_btn.clicked.connect(self.open_export)
            import_btn = button("Importuj…")
            import_btn.clicked.connect(self.open_import)
            kl.addWidget(ResponsiveRow(export_btn, import_btn))
            backup = kopia.latest_backup()
            if backup is not None:
                restore = HoldButton("Przywróć dane sprzed ostatniego importu", t)
                restore.confirmed.connect(lambda b=backup: self.restore_backup(b))
                kl.addWidget(restore)
            lay.addWidget(c)

        adv_btn = QPushButton()
        adv_btn.setObjectName("row")
        adv_btn.setFixedHeight(60)
        adv_btn.setCursor(Qt.CursorShape.PointingHandCursor)
        adv_btn.setAccessibleName("Zaawansowane")
        al = QHBoxLayout(adv_btn)
        al.setContentsMargins(14, 6, 14, 6)
        texts = QVBoxLayout()
        texts.setSpacing(0)
        name = label("Zaawansowane")
        name.setStyleSheet("font-weight: 600;")
        texts.addWidget(name)
        texts.addWidget(label("Symulator, nagrywanie, logi, dziennik, aktualizacje", "small"))
        al.addLayout(texts, 1)
        chevron = k.glyph(k.G_CHEVRON_DOWN, t.text_2)
        al.addWidget(chevron)
        for w in (name, chevron):
            w.setAttribute(Qt.WidgetAttribute.WA_TransparentForMouseEvents)
        lay.addWidget(adv_btn)
        adv, advl = section("Zaawansowane")
        if not self.advanced_open:
            adv.hide()  # widoczność dopiero po włożeniu do układu
        adv_btn.clicked.connect(lambda: self.toggle_advanced(adv))
        sim = Switch(t, "Symulator bieżni", self.settings["simulator"])
        sim.toggled.connect(self.on_simulator)
        keys = self.config["hotkeys"]
        self.setting_row(
            advl,
            "Symulator bieżni",
            f"Bez bieżni: {keys['sim_slower']} wolniej, {keys['sim_faster']} szybciej",
            sim,
        )
        rec = Switch(t, "Nagrywaj przejścia", self.settings["record"])
        rec.toggled.connect(self.on_record)
        self.setting_row(advl, "Nagrywaj przejścia", "Odczyty trafiają do folderu nagrania", rec)
        detailed = Switch(t, "Szczegółowe logi Bluetooth", self.settings["detailed_logs"])
        detailed.toggled.connect(self.on_detailed_logs)
        self.setting_row(
            advl,
            "Szczegółowe logi Bluetooth",
            "Rozmowa z bieżnią w logach - pomaga naprawić połączenie."
            " Bez klawiszy i danych z gier.",
            detailed,
        )
        journal = button("Dziennik")
        journal.clicked.connect(lambda: self.pages.go(self.LOG))
        folder = button("Folder logów")
        folder.clicked.connect(lambda: self.open_folder(Path("logs")))
        advl.addWidget(ResponsiveRow(journal, folder))
        self.about_status = label("", "muted")
        self.about_status.setWordWrap(True)
        advl.addWidget(self.about_status)
        check = button("Sprawdź aktualizacje")
        check.clicked.connect(lambda: self.run_task(self.check_update_task, True))
        check.setEnabled(FROZEN)
        advl.addWidget(check)
        lay.addWidget(adv)

        links = []
        for text, url in (
            ("Strona autora", HOMEPAGE),
            ("Prywatność", f"https://github.com/{REPOSITORY}/blob/main/POLITYKA-PRYWATNOSCI.md"),
            ("Kod", f"https://github.com/{REPOSITORY}"),
        ):
            links.append(f"<a style='color:{t.accent}' href='{url}'>{text}</a>")
        about = label(f"Bieżnia Gamingowa · © 2026 {__author__}<br>" + " · ".join(links), "small")
        about.setAlignment(Qt.AlignmentFlag.AlignCenter)
        about.setOpenExternalLinks(False)
        about.linkActivated.connect(lambda url: QDesktopServices.openUrl(QUrl(url)))
        lay.addWidget(about)
        lay.addStretch(1)
        self.render_device_label()
        return page

    def toggle_advanced(self: App, box: QWidget) -> None:
        self.advanced_open = not self.advanced_open
        box.setVisible(self.advanced_open)

    def profile_summary(self: App, s: dict[str, Any]) -> str:
        merged = {**self.defaults, **s}
        direction = merged.get("direction", "keyboard")
        if direction == "gamepad":
            steps = gamepad.parse_steps(merged.get("pad_steps"))
            return f"Pad · {len(steps)} skoki · do {fmt_num(max(sp for _a, sp in steps))} km/h"
        if direction == "keyboard":
            return (
                f"Klawisze · {fmt_num(float(merged['walk_speed']))} /"
                f" {fmt_num(float(merged['run_speed']))} km/h"
            )
        return f"Chodzenie · chód od {fmt_num(float(merged['stand_below']))} km/h"

    # --- karta sterowania (tylko bieżący tryb) ---------------------------------------------

    def build_control_settings(self: App) -> None:
        box = self.control_box
        while box.count():
            item = box.takeAt(0)
            if item is None:
                break
            if (w := item.widget()) is not None:
                w.hide()
                w.deleteLater()
            elif (inner := item.layout()) is not None:
                clear(inner)
        t, s = self.t, self.settings
        active = s["direction"]
        if getattr(self, "settings_view", None) not in MODE_NAMES:
            self.settings_view = active
        direction = self.settings_view
        self.render_mode_view()
        if direction == active:
            box.addWidget(
                label("✓ to aktywny tryb. Wybierz inny, żeby ustawić go zawczasu.", "small")
            )
        else:
            row = QHBoxLayout()
            row.addWidget(
                label(
                    f"Aktywny jest tryb {MODE_NAMES[active]} - te ustawienia zadziałają po"
                    " zmianie trybu.",
                    "small",
                ),
                1,
            )
            use = button(f"Włącz {MODE_NAMES[direction]}")
            use.clicked.connect(lambda: self.on_direction(direction))
            row.addWidget(use)
            box.addLayout(row)
        self.walk_btn = self.run_btn = None
        self.walk_from_step = self.run_from_step = None
        self.step_ladder = None
        self.pad_steppers = []
        if direction in ("treadmill", "keyboard"):
            walk, run = self.bridge.keys
            self.walk_btn = KeyButton("Chód", walk)
            self.run_btn = KeyButton("Bieg", run)
            self.walk_btn.captured.connect(lambda key: self.on_key("walk", key))
            self.run_btn.captured.connect(lambda key: self.on_key("run", key))
            box.addWidget(ResponsiveRow(self.walk_btn, self.run_btn))
        if direction == "treadmill":
            self.output_seg = Segmented(
                [("dry", "Podgląd"), ("keys", "Klawiatura")], s["output"], "Wyjście"
            )
            self.output_seg.changed.connect(self.on_output)
            box.addWidget(self.output_seg)
            top = float(self.config["mapping"]["max_speed"])
            self.walk_from_step = k.Stepper(t, "Chód od", "", float(s["stand_below"]), 0.0, top)
            self.run_from_step = k.Stepper(t, "Bieg od", "", float(s["run_on"]), 0.0, top)
            self.walk_from_step.changed.connect(lambda v: self.on_threshold("walk", v))
            self.run_from_step.changed.connect(lambda v: self.on_threshold("run", v))
            box.addWidget(self.walk_from_step)
            box.addWidget(self.run_from_step)
            box.addWidget(
                label(
                    f"Bieg wyłącza się {fmt_num(HYSTERESIS)} km/h niżej, żeby Shift nie migał.",
                    "small",
                )
            )
            quick = Switch(t, "Szybkie zatrzymanie", s["quick_stop"])
            quick.toggled.connect(self.on_quick_stop)
            self.setting_row(
                box, "Szybkie zatrzymanie postaci", "Gdy taśma gwałtownie hamuje", quick
            )
        elif direction == "keyboard":
            walk, run = self.bridge.keys
            for key, title, sub in (
                ("idle_speed", "Bez klawisza", "taśma zwalnia do"),
                ("walk_speed", walk.name, "chód"),
                ("walk_speed_run", f"{run.name} + {walk.name}", "bieg"),
            ):
                real = "run_speed" if key == "walk_speed_run" else key
                st = k.Stepper(
                    t, title, sub, float(s[real]), kingsmith.SPEED_MIN, kingsmith.SPEED_MAX
                )
                st.changed.connect(lambda v, r=real: self.on_belt_speed(r, v))
                box.addWidget(st)
        else:
            self.pad_status = label("", "small")
            self.pad_status.setWordWrap(True)
            box.addWidget(self.pad_status)
            box.addWidget(
                label(
                    "Przeciągnij kółka, żeby zmienić, od jakiego wychylenia działa skok."
                    " Trójkąt to gałka teraz.",
                    "small",
                )
            )
            self.step_ladder = k.StepLadder(t, gamepad.MIN_GAP)
            self.step_ladder.moved.connect(self.on_pad_at)
            box.addWidget(self.step_ladder)
            row = QHBoxLayout()
            row.addWidget(label("Liczba skoków"), 1)
            self.pad_count_seg = Segmented(
                [(str(n), str(n)) for n in range(2, 6)], str(len(s["pad_steps"])), "Liczba skoków"
            )
            self.pad_count_seg.setFixedWidth(220)
            self.pad_count_seg.changed.connect(lambda v: self.on_pad_count(int(v)))
            row.addWidget(self.pad_count_seg)
            box.addLayout(row)
            box.addWidget(label("Gałka puszczona", "small"))
            self.pad_idle_seg = Segmented(
                [("stop", "Taśma staje"), ("slow", "Skok 1"), ("zero", "0 km/h (test)")],
                s["pad_idle"],
                "Gałka puszczona",
            )
            self.pad_idle_seg.changed.connect(self.on_pad_idle)
            box.addWidget(self.pad_idle_seg)
            idle = self.pad_idle_text()
            for i, (at, speed) in enumerate(s["pad_steps"]):
                title = "Gałka puszczona" if i == 0 else f"Skok {i + 1}"
                sub = "" if i == 0 else f"od {at}% wychylenia"
                fmt = (lambda _v, txt=idle: txt) if (i == 0 and idle) else None
                st = k.Stepper(
                    t, title, sub, float(speed), kingsmith.SPEED_MIN, kingsmith.SPEED_MAX
                )
                st.set_format(fmt)
                st.changed.connect(lambda v, n=i: self.on_pad_speed(n, v))
                box.addWidget(st)
                self.pad_steppers.append(st)
            self.render_ladder()
        self.render_output_hint()

    def settings_direction_changed(self: App) -> None:
        """Tryb zmieniony, gdy Ustawienia są na ekranie: karta sterowania płynnie, opis profilu
        w miejscu - bez przebudowy całego widoku."""
        store = self.store
        if store is not None and store.profile.id in self.profile_rows:
            with contextlib.suppress(RuntimeError):
                self.profile_rows[store.profile.id].sub.setText(
                    self.profile_summary(store.profile.settings)
                )
        self.settings_view = self.settings["direction"]
        self.control_body.swap(self.build_control_settings, self.t.surface)
        if self.pages.currentIndex() == self.SETTINGS:
            # widoczny widok jest aktualny; ukryty mógł być nieaktualny już wcześniej -
            # wtedy przebuduje się przy wejściu
            self.settings_built = self.settings_stamp()

    def render_mode_view(self: App) -> None:
        """Znaczek ✓ przy aktywnym trybie i zaznaczony tryb, którego ustawienia widać."""
        active = self.settings["direction"]
        with contextlib.suppress(RuntimeError):
            for key, b in self.mode_view.buttons.items():
                text = MODE_NAMES[key] + (" ✓" if key == active else "")
                if b.text() != text:
                    b.setText(text)
            self.mode_view.set_value(self.settings_view)

    def show_mode_settings(self: App, mode: str) -> None:
        """Podgląd i zmiana ustawień innego trybu - aktywny tryb zostaje."""
        self.settings_view = mode
        self.control_body.swap(self.build_control_settings, self.t.surface)

    def render_ladder(self: App) -> None:
        if self.step_ladder is None:
            return
        s, t = self.settings, self.t
        idle = self.pad_idle_text()
        steps = [
            (int(at), idle if (i == 0 and idle) else fmt_num(float(sp)))
            for i, (at, sp) in enumerate(s["pad_steps"])
        ]
        n = len(steps)
        colors = [t.text_2] + [t.accent] * (n - 2) + [t.run]
        self.step_ladder.set_steps(steps, colors)

    # --- obsługa ---------------------------------------------------------------------------------

    def apply_mapping(self: App) -> None:
        s = self.settings
        try:
            mapping = MappingConfig.from_dict(
                {
                    **self.config["mapping"],
                    "stand_below": float(s["stand_below"]),
                    "run_on": float(s["run_on"]),
                    "run_off": float(s["run_on"]) - HYSTERESIS,
                    "quick_stop": bool(s["quick_stop"]),
                }
            )
        except ValueError as exc:
            log.warning("Progi: %s", exc)
            return
        self.bridge.set_mapping(mapping)

    def on_threshold(self: App, which: str, value: float) -> None:
        """Progi chodu i biegu: zmieniony wygrywa, drugi przesuwa się o odstęp."""
        top = float(self.config["mapping"]["max_speed"])
        gap = HYSTERESIS + 0.2
        walk, run = float(self.settings["stand_below"]), float(self.settings["run_on"])
        if which == "walk":
            walk = value
            if run < walk + gap:
                run = min(top, walk + gap)
                walk = min(walk, run - gap)
        else:
            run = value
            if run < walk + gap:
                walk = max(0.0, run - gap)
                run = max(run, walk + gap)
        walk, run = round(walk, 1), round(run, 1)
        for st, v in ((self.walk_from_step, walk), (self.run_from_step, run)):
            if st is not None:
                st.set_value(v)
        self.settings.update(stand_below=walk, run_on=run, run_off=round(run - HYSTERESIS, 1))
        self.apply_mapping()
        self.save()
        self.render_zones()

    def on_quick_stop(self: App, on: bool) -> None:
        self.settings["quick_stop"] = on
        self.apply_mapping()
        self.save()
        log.info("Szybkie zatrzymanie %s", "włączone" if on else "wyłączone")

    def on_pad_at(self: App, index: int, at: int) -> None:
        steps = list(self.settings["pad_steps"])
        steps[index] = (at, steps[index][1])
        self.settings["pad_steps"] = steps
        self.save()
        if index < len(self.pad_steppers):
            self.pad_steppers[index].sub_label.setText(f"od {at}% wychylenia")
        self.render_zones()
        self.render_output_hint()

    def on_pad_speed(self: App, index: int, kmh: float) -> None:
        steps = list(self.settings["pad_steps"])
        steps[index] = (steps[index][0], round(kmh, 1))
        self.settings["pad_steps"] = steps
        self.save()
        self.belt_target = None
        self.render_ladder()
        self.render_zones()
        self.render_output_hint()

    def on_pad_count(self: App, count: int) -> None:
        if count == len(self.settings["pad_steps"]):
            return
        self.change("pad_steps", list(gamepad.PRESETS[count]))
        self.belt_target = None
        self.control_body.swap(self.build_control_settings, self.t.surface)
        self.render_zones()

    def on_pad_idle(self: App, mode: str) -> None:
        self.change("pad_idle", mode)
        log.info("Pad - gałka puszczona: %s", mode)
        self.belt_target = None
        if self.pad_steppers:  # tylko pierwszy wiersz i oś - bez przebudowy karty
            idle = self.pad_idle_text()
            self.pad_steppers[0].set_format((lambda _v, txt=idle: txt) if idle else None)
            self.render_ladder()
        self.render_zones()
        self.render_output_hint()

    def on_belt_speed(self: App, key: str, kmh: float) -> None:
        self.change(key, round(kmh, 1))
        self.belt_target = None
        self.render_zones()
        self.render_output_hint()

    def on_switch_visibility(self: App, key: str, on: bool) -> None:
        self.change(key, on)
        self.render_header()

    def on_profile_color(self: App, name: str) -> None:
        store = self.store
        if store is None:
            return
        store.update_profile(store.profile.id, color=name)
        self.settings["profile_color"] = name
        log.info("Kolor profilu %s: %s", store.profile.name, name)
        self.apply_look()
        self.render_header()
        self.render_zones()

    def render_device_label(self: App) -> None:
        if not hasattr(self, "device_label"):
            return
        name = self.settings.get("device_name") or ""
        address = self.settings.get("device_address") or ""
        text = f"{name} · {address}" if address else "Brak zapamiętanej bieżni"
        try:
            self.device_label.setText(text)
            self.forget_btn.setEnabled(bool(address))
        except RuntimeError:
            pass  # karta właśnie przebudowana
