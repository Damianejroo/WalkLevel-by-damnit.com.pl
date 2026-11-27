// Language of the interface (docs/i18n.md). The built-in pack (English, Polish texts for keys
// not translated yet) is bundled; the core sends the packs of the shown language
// (`language_pack`) and the language slice tells when to ask again. Lookup: packs of the
// language and its BCP 47 parents, then the built-in pack. Messages are ICU MessageFormat.
import builtIn from './texts-built-in.json';
import { formatMessage, parseMessage, MessageError, type FormatContext, type Part, type Values } from './message-format';
import { localeFormat, type LocaleFormat, type NumberStyle } from './locale-format';

export type TextKey = keyof typeof builtIn.texts;
export type { Values };

export interface PackView {
  code: string;
  direction: string;
  overlays: { code: string; texts: Record<string, string> }[];
}

interface Overlay {
  code: string;
  format: LocaleFormat;
  texts: Record<string, string>;
}

const texts: Record<string, string> = builtIn.texts;
const fromSource = new Set<string>(builtIn.fromSource);
const formats = new Map<string, LocaleFormat>();

function formatFor(code: string): LocaleFormat {
  let f = formats.get(code);
  if (!f) {
    const style = (builtIn.languages as { code: string; number?: NumberStyle }[]).find((l) => l.code === code)?.number;
    f = localeFormat(code, style);
    formats.set(code, f);
  }
  return f;
}

/** Shown language; `revision` changes with every pack, so texts in views read it. */
export const language = $state({ code: builtIn.language, direction: 'ltr', revision: 0 });

let overlays: Overlay[] = [];
let shown = formatFor(builtIn.language);
const parsed = new Map<string, Part[] | null>();

export function isTextKey(key: string): key is TextKey {
  return Object.hasOwn(texts, key);
}

/** Packs of the shown language from the core (or the mock core). */
export function applyPack(pack: PackView) {
  overlays = pack.overlays.map((o) => ({ code: o.code, texts: o.texts, format: formatFor(o.code) }));
  shown = formatFor(pack.code);
  language.code = pack.code;
  language.direction = pack.direction === 'rtl' ? 'rtl' : 'ltr';
  language.revision++;
  const root = document.documentElement;
  root.lang = pack.code;
  root.dir = language.direction;
}

function parse(text: string): Part[] | null {
  let parts = parsed.get(text);
  if (parts === undefined) {
    try {
      parts = parseMessage(text);
    } catch (e) {
      if (!(e instanceof MessageError)) throw e;
      console.error(e.message);
      parts = null;
    }
    parsed.set(text, parts);
  }
  return parts;
}

/** Text of a key with variables. A missing variable stays visible as `{name}`. */
export function translate(key: TextKey, values: Values = {}): string {
  void language.revision;
  let pattern: string | undefined;
  let rules = shown;
  for (const o of overlays) {
    if (Object.hasOwn(o.texts, key)) {
      pattern = o.texts[key];
      rules = o.format;
      break;
    }
  }
  if (pattern === undefined) {
    pattern = texts[key];
    if (pattern === undefined) {
      console.error(`No text ${key} in docs/texts.md`);
      return key;
    }
    rules = formatFor(fromSource.has(key) ? builtIn.source : builtIn.language);
  }
  const parts = parse(pattern);
  if (!parts) return pattern;
  const ctx: FormatContext = {
    plural(value) {
      const n = typeof value === 'number'
        ? { value, fractionDigits: Number.isInteger(value) ? 0 : 1 }
        : shown.parse(value);
      return n === null ? null : { category: rules.plural(Math.abs(n.value), n.fractionDigits), value: n.value };
    },
    number: (value) => shown.number(value, Number.isInteger(value) ? 0 : 1),
  };
  return formatMessage(parts, values, ctx);
}

/** Number in the shown language: `places` digits after the decimal separator. */
export function formatNumber(value: number, places = 0): string {
  void language.revision;
  return shown.number(value, places);
}
