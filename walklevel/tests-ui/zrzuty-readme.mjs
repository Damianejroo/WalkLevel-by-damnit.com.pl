// Zrzuty do README (docs/readme/*.png) z atrapą rdzenia: node tests-ui/zrzuty-readme.mjs [plik ...]
// Serwer deweloperski Vite (bez budowania, dist/ nietknięte), Microsoft Edge przez playwright-core.
// Zegar strony przewinięty o `minut`,
// żeby licznik sesji atrapy pokazał kroki, dystans i czas (moment dobrany tak, by wskaźnik był w strefie chodu).
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';
import { chromium } from 'playwright-core';

const katalog = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const cel = path.join(katalog, 'docs', 'readme');

const ZRZUTY = [
  { plik: 'main-window-dark', ekran: 'main', stan: 'chodzenie', motyw: 'ciemny', szer: 720, minut: 17.58 },
  { plik: 'history-dark', ekran: 'history', stan: 'panel-wylaczony', motyw: 'ciemny', szer: 480, klik: ['Ten miesiąc'] },
  { plik: 'achievements-dark', ekran: 'achievements', stan: 'panel-wylaczony', motyw: 'ciemny', szer: 480 },
  { plik: 'ranking-dark', ekran: 'online', stan: 'panel-wylaczony', motyw: 'ciemny', szer: 480, klik: ['Ranking'] },
  { plik: 'settings-dark', ekran: 'settings', stan: 'panel-wylaczony', motyw: 'ciemny', szer: 480, klik: ['Gry i sterowanie'] },
  { plik: 'main-window-light', ekran: 'main', stan: 'klawisze', motyw: 'jasny', szer: 720, minut: 26 + 4 / 60 },
];
const WYSOKOSC = 860;
const tylko = process.argv.slice(2);

const serwer = await createServer({ root: katalog, logLevel: 'warn', server: { port: 5191, strictPort: false } });
await serwer.listen();
const adres = serwer.resolvedUrls.local[0];
const przegladarka = await chromium.launch({ channel: 'msedge', headless: true });
fs.mkdirSync(cel, { recursive: true });

try {
  for (const { plik, ekran, stan, motyw, szer, minut = 0, klik = [] } of ZRZUTY) {
    if (tylko.length && !tylko.includes(plik)) continue;
    const kontekst = await przegladarka.newContext({
      viewport: { width: szer, height: WYSOKOSC },
      deviceScaleFactor: 2,
      reducedMotion: 'reduce',
      colorScheme: motyw === 'jasny' ? 'light' : 'dark',
      locale: 'pl-PL',
    });
    const strona = await kontekst.newPage();
    await strona.clock.install();
    await strona.goto(`${adres}?ekran=${ekran}&stan=${stan}&motyw=${motyw}`);
    await strona.clock.runFor(1000);
    await strona.waitForFunction(() => document.documentElement.dataset.gotowe === '1', null, { polling: 100 });
    await strona.evaluate(() => document.fonts.ready);
    for (const tekst of klik) {
      await strona.getByText(tekst, { exact: true }).first().click();
      await strona.clock.runFor(500);
    }
    if (minut) await strona.clock.runFor(minut * 60_000);
    await strona.clock.runFor(1000);
    await strona.screenshot({ path: path.join(cel, `${plik}.png`) });
    console.log(`ok   ${plik}`);
    await kontekst.close();
  }
} finally {
  await przegladarka.close();
  await serwer.close();
}
