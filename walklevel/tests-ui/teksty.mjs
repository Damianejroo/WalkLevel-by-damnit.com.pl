// Generator słownika tekstów: docs/teksty.md -> src/lib/teksty-slownik.ts
// Uruchomienie: npm run teksty   (sprawdzenie bez zapisu: node tests-ui/teksty.mjs --sprawdz)
// docs/teksty.md jest jedynym źródłem tekstów - pliku wynikowego nie edytuje się ręcznie.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const katalog = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const zrodlo = path.join(katalog, 'docs', 'teksty.md');
const cel = path.join(katalog, 'src', 'lib', 'teksty-slownik.ts');

/** Klucz z nazwy na ekranie: małe litery, bez polskich znaków, spacje -> _. */
const slug = (s) =>
  s.normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/ł/g, 'l').replace(/Ł/g, 'L')
    .toLowerCase().replace(/[^a-z0-9]+/g, '_').replace(/^_|_$/g, '');

function komorki(linia) {
  const wynik = [];
  let biezaca = '';
  let wKodzie = false;
  for (let i = 1; i < linia.length; i++) {
    const z = linia[i];
    if (z === '`') wKodzie = !wKodzie;
    if (z === '|' && !wKodzie && linia[i - 1] !== '\\') { wynik.push(biezaca.trim()); biezaca = ''; }
    else biezaca += z;
  }
  return wynik;
}

export function generuj() {
  const md = fs.readFileSync(zrodlo, 'utf8');
  const wpisy = new Map();
  let sekcja = '';
  const dodaj = (klucz, tekst) => {
    if (wpisy.has(klucz)) throw new Error(`Powtórzony klucz ${klucz}`);
    wpisy.set(klucz, tekst.replace(/\\n/g, '\n').replace(/\\\|/g, '|'));
  };
  for (const linia of md.split(/\r?\n/)) {
    if (linia.startsWith('## ')) sekcja = linia;
    if (!linia.startsWith('| `')) continue;
    // 0.5 (nazwy zmiennych) i 28 (treści z definicje.json - UI czyta je z definicji) - poza słownikiem
    if (sekcja.startsWith('## 0.') || sekcja.startsWith('## 28.')) continue;
    const [kKlucz, obecny, nowy, uwaga = ''] = komorki(linia);
    const tekst = nowy === 'bez zmian' ? obecny : nowy;
    const zakres = kKlucz.match(/^`([a-z0-9_.]+)\.[a-z0-9_]+` … `[a-z0-9_.]+`$/);
    if (zakres) {
      for (const nazwa of obecny.split(',').map((s) => s.trim())) dodaj(`${zakres[1]}.${slug(nazwa)}`, nazwa);
      continue;
    }
    const gwiazdka = kKlucz.match(/^`([a-z0-9_.]+)\.\*`$/);
    if (gwiazdka) {
      for (const nazwa of obecny.split(',').map((s) => s.trim())) dodaj(`${gwiazdka[1]}.${slug(nazwa)}`, nazwa);
      continue;
    }
    const pojedynczy = kKlucz.match(/^`([a-z0-9_.]+)`$/);
    if (!pojedynczy) throw new Error(`Nieznany klucz: ${kKlucz}`);
    const klucz = pojedynczy[1];
    // opisy zamiast tekstu (np. „{jeden z ble.powod.*}”, „(nazwy.py: …)”) - nie są tekstami do pokazania
    if (/^\{jeden z /.test(tekst) || /^\(`/.test(tekst)) continue;
    dodaj(klucz, tekst);
    // „Analogicznie: Kółko, Kwadrat, …” w uwadze - rodzeństwo klucza
    const analog = uwaga.match(/^Analogicznie: (.+?)\.?$/);
    if (analog) {
      const rodzic = klucz.slice(0, klucz.lastIndexOf('.'));
      for (const nazwa of analog[1].split(',').map((s) => s.trim())) dodaj(`${rodzic}.${slug(nazwa)}`, nazwa);
    }
  }
  const linie = [...wpisy].map(([k, v]) => `  ${JSON.stringify(k)}: ${JSON.stringify(v)},`);
  return (
    '// PLIK GENEROWANY z docs/teksty.md przez `npm run teksty` - nie edytować ręcznie.\n' +
    '// Kolumna „Nowy tekst” (przy „bez zmian” - „Obecny tekst”).\n' +
    `export const SLOWNIK = {\n${linie.join('\n')}\n} as const;\n`
  );
}

const tekst = generuj();
if (process.argv.includes('--sprawdz')) {
  const obecny = fs.existsSync(cel) ? fs.readFileSync(cel, 'utf8') : '';
  if (obecny !== tekst) {
    console.error('src/lib/teksty-slownik.ts nie zgadza się z docs/teksty.md - uruchom: npm run teksty');
    process.exit(1);
  }
  console.log('Słownik tekstów zgodny z docs/teksty.md');
} else {
  fs.writeFileSync(cel, tekst);
  console.log(`Zapisano ${path.relative(katalog, cel)}`);
}
