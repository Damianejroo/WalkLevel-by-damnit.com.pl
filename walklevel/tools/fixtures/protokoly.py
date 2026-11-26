"""Fixtury modułu protokoly (walklevel-core, treadmill/) z prawdziwej implementacji w Pythonie.

Uruchamiać z katalogu głównego repozytorium:
    set PYTHONPATH=.
    .venv\\Scripts\\python.exe walklevel\\tools\\fixtures\\protokoly.py

Wyniki: walklevel/crates/core/tests/fixtures/protokoly/*.json. Losowe dane mają stałe ziarno,
więc ponowne uruchomienie daje te same pliki.
"""

from __future__ import annotations

import asyncio
import json
import math
import random
from dataclasses import asdict, replace
from pathlib import Path
from typing import Any

from bieznia import sterowniki as ster
from bieznia.sterowniki import pilnowanie, profile, symulator
from bieznia.sterowniki.ftms import protokol as ftms
from bieznia.sterowniki.ftms import sterownik as ftms_driver
from bieznia.sterowniki.kingsmith import protokol as ks
from bieznia.sterowniki.kingsmith import sterownik as ks_driver
from bieznia.sterowniki.kontrakt import Capabilities

ROOT = Path(__file__).resolve().parents[3]
OUT = ROOT / "walklevel" / "crates" / "core" / "tests" / "fixtures" / "protokoly"
DANE = ROOT / "tests" / "dane"
rng = random.Random(20261126)


def save(name: str, data: Any) -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    text = json.dumps(data, ensure_ascii=False, indent=1)
    (OUT / name).write_text(text + "\n", encoding="utf-8", newline="\n")


def caps_dict(c: Capabilities) -> dict[str, Any]:
    d = asdict(c)
    d["incline"] = list(c.incline) if c.incline else None
    return d


# --- KingSmith ---------------------------------------------------------------------------

TEXTS = [
    "",
    "a",
    "ab",
    "abc",
    "shake",
    "net",
    "get_dn",
    "version",
    "time_posix 1764150000",
    "format error",
    "shake 00",
    "props CurrentSpeed 3.5 spm 98",
    "props CurrentSpeed 3.5 RunningSteps 120 runState 1",
    "props ControlMode 1 spm 0",
    "version 0014",
    "servers getProp 1 2 7 12 23 24 31",
    "servers getProp 1 9 15 2 10 11 12 13 14",
    "props runState 1",
    "props CurrentSpeed 12",
    "zażółć gęślą jaźń",
    "props ;urrentSpeed 3.5",
    "linia\r\n",
    "tab\tw środku",
]


def alphabet_noise(table: str, length: int) -> bytes:
    chars = ks.TABLES[table]
    return bytes(rng.choice(chars) for _ in range(length))


def kingsmith_fixture() -> dict[str, Any]:
    encode = []
    frames: list[bytes] = []
    for text in TEXTS:
        for name in ks.TABLES:
            data = ks.encode(text, name)
            encode.append({"text": text, "table": name, "hex": data.hex()})
            frames.append(data[:-1])
    # zmienione ramki (jedna litera inna) i szum z alfabetu tablic
    for _ in range(150):
        base = bytearray(rng.choice(frames) or b"QQ==")
        i = rng.randrange(len(base))
        base[i] = rng.choice(ks.TABLES[rng.choice(list(ks.TABLES))])
        frames.append(bytes(base))
    for _ in range(150):
        frames.append(alphabet_noise(rng.choice(list(ks.TABLES)), rng.randrange(0, 40)))
    frames += [b"QQ==", b"QR==", b"QQ=", b"QUJD=", b"=QQ=", b"QQ==QQ==", b"====", b"\x00\x01", b"\xff"]
    for line in (DANE / "x21-ramki.txt").read_text(encoding="utf-8").splitlines():
        if line.startswith("#"):
            continue
        raw = bytes.fromhex(line.split()[1])
        frames += [f for f in raw.split(ks.END) if f]
    decode = []
    for frame in frames:
        decode.append({"hex": frame.hex(), "texts": {n: ks.decode(frame, n) for n in ks.TABLES}})

    reply_texts = TEXTS + [d for case in decode for d in case["texts"].values() if d]
    reply_texts = list(dict.fromkeys(reply_texts))[:400]
    known = [{"text": t, "known": ks.is_known_reply(t)} for t in reply_texts]
    props_texts = reply_texts + [
        "props CurrentSpeed 3.5 mcu_version 1.2.3 spm 98 goal x Error -5000",
        "props a 1_0 b 1e3 c inf d nan e -0 f .5 g 5.",
        "props a 1__0 b _1 c 1_ d 0x10 e 1,5",
        "props odd",
        "props",
        "props\tCurrentSpeed\t2.5",
        "props CurrentSpeed 2.5 CurrentSpeed 3.0",
    ]
    # JSON nie zna inf/nan - takie wartości jako tekst ("inf", "-inf", "nan")
    props = [
        {"text": t, "props": {k: v if math.isfinite(v) else repr(v) for k, v in ks.parse_props(t).items()}}
        for t in props_texts
    ]
    speeds = [0, -1, 0.2, 0.5, 0.25, 0.35, 0.45, 1.05, 1.15, 1.25, 1.5, 2.45, 2.55, 3.0, 11.95, 12.0, 30]
    speeds += [round(rng.uniform(0, 13), 3) for _ in range(200)]
    speed = [{"kmh": k, "command": ks.speed_command(k)} for k in speeds]
    names = ["KS-NACH-MXG-1234", "ks-nach-mxg", "KS-HDSY-X21C", "", "KS-NACH-X21C", "WalkingPad"]
    order = [{"name": n, "order": ks.table_order(n)} for n in names]

    codec = []
    for table in ks.TABLES:
        for start in (None, ks.table_order("KS-NACH-MXG")):
            c = ks.Codec(start)
            steps = []
            msgs = list(TEXTS[4:18])
            rng.shuffle(msgs)
            for msg in msgs:
                if rng.random() < 0.2:
                    c.try_next_table()
                    steps.append({"op": "next", "table": c.table, "candidates": c.candidates})
                    continue
                if rng.random() < 0.15:
                    frame = alphabet_noise(table, rng.randrange(1, 30))
                else:
                    frame = ks.encode(msg, table)[:-1]
                res = c.decode(frame)
                steps.append(
                    {
                        "op": "decode",
                        "hex": frame.hex(),
                        "text": res.text if res else None,
                        "trusted": res.trusted if res else None,
                        "table": c.table,
                        "candidates": list(c.candidates),
                        "locked": c.locked,
                        "describe": c.describe(),
                    }
                )
            codec.append({"table": table, "start": start, "steps": steps})

    buffers = []
    for _ in range(20):
        fb = ks.FrameBuffer()
        feeds = []
        for _ in range(rng.randrange(1, 8)):
            kind = rng.random()
            if kind < 0.6:
                msg = ks.encode(rng.choice(TEXTS), rng.choice(list(ks.TABLES)))
                for part in ks.chunks(msg, rng.choice([1, 5, 16, 20])):
                    feeds.append({"hex": part.hex(), "frames": [f.hex() for f in fb.feed(part)]})
            elif kind < 0.8:
                junk = bytes(rng.randrange(256) for _ in range(rng.randrange(0, 30)))
                feeds.append({"hex": junk.hex(), "frames": [f.hex() for f in fb.feed(junk)]})
            else:
                junk = b"x" * rng.choice([100, ks.MAX_BUFFER, ks.MAX_BUFFER + 1])
                feeds.append({"hex": junk.hex(), "frames": [f.hex() for f in fb.feed(junk)]})
        buffers.append(feeds)
    return {
        "encode": encode,
        "decode": decode,
        "known": known,
        "props": props,
        "speed_command": speed,
        "table_order": order,
        "codec": codec,
        "frame_buffer": buffers,
    }


class _Clock:
    t = 0.0


def x21_trace_fixture() -> dict[str, Any]:
    """Ślad testera przez odbiór sterownika KingSmith (czas = czas z pliku)."""
    clock = _Clock()
    ks_driver.time.monotonic = lambda: clock.t  # type: ignore[attr-defined]
    readings: list[dict[str, Any]] = []
    prof = profile.profile_for("KS-NACH-X21C", "kingsmith")
    channel = ks_driver.Channel("00011234-0000-1000-8000-00805f9b34fb", None, None)
    driver = ks_driver.KingSmithDriver(
        None,  # type: ignore[arg-type]
        [channel],
        "KS-NACH-X21C",
        lambda s, k: readings.append({"t": clock.t, "speed": s, "steps": k}),
        prof,
    )
    events = []
    for line in (DANE / "x21-ramki.txt").read_text(encoding="utf-8").splitlines():
        if line.startswith("#"):
            continue
        t, hx = line.split()
        clock.t = float(t)
        before = len(readings)
        driver._on_notify(channel, None, bytearray(bytes.fromhex(hx)))
        events.append(
            {
                "t": clock.t,
                "hex": hx,
                "readings": readings[before:],
                "table": driver.codec.table,
                "candidates": list(driver.codec.candidates),
                "run_state": driver.run_state,
                "control_mode": driver.control_mode,
                "last_error": driver.last_error,
                "firmware": driver.info.firmware,
                "belt_running": driver.belt_running(),
            }
        )
    return {
        "device_name": "KS-NACH-X21C",
        "events": events,
        "counts": dict(driver.counts),
        "final_table": driver.codec.table,
    }


# --- FTMS --------------------------------------------------------------------------------

REAL_IDLE = bytes.fromhex(
    "9c 25 00 00 5a 00 00 5a 00 00 00 00 00 00 00 0c 00 00 00 00 00 e0 00 23 01 00"
)
REAL_WALK = bytes.fromhex(
    "9c 25 64 00 14 00 00 00 00 00 00 00 00 00 00 00 02 00 00 00 00 00 4a 00 00 00 00"
)


def ftms_fixture() -> dict[str, Any]:
    frames = [REAL_IDLE, REAL_WALK, REAL_IDLE[:10], b"\x01", b"", bytes([0, 0, 0x5E, 0x01])]
    frames.append(bytes([0x01, 0x00, 0x5E, 0x01]))
    for line in (DANE / "urevo-5l-ramki.txt").read_text(encoding="utf-8").splitlines():
        if line.startswith("#"):
            continue
        _t, kind, hx = line.split()
        if kind == "data":
            frames.append(bytes.fromhex(hx))
    for _ in range(300):
        flags = rng.randrange(0, 0x4000)
        body = bytes(rng.randrange(256) for _ in range(rng.randrange(0, 40)))
        frames.append(flags.to_bytes(2, "little") + body)
    decode = []
    for f in frames:
        d = ftms.decode_treadmill_data(f)
        decode.append({"hex": f.hex(), "data": asdict(d) if d else None})
    speeds = [0.0, 0.5, 1.0, 2.5, 6.0, 0.005, 0.015, 1.005, 2.675, 655.35]
    speeds += [round(rng.uniform(0, 20), 4) for _ in range(100)]
    set_speed = [{"kmh": k, "hex": ftms.encode_set_speed(k).hex()} for k in speeds]
    inclines = [-3.0, 0.0, 0.05, 0.15, 9.0, 1.25] + [round(rng.uniform(-10, 15), 3) for _ in range(50)]
    set_incline = [{"pct": p, "hex": ftms.encode_set_incline(p).hex()} for p in inclines]
    responses = []
    for raw in [b"\x80\x07\x01", b"\x01\x02", b"\x80\x07", b"\x80\x02\x03\x00", b""]:
        r = ftms.parse_response(raw)
        responses.append({"hex": raw.hex(), "result": list(r) if r else None})
    ranges = []
    for raw in [bytes.fromhex("64 00 58 02 0a 00"), b"\x01\x02", bytes.fromhex("ff ff 00 80 01 00 99")]:
        s = ftms.decode_speed_range(raw)
        i = ftms.decode_incline_range(raw)
        ranges.append({"hex": raw.hex(), "speed": list(s) if s else None, "incline": list(i) if i else None})
    return {
        "decode": decode,
        "set_speed": set_speed,
        "set_incline": set_incline,
        "responses": responses,
        "ranges": ranges,
        "commands": {
            "request_control": ftms.encode_request_control().hex(),
            "start": ftms.encode_start().hex(),
            "stop": ftms.encode_stop().hex(),
            "pause": ftms.encode_pause().hex(),
        },
    }


def urevo_trace_fixture() -> dict[str, Any]:
    clock = _Clock()
    ftms_driver.time.monotonic = lambda: clock.t  # type: ignore[attr-defined]
    readings: list[dict[str, Any]] = []
    prof = profile.profile_for("URTM054", "ftms")
    driver = ftms_driver.FtmsDriver(
        None,  # type: ignore[arg-type]
        [],
        "URTM054",
        lambda s, k: readings.append({"t": clock.t, "speed": s, "steps": k}),
        prof,
    )
    driver.capabilities = replace(driver.capabilities, steps=False)  # sprawdzamy przełączenie
    events = []
    for line in (DANE / "urevo-5l-ramki.txt").read_text(encoding="utf-8").splitlines():
        if line.startswith("#"):
            continue
        t, kind, hx = line.split()
        clock.t = float(t)
        if kind == "cmd":
            continue
        before = len(readings)
        if kind == "data":
            driver._on_data(None, bytearray(bytes.fromhex(hx)))
        else:
            driver._on_status(None, bytearray(bytes.fromhex(hx)))
        events.append(
            {
                "t": clock.t,
                "kind": kind,
                "hex": hx,
                "readings": readings[before:],
                "running": driver.belt_running(),
                "belt_speed": list(driver.belt_speed()) if driver.belt_speed() else None,
                "steps_cap": driver.capabilities.steps,
            }
        )
    return {"events": events, "counts": dict(driver.counts)}


# --- profile -----------------------------------------------------------------------------


def profile_dict(p: profile.ModelProfile | None) -> dict[str, Any] | None:
    if p is None:
        return None
    return {
        "id": p.id,
        "name": p.name,
        "driver": p.driver,
        "match": list(p.match),
        "capabilities": [[k, v] for k, v in p.capabilities],
        "options": [[k, v] for k, v in p.options],
        "source": p.source,
    }


def profiles_fixture() -> dict[str, Any]:
    base = {"format": 1, "id": "a", "name": "A", "driver": "ftms"}
    cases: list[Any] = [
        json.loads(p.read_text(encoding="utf-8"))
        for p in sorted(profile.BUILTIN_DIR.glob("*.json"))
        if p.name != "indeks.json"
    ]
    cases += [
        {"format": 2, "id": "a", "name": "A", "driver": "ftms"},
        {"format": 1, "id": "a b", "name": "A", "driver": "ftms"},
        {"format": 1, "id": "a", "name": "A", "driver": "python"},
        dict(base, capabilities={"exec": 1}),
        dict(base, capabilities={"speed_max": 99}),
        dict(base, options={"x": [1]}),
        dict(base, capabilities={"speed_min": 5, "speed_max": 2}),
        dict(base, capabilities={"speed_min": 5}),
        dict(base, capabilities={"speed_max": 0.5}),
        dict(base, capabilities={"pause": 1}),
        dict(base, capabilities={"speed_min": True}),
        dict(base, capabilities={"incline": 1}),
        dict(base, capabilities={"control": False}),
        dict(base, capabilities={"steps": False, "start_delay_s": 15, "telemetry_s": 0.1}),
        dict(base, capabilities=None, options=None),
        dict(base, capabilities={}, options=[]),
        dict(base, options={"table": "v7", "n": 1, "f": 1.5, "b": True}),
        dict(base, options={"x": None}),
        dict(base, options=[1]),
        dict(base, match=["X", ""]),
        dict(base, match="X"),
        dict(base, match=None),
        dict(base, match=["B", "a"]),
        dict(base, name="  Spacje  "),
        dict(base, name="   "),
        dict(base, name=5),
        dict(base, id=""),
        dict(base, id="---"),
        dict(base, id="ąę-1"),
        dict(base, id=5),
        dict(base, format=1.0),
        dict(base, format=True),
        dict(base, format="1"),
        dict(base, driver="kingsmith"),
        dict(base, driver="symulator"),
        {"id": "a"},
        [],
        "tekst",
        None,
    ]
    parsed = []
    for case in cases:
        try:
            parsed.append({"input": case, "profile": profile_dict(profile.parse_profile(case))})
        except profile.ProfileError:
            parsed.append({"input": case, "profile": None})
    profile.all_profiles(reload=True)
    names = [
        "URTM054", "urtm054-x", "URTM061", "KS-HDSY-X21C", "ks-hdsy-x21c", "KS-NACH-MXG",
        "KS-NACH-X21C", "WalkingPad A1", "Mi Band", "", "UREVO", "SpaceWalk E1", "KS-",
        "JBL Flip 5", "URT",
    ]
    lookup = []
    for name in names:
        lookup.append(
            {
                "name": name,
                "any": profile_dict(profile.profile_for(name)),
                "kingsmith": (p.id if (p := profile.profile_for(name, "kingsmith")) else None),
                "ftms": (p.id if (p := profile.profile_for(name, "ftms")) else None),
                "known_capabilities": caps_dict(ster.known_capabilities(name)),
                "model_name": ster.model_name(name),
                "looks_like": ster.looks_like_treadmill(name),
            }
        )
    uuids = [
        ["00001826-0000-1000-8000-00805f9b34fb"],
        ["00001826-0000-1000-8000-00805F9B34FB"],
        ["00011234-0000-1000-8000-00805f9b34fb"],
        ["00021234-0000-1000-8000-00805f9b34fb"],
        ["00031234-0000-1000-8000-00805f9b34fb"],
        ["0000fed7-0000-1000-8000-00805f9b34fb"],
        ["0000180a-0000-1000-8000-00805f9b34fb"],
        [],
    ]
    services = [{"uuids": u, "looks_like": ster.looks_like_treadmill(None, u)} for u in uuids]
    return {
        "parse": parsed,
        "lookup": lookup,
        "services": services,
        "simulator_caps": caps_dict(ster.known_capabilities("cokolwiek", simulated=True)),
        "kingsmith_caps": caps_dict(ster.KINGSMITH_CAPS),
        "ftms_caps": caps_dict(ster.FTMS_CAPS),
        "default_caps": caps_dict(Capabilities()),
        "all": [profile_dict(p) for p in profile.all_profiles()],
    }


def capabilities_fixture() -> dict[str, Any]:
    caps = [Capabilities(), ster.FTMS_CAPS, ster.KINGSMITH_CAPS]
    values = [-1, 0, 0.0001, 0.25, 0.35, 0.45, 0.5, 0.95, 1.05, 2.25, 5.95, 6.0, 6.05, 12.05, 99]
    values += [round(rng.uniform(-1, 14), 3) for _ in range(150)]
    clamp = [{"caps": i, "kmh": v, "out": c.clamp(v)} for i, c in enumerate(caps) for v in values]
    ranges = [(1.0, 6.0, 0.1), (0.0, 6.0, 0.1), (2.0, 1.0, 0.1), (0.5, 8.0, 0.0), (-1, 3, 0.5)]
    with_range = [
        {"caps": i, "range": list(r), "out": caps_dict(c.with_range(*r))}
        for i, c in enumerate(caps)
        for r in ranges
    ]
    return {"caps": [caps_dict(c) for c in caps], "clamp": clamp, "with_range": with_range}


# --- pilnowanie ----------------------------------------------------------------------------


def watchdog_fixture() -> list[dict[str, Any]]:
    """Losowe scenariusze SpeedGuard: bieżnia gubi część poleceń, czas płynie skokami."""
    scenarios = []
    caps = ster.KINGSMITH_CAPS
    for _ in range(40):
        clock = [100.0]
        belt = {"speed": rng.choice([0.0, 1.0, 2.0]), "at": 0.0, "outcomes": []}
        outcomes = [rng.choice(["ok", "ok", "ignore", "fail"]) for _ in range(200)]
        calls: list[float] = []

        async def send(kmh: float) -> bool:
            calls.append(kmh)
            outcome = outcomes[len(calls) - 1]
            if outcome == "ok":
                belt["speed"] = caps.clamp(kmh)
            return outcome != "fail"

        def reading() -> tuple[float, float] | None:
            return None if belt["at"] == 0.0 else (belt["speed"], belt["at"])

        guard = pilnowanie.SpeedGuard(send, reading, caps.clamp, clock=lambda: clock[0])
        ops = []
        steps = []

        async def run() -> None:
            for _ in range(rng.randrange(10, 60)):
                r = rng.random()
                if r < 0.15:
                    op = {"op": "set", "kmh": rng.choice([0.0, 0.3, 1.0, 1.5, 2.5, 3.5, 13.0])}
                    ok = await guard.set(op["kmh"])
                    op["ok"] = ok
                elif r < 0.2:
                    op = {"op": "reset"}
                    guard.reset()
                elif r < 0.3:
                    op = {"op": "belt", "speed": rng.choice([0.0, 1.0, 1.8, 2.4, 3.0])}
                    belt["speed"] = op["speed"]
                else:
                    op = {
                        "op": "check",
                        "dt": rng.choice([0.5, 1.0, 2.0, 2.5, 3.0, 5.0, 9.0]),
                        "fresh": rng.random() < 0.8,
                    }
                    clock[0] += op["dt"]
                    if op["fresh"]:
                        belt["at"] = clock[0]
                    await guard.check()
                ops.append(op)
                s = guard.stats
                steps.append(
                    {
                        "calls": len(calls),
                        "target": guard.target,
                        "confirmed": guard.confirmed,
                        "stats": [s.sent, s.ok, s.failed, s.resends, list(s.latencies)],
                    }
                )

        start_speed = belt["speed"]
        asyncio.run(run())
        scenarios.append(
            {
                "start_speed": start_speed,
                "outcomes": outcomes[: len(calls)],
                "calls": calls,
                "ops": ops,
                "steps": steps,
            }
        )
    return scenarios


# --- symulator -----------------------------------------------------------------------------


def simulator_fixture() -> list[dict[str, Any]]:
    scenarios = []
    for _ in range(20):
        clock = [0.0]
        readings: list[list[Any]] = []
        caps = replace(symulator.CAPS, start_delay_s=0.0)
        drv = symulator.SimulatorDriver(
            lambda s, k: readings.append([s, k]), clock=lambda: clock[0], capabilities=caps
        )
        ops = []
        steps = []

        async def run() -> None:
            await drv.start()
            for _ in range(rng.randrange(20, 80)):
                r = rng.random()
                if r < 0.1:
                    op: dict[str, Any] = {"op": "start_belt"}
                    op["result"] = await drv.start_belt()
                elif r < 0.15:
                    op = {"op": "stop_belt"}
                    await drv.stop_belt()
                elif r < 0.3:
                    op = {"op": "set_speed", "kmh": round(rng.uniform(-1, 8), 2)}
                    op["result"] = await drv.set_speed(op["kmh"])
                elif r < 0.38:
                    op = {"op": "remote", "steps": rng.choice([-2, -1, 1, 1, 2])}
                    drv.remote(op["steps"])
                else:
                    op = {"op": "advance", "dt": rng.choice([0.1, 0.25, 0.5, 0.74, 0.75, 1.0, 2.0])}
                    clock[0] += op["dt"]
                    drv.advance()
                ops.append(op)
                bs = drv.belt_speed()
                steps.append(
                    {
                        "readings": len(readings),
                        "speed": drv.speed,
                        "steps": drv.steps,
                        "target": drv.target,
                        "running": drv.running,
                        "belt_speed": list(bs) if bs else None,
                        "counts": dict(drv.counts),
                    }
                )

        asyncio.run(run())
        scenarios.append({"ops": ops, "steps": steps, "readings": readings})
    return scenarios


def main() -> None:
    save("kingsmith.json", kingsmith_fixture())
    save("x21_trace.json", x21_trace_fixture())
    save("ftms.json", ftms_fixture())
    save("urevo_trace.json", urevo_trace_fixture())
    save("profiles.json", profiles_fixture())
    save("capabilities.json", capabilities_fixture())
    save("watchdog.json", watchdog_fixture())
    save("simulator.json", simulator_fixture())
    print(f"Zapisano fixtury w {OUT}")


if __name__ == "__main__":
    main()
