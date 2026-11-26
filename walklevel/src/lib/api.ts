// Jedyne miejsce z invoke/listen w interfejsie (CLAUDE.md, docs/kontrakt.md rozdz. 6).
// Ekrany wołają tylko funkcje z tego pliku. W zwykłej przeglądarce (bez Tauri) te same
// funkcje obsługuje atrapa rdzenia (src/lib/atrapa.ts), ładowana dopiero wtedy.
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type * as K from './typy';
import { t } from './teksty';

/** Okno działa w Tauri (WebView2 z rdzeniem), a nie w zwykłej przeglądarce. */
export const wTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

export interface Silnik {
  wywolaj(komenda: string, argumenty?: Record<string, unknown>): Promise<unknown>;
  sluchaj(zdarzenie: string, obsluga: (tresc: unknown) => void): Promise<() => void>;
}

let silnik: Promise<Silnik> | null = null;

function pobierzSilnik(): Promise<Silnik> {
  silnik ??= wTauri
    ? Promise.resolve<Silnik>({
        wywolaj: (komenda, argumenty) => invoke(komenda, argumenty),
        sluchaj: (zdarzenie, obsluga) => listen(zdarzenie, (e) => obsluga(e.payload)),
      })
    : import('./atrapa').then((m) => m.utworzAtrape());
  return silnik;
}

/** Odrzucona obietnica komendy zawsze niesie CmdError (komunikat po polsku). */
export function jestBledemKomendy(e: unknown): e is K.CmdError {
  return typeof e === 'object' && e !== null && 'code' in e && 'message' in e;
}

function naBlad(e: unknown): K.CmdError {
  if (jestBledemKomendy(e)) return e;
  // Nieprzewidziany błąd (np. komenda bez rejestracji) - nigdy surowy komunikat dla gracza.
  console.error('Błąd komendy', e);
  return { code: 'server', message: t('wspolne.blad_nieznany') };
}

async function wywolaj<T>(komenda: string, argumenty?: Record<string, unknown>): Promise<T> {
  const s = await pobierzSilnik();
  try {
    return (await s.wywolaj(komenda, argumenty)) as T;
  } catch (e) {
    throw naBlad(e);
  }
}

/** Nasłuch zdarzenia rdzenia; zwraca funkcję kończącą nasłuch. */
export async function sluchaj<N extends keyof K.Zdarzenia>(
  zdarzenie: N,
  obsluga: (tresc: K.Zdarzenia[N]) => void,
): Promise<() => void> {
  const s = await pobierzSilnik();
  return s.sluchaj(zdarzenie, (tresc) => obsluga(tresc as K.Zdarzenia[N]));
}

// ============================================================================
// 6.1 Start, okna, ogólne
// ============================================================================

export const appSnapshot = () => wywolaj<K.AppSnapshot>('app_snapshot');
export const uiReady = (window: K.WindowLabel) => wywolaj<void>('ui_ready', { window });
export const appQuit = () => wywolaj<void>('app_quit');
export const windowShowMain = () => wywolaj<void>('window_show_main');
export const openFolder = (kind: K.FolderKind) => wywolaj<void>('open_folder', { kind });
export const openLink = (kind: K.LinkKind) => wywolaj<void>('open_link', { kind });
export const logTail = () => wywolaj<string[]>('log_tail');
export const diagnosticsRun = () => wywolaj<string>('diagnostics_run');

// ============================================================================
// 6.2 Ustawienia
// ============================================================================

export const settingsSet = <S extends K.SettingKey>(key: S, value: K.Settings[S]) =>
  wywolaj<void>('settings_set', { key, value });
export const thresholdsSet = (which: K.ThresholdWhich, kmh: number) =>
  wywolaj<void>('thresholds_set', { which, kmh });
export const padStepsCount = (count: 2 | 3 | 4 | 5) => wywolaj<void>('pad_steps_count', { count });
export const padStepSet = (index: number, at: number | null, kmh: number | null) =>
  wywolaj<void>('pad_step_set', { index, at, kmh });
export const trainingSpeedSet = (index: 0 | 1 | 2 | 3, kmh: number) =>
  wywolaj<void>('training_speed_set', { index, kmh });
export const keyCapture = (role: K.KeyRole | null) => wywolaj<void>('key_capture', { role });
export const keySet = (role: K.KeyRole, key: K.KeyCapture) => wywolaj<K.KeyBinding>('key_set', { role, key });
export const padCaptureStart = () => wywolaj<void>('pad_capture_start');
export const padCaptureCancel = () => wywolaj<void>('pad_capture_cancel');

// ============================================================================
// 6.3 Sterowanie i trening
// ============================================================================

/** `keyboard` - przełącznik użyty Enterem albo spacją (dopisek w dzienniku rdzenia). */
export const controlSet = (on: boolean, keyboard = false) => wywolaj<void>('control_set', { on, keyboard });
export const controlToggle = () => wywolaj<void>('control_toggle');
export const trainingLevelSet = (level: 0 | 1 | 2 | 3) => wywolaj<void>('training_level_set', { level });
export const trainingLevelStep = (step: 1 | -1) => wywolaj<void>('training_level_step', { step });
export const trainingStop = () => wywolaj<void>('training_stop');

// ============================================================================
// 6.4 Bieżnia
// ============================================================================

export const deviceScan = () => wywolaj<void>('device_scan');
export const deviceConnect = (address: string, name: string) => wywolaj<void>('device_connect', { address, name });
export const deviceDisconnect = () => wywolaj<void>('device_disconnect');
export const deviceBeltStop = () => wywolaj<void>('device_belt_stop');
export const deviceForget = () => wywolaj<void>('device_forget');
export const deviceStatusAction = () => wywolaj<void>('device_status_action');
export const modelsRefresh = () => wywolaj<void>('models_refresh');
export const modelsFetch = () => wywolaj<string>('models_fetch');
export const modelsAddFile = () => wywolaj<string | null>('models_add_file');
export const modelsRemove = (id: string) => wywolaj<void>('models_remove', { id });

// ============================================================================
// 6.5 Osoby i profile gier
// ============================================================================

export const wizardSeed = (mode: K.WizardMode) => wywolaj<K.WizardSeed>('wizard_seed', { mode });
export const nameRandom = () => wywolaj<string>('name_random');
export const wizardFinish = (result: K.WizardResult) => wywolaj<void>('wizard_finish', { result });
export const personSwitch = (personId: string) => wywolaj<void>('person_switch', { personId });
export const personUpdate = (personId: string, patch: K.PersonPatch) => wywolaj<void>('person_update', { personId, patch });
export const personRemove = (personId: string) => wywolaj<void>('person_remove', { personId });
/** `from` i `keyboard` trafiają tylko do dziennika rdzenia (skąd przyszła zmiana profilu). */
export const profileSwitch = (profileId: string, from: 'main' | 'settings', keyboard = false) =>
  wywolaj<void>('profile_switch', { profileId, from, keyboard });
export const gamePick = (kind: K.GamePickKind) => wywolaj<K.GamePick | null>('game_pick', { kind });
export const gamePickDiscard = (pickId: string) => wywolaj<void>('game_pick_discard', { pickId });
export const profileCreate = (draft: K.ProfileDraft) => wywolaj<string>('profile_create', { draft });
export const profileUpdate = (profileId: string, patch: K.ProfilePatch) =>
  wywolaj<void>('profile_update', { profileId, patch });
export const profileDuplicate = (profileId: string) => wywolaj<string>('profile_duplicate', { profileId });
export const profileRemove = (profileId: string) => wywolaj<void>('profile_remove', { profileId });

// ============================================================================
// 6.6 Historia
// ============================================================================

export const historyQuery = (query: K.HistoryQuery) => wywolaj<K.HistoryView>('history_query', { query });
export const historySession = (sessionId: string) => wywolaj<K.SessionDetail | null>('history_session', { sessionId });
export const historySessionDelete = (sessionId: string) => wywolaj<void>('history_session_delete', { sessionId });
export const historySessionHideOnline = (sessionId: string, hidden: boolean) =>
  wywolaj<void>('history_session_hide_online', { sessionId, hidden });

// ============================================================================
// 6.7 Poziomy i osiągnięcia
// ============================================================================

export const levelsRefresh = () => wywolaj<void>('levels_refresh');
export const levelsSamplePopup = () => wywolaj<void>('levels_sample_popup');

// ============================================================================
// 6.8 Konto online (wybranej osoby)
// ============================================================================

export const accountLogin = (email: string, password: string, newPerson: boolean) =>
  wywolaj<K.AccountView>('account_login', { email, password, newPerson });
export const accountRegister = (code: string, email: string, password: string, name: string, newPerson: boolean) =>
  wywolaj<K.AccountView>('account_register', { code, email, password, name, newPerson });
export const accountRemindPassword = (email: string) => wywolaj<void>('account_remind_password', { email });
export const accountLogout = () => wywolaj<void>('account_logout');
export const accountDelete = (password: string) => wywolaj<void>('account_delete', { password });
export const accountChangePassword = (oldPassword: string, newPassword: string) =>
  wywolaj<void>('account_change_password', { oldPassword, newPassword });
export const accountSendVerification = () => wywolaj<K.VerificationResult>('account_send_verification');
export const accountCheckVerified = (force: boolean) => wywolaj<boolean>('account_check_verified', { force });
export const accountSetRemember = (on: boolean) => wywolaj<void>('account_set_remember', { on });
export const accountSetUpload = (on: boolean) => wywolaj<void>('account_set_upload', { on });
export const accountSetVisibility = (value: K.SessionVisibility) => wywolaj<void>('account_set_visibility', { value });
export const accountSetLive = (on: boolean) => wywolaj<void>('account_set_live', { on });
export const accountSetProfileVisibility = (value: K.ProfileVisibility) =>
  wywolaj<void>('account_set_profile_visibility', { value });
export const accountSetGamesVisibility = (value: K.GamesVisibility) =>
  wywolaj<void>('account_set_games_visibility', { value });
export const accountRename = (name: string) => wywolaj<void>('account_rename', { name });
export const accountAcceptTerms = () => wywolaj<void>('account_accept_terms');
export const accountUploadNow = () => wywolaj<number>('account_upload_now');
export const accountExportMyData = () => wywolaj<string>('account_export_my_data');

// ============================================================================
// 6.9 Dane online
// ============================================================================

export const onlineLoad = (tab: K.OnlineTab, force: boolean) => wywolaj<void>('online_load', { tab, force });
export const onlineRankingScope = (scope: K.RankingScope) => wywolaj<void>('online_ranking_scope', { scope });
export const onlinePeopleSearch = (search: string, page: number) =>
  wywolaj<void>('online_people_search', { search, page });
export const onlinePersonOpen = (userId: string) => wywolaj<K.OnlinePerson>('online_person_open', { userId });
export const onlineInviteName = (name: string) => wywolaj<K.InviteState>('online_invite_name', { name });
export const onlineInviteUser = (userId: string) => wywolaj<K.InviteState>('online_invite_user', { userId });
export const onlineInvitationAnswer = (invitationId: string, accept: boolean) =>
  wywolaj<void>('online_invitation_answer', { invitationId, accept });
export const onlineInvitationWithdraw = (invitationId: string) =>
  wywolaj<void>('online_invitation_withdraw', { invitationId });
export const onlineFriendRemove = (friendId: string) => wywolaj<void>('online_friend_remove', { friendId });
export const onlineRegistrationCode = () => wywolaj<K.RegistrationCode>('online_registration_code');

// ============================================================================
// 6.10 Kopia danych
// ============================================================================

export const backupExportSummary = (choice: K.ExportChoice) =>
  wywolaj<K.ExportSummary>('backup_export_summary', { choice });
export const backupExport = (choice: K.ExportChoice) => wywolaj<K.ExportSummary | null>('backup_export', { choice });
export const backupImportOpen = () => wywolaj<K.ImportPreview | null>('backup_import_open');
export const backupImportApply = (choice: K.ImportChoice) => wywolaj<K.ImportReport>('backup_import_apply', { choice });
export const backupRestoreLatest = () => wywolaj<void>('backup_restore_latest');

// ============================================================================
// 6.11 Zgłoszenia, 6.12 Aktualizacje
// ============================================================================

export const reportSend = (description: string, contact: string, periodS: number) =>
  wywolaj<void>('report_send', { description, contact, periodS });
export const reportSave = (description: string, contact: string, periodS: number) =>
  wywolaj<string>('report_save', { description, contact, periodS });
export const updateCheck = (install: boolean) => wywolaj<void>('update_check', { install });

// ============================================================================
// 6.13 Okna pomocnicze (przycisk, popup)
// ============================================================================

export const bubbleClick = () => wywolaj<void>('bubble_click');
export const bubbleOpen = () => wywolaj<void>('bubble_open');
export const bubbleDragStart = () => wywolaj<void>('bubble_drag_start');
export const bubbleDragEnd = () => wywolaj<void>('bubble_drag_end');
export const popupReady = () => wywolaj<K.PopupsView>('popup_ready');
