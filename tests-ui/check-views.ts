// Test „nic nie wystaje” + zrzuty (docs/design.md 11.2). Uruchomienie: npm run test:ui
//   [--no-build] [--screen=history|interakcje] [--no-screenshots]
// Buduje aplikację (produkcyjnie - bez atrapy, i z atrapą do dist-atrapa/), serwuje ją
// (vite preview) z atrapą rdzenia i w zainstalowanym Microsoft Edge
// (playwright-core, bez pobierania przeglądarek) sprawdza każdy ekran x scenariusz x szerokość x
// motyw: brak poziomego przewijania, żaden widoczny element poza oknem ani ucięty bez
// wielokropka (window.wystaje z src/dev/overflow-check.ts), brak błędów w konsoli, aktywny ten
// ekran, o który chodziło. Na końcu zachowanie po kliknięciach (tests-ui/interactions.ts).
// Zrzuty: docs/screenshots/<ekran>-<scenariusz>-<szer>-<motyw>.png (cała wysokość treści ekranu).
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { chromium } from 'playwright-core';
import { DOMYSLNE, EKRANY, MOTYWY, OKNA, SZEROKOSCI, WYSOKOSC } from './screens.ts';
import { buildMock, previewMock, productionBuildProblems } from './mock-build.ts';
import { checkInteractions } from './interactions.ts';

const katalog = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const zrzuty = path.join(katalog, 'docs', 'screenshots');
const arg = (nazwa: string) => process.argv.find((a) => a.startsWith(`--${nazwa}`));
const tylkoEkran = arg('screen=')?.split('=')[1];

// 1. słownik tekstów zgodny z docs/texts.md
const teksty = spawnSync(process.execPath, [path.join(katalog, 'tests-ui', 'texts.ts'), '--check'], { stdio: 'inherit' });
if (teksty.status !== 0) process.exit(1);

// 2. build without the mock core (what the app ships), then with it (dist-atrapa/) and preview
if (!arg('no-build')) {
  const produkcja = await productionBuildProblems();
  if (produkcja.length) {
    console.log(`BŁĄD build produkcyjny\n  ${produkcja.join('\n  ')}`);
    process.exit(1);
  }
  await buildMock();
}
const serwer = await previewMock(4173);
const adres = serwer.resolvedUrls!.local[0];

const przegladarka = await chromium.launch({ channel: 'msedge', headless: true });
fs.mkdirSync(zrzuty, { recursive: true });

const bledy: { nazwa: string; problemy: string[] }[] = [];
const widziane = new Set<string>();
let sprawdzone = 0;
try {
  for (const motyw of MOTYWY) {
    for (const szer of SZEROKOSCI) {
      const kontekst = await przegladarka.newContext({
        viewport: { width: szer, height: WYSOKOSC },
        reducedMotion: 'reduce',
        colorScheme: motyw === 'jasny' ? 'light' : 'dark',
        locale: 'pl-PL',
      });
      const strona = await kontekst.newPage();
      let konsola: string[] = [];
      strona.on('console', (m) => m.type() === 'error' && konsola.push(m.text()));
      strona.on('pageerror', (e) => konsola.push(String(e)));

      for (const { ekran, scenariusze, szerokosci = DOMYSLNE } of EKRANY) {
        if (tylkoEkran && ekran !== tylkoEkran) continue;
        if (!szerokosci.includes(szer)) continue;
        for (const scenariusz of scenariusze) {
          const nazwa = `${ekran}-${scenariusz}-${szer}-${motyw}`;
          if (widziane.has(nazwa)) throw new Error(`tests-ui/screens.ts: ${nazwa} jest na liście więcej niż raz`);
          widziane.add(nazwa);
          konsola = [];
          await strona.setViewportSize({ width: szer, height: WYSOKOSC });
          await strona.goto(`${adres}?ekran=${ekran}&stan=${scenariusz}&motyw=${motyw}`);
          await strona.waitForFunction(
            (m) => document.documentElement.dataset.gotowe === '1' && document.documentElement.dataset.theme === m && typeof window.wystaje === 'function',
            motyw === 'jasny' ? 'light' : 'dark',
          );
          await strona.evaluate(() => document.fonts.ready);
          await strona.waitForTimeout(300); // nawigacja z rdzenia (np. konfigurator), pomiary ResizeObserver

          const wynik = await strona.evaluate(() => ({
            przewijanie: document.documentElement.scrollWidth - window.innerWidth,
            wystaje: window.wystaje(document.body),
            aktywny: document.querySelector('.ekran[data-stan="aktywny"]')?.getAttribute('data-ekran') ?? '',
          }));
          const problemy = [
            // a scenario that ends on another screen would check the wrong view
            ...(wynik.aktywny !== ekran ? [`aktywny ekran: ${wynik.aktywny || 'brak'} zamiast ${ekran}`] : []),
            ...(wynik.przewijanie > 0 ? [`poziome przewijanie strony: ${wynik.przewijanie} px`] : []),
            ...wynik.wystaje,
            ...konsola.map((k) => `konsola: ${k}`),
          ];
          sprawdzone++;
          if (problemy.length) {
            bledy.push({ nazwa, problemy });
            console.log(`BŁĄD ${nazwa}\n  ${problemy.slice(0, 12).join('\n  ')}`);
          } else {
            console.log(`ok   ${nazwa}`);
          }

          if (!arg('no-screenshots')) {
            // cała wysokość treści: okno wydłużone do wysokości przewijanej kolumny aktywnego ekranu
            const pelna = await strona.evaluate(() => {
              const e = document.querySelector('.ekran[data-stan="aktywny"]');
              const p = e?.querySelector('.przewijanie');
              return p ? Math.ceil(p.getBoundingClientRect().top + p.scrollHeight) : 0;
            });
            if (pelna > WYSOKOSC) await strona.setViewportSize({ width: szer, height: Math.min(pelna, 6000) });
            await strona.screenshot({ path: path.join(zrzuty, `${nazwa}.png`) });
          }
        }
      }
      await kontekst.close();
    }
  }

  // Osobne okna (popup, przycisk): stały rozmiar, podgląd ?demo=1 z marginesem 16 px wokół okna
  for (const motyw of MOTYWY) {
    for (const { ekran, strona: plik, okno, scenariusze } of OKNA) {
      if (tylkoEkran && ekran !== tylkoEkran) continue;
      const kontekst = await przegladarka.newContext({
        viewport: { width: okno.szer + 32, height: okno.wys + 32 },
        deviceScaleFactor: 2,
        reducedMotion: 'reduce',
        colorScheme: motyw === 'jasny' ? 'light' : 'dark',
        locale: 'pl-PL',
      });
      const strona = await kontekst.newPage();
      let konsola: string[] = [];
      strona.on('console', (m) => m.type() === 'error' && konsola.push(m.text()));
      strona.on('pageerror', (e) => konsola.push(String(e)));
      for (const { nazwa: scenariusz, zapytanie, okno: oknoScenariusza } of scenariusze) {
        const nazwa = `${ekran}-${scenariusz}-${motyw}`;
        konsola = [];
        // scenariusz może mieć własny rozmiar okna (przycisk w trybie Trening)
        const o = oknoScenariusza ?? okno;
        await strona.setViewportSize({ width: o.szer + 32, height: o.wys + 32 });
        await strona.goto(`${adres}${plik}?demo=1&${zapytanie}&motyw=${motyw}`);
        await strona.waitForFunction(() => document.documentElement.dataset.gotowe === '1' && typeof window.wystaje === 'function');
        await strona.evaluate(() => document.fonts.ready);
        await strona.waitForTimeout(300);
        const wynik = await strona.evaluate(({ szer, wys }) => {
          const app = document.getElementById('app')!;
          const o = app.getBoundingClientRect();
          const rozmiar = Math.round(o.width) !== szer || Math.round(o.height) !== wys
            ? [`okno ${Math.round(o.width)} × ${Math.round(o.height)} zamiast ${szer} × ${wys}`]
            : [];
          const poza = [...app.querySelectorAll('*')]
            .filter((el) => el.checkVisibility({ visibilityProperty: true }))
            .filter((el) => {
              const r = el.getBoundingClientRect();
              return (r.width || r.height) && (r.bottom > o.bottom + 0.5 || r.top < o.top - 0.5 || r.left < o.left - 0.5);
            })
            .map((el) => `poza oknem (góra/dół/lewo): ${el.tagName.toLowerCase()}.${el.getAttribute('class') ?? ''}`);
          return { wystaje: [...rozmiar, ...window.wystaje(app, o.right + 0.5), ...poza], elementy: app.querySelectorAll('*').length };
        }, o);
        const problemy = [
          ...wynik.wystaje,
          ...(wynik.elementy === 0 ? ['okno puste'] : []),
          ...konsola.map((k) => `konsola: ${k}`),
        ];
        sprawdzone++;
        if (problemy.length) {
          bledy.push({ nazwa, problemy });
          console.log(`BŁĄD ${nazwa}\n  ${problemy.slice(0, 12).join('\n  ')}`);
        } else {
          console.log(`ok   ${nazwa}`);
        }
        if (!arg('no-screenshots')) await strona.screenshot({ path: path.join(zrzuty, `${nazwa}.png`) });
      }
      await kontekst.close();
    }
  }

  // 3. behaviour (clicks, failed commands, panel header) - tests-ui/interactions.ts
  if (!tylkoEkran || tylkoEkran === 'interakcje') {
    for (const wynik of await checkInteractions(przegladarka, adres)) {
      sprawdzone++;
      if (wynik.problemy.length) {
        bledy.push(wynik);
        console.log(`BŁĄD ${wynik.nazwa}\n  ${wynik.problemy.slice(0, 12).join('\n  ')}`);
      } else {
        console.log(`ok   ${wynik.nazwa}`);
      }
    }
  }
} finally {
  await przegladarka.close();
  await new Promise((ok) => serwer.httpServer.close(ok));
}

console.log(`\nSprawdzone widoki: ${sprawdzone}, z problemami: ${bledy.length}`);
if (bledy.length) process.exit(1);
