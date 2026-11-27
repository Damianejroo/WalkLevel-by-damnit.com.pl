// Packs of the shown language for each window (docs/i18n.md): loaded before the first render
// and again when the `language` slice reports another shown language.
import * as api from './api';
import { applyPack, language } from './i18n.svelte';

/** Loads the packs of the shown language; without them the built-in pack stays. */
export async function loadLanguagePack(): Promise<void> {
  try {
    applyPack(await api.languagePack());
  } catch (e) {
    console.error('Language pack unavailable', e);
  }
}

/** Follows the core: another shown language -> new packs, then `after` (e.g. fresh statics). */
export function followLanguage(shown: string, after?: () => Promise<void>): void {
  if (shown === language.code) return;
  void loadLanguagePack().then(() => after?.());
}

/** For windows without the state store (popup, floating button). */
export async function watchLanguage(): Promise<void> {
  await api.sluchaj('wl:stan', (ev) => {
    if (ev.slice === 'language') followLanguage((ev.value as { shown: string }).shown);
  });
}
