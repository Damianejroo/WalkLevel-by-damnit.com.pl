// Language packs (docs/i18n.md): docs/texts.md (Polish, the source of keys) -> locales/pl/*.json,
// checks of the other languages in locales/<code>/*.json, the built-in pack
// src/lib/texts-built-in.json and CLDR samples for the Rust tests.
//   npm run texts                        write the generated files
//   node tests-ui/texts.ts --check       only check (npm run check, npm run test:ui)
//   node tests-ui/texts.ts --missing=en[:namespace]
//                                        print untranslated keys of a language as JSON (Polish text)
// Messages are ICU MessageFormat. Generated files are never edited by hand.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { MessageError, argumentsOf, parseMessage, type Part } from '../src/lib/message-format.ts';
import { localeFormat, pluralCategories, type NumberStyle } from '../src/lib/locale-format.ts';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const SOURCE_MD = path.join(ROOT, 'docs', 'texts.md');
const DEFINITIONS = path.join(ROOT, 'crates', 'core', 'assets', 'definicje.json');
const LOCALES = path.join(ROOT, 'locales');
const MANIFEST = path.join(LOCALES, 'languages.json');
const BUILT_IN = 'src/lib/texts-built-in.json';
const CLDR_SAMPLES = 'crates/core/tests/fixtures/texts/cldr-samples.json';
/** Marker in the note column of docs/texts.md: the text is the same in every language. */
const SAME_EVERYWHERE = /bez tłumaczenia/i;

export interface Language {
  /** BCP 47 tag */
  code: string;
  name: string;
  englishName: string;
  /** complete: every key is required; in-progress: missing keys fall back to the built-in pack */
  status: 'complete' | 'in-progress';
  direction: 'ltr' | 'rtl';
  number?: NumberStyle;
}

export interface Manifest {
  format: number;
  source: string;
  fallback: string;
  languages: Language[];
}

interface SourceText {
  text: string;
  /** context for translators (ARB `description`) */
  note: string;
  sameEverywhere: boolean;
}

/** Key from a name on screen: lower case, no Polish letters, spaces -> _. */
const slug = (s: string) =>
  s.normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/ł/g, 'l').replace(/Ł/g, 'L')
    .toLowerCase().replace(/[^a-z0-9]+/g, '_').replace(/^_|_$/g, '');

function cells(line: string): string[] {
  const out: string[] = [];
  let current = '';
  let inCode = false;
  for (let i = 1; i < line.length; i++) {
    const c = line[i];
    if (c === '`') inCode = !inCode;
    if (c === '|' && !inCode && line[i - 1] !== '\\') { out.push(current.trim()); current = ''; }
    else current += c;
  }
  return out;
}

const cellText = (s: string) => s.replace(/\\n/g, '\n').replace(/\\\|/g, '|');

function readSourceTexts(): { texts: Map<string, SourceText>; variables: Map<string, string> } {
  const md = fs.readFileSync(SOURCE_MD, 'utf8');
  const texts = new Map<string, SourceText>();
  const variables = new Map<string, string>();
  let section = '';
  const add = (key: string, text: string, note: string, sameEverywhere: boolean) => {
    if (texts.has(key)) throw new Error(`Repeated key ${key}`);
    texts.set(key, { text: cellText(text), note: cellText(note), sameEverywhere });
  };
  for (const line of md.split(/\r?\n/)) {
    if (line.startsWith('## ') || line.startsWith('### ')) section = line;
    if (section.startsWith('### 0.5') && line.startsWith('| `{')) {
      const [names, meaning] = cells(line);
      for (const m of names.matchAll(/`\{([a-z_0-9]+)\}`/g)) variables.set(m[1], meaning);
      continue;
    }
    if (!line.startsWith('| `')) continue;
    // section 0 (conventions) and 28 (texts of definicje.json, see readDefinitionTexts) are not keys
    if (section.startsWith('## 0.') || section.startsWith('### 0.') || section.startsWith('## 28.')) continue;
    const [keyCell, before, after, note = ''] = cells(line);
    const text = after === 'bez zmian' ? before : after;
    const same = SAME_EVERYWHERE.test(note);
    const range = keyCell.match(/^`([a-z0-9_.-]+)\.[a-z0-9_-]+` … `[a-z0-9_.-]+`$/);
    const star = keyCell.match(/^`([a-z0-9_.-]+)\.\*`$/);
    if (range || star) {
      const parent = (range ?? star)![1];
      for (const name of before.split(',').map((s) => s.trim())) add(`${parent}.${slug(name)}`, name, note, same);
      continue;
    }
    const single = keyCell.match(/^`([a-z0-9_.-]+)`$/);
    if (!single) throw new Error(`Unknown key: ${keyCell}`);
    const key = single[1];
    // descriptions instead of a text (e.g. „{jeden z ble.powod.*}”, „(nazwy.py: …)”)
    if (/^\{jeden z /.test(text) || /^\(`/.test(text)) continue;
    add(key, text, note, same);
    // „Analogicznie: Kółko, Kwadrat, …” in the note - siblings of the key
    const analog = note.match(/^Analogicznie: (.+?)\.?$/);
    if (analog) {
      const parent = key.slice(0, key.lastIndexOf('.'));
      for (const name of analog[1].split(',').map((s) => s.trim())) add(`${parent}.${slug(name)}`, name, '', same);
    }
  }
  return { texts, variables };
}

/**
 * Names and descriptions of ranks and achievements. definicje.json stays as it is (the account
 * server reads the same file); the `levels` namespace carries its texts for translation.
 */
function readDefinitionTexts(): Map<string, SourceText> {
  const defs = JSON.parse(fs.readFileSync(DEFINITIONS, 'utf8')) as {
    rangi: { od_poziomu: number; nazwa: string; opis: string }[];
    osiagniecia: { id: string; nazwa: string; klimat: string; warunek: string }[];
  };
  const out = new Map<string, SourceText>();
  const add = (key: string, text: string, note: string) => out.set(key, { text, note, sameEverywhere: false });
  for (const r of defs.rangi) {
    add(`levels.rank.${r.od_poziomu}.name`, r.nazwa, `Nazwa rangi od poziomu ${r.od_poziomu} (postać z gry, książki albo filmu).`);
    add(`levels.rank.${r.od_poziomu}.about`, r.opis, `Opis rangi od poziomu ${r.od_poziomu}: ile ta postać chodzi.`);
  }
  for (const a of defs.osiagniecia) {
    if (!/^[a-z0-9_]+$/.test(a.id)) throw new Error(`Achievement id ${a.id} cannot be a text key`);
    add(`levels.achievement.${a.id}.name`, a.nazwa, `Nazwa osiągnięcia ${a.id}.`);
    add(`levels.achievement.${a.id}.flavor`, a.klimat, `Klimat osiągnięcia ${a.id}: zdanie ze świata gry albo książki.`);
    add(`levels.achievement.${a.id}.condition`, a.warunek, `Warunek osiągnięcia ${a.id}, sucho.`);
  }
  return out;
}

export const namespaceOf = (key: string) => key.slice(0, key.indexOf('.'));
/** File of a namespace: kebab-case name (profil_online -> profil-online.json). */
export const namespaceFile = (namespace: string) => `${namespace.replace(/_/g, '-')}.json`;

function parse(key: string, text: string, problems: string[]): Part[] | null {
  try {
    return parseMessage(text);
  } catch (e) {
    if (!(e instanceof MessageError)) throw e;
    problems.push(`${key}: ${e.message}`);
    return null;
  }
}

/** Problems of one text of `language` (the source text too) against the source message. */
function checkText(key: string, source: Part[], text: string, language: Language, isSource: boolean): string[] {
  const problems: string[] = [];
  const parts = parse(key, text, problems);
  if (!parts) return problems;
  const want = argumentsOf(source);
  const got = argumentsOf(parts);
  const names = (m: Map<string, unknown>) => [...m.keys()].sort().join(', ') || '-';
  if (names(want) !== names(got)) problems.push(`${key}: variables ${names(got)} instead of ${names(want)}`);
  const categories = pluralCategories(language.code);
  for (const [name, arg] of got) {
    for (const [i, options] of arg.options.entries()) {
      if (!arg.types.has('plural')) {
        if (!isSource && want.get(name)?.options[i] && [...options].some((o) => !want.get(name)!.options[i].has(o))) {
          problems.push(`${key}: {${name}, select} options differ from the source`);
        }
        continue;
      }
      const wrong = [...options].filter((o) => !o.startsWith('=') && !categories.includes(o));
      const lacking = categories.filter((c) => !options.has(c));
      if (wrong.length) problems.push(`${key}: {${name}, plural} has ${wrong.join(', ')}; ${language.code} uses ${categories.join(', ')}`);
      if (lacking.length) problems.push(`${key}: {${name}, plural} lacks ${lacking.join(', ')} for ${language.code}`);
    }
  }
  return problems;
}

function readJson(file: string): unknown {
  return JSON.parse(fs.readFileSync(file, 'utf8').replace(/^﻿/, ''));
}

export function readManifest(): Manifest {
  return readJson(MANIFEST) as Manifest;
}

/** Translations of one language from locales/<code>/*.json and problems of their layout. */
function readTranslations(code: string, keys: Map<string, SourceText>): { texts: Map<string, string>; problems: string[] } {
  const dir = path.join(LOCALES, code);
  const texts = new Map<string, string>();
  const problems: string[] = [];
  if (!fs.existsSync(dir)) return { texts, problems };
  const files = new Map([...new Set([...keys.keys()].map(namespaceOf))].map((ns) => [namespaceFile(ns), ns]));
  for (const file of fs.readdirSync(dir).sort()) {
    if (!file.endsWith('.json')) continue;
    const namespace = files.get(file);
    const where = `locales/${code}/${file}`;
    if (!namespace) {
      problems.push(`${where}: no namespace with this name`);
      continue;
    }
    const data = readJson(path.join(dir, file));
    if (typeof data !== 'object' || data === null || Array.isArray(data)) {
      problems.push(`${where}: not a JSON object`);
      continue;
    }
    for (const [key, value] of Object.entries(data)) {
      if (key === '@@locale') {
        if (value !== code) problems.push(`${where}: @@locale ${String(value)} instead of ${code}`);
      } else if (key.startsWith('@')) continue; // metadata for translators (ARB)
      else if (namespaceOf(key) !== namespace) problems.push(`${where}: ${key} belongs to another namespace`);
      else if (!keys.has(key)) problems.push(`${where}: ${key} is not in docs/texts.md`);
      else if (typeof value !== 'string') problems.push(`${where}: ${key} is not a text`);
      else texts.set(key, value);
    }
  }
  return { texts, problems };
}

const json = (value: unknown) => JSON.stringify(value, null, 2) + '\n';

/** Samples of Intl.PluralRules and Intl.NumberFormat that the Rust side (ICU4X) must match. */
function cldrSamples(manifest: Manifest) {
  const numbers = ['0', '1', '2', '3', '4', '5', '11', '12', '14', '21', '22', '25', '101', '102', '112', '1000000', '0.5', '1.0', '1.5', '2.4', '6.0', '12.75'];
  const languages = [...manifest.languages.map((l) => l.code), 'en-XA', 'de', 'fr', 'es', 'pt-BR'];
  return Object.fromEntries(
    languages.map((code) => {
      const f = localeFormat(code, manifest.languages.find((l) => l.code === code)?.number);
      return [code, {
        plural: numbers.map((n) => [n, f.plural(Number(n), n.includes('.') ? n.length - n.indexOf('.') - 1 : 0)]),
        number: [[0, 0], [7, 0], [1234, 0], [12345, 0], [1234567, 0], [2.54, 1], [6, 1], [-3.4, 1], [-0.04, 1], [12345.678, 2]]
          .map(([value, places]) => [value, places, f.number(value, places)]),
      }];
    }),
  );
}

export interface Result {
  /** path relative to the repository -> content */
  files: Map<string, string>;
  problems: string[];
  keys: Map<string, SourceText>;
  /** language -> keys still to translate */
  missing: Map<string, string[]>;
  translations: Map<string, Map<string, string>>;
  manifest: Manifest;
}

export function generate(): Result {
  const manifest = readManifest();
  const source = manifest.languages.find((l) => l.code === manifest.source);
  const fallback = manifest.languages.find((l) => l.code === manifest.fallback);
  if (!source || !fallback) throw new Error('locales/languages.json: source and fallback must be listed languages');
  const { texts: keys, variables } = readSourceTexts();
  for (const [key, text] of readDefinitionTexts()) keys.set(key, text);

  const files = new Map<string, string>();
  const problems: string[] = [];
  const parsed = new Map<string, Part[]>();
  const byNamespace = new Map<string, Record<string, unknown>>();
  for (const [key, { text, note }] of keys) {
    const ns = namespaceOf(key);
    if (!ns) problems.push(`${key}: a key needs a namespace (text before the first dot)`);
    const parts = parse(key, text, problems);
    if (!parts) continue;
    parsed.set(key, parts);
    problems.push(...checkText(key, parts, text, source, true));
    const meta: Record<string, unknown> = {};
    if (note) meta.description = note;
    const names = [...argumentsOf(parts).keys()];
    if (names.length) meta.placeholders = Object.fromEntries(names.map((n) => [n, variables.has(n) ? { description: variables.get(n) } : {}]));
    const entries = byNamespace.get(ns) ?? { '@@locale': source.code };
    entries[key] = text;
    if (Object.keys(meta).length) entries[`@${key}`] = meta;
    byNamespace.set(ns, entries);
  }
  for (const [ns, entries] of byNamespace) files.set(`locales/${source.code}/${namespaceFile(ns)}`, json(entries));

  const missing = new Map<string, string[]>();
  const translations = new Map<string, Map<string, string>>();
  for (const language of manifest.languages) {
    if (language.code === source.code) continue;
    const { texts, problems: layout } = readTranslations(language.code, keys);
    translations.set(language.code, texts);
    problems.push(...layout);
    for (const [key, text] of texts) {
      const src = parsed.get(key);
      if (!src) continue;
      problems.push(...checkText(key, src, text, language, false));
      if (keys.get(key)!.text.split(' / ').length !== text.split(' / ').length) {
        problems.push(`${key}: ${keys.get(key)!.text.split(' / ').length} parts split by " / " expected (the screen splits them)`);
      }
    }
    const todo = [...keys].filter(([key, s]) => !texts.has(key) && !s.sameEverywhere).map(([key]) => key);
    missing.set(language.code, todo);
    if (language.status === 'complete' && todo.length) {
      problems.push(`${language.code}: ${todo.length} keys without translation (status "complete" in locales/languages.json), e.g. ${todo.slice(0, 5).join(', ')}`);
    }
  }

  // Built-in pack: the fallback language with source texts for keys it does not translate yet.
  const fallbackTexts = translations.get(fallback.code) ?? new Map<string, string>();
  const texts: Record<string, string> = {};
  const fromSource: string[] = [];
  for (const [key, { text }] of keys) {
    const translated = fallbackTexts.get(key);
    texts[key] = translated ?? text;
    if (translated === undefined) fromSource.push(key);
  }
  files.set(BUILT_IN, json({
    format: manifest.format,
    generated: 'npm run texts - do not edit',
    language: fallback.code,
    source: source.code,
    languages: manifest.languages,
    namespaces: [...byNamespace.keys()],
    fromSource,
    // the same in every language (the pseudo-language leaves them too)
    sameEverywhere: [...keys].filter(([, s]) => s.sameEverywhere).map(([key]) => key),
    texts,
  }));
  files.set(CLDR_SAMPLES, json(cldrSamples(manifest)));
  return { files, problems, keys, missing, translations, manifest };
}

function staleSourceFiles(files: Map<string, string>, code: string): string[] {
  const dir = path.join(LOCALES, code);
  if (!fs.existsSync(dir)) return [];
  return fs.readdirSync(dir).filter((f) => f.endsWith('.json') && !files.has(`locales/${code}/${f}`)).map((f) => `locales/${code}/${f}`);
}

function main() {
  const result = generate();
  const missingArg = process.argv.find((a) => a.startsWith('--missing='));
  if (missingArg) {
    const [code, namespace] = missingArg.slice('--missing='.length).split(':');
    const todo = result.missing.get(code);
    if (!todo) throw new Error(`Unknown language ${code} (locales/languages.json, not the source language)`);
    const out: Record<string, string> = {};
    for (const key of todo) if (!namespace || namespaceOf(key) === namespace) out[key] = result.keys.get(key)!.text;
    process.stdout.write(json(out));
    return;
  }

  if (result.problems.length) {
    console.error(`Language packs: ${result.problems.length} problems\n  ${result.problems.join('\n  ')}`);
    process.exit(1);
  }
  const source = result.manifest.source;
  const stale = staleSourceFiles(result.files, source);
  const summary = [...result.missing]
    .map(([code, todo]) => {
      const status = result.manifest.languages.find((l) => l.code === code)?.status;
      return `${code}: ${result.translations.get(code)?.size ?? 0} translated, ${todo.length} missing (${status})`;
    })
    .join('; ');

  if (process.argv.includes('--check')) {
    const differ = [...result.files].filter(([file, content]) => {
      const full = path.join(ROOT, file);
      return !fs.existsSync(full) || fs.readFileSync(full, 'utf8') !== content;
    });
    if (differ.length || stale.length) {
      console.error(`Generated texts differ from docs/texts.md and locales/ - run: npm run texts\n  ${[...differ.map(([f]) => f), ...stale].join('\n  ')}`);
      process.exit(1);
    }
    console.log(`Texts: ${result.keys.size} keys; ${summary}`);
    return;
  }
  for (const file of stale) fs.rmSync(path.join(ROOT, file));
  for (const [file, content] of result.files) {
    const full = path.join(ROOT, file);
    fs.mkdirSync(path.dirname(full), { recursive: true });
    if (!fs.existsSync(full) || fs.readFileSync(full, 'utf8') !== content) fs.writeFileSync(full, content);
  }
  console.log(`Texts: ${result.keys.size} keys written to locales/${source}/, ${BUILT_IN} and ${CLDR_SAMPLES}; ${summary}`);
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
