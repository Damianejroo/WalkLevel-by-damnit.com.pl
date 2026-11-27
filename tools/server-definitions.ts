// Account server definitions file built from the app definitions: thresholds only, no texts.
// Run from the repository root: npm run server-definitions
// Reads docs/definitions-v2.json, writes docs/definitions-v2-server.json. The Rust test
// `server_file_matches_app_definitions` (crates/core/tests/levels_fixtures.rs) checks that both agree.
import fs from 'node:fs';

const SOURCE = 'docs/definitions-v2.json';
const OUTPUT = 'docs/definitions-v2-server.json';

// achievement type -> threshold field in the app definitions (written as `prog`)
const THRESHOLD: Record<string, string> = {
  dystans: 'km',
  sesja_km: 'km',
  gra_km: 'km',
  gry_km: 'km',
  dzien_kroki: 'kroki',
  seria: 'dni',
  znajomi: 'ile',
  polecenia: 'ile',
};
const EXTRA: Record<string, string[]> = { gry_km: ['gier'], seria: ['km_dziennie'], start_miedzy: ['od', 'do'] };
const TYPES = new Set([...Object.keys(THRESHOLD), 'gra_pierwsza', 'trening', 'start_miedzy']);

// Numbers written with a fraction or exponent stay floats, so 2.0 is written back as 2.0.
class Float {
  readonly value: number;
  constructor(value: number) {
    this.value = value;
  }
}

type Json = null | boolean | number | Float | string | Json[] | { [key: string]: Json };
type Achievement = { [key: string]: Json };

function parse(text: string): Json {
  return JSON.parse(text, (_key, value, context?: { source?: string }) =>
    typeof value === 'number' && context?.source && /[.eE]/.test(context.source) ? new Float(value) : value,
  );
}

function formatFloat(value: number): string {
  return Number.isInteger(value) ? `${value}.0` : String(value);
}

// Same output as Python json.dumps(value, ensure_ascii=False, indent=1).
function dump(value: Json, depth = 0): string {
  const pad = ' '.repeat(depth + 1);
  const end = ' '.repeat(depth);
  if (value instanceof Float) return formatFloat(value.value);
  if (Array.isArray(value)) {
    if (!value.length) return '[]';
    return `[\n${value.map((item) => pad + dump(item, depth + 1)).join(',\n')}\n${end}]`;
  }
  if (value !== null && typeof value === 'object') {
    const entries = Object.entries(value);
    if (!entries.length) return '{}';
    return `{\n${entries.map(([key, item]) => `${pad}${JSON.stringify(key)}: ${dump(item, depth + 1)}`).join(',\n')}\n${end}}`;
  }
  return JSON.stringify(value);
}

function isNumber(value: Json): boolean {
  return typeof value === 'number' || typeof value === 'boolean' || value instanceof Float;
}

function serverDefinitions(definitions: { [key: string]: Json }): Json {
  const achievements = (definitions.osiagniecia as Achievement[]).map((achievement) => {
    const type = achievement.typ as string;
    if (!TYPES.has(type)) throw new Error(`type ${type} (${String(achievement.id)}) is unknown to the server`);
    const row: Achievement = { id: achievement.id, typ: type };
    if (type in THRESHOLD) row.prog = achievement[THRESHOLD[type]];
    for (const key of EXTRA[type] ?? []) row[key] = achievement[key];
    row.rz = achievement.rzadkosc;
    return row;
  });
  const rules = Object.fromEntries(
    Object.entries(definitions.zasady as { [key: string]: Json }).filter(([, value]) => isNumber(value)),
  );
  return {
    wersja: definitions.wersja,
    xp_za_km: definitions.xp_za_km,
    xp_za_rzadkosc: definitions.xp_za_rzadkosc,
    poziomy_xp: definitions.poziomy_xp,
    zasady: rules,
    osiagniecia: achievements,
  };
}

const definitions = parse(fs.readFileSync(SOURCE, 'utf8')) as { [key: string]: Json };
fs.writeFileSync(OUTPUT, `${dump(serverDefinitions(definitions))}\n`);
console.log(`wrote ${OUTPUT} (${(definitions.osiagniecia as Json[]).length} achievements)`);
