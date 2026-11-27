//! App state tests on a temporary data folder (no window, no treadmill connection).

use std::fs;
use std::path::PathBuf;

use walklevel_core::account::AccountState;
use walklevel_core::backup;
use walklevel_core::model::Session;
use walklevel_core::paths::AppPaths;
use walklevel_core::treadmill;

use crate::state::{App, KontoWidok};
use crate::views::{ErrorCode, SliceName};

struct TestApp {
    app: App,
    dir: PathBuf,
}

impl Drop for TestApp {
    fn drop(&mut self) {
        self.app.history = None;
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn test_app(name: &str) -> TestApp {
    let dir = std::env::temp_dir().join(format!("walklevel-test-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let paths = AppPaths::new(&dir, &dir);
    paths.ensure().expect("data folder");
    let config = crate::load_config(&paths);
    let app = crate::build_app(paths, config, treadmill::monotonic_clock(), None);
    TestApp { app, dir }
}

fn session(id: &str, person: &str) -> Session {
    Session {
        id: id.into(),
        person: person.into(),
        person_name: "Ala".into(),
        profile: "p1".into(),
        profile_name: "Gra".into(),
        start: 1_700_000_000.0,
        end: 1_700_001_800.0,
        steps: 3200,
        distance_km: 2.1,
        active_s: 1800.0,
        device_model: String::new(),
        driver: String::new(),
        device_profile: String::new(),
        firmware: String::new(),
        app_version: String::new(),
        source: "symulator".into(),
        visibility: "znajomi".into(),
        tz_offset_min: 60,
        updated: 1_700_001_800.0,
        mode: "gra".into(),
    }
}

fn signed_in(app: &mut App, person: &str) {
    let mut state = AccountState::for_person(person);
    state.user_id = format!("user-{person}");
    app.konta
        .insert(person.into(), KontoWidok { state, pending: 0 });
}

#[test]
fn hiding_a_session_online_changes_the_history_stamp_each_time() {
    let mut t = test_app("hide-stamp");
    let mut s = session("a1", "osoba-b");
    t.app.history.as_ref().unwrap().save(&mut s).unwrap();
    let before = t.app.history_stamp().stamp;
    t.app.history_session_hide_online("a1", true).unwrap();
    let hidden = t.app.history_stamp().stamp;
    t.app.history_session_hide_online("a1", false).unwrap();
    let shown = t.app.history_stamp().stamp;
    assert_ne!(before, hidden);
    assert_ne!(
        hidden, shown,
        "showing the session again must refresh the screens too"
    );
    assert!(t.app.dirty.contains(&SliceName::History));
}

#[test]
fn hiding_a_session_online_is_sent_for_its_owner() {
    let mut t = test_app("hide-owner");
    let mut s = session("b1", "osoba-b");
    t.app.history.as_ref().unwrap().save(&mut s).unwrap();
    assert_eq!(t.app.history_session_hide_online("b1", true).unwrap(), None);
    signed_in(&mut t.app, "osoba-b");
    assert_eq!(
        t.app
            .history_session_hide_online("b1", false)
            .unwrap()
            .as_deref(),
        Some("osoba-b")
    );
}

#[test]
fn a_second_models_download_while_one_runs_is_busy() {
    let mut t = test_app("models-busy");
    assert!(t.app.begin_models_fetch().is_ok());
    let Err(second) = t.app.begin_models_fetch() else {
        panic!("second download started");
    };
    assert_eq!(second.code, ErrorCode::Busy);
}

#[test]
fn a_failed_import_keeps_the_opened_backup() {
    let mut t = test_app("import-kept");
    let file = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../crates/core/tests/fixtures/backup/full.bieznia");
    t.app.import_preview = Some(backup::read(&file).expect("backup fixture"));
    t.app.history = None;
    let choice = backup::ImportChoice {
        people: Default::default(),
        settings: false,
        results: true,
        new_person: Default::default(),
        keep_my_settings: true,
    };
    let Err(first) = t.app.import_apply(&choice) else {
        panic!("import without history succeeded");
    };
    assert_eq!(first.code, ErrorCode::Unavailable);
    assert!(
        t.app.import_preview.is_some(),
        "the backup must stay chosen"
    );
}

#[test]
fn a_failed_restore_opens_the_history_again() {
    let mut t = test_app("restore-failed");
    let folder = t
        .app
        .paths
        .data_files()
        .join(walklevel_core::store::BACKUP_DIR)
        .join(format!("{}test", backup::BEFORE_IMPORT));
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join(walklevel_core::store::STORE_FILE), b"{}").unwrap();
    fs::write(folder.join("historia.db"), b"this is not a database").unwrap();
    assert_eq!(
        backup::latest_backup(&t.app.paths.data_files()).as_deref(),
        Some(folder.as_path())
    );
    assert!(t.app.restore_latest_backup().is_err());
    assert!(
        t.app.history.is_some(),
        "history must not stay closed until restart"
    );
}
