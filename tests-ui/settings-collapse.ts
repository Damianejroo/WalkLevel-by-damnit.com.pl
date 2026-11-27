// Sprawdzenie zwijania kategorii Ustawień (docs/design.md 8.5): po zwinięciu nisko przewiniętej
// kategorii kliknięty nagłówek zostaje na tym samym miejscu ekranu (wcześniej uciekał w dół).
//   npm run test:ui && node tests-ui/settings-collapse.ts   (build with the mock core: dist-atrapa/)
import { chromium } from 'playwright-core';
import { previewMock } from './mock-build.ts';

const serwer = await previewMock(4174);
const adres = serwer.resolvedUrls!.local[0];
const przegladarka = await chromium.launch({ channel: 'msedge', headless: true });
const strona = await przegladarka.newPage({ viewport: { width: 740, height: 700 } });
let blad = false;
try {
  await strona.goto(`${adres}?ekran=settings`);
  await strona.locator('.akordeon > .naglowek').first().waitFor();
  // zostaw otwartą tylko jedną kategorię - wtedy zwinięcie skraca stronę poniżej przewinięcia
  for (const n of await strona.locator('.akordeon > .naglowek[aria-expanded="true"]').all()) {
    await n.click();
    await strona.waitForTimeout(200);
  }
  await strona.evaluate(() => (document.querySelector('[data-ekran=settings] .przewijanie')!.scrollTop = 0));
  const naglowek = strona.locator('.akordeon > .naglowek', { hasText: 'Gry i sterowanie' });
  await naglowek.click();
  await strona.waitForTimeout(300);
  // przewiń tak, żeby nagłówek był blisko góry, a pod nim dużo rozwiniętej treści
  await naglowek.evaluate((n) => {
    const okno = n.closest('.przewijanie')!;
    okno.scrollTop += n.getBoundingClientRect().top - okno.getBoundingClientRect().top - 20;
  });
  await strona.waitForTimeout(100);
  console.log('przewinięcie', await strona.evaluate(() => document.querySelector('[data-ekran=settings] .przewijanie')!.scrollTop));
  const uklad = () =>
    strona.evaluate(() =>
      [...document.querySelectorAll('[data-ekran=settings] .akordeon')].map((a) => `${a.querySelector('.tytul')?.textContent}:${Math.round(a.getBoundingClientRect().top)}/${Math.round(a.getBoundingClientRect().height)}`).join(' | '),
    );
  console.log('przed:', await uklad());
  const przed = await naglowek.evaluate((n) => n.getBoundingClientRect().top);
  await naglowek.click();
  await strona.waitForTimeout(400);
  console.log('po:   ', await uklad());
  console.log('zapas i przewinięcie po:', await strona.evaluate(() => { const o = document.querySelector('[data-ekran=settings] .przewijanie')!; return [(o.firstElementChild as HTMLElement).style.paddingBottom, o.scrollTop, o.scrollHeight, o.clientHeight]; }));
  const po = await naglowek.evaluate((n) => n.getBoundingClientRect().top);
  const roznica = Math.abs(po - przed);
  console.log(`nagłówek: przed ${przed.toFixed(1)} px, po zwinięciu ${po.toFixed(1)} px`);
  if (roznica > 1) {
    console.log(`BŁĄD: nagłówek przesunął się o ${roznica.toFixed(1)} px`);
    blad = true;
  }
  // przewinięcie w górę zjada zapas - na samym dole nie zostaje pusta przestrzeń ponad CSS
  await strona.evaluate(() => (document.querySelector('[data-ekran=settings] .przewijanie')!.scrollTop = 0));
  await strona.waitForTimeout(100);
  const zapas = await strona.evaluate(() => document.querySelector<HTMLElement>('[data-ekran=settings] .przewijanie > .tresc')?.style.paddingBottom ?? '');
  console.log(`zapas po przewinięciu w górę: '${zapas}'`);
  if (zapas) blad = true;
} finally {
  await przegladarka.close();
  serwer.httpServer.close();
}
process.exit(blad ? 1 : 0);

