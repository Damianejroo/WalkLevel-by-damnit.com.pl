//! Odczyt danych online z odpowiedzi serwera (account::online) i reguła stanu na żywo.

use serde_json::{Value, json};
use walklevel_core::account::online::{self, Friend, PeriodTotals};
use walklevel_core::account::{LiveUpdate, LiveWalk, live_update};

#[test]
fn period_and_games_tolerate_missing_and_wrong_types() {
    assert_eq!(online::period(None), PeriodTotals::default());
    assert_eq!(
        online::period(Some(&json!([1, 2]))),
        PeriodTotals::default()
    );
    let p = online::period(Some(
        &json!({"kroki": 1234.9, "dystans_km": 1.5, "ruch_s": 60, "sesje": 3}),
    ));
    assert_eq!(
        p,
        PeriodTotals {
            steps: 1234,
            distance_km: 1.5,
            active_s: 60.0,
            sessions: Some(3)
        }
    );
    // liczba jako tekst nie jest liczbą
    assert_eq!(online::period(Some(&json!({"kroki": "5"}))).steps, 0);
    assert_eq!(online::period(Some(&json!({}))).sessions, None);

    assert_eq!(online::games(None), None);
    assert_eq!(online::games(Some(&Value::Null)), None);
    let g = online::games(Some(&json!([
        {"gra": "  Skyrim ", "gra_id": "g1", "kroki": 10, "dystans_km": 0.5, "ruch_s": 30},
        {"gra": "", "gra_id": ""},
        "zły wpis"
    ])))
    .expect("lista gier");
    assert_eq!(g.len(), 2);
    assert_eq!(g[0].game, "Skyrim");
    assert_eq!(g[0].game_id.as_deref(), Some("g1"));
    assert_eq!(g[1].game, "");
    assert_eq!(g[1].game_id, None);
}

#[test]
fn stats_flatten_totals_with_games() {
    let s = online::stats(&json!({"kroki": 100, "gry": null}));
    assert_eq!(s.totals.steps, 100);
    assert_eq!(s.games, None);
    let out = serde_json::to_value(&s).expect("json");
    assert_eq!(out["steps"], json!(100));
    assert_eq!(out["distanceKm"], json!(0.0));
    assert!(out["games"].is_null());
}

#[test]
fn friends_with_level_live_state_and_week() {
    let f = online::friends(&json!([
        {"id": "u2", "name": "Bob", "poziom": 7, "na_zywo": {"kmh": 4.5, "kroki": 300},
         "tydzien": {"kroki": 5000}},
        {"id": 42, "name": "Ala", "poziom": 0, "na_zywo": null},
        {"id": "u4", "name": "Max", "poziom": 250},
        "nie obiekt"
    ]));
    assert_eq!(f.len(), 3);
    assert_eq!(f[0].level, Some(7));
    let live = f[0].live.as_ref().expect("na żywo");
    assert_eq!((live.kmh, live.steps), (4.5, 300));
    assert_eq!(f[0].week.steps, 5000);
    assert_eq!(f[1].id, "42"); // liczba jako tekst
    assert_eq!(f[1].level, None); // poziom 0 - brak (UI bierze swój)
    assert_eq!(f[1].live, None);
    assert_eq!(f[2].level, Some(100)); // najwyżej 100
    assert!(online::friends(&json!({"nie": "lista"})).is_empty());
}

#[test]
fn friends_0_6_2_xp_description_and_live_game() {
    let f = online::friends(&json!([
        {"id": "u-ola", "name": "OlaBiega", "poziom": 14, "xp": 50290, "opis": " Do Mordoru ",
         "tydzien": {"kroki": 31240},
         "na_zywo": {"kmh": 3.4, "kroki": 2100, "ruch_s": 1500, "od_utc_s": 1000.0,
                     "dystans_km": 1.52, "tryb": "gra", "gra": "World of Warcraft"}},
        {"id": "u-ania", "name": "AniaNaBieżni", "poziom": 26, "xp": 158400, "opis": "",
         "na_zywo": {"kmh": 5.1, "kroki": 4800, "ruch_s": 2600, "od_utc_s": 900.0,
                     "dystans_km": 4.05, "tryb": "gra", "gra": null}},
        {"id": "u-x", "name": "Bez zgody", "poziom": null, "xp": null, "opis": null,
         "na_zywo": {"kmh": 4.0, "kroki": 10, "ruch_s": 5, "od_utc_s": 1.0,
                     "dystans_km": null, "tryb": null, "gra": null}},
        {"id": "u-k", "name": "Kuba", "na_zywo": {"kmh": 4.2, "tryb": "trening", "dystans_km": 500}}
    ]));
    let ola = &f[0];
    assert_eq!((ola.level, ola.xp), (Some(14), Some(50290)));
    assert_eq!(ola.description.as_deref(), Some("Do Mordoru"));
    let live = ola.live.as_ref().expect("na żywo");
    assert_eq!(
        (live.active_s, live.since, live.distance_km),
        (1500.0, Some(1000.0), Some(1.52))
    );
    assert_eq!(
        (live.mode.as_deref(), live.game.as_deref()),
        (Some("gra"), Some("World of Warcraft"))
    );
    let ania = f[1].live.as_ref().expect("na żywo");
    assert_eq!(
        (ania.mode.as_deref(), ania.game.as_deref()),
        (Some("gra"), None)
    ); // ukryta gra - „W grze”
    assert_eq!(f[1].description, None); // pusty opis = brak
    let x = &f[2];
    assert_eq!(
        (x.level, x.xp, x.description.as_deref()),
        (None, None, None)
    );
    let xl = x.live.as_ref().expect("na żywo");
    assert_eq!((xl.distance_km, xl.mode.as_deref()), (None, None));
    let k = f[3].live.as_ref().expect("na żywo");
    assert_eq!(
        (k.mode.as_deref(), k.distance_km),
        (Some("trening"), Some(300.0))
    );
    assert_eq!(f[0].today, None); // serwer bez „dzis” (starszy) - brak
    let out = serde_json::to_value(&f[0]).expect("json");
    assert_eq!(out["live"]["distanceKm"], json!(1.52));
    assert_eq!(out["description"], json!("Do Mordoru"));
}

#[test]
fn today_totals_for_friends_and_profile() {
    let f = online::friends(&json!([
        {"id": "a", "name": "A", "tydzien": {"kroki": 9000},
         "dzis": {"kroki": 3200, "dystans_km": 2.4, "ruch_s": 1500, "sesje": 2}},
        {"id": "b", "name": "B", "dzis": null},
        {"id": "c", "name": "C"}
    ]));
    let a = f[0].today.as_ref().expect("dziś");
    assert_eq!(
        (a.steps, a.distance_km, a.active_s, a.sessions),
        (3200, 2.4, 1500.0, Some(2))
    );
    assert_eq!((f[1].today.as_ref(), f[2].today.as_ref()), (None, None));
    assert_eq!(
        serde_json::to_value(&f[0]).expect("json")["today"]["steps"],
        json!(3200)
    );
    let p = online::person(
        &json!({"id": "u2", "dzis": {"kroki": 10}, "tydzien": {"kroki": 20}}),
        "u1",
    );
    assert_eq!(p.today.as_ref().map(|d| d.steps), Some(10));
    assert_eq!(
        online::person(&json!({"id": "u2", "dzis": null}), "u1").today,
        None
    );
    assert_eq!(online::person(&json!({"id": "u2"}), "u1").today, None);
}

#[test]
fn invitations_take_name_from_person_then_record() {
    let inv = online::invitations(&json!({
        "przychodzace": [
            {"id": "i1", "osoba": {"id": "u2", "name": "Bob"}, "utworzono_utc_s": 1700000000.0},
            {"id": "i2", "name": "Ala"},
            {"id": "i3"}
        ],
        "wyslane": null
    }));
    assert_eq!(inv.incoming.len(), 3);
    assert_eq!(inv.incoming[0].person.id, "u2");
    assert_eq!(inv.incoming[0].person.name, "Bob");
    assert_eq!(inv.incoming[0].created_at, Some(1_700_000_000.0));
    assert_eq!(inv.incoming[1].person.name, "Ala");
    assert_eq!(inv.incoming[1].person.id, "");
    assert_eq!(inv.incoming[2].person.name, "?");
    assert!(inv.sent.is_empty());
}

fn friend(id: &str, name: &str) -> Friend {
    Friend {
        id: id.into(),
        name: name.into(),
        level: None,
        xp: None,
        description: None,
        live: None,
        week: PeriodTotals::default(),
        today: None,
    }
}

#[test]
fn ranking_own_row_gets_user_id_and_friends_are_found_by_name() {
    let rows = json!([
        {"miejsce": 1, "name": "Bob", "poziom": 12, "kroki": 9000, "dystans_km": 6.1},
        {"miejsce": 2, "name": "Ola_77", "ja": true, "id": "inne", "kroki": 8000},
        {"miejsce": 3, "name": "Obcy", "kroki": 100},
        {"miejsce": 4, "id": "u9", "name": "Zed"}
    ]);
    let r = online::ranking(&rows, "tydzien", "u1", &[friend("u2", "Bob")]);
    assert_eq!(r.scope, "tydzien");
    let ids: Vec<&str> = r.rows.iter().map(|x| x.id.as_str()).collect();
    assert_eq!(ids, ["u2", "u1", "", "u9"]);
    assert!(r.rows[1].me && !r.rows[0].me);
    assert_eq!(r.rows[0].level, Some(12));
    assert_eq!(r.rows[1].level, None); // bez poziomu z serwera - poziom z aplikacji
    assert_eq!(r.rows[0].place, 1);
    let out = serde_json::to_value(&r.rows[1]).expect("json");
    assert!(out["level"].is_null());
    assert_eq!(out["distanceKm"], json!(0.0));
    // ja bez id użytkownika (niezalogowany) - po pseudonimie znajomego
    let r = online::ranking(
        &json!([{"ja": true, "name": "Bob"}]),
        "razem",
        "",
        &[friend("u2", "Bob")],
    );
    assert_eq!(r.rows[0].id, "u2");
    assert!(
        online::ranking(&Value::Null, "razem", "u1", &[])
            .rows
            .is_empty()
    );
}

#[test]
fn people_page_defaults_and_values() {
    let empty = online::people_page(&json!({}));
    assert_eq!(
        (empty.page, empty.has_next, empty.people.len()),
        (1, false, 0)
    );
    let p = online::people_page(&json!({
        "strona": 2, "nastepna": true,
        "osoby": [
            {"id": "u2", "name": "Bob", "poziom": 3, "tydzien": {"kroki": 77}, "miejsce": 5, "znajomy": true},
            {"id": "u3", "name": "Ala"}
        ]
    }));
    assert_eq!((p.page, p.has_next), (2, true));
    assert_eq!(p.people[0].week_steps, 77);
    assert_eq!(p.people[0].place, Some(5));
    assert!(p.people[0].friend);
    assert_eq!(p.people[1].week_steps, 0);
    assert_eq!(p.people[1].place, None);
    let out = serde_json::to_value(&p).expect("json");
    assert_eq!(out["hasNext"], json!(true));
    assert_eq!(out["people"][0]["weekSteps"], json!(77));
}

#[test]
fn person_profile_me_achievements_and_hidden_games() {
    let v = json!({
        "id": "u1", "name": "Ola_77", "poziom": 20, "xp": 12345.7,
        "osiagniecia": 4,
        "osiagniecia_lista": [{"id": "a1", "zdobyte_utc_s": 1.5}],
        "od_utc_s": 1600000000.0, "bieznia_model": "", "znajomy": false,
        "tydzien": {"kroki": 10}, "razem": "brak", "gry": null
    });
    let p = online::person(&v, "u1");
    assert!(p.me);
    assert_eq!(p.level, Some(20));
    assert_eq!(p.xp, Some(12345));
    assert_eq!(p.achievements_count, Some(4));
    assert_eq!(p.achievements.as_ref().map(Vec::len), Some(1));
    assert_eq!(
        p.achievements.as_ref().map(|a| a[0].earned_at),
        Some(Some(1.5))
    );
    assert_eq!(p.treadmill_model, None);
    assert_eq!(p.week.as_ref().map(|w| w.steps), Some(10));
    assert_eq!(p.total, None);
    assert!(p.games_hidden && p.games.is_none());
    // lista osiągnięć zamiast liczby; obcy profil; gry pokazane
    let p = online::person(
        &json!({"id": "u2", "osiagniecia": [1, 2, 3], "gry": [], "bieznia_model": "R1"}),
        "u1",
    );
    assert!(!p.me && !p.games_hidden);
    assert_eq!(p.achievements_count, Some(3));
    assert_eq!(p.achievements, None);
    assert_eq!(p.games, Some(vec![]));
    assert_eq!(p.treadmill_model.as_deref(), Some("R1"));
    assert_eq!(p.description, None);
    let p = online::person(&json!({"id": "u2", "opis": "Cel: 5 km dziennie"}), "u1");
    assert_eq!(p.description.as_deref(), Some("Cel: 5 km dziennie"));
    // cudzy profil: lista osiągnięć bez czasu zdobycia (prywatność) - brak, nie 1970
    let p = online::person(
        &json!({"id": "u2", "osiagniecia_lista": [{"id": "km1"}, {"id": "s3"}]}),
        "u1",
    );
    let lista = p.achievements.expect("lista");
    assert_eq!(
        lista.iter().map(|a| a.earned_at).collect::<Vec<_>>(),
        [None, None]
    );
    assert_eq!(lista[1].id, "s3");
    // pusty id nigdy nie jest „mną”
    assert!(!online::person(&json!({}), "").me);
}

#[test]
fn registration_code_and_invite_answer() {
    let c = online::registration_code(&json!({"kod": "ABC-123", "wazny_do_utc_s": 99.0}));
    assert_eq!((c.code.as_str(), c.valid_until), ("ABC-123", Some(99.0)));
    let c = online::registration_code(&json!({"kod": "X", "wazny_do": 5}));
    assert_eq!(c.valid_until, Some(5.0));
    assert_eq!(online::registration_code(&json!({})).valid_until, None);

    let a = online::invite_answer(&json!({"stan": "przyjeta", "osoba": {"name": "Bob"}}));
    assert!(a.accepted);
    assert_eq!((a.name.as_str(), a.state()), ("Bob", "przyjeta"));
    let a = online::invite_answer(&json!({"stan": "wyslana"}));
    assert!(!a.accepted);
    assert_eq!((a.name.as_str(), a.state()), ("", "oczekuje"));
}

#[test]
fn live_state_walking_then_one_stop() {
    let mut on = false;
    let at = |kmh: f64| {
        Some(LiveWalk {
            kmh,
            steps: 10,
            active_s: 5.0,
            session_id: "s1".into(),
            ..LiveWalk::default()
        })
    };
    // bez sesji albo w miejscu - nic
    assert_eq!(live_update(&mut on, None), None);
    assert_eq!(live_update(&mut on, at(0.05)), None);
    let walking = live_update(&mut on, at(4.0));
    assert_eq!(walking, at(4.0).map(LiveUpdate::Walking));
    assert!(on);
    assert_eq!(live_update(&mut on, at(0.0)), Some(LiveUpdate::Stopped));
    assert!(!on);
    assert_eq!(live_update(&mut on, None), None);
}

#[test]
fn profile_games_by_name_with_other_games_row() {
    let p = online::person(
        &json!({
            "id": "u2",
            "gry": [
                {"nazwa": " Elden   Ring ", "kroki": 900, "dystans_km": 0.7, "ruch_s": 600, "sesje": 2},
                {"nazwa": "Skyrim", "gra_id": "aaaaaaaaaa", "publiczna": true, "kroki": 300}
            ],
            "inne_gry": {"kroki": 120, "dystans_km": 0.1, "ruch_s": 90, "sesje": 1, "gier": 2}
        }),
        "u1",
    );
    let games = p.games.expect("gry");
    assert_eq!(games[0].game, "Elden   Ring");
    assert_eq!(games[1].game_id.as_deref(), Some("aaaaaaaaaa"));
    let other = p.other_games.expect("inne gry");
    assert_eq!(
        (other.steps, other.sessions, other.games),
        (120, Some(1), 2)
    );
    let out = serde_json::to_value(online::person(
        &json!({"id": "u2", "gry": null, "inne_gry": null}),
        "u1",
    ))
    .expect("json");
    assert!(out["otherGames"].is_null() && out["gamesHidden"] == json!(true));
    // zero gier w „inne_gry” - bez wiersza
    let p = online::person(&json!({"inne_gry": {"kroki": 5, "gier": 0}}), "u1");
    assert_eq!(p.other_games, None);
}

#[test]
fn achievement_stats_percent_only_from_enough_accounts() {
    let s = online::achievement_stats(&json!({
        "kont": 40, "od_kont": 25,
        "procent": {"a1": 12.5, "a2": "zle", "a3": 140, "a4": 0}
    }));
    assert_eq!(s.accounts, Some(40));
    assert_eq!(s.percent.len(), 2);
    assert_eq!(s.percent["a1"], 12.5);
    assert_eq!(s.percent["a4"], 0.0);
    let few = online::achievement_stats(&json!({"kont": null, "od_kont": 25, "procent": {}}));
    assert_eq!(few.accounts, None);
    assert!(few.percent.is_empty());
    assert_eq!(
        online::achievement_stats(&json!("x")),
        online::AchievementStats::default()
    );
}
