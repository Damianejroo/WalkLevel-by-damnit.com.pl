// Language in the mock core (docs/i18n.md): `?jezyk=pl|en|en-XA|de`, default Polish. Packs come
// from locales/ in the repository, en-XA is the pseudo-language of the view tests. Applied at
// once, so the texts the mock writes into its state are in the same language as the views.
//   jezyk-siec=brak   downloading a language fails as without internet (error card)
import type * as K from '../lib/contract';
import builtIn from '../lib/texts-built-in.json';
import { applyPack } from '../lib/i18n.svelte';
import { pseudoTexts } from './pseudo-locale';

const files = import.meta.glob<Record<string, string>>('/locales/*/*.json', { eager: true, import: 'default' });

function packTexts(code: string): Record<string, string> | null {
  const out: Record<string, string> = {};
  let found = false;
  for (const [file, data] of Object.entries(files)) {
    if (file.split('/')[2] !== code) continue;
    found = true;
    for (const [k, v] of Object.entries(data)) if (!k.startsWith('@')) out[k] = v;
  }
  return found ? out : null;
}

const PSEUDO = 'en-XA';
const GERMAN: K.LanguageEntry = {
  code: 'de', name: 'Deutsch', englishName: 'German', status: 'in-progress', direction: 'ltr', availability: 'download',
};

export function createMockLanguage(params: URLSearchParams) {
  const languages: K.LanguageEntry[] = [
    ...(builtIn.languages as Omit<K.LanguageEntry, 'availability'>[]).map((l) => ({
      ...l,
      availability: (l.code === builtIn.language ? 'built-in' : 'installed') as K.LanguageAvailability,
    })),
    { ...GERMAN },
  ];
  const view: K.LanguageView = { chosen: 'pl', shown: 'pl', direction: 'ltr', languages, download: null, error: null };

  function pack(): K.LanguagePack {
    if (view.shown === PSEUDO) return { code: PSEUDO, direction: 'ltr', overlays: [{ code: PSEUDO, texts: pseudoTexts(builtIn.texts, builtIn.sameEverywhere) }] };
    const texts = view.shown === builtIn.language ? null : packTexts(view.shown);
    return { code: view.shown, direction: view.direction, overlays: texts ? [{ code: view.shown, texts }] : [] };
  }

  // later changes go the way of the real core: slice event -> language_pack
  function show(code: string) {
    view.chosen = code;
    view.shown = code;
  }

  const start = params.get('jezyk');
  show(start === PSEUDO || languages.some((l) => l.code === start) ? start! : 'pl');
  applyPack(pack());

  return {
    view,
    pack,
    /** `language_set`: an installed language at once, German after a short "download". */
    set(code: string, changed: () => void, failed: () => void): Promise<void> {
      const entry = languages.find((l) => l.code === code);
      if (!entry) return Promise.resolve();
      view.error = null;
      if (entry.availability !== 'download') {
        show(code);
        changed();
        return Promise.resolve();
      }
      view.chosen = code;
      return new Promise((done) => {
        let step = 0;
        const total = builtIn.namespaces.length;
        const timer = setInterval(() => {
          step = Math.min(total, step + 12);
          view.download = { code, done: step, total };
          changed();
          if (params.get('jezyk-siec') === 'brak' && step >= 12) {
            clearInterval(timer);
            view.download = null;
            failed();
            done();
          } else if (step >= total) {
            clearInterval(timer);
            view.download = null;
            entry.availability = 'downloaded';
            show(code);
            changed();
            done();
          }
        }, 150);
      });
    },
  };
}
