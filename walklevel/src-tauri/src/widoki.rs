//! Typy kontraktu rdzeń -> interfejs (docs/kontrakt.md rozdz. 4-5): wycinki stanu, zdarzenia,
//! argumenty i wyniki komend. Nazwy pól w JSON-ie - camelCase, 1:1 z `src/lib/typy.ts`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

// --- błędy komend --------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)] // pełna lista kodów z kontraktu (część używa dopiero BLE i dialogi)
pub enum ErrorCode {
    Invalid,
    NotFound,
    Busy,
    Offline,
    LoggedOut,
    UpdateRequired,
    Server,
    Io,
    Device,
    Key,
    Cancelled,
    Unavailable,
}

/// Błąd komendy: komunikat po polsku gotowy do pokazania (odrzucona obietnica `invoke`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CmdError {
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, String>,
}

impl CmdError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            fields: BTreeMap::new(),
        }
    }
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Invalid, message)
    }
    pub fn io(e: impl std::fmt::Display) -> Self {
        Self::new(
            ErrorCode::Io,
            crate::teksty::t("wspolne.blad_zapisu", &[("szczegoly", &e.to_string())]),
        )
    }
    /// Polecenie, którego interfejs nie powinien wysłać (nieznana wartość) - szczegół do logu,
    /// użytkownik dostaje ogólny komunikat (wspolne.blad_nieznany).
    pub fn unexpected(detail: &str) -> Self {
        log::warn!("Nieoczekiwane polecenie: {detail}");
        Self::invalid(crate::teksty::t("wspolne.blad_nieznany", &[]))
    }
}

impl From<walklevel_core::account::KontoError> for CmdError {
    fn from(e: walklevel_core::account::KontoError) -> Self {
        use walklevel_core::account::KontoErrorKind as K;
        let code = match e.kind {
            K::Invalid => ErrorCode::Invalid,
            K::Offline => ErrorCode::Offline,
            K::LoggedOut => ErrorCode::LoggedOut,
            K::UpdateRequired => ErrorCode::UpdateRequired,
            K::Server => ErrorCode::Server,
            K::Io => ErrorCode::Io,
        };
        Self {
            code,
            message: e.message,
            fields: e.fields,
        }
    }
}

impl From<walklevel_core::store::StoreError> for CmdError {
    fn from(e: walklevel_core::store::StoreError) -> Self {
        Self::io(e)
    }
}

impl From<walklevel_core::history::HistoryError> for CmdError {
    fn from(e: walklevel_core::history::HistoryError) -> Self {
        Self::new(
            ErrorCode::Io,
            format!("{} ({e})", crate::teksty::t("historia.niedostepna", &[])),
        )
    }
}

impl From<std::io::Error> for CmdError {
    fn from(e: std::io::Error) -> Self {
        Self::io(e)
    }
}

pub type CmdResult<T> = Result<T, CmdError>;

// --- zdarzenia -----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SliceName {
    App,
    Settings,
    People,
    Device,
    Control,
    Levels,
    History,
    Account,
    Online,
    Update,
    Report,
    Models,
    Bubble,
}

impl SliceName {
    pub const ALL: [SliceName; 13] = [
        SliceName::App,
        SliceName::Settings,
        SliceName::People,
        SliceName::Device,
        SliceName::Control,
        SliceName::Levels,
        SliceName::History,
        SliceName::Account,
        SliceName::Online,
        SliceName::Update,
        SliceName::Report,
        SliceName::Models,
        SliceName::Bubble,
    ];
}

#[derive(Debug, Clone, Serialize)]
pub struct SliceEvent {
    pub rev: u64,
    pub slice: SliceName,
    pub value: Value,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ToastKind {
    Info,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct Toast {
    pub text: String,
    pub kind: ToastKind,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NavRequest {
    pub to: &'static str,
    pub reason: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_mode: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GaugeMode {
    Idle,
    Walk,
    Run,
    Offline,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PadTelemetry {
    pub name: Option<String>,
    pub deflection: f64,
    pub step: u32,
    pub status_text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Telemetry {
    pub t_ms: u64,
    pub speed_kmh: f64,
    pub live: bool,
    pub enabled: bool,
    pub mode: GaugeMode,
    pub active: i32,
    pub caption: String,
    pub steps: Option<i64>,
    pub cadence: Option<f64>,
    pub distance_km: f64,
    pub active_s: f64,
    pub belt_target_kmh: Option<f64>,
    pub pad: Option<PadTelemetry>,
}

// --- wspólne kawałki ----------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyBinding {
    pub scan: u16,
    pub extended: bool,
    pub name: String,
    pub vk: i64,
    pub mods: Vec<String>,
    pub label: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyCapture {
    pub code: String,
    pub key: String,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
    #[allow(dead_code)] // rozróżnienie lewy/prawy daje już `code`
    pub location: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IconRef {
    pub color: String,
    pub glyph: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Totals {
    pub steps: i64,
    pub distance_km: f64,
    pub active_s: f64,
    pub avg_kmh: f64,
}

impl From<&walklevel_core::model::Totals> for Totals {
    fn from(t: &walklevel_core::model::Totals) -> Self {
        Self {
            steps: t.steps,
            distance_km: t.distance_km,
            active_s: t.active_s,
            avg_kmh: t.avg_kmh(),
        }
    }
}

// --- wycinki -------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub title: String,
    pub installed: bool,
    pub data_dir: String,
    pub first_run: bool,
    pub legacy02: bool,
    pub updated_from: Option<String>,
    pub latest_backup: Option<String>,
    pub accounts_enabled: bool,
    pub report_endpoint: bool,
    pub window_focused: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub device_address: String,
    pub device_name: String,
    pub belt_control: bool,
    pub quick_stop: bool,
    pub toggle_key: KeyBinding,
    pub faster_key: KeyBinding,
    pub slower_key: KeyBinding,
    pub toggle_pad: String,
    pub simulator: bool,
    pub record: bool,
    pub detailed_logs: bool,
    pub theme: String,
    pub large_text: bool,
    pub always_on_top: bool,
    pub floating_button: bool,
    pub show_person_switch: bool,
    pub show_profile_switch: bool,
    pub achievement_popups: bool,
    pub achievement_sound: bool,
    /// panel znajomych z boku okna (0.6.2)
    pub friends_panel: bool,
    /// "right" / "left"
    pub friends_panel_side: String,
    /// zwinięty do paska 64 px
    pub friends_panel_collapsed: bool,
    /// komunikat w oknie, gdy znajomy zacznie chodzić
    pub friends_notify: bool,
    pub direction: String,
    pub output: String,
    pub walk_key: KeyBinding,
    pub run_key: KeyBinding,
    pub stand_below: f64,
    pub run_on: f64,
    pub run_off: f64,
    pub idle_speed: f64,
    pub walk_speed: f64,
    pub run_speed: f64,
    pub key_hold_s: f64,
    pub key_idle: String,
    pub pad_steps: Vec<(u32, f64)>,
    pub pad_idle: String,
    pub training_levels: [f64; 4],
    pub training_level: usize,
    pub profile_color: String,
    pub person_color: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSummary {
    pub direction: String,
    pub walk_speed: f64,
    pub run_speed: f64,
    pub pad_step_count: usize,
    pub pad_max_kmh: f64,
    pub training_min: f64,
    pub training_max: f64,
    pub stand_below: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameProfileView {
    pub id: String,
    pub name: String,
    pub color: String,
    pub icon_color: Option<String>,
    pub icon: IconRef,
    pub exe: Option<String>,
    pub online: bool,
    pub summary: ProfileSummary,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonView {
    pub id: String,
    pub name: String,
    pub color: String,
    pub active_profile_id: String,
    pub profiles: Vec<GameProfileView>,
    pub today_steps: i64,
    pub has_account: bool,
    pub account_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeopleView {
    pub ready: bool,
    pub active_person_id: String,
    pub active_profile_id: String,
    pub ask_at_start: bool,
    pub people: Vec<PersonView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamePick {
    pub pick_id: String,
    pub name: String,
    pub exe: String,
    pub exe_name: String,
    pub folder_name: String,
    pub icon: IconRef,
    pub icon_color: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfoView {
    pub model: String,
    pub driver: String,
    pub profile: String,
    pub name: String,
    pub firmware: String,
    pub simulated: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FoundDevice {
    pub address: String,
    pub name: String,
    pub rssi: i16,
    pub treadmill: bool,
    pub model: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkStatus {
    pub tone: &'static str,
    pub busy: bool,
    pub title: String,
    pub sub: String,
    pub action: &'static str,
    pub action_label: String,
    pub primary: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostics {
    pub running: bool,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceView {
    pub link: &'static str,
    pub link_text: String,
    pub status: LinkStatus,
    pub device: Option<DeviceInfoView>,
    pub caps: walklevel_core::treadmill::Capabilities,
    pub remembered: bool,
    pub label: String,
    pub scanning: bool,
    pub scan_text: String,
    pub devices: Vec<FoundDevice>,
    pub bluetooth_off: bool,
    pub diagnostics: Diagnostics,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlView {
    pub enabled: bool,
    pub direction: String,
    pub sets_speed: bool,
    pub hint: String,
    pub subtitle: String,
    pub toggle_label: String,
    pub walk_name: String,
    pub run_name: String,
    pub pad_button_label: String,
    pub pad_capturing: bool,
    pub key_capturing: Option<&'static str>,
    pub belt_commands: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RankView {
    pub level: u32,
    pub name: String,
    pub about: String,
}

impl From<&walklevel_core::levels::Rank> for RankView {
    fn from(r: &walklevel_core::levels::Rank) -> Self {
        Self {
            level: r.level,
            name: r.name.clone(),
            about: r.about.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AchievementProgress {
    pub id: String,
    pub earned_at: Option<f64>,
    pub current: f64,
    pub target: f64,
    pub fraction: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LevelsView {
    pub person_id: String,
    pub km: f64,
    pub xp: i64,
    pub level: u32,
    pub max_level: u32,
    pub level_floor_xp: i64,
    pub level_ceiling_xp: i64,
    pub level_fraction: f64,
    pub km_to_next_level: f64,
    pub rank: RankView,
    pub next_rank: Option<RankView>,
    pub earned_count: u32,
    pub total: u32,
    pub progress: Vec<AchievementProgress>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryStampView {
    pub available: bool,
    pub stamp: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryQuery {
    pub scope: String,
    pub range: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartPart {
    pub name: String,
    pub steps: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartBar {
    pub label: String,
    pub weekday: Option<u8>,
    pub parts: Vec<ChartPart>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryChart {
    pub hourly: bool,
    pub bars: Vec<ChartBar>,
    pub colors: BTreeMap<String, String>,
    pub legend: Vec<String>,
    pub empty: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryGroup {
    pub name: String,
    pub totals: Totals,
    pub share: f64,
    pub color: String,
    pub icon: Option<IconRef>,
    pub avatar_color: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineMark {
    pub hidden: bool,
    pub sent: bool,
    pub rejected: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRow {
    pub id: String,
    pub start: f64,
    pub end: f64,
    pub person_id: String,
    pub person_name: String,
    pub profile_id: String,
    pub profile_name: String,
    pub steps: i64,
    pub distance_km: f64,
    pub active_s: f64,
    pub avg_kmh: f64,
    pub source: String,
    pub mode: String,
    pub icon: IconRef,
    pub online: Option<OnlineMark>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDetail {
    #[serde(flatten)]
    pub row: SessionRow,
    pub cadence: Option<f64>,
    pub whole_s: f64,
    pub device_model: String,
    pub driver: String,
    pub firmware: String,
    pub app_version: String,
    pub tz_offset_min: i64,
    pub linked: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryView {
    pub query: HistoryQuery,
    pub stamp: String,
    pub many: bool,
    pub everyone: bool,
    pub totals: Totals,
    pub chart: HistoryChart,
    pub group_key: &'static str,
    pub groups: Vec<HistoryGroup>,
    pub recent: Vec<SessionRow>,
    pub online: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountView {
    pub enabled: bool,
    pub person_id: String,
    pub logged_in: bool,
    pub user_id: String,
    pub name: String,
    pub email: String,
    pub verified: bool,
    pub upload: bool,
    pub visibility: String,
    pub live: bool,
    /// status dla znajomych: wszystko / chodze / niewidoczny
    pub live_status: String,
    /// opis dla znajomych
    pub description: String,
    pub profile_visibility: String,
    pub games_visibility: String,
    pub terms: String,
    pub terms_current: String,
    pub terms_outdated: bool,
    pub games_allowed: bool,
    pub remember: bool,
    pub terms_url: String,
    pub busy: Vec<String>,
    pub message: String,
    pub verify_polling: bool,
    pub pending_uploads: u32,
}

// dane online z serwera - typy i ich odczyt w rdzeniu
pub use walklevel_core::account::online::{
    AchievementStats, Friend, Invitations, OnlinePerson, OnlineStats, PeoplePage, Ranking,
    RegistrationCode,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeopleQuery {
    pub search: String,
    pub page: u32,
}

impl Default for PeopleQuery {
    fn default() -> Self {
        Self {
            search: String::new(),
            page: 1,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineView {
    pub person_id: String,
    pub loading: Vec<String>,
    pub stats: Option<OnlineStats>,
    pub friends: Option<Vec<Friend>>,
    pub invitations: Option<Invitations>,
    pub ranking: Option<Ranking>,
    pub people: Option<PeoplePage>,
    pub people_query: PeopleQuery,
    pub person: Option<OnlinePerson>,
    pub reg_code: Option<RegistrationCode>,
    /// ile procent kont ma każde osiągnięcie (ekran Osiągnięcia; pobierane najwyżej raz na dobę)
    pub achievement_stats: Option<AchievementStats>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateView {
    pub available: Option<String>,
    pub busy: bool,
    pub status: String,
    pub installing: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportView {
    pub busy: bool,
    pub status: String,
    pub last_file: Option<String>,
    pub sent_number: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelItem {
    pub id: String,
    pub name: String,
    pub driver: String,
    pub driver_name: String,
    pub r#match: Vec<String>,
    pub caps: walklevel_core::treadmill::Capabilities,
    pub generic: bool,
    pub mine: bool,
    pub source: String,
    pub removable: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelsView {
    pub items: Vec<ModelItem>,
    pub busy: bool,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BubbleView {
    pub visible: bool,
    pub enabled: bool,
    pub theme: String,
    pub accent: String,
    /// tryb Trening: przy kółku +, − i Stop (okno większe, okna::BUBBLE_TRAINING_*)
    pub training: bool,
    /// poza Treningiem: taśma jedzie (histereza 1 s) i przyjmuje stop - pod kółkiem Stop
    /// (okno 56 × 84, okna::BUBBLE_STOP_H)
    pub stop: bool,
    /// poziom Treningu 0-3
    pub training_level: u32,
    /// nazwy klawiszy szybciej/wolniej (podpowiedzi + i −)
    pub faster_label: String,
    pub slower_label: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PopupItem {
    pub id: u64,
    pub kicker: String,
    pub title: String,
    pub line: String,
    pub color: String,
    pub rarity: String,
    pub mark: String,
    pub badge_text: Option<String>,
    pub progress: Option<f64>,
    pub leaving: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PopupsView {
    pub items: Vec<PopupItem>,
}

// --- argumenty komend ----------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WizardGame {
    pub pick_id: String,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WizardResult {
    pub mode: String,
    pub name: String,
    pub color: String,
    pub copy_profiles: bool,
    pub game: Option<WizardGame>,
    pub direction: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WizardSeed {
    pub name: String,
    pub suggestions: Vec<String>,
    pub color: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileDraft {
    pub name: String,
    pub color: String,
    pub icon_color: Option<String>,
    pub glyph: Option<String>,
    pub pick_id: Option<String>,
    pub copy_from_active: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfilePatch {
    pub name: Option<String>,
    pub color: Option<String>,
    pub glyph: Option<String>,
    pub pick_id: Option<String>,
    pub online: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonPatch {
    pub name: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportChoice {
    pub people: BTreeMap<String, Vec<String>>,
    pub settings: bool,
    pub results: bool,
    pub logs: bool,
    pub range: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSummary {
    pub people: u32,
    pub profiles: u32,
    pub sessions: u32,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPerson {
    pub id: String,
    pub name: String,
    pub color: String,
    pub profiles: u32,
    pub sessions: u32,
    pub exists: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub file: String,
    pub app_version: String,
    pub has_settings: bool,
    pub has_results: bool,
    pub description: String,
    pub people: Vec<ImportPerson>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportChoice {
    pub people: Vec<String>,
    pub settings: bool,
    pub results: bool,
    pub new_person: Vec<String>,
    pub keep_my_settings: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub people_added: u32,
    pub people_merged: u32,
    pub profiles_added: u32,
    pub profiles_updated: u32,
    pub sessions_added: u32,
    pub backup: Option<String>,
}
