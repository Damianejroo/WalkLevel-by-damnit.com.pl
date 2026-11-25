"""Sterowanie: włączanie, kierunek (chodzenie -> klawisze albo klawisze/pad -> taśma),
prędkość z pada i z klawiszy, start i stop taśmy."""

from __future__ import annotations

import contextlib
import logging
import time
from typing import TYPE_CHECKING

from bieznia import pad as gamepad
from bieznia.gui.widgets import (
    fmt_num,
)
from bieznia.settings import save_enabled
from bieznia.winapi import vk_from_scan

if TYPE_CHECKING:
    from bieznia.gui.app import App

log = logging.getLogger("bieznia")

HYSTERESIS = 0.6
PAD_SETTLE_S = 0.15  # gałka w drodze przez skoki - wysyłamy dopiero, gdy się zatrzyma
PAD_STOP_SETTLE_S = 0.4  # gałka przechodzi przez środek przy zmianie kierunku - to nie stop
PAD_CAPTURE_S = 10.0  # tyle czekamy na przycisk przy przypisywaniu
STOPPED = -1.0  # belt_target: taśma zatrzymana przez puszczoną gałkę


class ControlScreen:
    pad_state: gamepad.PadState | None
    pad_buttons: frozenset[str]
    pad_capture_until: float

    def toggle_from_bubble(self: App) -> None:
        self.set_control(not self.bridge.state.enabled)

    def set_control(self: App, on: bool) -> None:
        """Jedno miejsce dla skrótu, przełącznika i przycisku na ekranie.

        Wyłączenie najpierw puszcza klawisze (postać staje od razu), potem zatrzymuje taśmę.
        """
        changed = on != self.bridge.state.enabled
        self.bridge.set_enabled(on)
        self.enable_switch.set_silently(on)
        save_enabled(on)
        self.belt_target = None  # po zmianie prędkość z klawiszy idzie od nowa
        sets_speed = self.sets_speed()
        pad_stop = self.pad_stops_belt()  # taśmę uruchamia dopiero ruch gałki
        if changed and not on:
            if self.belt_commands_possible() or pad_stop:
                self.worker.belt("stop")
            elif sets_speed:
                self.worker.set_speed(self.idle_target())  # bez stopu: zwolnij
        if changed and on and self.belt_commands_possible() and not pad_stop:
            # czy taśma już jedzie, sprawdza sterownik (stan bieżni, nie prędkość)
            self.worker.belt("start")
            delay = self.caps.start_delay_s
            countdown = f" (odliczanie ok. {delay:.0f} s)" if delay >= 2 else ""
            self.render_output_hint(f"Uruchamiam bieżnię{countdown}…")

    def belt_commands_possible(self: App) -> bool:
        """Start i stop taśmy z aplikacji: włączone w ustawieniach i bieżnia na to pozwala.
        Symulator też - to taki sam sterownik, tylko bez Bluetooth."""
        return bool(self.settings["belt_control"]) and self.caps.control

    def on_belt_control(self: App, on: bool) -> None:
        self.change("belt_control", on)
        self.render_control_subtitle()

    # --- przycisk pada zamiast (albo obok) klawisza włączania ------------------------------

    def read_pad(self: App, capturing_keys: bool) -> None:
        """Pad czytany raz na klatkę - gdy steruje gałką, ma przypisany przycisk albo właśnie
        przypisujemy przycisk. Wciśnięcie przypisanego przycisku działa jak klawisz F8."""
        wanted = (
            self.settings["direction"] == "gamepad"
            or bool(self.settings["toggle_pad"])
            or self.pad_capture_until > 0
        )
        if not wanted:
            self.pad_state, self.pad_buttons = None, frozenset()
            return
        self.pad_state = self.pad.read()
        buttons = self.pad_state.buttons if self.pad_state else frozenset()
        pressed = buttons - self.pad_buttons
        self.pad_buttons = buttons
        if self.pad_capture_until > 0:
            if pressed:
                button = next((b for b in gamepad.BUTTON_ORDER if b in pressed), sorted(pressed)[0])
                self.set_toggle_pad(button)
            elif time.monotonic() > self.pad_capture_until:
                self.pad_capture_until = 0.0
                self.render_pad_button()
            return
        toggle = self.settings["toggle_pad"]
        if toggle and toggle in pressed and not capturing_keys:
            self.set_control(not self.bridge.state.enabled)

    def pad_button_label(self: App) -> str:
        button = self.settings["toggle_pad"]
        if not button:
            return ""
        name = self.pad_state.name if self.pad_state else self.pad.name
        return gamepad.button_label(button, name)

    def capture_pad_button(self: App) -> None:
        self.pad_capture_until = time.monotonic() + PAD_CAPTURE_S
        self.pad_buttons = frozenset(self.pad_state.buttons) if self.pad_state else frozenset()
        self.render_pad_button()

    def set_toggle_pad(self: App, button: str) -> None:
        self.pad_capture_until = 0.0
        self.change("toggle_pad", button)
        label_text = self.pad_button_label() or "brak"
        log.info("Przycisk pada włączający sterowanie: %s", button or "brak")
        if button in ("south", "east"):
            self.toast(f"{label_text} to w wielu grach skok albo akcja - rozważ inny przycisk.")
        self.render_pad_button()
        self.render_control_subtitle()

    def render_pad_button(self: App) -> None:
        btn = getattr(self, "pad_toggle_btn", None)
        if btn is None:
            return
        if self.pad_capture_until > 0:
            text = "Naciśnij przycisk na padzie…"
        else:
            text = self.pad_button_label() or "Przypisz przycisk pada"
        with contextlib.suppress(RuntimeError):
            if btn.text() != text:
                btn.setText(text)
            clear = getattr(self, "pad_clear_btn", None)
            if clear is not None:
                clear.setVisible(bool(self.settings["toggle_pad"]) and self.pad_capture_until == 0)

    def render_control_subtitle(self: App) -> None:
        belt = bool(self.settings["belt_control"])
        start = "uruchamia i zatrzymuje taśmę" if belt else "włącza i wyłącza sterowanie"
        pad_button = self.pad_button_label()
        if pad_button:
            start = f"(albo pad: {pad_button}) {start}"
        direction = self.settings["direction"]
        if direction == "keyboard":
            text = f"{start} · {self.bridge.keys[0].name} przyspiesza"
        elif direction == "gamepad":
            text = f"{start} · gałka ustawia tempo"
        else:
            text = f"{start} · działa też w grze"
        if self.control_subtitle.text() != text:
            self.control_subtitle.setText(text)

    def on_direction(self: App, kind: str) -> None:
        if kind == self.settings["direction"]:
            return
        self.change("direction", kind)
        log.info("Kierunek sterowania: %s", kind)
        self.apply_direction()
        self.settings_view = kind
        self.settings_direction_changed()  # karta w Ustawieniach: płynnie, gdy widać, w miejscu

    def sets_speed(self: App) -> bool:
        """Klawisze albo pad ustawiają prędkość taśmy (a nie odwrotnie)."""
        return self.settings["direction"] in ("keyboard", "gamepad")

    def idle_target(self: App) -> float:
        if self.settings["direction"] == "gamepad":
            if self.settings["pad_idle"] == "zero":
                return 0.0
            return float(self.settings["pad_steps"][0][1])
        return float(self.settings["idle_speed"])

    def apply_direction(self: App) -> None:
        """Klawisze/pad -> bieżnia: aplikacja niczego nie naciska, tylko słucha gracza."""
        sets_speed = self.sets_speed()
        self.bridge.set_output("dry" if sets_speed else self.settings["output"])
        self.belt_target = None
        self.mode_tiles.set_value(self.settings["direction"])
        self.render_control_subtitle()
        self.render_zones()
        self.render_output_hint()

    def pad_idle_text(self: App) -> str | None:
        """Co robi taśma przy puszczonej gałce, gdy to nie prędkość skoku 1."""
        return {"stop": "stop", "zero": "0 km/h"}.get(self.settings["pad_idle"])

    def pad_target(self: App, now: float) -> tuple[float, str, str, bool]:
        """Prędkość ze skoku gałki. Ostatnie pole: skok trzyma się już chwilę (można wysłać)."""
        steps = self.settings["pad_steps"]
        deflection = self.pad_state.deflection if self.pad_state else 0.0
        index = self.pad_picker.update(steps, deflection)
        if index != self.pad_step:
            self.pad_step, self.pad_step_since = index, now
        idle = self.pad_idle_text() if index == 0 else None
        speed = 0.0 if idle else float(steps[index][1])
        held = "idle" if index == 0 else ("run" if index == len(steps) - 1 else "walk")
        belt = "taśma stoi" if idle == "stop" else f"taśma {fmt_num(speed)} km/h"
        if self.pad_state is None:
            caption = f"Brak pada - {belt}"
        else:
            caption = f"Gałka {round(deflection * 100)}% - skok {index + 1} - {belt}"
        wait = PAD_STOP_SETTLE_S if idle == "stop" else PAD_SETTLE_S
        settled = now - self.pad_step_since >= wait
        return speed, held, caption, settled

    def send_pad(self: App, target: float) -> None:
        """Prędkość z pada; puszczona gałka w trybie stop zatrzymuje taśmę, ruch ją budzi."""
        if self.pad_step == 0 and self.settings["pad_idle"] == "stop":
            if self.belt_target != STOPPED:
                self.belt_target = STOPPED
                self.worker.belt("stop")
            return
        if target == self.belt_target:
            return
        self.belt_target = target
        wake = target > 0 and (self.pad_stops_belt() or bool(self.settings["belt_control"]))
        self.worker.set_speed(target, start_if_stopped=wake)

    def pad_stops_belt(self: App) -> bool:
        return bool(self.settings["direction"] == "gamepad" and self.settings["pad_idle"] == "stop")

    def render_pad_status(self: App) -> None:
        """Na karcie pada w Ustawieniach: który pad i gdzie jest gałka (na żywo)."""
        if self.step_ladder is None or self.pages.currentIndex() != self.SETTINGS:
            return
        state = self.pad_state
        text = self.pad_status_text()
        with contextlib.suppress(RuntimeError):  # karta właśnie przebudowana
            if self.pad_status.text() != text:
                self.pad_status.setText(text)
            self.step_ladder.set_stick(state.deflection if state else 0.0)

    def pad_status_text(self: App) -> str:
        state = self.pad_state
        if state is None:
            return "Nie widzę pada. Podłącz DualSense albo pada Xbox (kablem albo Bluetooth)."
        step = self.pad_step + 1
        return f"{state.name}: wychylenie {round(state.deflection * 100)}%, skok {step}."

    def keyboard_target(self: App) -> tuple[float, str]:
        """Prędkość taśmy z trzymanych klawiszy: chód + bieg, sam chód albo nic."""
        walk, run = self.bridge.keys
        walk_down = self.key_down(walk.vk or vk_from_scan(walk.scan, walk.extended))
        run_down = self.key_down(run.vk or vk_from_scan(run.scan, run.extended))
        if walk_down and run_down:
            return self.settings["run_speed"], "run"
        if walk_down:
            return self.settings["walk_speed"], "walk"
        return self.settings["idle_speed"], "idle"
