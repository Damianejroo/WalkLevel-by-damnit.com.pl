// Interface builds for the view tests. The mock core (src/dev/mock.ts) and the "nothing sticks
// out" tool are only in the `atrapa` mode build in dist-atrapa/. The production build (dist/,
// shipped in the app) must not contain them.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { build, preview } from 'vite';

export const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
export const MOCK_OUT_DIR = path.join(ROOT, 'dist-atrapa');

export async function buildMock(): Promise<void> {
  await build({ root: ROOT, mode: 'atrapa', logLevel: 'warn', build: { outDir: MOCK_OUT_DIR, emptyOutDir: true } });
}

export function previewMock(port: number) {
  if (!fs.existsSync(path.join(MOCK_OUT_DIR, 'index.html'))) {
    throw new Error('Missing dist-atrapa/ - run npm run test:ui first (it builds the interface with the mock core).');
  }
  return preview({ root: ROOT, mode: 'atrapa', logLevel: 'warn', build: { outDir: MOCK_OUT_DIR }, preview: { port, strictPort: false } });
}

/** Builds for production into a temporary folder and lists what of the mock core got in. */
export async function productionBuildProblems(): Promise<string[]> {
  const out = fs.mkdtempSync(path.join(os.tmpdir(), 'walklevel-dist-'));
  try {
    await build({ root: ROOT, logLevel: 'warn', build: { outDir: out, emptyOutDir: true } });
    const assets = path.join(out, 'assets');
    const problems: string[] = [];
    for (const file of fs.readdirSync(assets)) {
      if (/^(mock|overflow-check)-/.test(file)) problems.push(`production build contains ${file}`);
      if (file.endsWith('.js') && fs.readFileSync(path.join(assets, file), 'utf8').includes('Atrapa nie zna komendy')) {
        problems.push(`production build contains the mock core in ${file}`);
      }
    }
    return problems;
  } finally {
    fs.rmSync(out, { recursive: true, force: true });
  }
}
