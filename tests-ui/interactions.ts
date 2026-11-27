// Behaviour checks with the mock core (run by tests-ui/check-views.ts after the view checks):
// the friends panel header (collapse button in place on both sides, initials for hidden levels)
// and collapsed strip on both sides, controls after a failed command,
// threshold fields, "add a person with an online account", the report description length,
// hiding a session online, the achievement date format, text from other users shown literally,
// the focus ring only from Tab navigation and the Settings > Language title, list and collapse.
import type { Browser, Page } from 'playwright-core';
import fs from 'node:fs';
import { napis, znak } from '../src/lib/ui/badge-glyphs.ts';

export interface Result {
  nazwa: string;
  problemy: string[];
}

// Polish texts (the mock core starts in Polish): locales/pl/<namespace>.json
const polish = (key: string): string => {
  const file = new URL(`../locales/pl/${key.slice(0, key.indexOf('.')).replace(/_/g, '-')}.json`, import.meta.url);
  const text = (JSON.parse(fs.readFileSync(file, 'utf8')) as Record<string, unknown>)[key];
  if (typeof text !== 'string') throw new Error(`No text ${key}`);
  return text;
};
const text = (key: string, vars: Record<string, string> = {}) =>
  polish(key).replace(/\{([a-z_0-9]+)\}/g, (all, name: string) => vars[name] ?? all);
const spaces = (s: string | null) => (s ?? '').replace(/\s+/g, ' ').trim();

async function open(page: Page, address: string, query: string) {
  await page.goto(`${address}?${query}`);
  await page.waitForFunction(() => document.documentElement.dataset.gotowe === '1');
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(300);
}

/** Collapse button and the panel edge next to the main column, for both panel states. */
async function headerLayout(page: Page) {
  return page.evaluate(() => {
    const panel = document.querySelector<HTMLElement>('.slot-panelu .panel')!;
    const r = panel.getBoundingClientRect();
    const right = document.querySelector('.uklad')!.getAttribute('data-strona-panelu') === 'right';
    const box = (sel: string) => panel.querySelector(sel)?.getBoundingClientRect() ?? null;
    const collapse = box('.zwijanie')!;
    const icon = panel.querySelector('.zwijanie .ikona');
    return {
      right,
      narrow: panel.classList.contains('waski'),
      // distance of the collapse button from the panel edge next to the main column
      fromColumn: right ? collapse.left - r.left : r.right - collapse.right,
      collapse: collapse.left,
      title: box('.tytul')?.left ?? null,
      plus: box('.ikony .ib:not(.zwijanie)')?.left ?? null,
      mirrored: icon ? getComputedStyle(icon).transform.startsWith('matrix(-1') : false,
    };
  });
}

async function panelHeader(browser: Browser, address: string): Promise<Result[]> {
  const out: Result[] = [];
  const page = await browser.newPage({ viewport: { width: 1000, height: 800 } });
  try {
    for (const side of ['prawa', 'lewa'] as const) {
      const problems: string[] = [];
      await open(page, address, `ekran=main&panel=pelny&strona=${side}`);
      const full = await headerLayout(page);
      // hidden level and invitations: the initial of the nickname, as in the collapsed strip
      const initials = await page.evaluate(() => {
        const panel = document.querySelector('.slot-panelu .panel')!;
        const row = [...panel.querySelectorAll('.wiersz')].find((r) => r.querySelector('.nick')?.textContent === 'ela_spaceruje');
        return {
          row: row?.querySelector('.initial-avatar')?.textContent ?? null,
          invitation: panel.querySelector('.zaproszenie .initial-avatar')?.textContent ?? null,
        };
      });
      if (initials.row !== 'E') problems.push(`pełny panel, ukryty poziom: inicjał „${initials.row}” zamiast „E”`);
      if (initials.invitation !== 'E') problems.push(`zaproszenie: inicjał „${initials.invitation}” zamiast „E”`);
      await open(page, address, `ekran=main&panel=waski&strona=${side}`);
      const strip = await headerLayout(page);
      if (full.right !== (side === 'prawa')) problems.push(`panel nie jest z ${side === 'prawa' ? 'prawej' : 'lewej'}`);
      if (Math.abs(full.fromColumn - strip.fromColumn) > 1) {
        problems.push(`przycisk zwijania przesuwa się przy zwinięciu: ${full.fromColumn.toFixed(1)} px -> ${strip.fromColumn.toFixed(1)} px od kolumny`);
      }
      if (full.title === null || full.plus === null) problems.push('brak tytułu albo plusa w nagłówku');
      else if (side === 'prawa' && !(full.collapse < full.title && full.title < full.plus)) {
        problems.push('z prawej: oczekiwany układ [zwiń] tytuł … [+]');
      } else if (side === 'lewa' && !(full.title < full.plus && full.plus < full.collapse)) {
        problems.push('z lewej: oczekiwany układ tytuł … [+] [zwiń]');
      }
      if (full.mirrored !== (side === 'prawa') || strip.mirrored !== (side === 'prawa')) {
        problems.push(`ikona zwijania ${side === 'prawa' ? 'nieodbita z prawej' : 'odbita z lewej'}`);
      }
      out.push({ nazwa: `interakcje-naglowek-panelu-${side}`, problemy: problems });
    }
  } finally {
    await page.close();
  }
  return out;
}

async function collapsedStrip(browser: Browser, address: string): Promise<Result[]> {
  const out: Result[] = [];
  const page = await browser.newPage({ viewport: { width: 1000, height: 800 } });
  try {
    for (const side of ['prawa', 'lewa'] as const) {
      const problems: string[] = [];
      await open(page, address, `ekran=main&panel=waski&strona=${side}`);
      const strip = await page.evaluate(() => {
        const panel = document.querySelector('.slot-panelu .panel.waski')!;
        const rows = [...panel.querySelectorAll<HTMLElement>('.wiersz')];
        return {
          count: panel.querySelector('.walking-count')?.textContent?.trim() ?? null,
          rows: rows.map((r) => ({
            walking: r.dataset.idzie === '1',
            dot: !!r.querySelector('.status-dot'),
            initial: r.querySelector('.initial-avatar')?.textContent ?? null,
            title: r.getAttribute('title'),
          })),
        };
      });
      const walking = strip.rows.filter((r) => r.walking).length;
      if (!walking) problems.push('w atrapie nikt nie idzie');
      if (strip.count !== String(walking)) problems.push(`licznik „${strip.count}” zamiast ${walking}`);
      for (const r of strip.rows) if (r.dot !== r.walking) problems.push(`kropka statusu niezgodna z „idzie teraz”: ${r.title}`);
      const ola = strip.rows.find((r) => r.title?.startsWith('OlaBiega'));
      const expectedOla = `${text('panel.pasek_idzie', { pseudonim: 'OlaBiega' })} · ${text('panel.gra', { gra: 'World of Warcraft', kmh: '3,4' })}`;
      if (spaces(ola?.title ?? null) !== spaces(expectedOla)) problems.push(`podpowiedź „${ola?.title}” zamiast „${expectedOla}”`);
      const ela = strip.rows.find((r) => r.title?.startsWith('ela_spaceruje'));
      if (ela?.initial !== 'E') problems.push(`inicjał przy ukrytym poziomie: „${ela?.initial}” zamiast „E”`);
      if (!/^ela_spaceruje · w tym tygodniu 9 100 kroków$/.test(spaces(ela?.title ?? null))) problems.push(`podpowiedź „${ela?.title}”`);
      const kuba = strip.rows.find((r) => r.title?.startsWith('KubaWędrowiec'));
      if (kuba?.initial !== 'K' || !kuba.dot) problems.push('idący z ukrytym poziomem: inicjał i kropka');

      await open(page, address, `ekran=main&stan=panel-waski-nikt&strona=${side}`);
      const nobody = await page.evaluate(() => ({
        count: !!document.querySelector('.slot-panelu .walking-count'),
        dots: document.querySelectorAll('.slot-panelu .status-dot').length,
      }));
      if (nobody.count || nobody.dots) problems.push('nikt nie idzie, a widać licznik albo kropki');
      out.push({ nazwa: `interakcje-pasek-panelu-${side}`, problemy: problems });
    }
  } finally {
    await page.close();
  }
  return out;
}

/** A switch and a segmented control go back when their command fails. */
async function failedCommand(browser: Browser, address: string): Promise<Result> {
  const problems: string[] = [];
  const page = await browser.newPage({ viewport: { width: 740, height: 900 } });
  try {
    await open(page, address, 'ekran=online&zakladka=me&blad=account_set_upload,account_set_visibility');
    const upload = page.getByRole('switch', { name: text('moje.wyniki.wysylaj') });
    const before = await upload.getAttribute('aria-checked');
    await upload.click();
    await page.waitForTimeout(500);
    const after = await upload.getAttribute('aria-checked');
    if (after !== before) problems.push(`„${text('moje.wyniki.wysylaj')}” po błędzie: ${after} zamiast ${before}`);

    const onlyMe = page.getByRole('radio', { name: text('moje.widocznosc.ja') }).first();
    const was = await onlyMe.getAttribute('aria-checked');
    await onlyMe.click();
    await page.waitForTimeout(500);
    if ((await onlyMe.getAttribute('aria-checked')) !== was) problems.push('segment „Kto widzi” zostaje przestawiony po błędzie');
  } catch (e) {
    problems.push(String(e));
  } finally {
    await page.close();
  }
  return { nazwa: 'interakcje-blad-komendy', problemy: problems };
}

/** Threshold fields: limits that the core keeps, and the stored value back after a failure. */
async function thresholds(browser: Browser, address: string): Promise<Result> {
  const problems: string[] = [];
  const page = await browser.newPage({ viewport: { width: 740, height: 900 } });
  const field = (name: string) => page.getByRole('textbox', { name: text('pole.wpisz', { nazwa: name }) });
  const walk = text('ustawienia.chodzenie.chod_od');
  const run = text('ustawienia.chodzenie.bieg_od');
  /** Types like a person: the field selects its number a frame after it gets focus. */
  const type = async (name: string, value: string) => {
    await field(name).click();
    await page.waitForTimeout(100);
    await page.keyboard.type(value);
    await page.keyboard.press('Enter');
  };
  /** Field value once it shows `expected` (or the last value after 2 s). */
  const settled = async (name: string, expected: string) => {
    let value = '';
    for (let waited = 0; waited <= 2000; waited += 100) {
      value = spaces(await field(name).inputValue());
      if (value === expected) break;
      await page.waitForTimeout(100);
    }
    return value;
  };
  try {
    await open(page, address, 'ekran=settings&stan=ustawienia-chodzenie');
    await type(walk, '99');
    const top = await settled(walk, '11,2 km/h');
    if (top !== '11,2 km/h') problems.push(`„${walk}” po wpisaniu 99: ${top} zamiast 11,2 km/h`);
    await type(run, '0,1');
    const bottom = await settled(run, '0,8 km/h');
    if (bottom !== '0,8 km/h') problems.push(`„${run}” po wpisaniu 0,1: ${bottom} zamiast 0,8 km/h`);

    await open(page, address, 'ekran=settings&stan=ustawienia-chodzenie&blad=thresholds_set');
    const stored = spaces(await field(walk).inputValue());
    await page.getByRole('button', { name: text('pole.wiecej', { nazwa: walk }) }).click();
    if (spaces(await field(walk).inputValue()) === stored) problems.push(`„${walk}”: klik „+” nic nie zmienił`);
    const shown = await settled(walk, stored);
    if (shown !== stored) problems.push(`„${walk}” po nieudanym zapisie: ${shown} zamiast ${stored}`);
  } catch (e) {
    problems.push(String(e));
  } finally {
    await page.close();
  }
  return { nazwa: 'interakcje-progi', problemy: problems };
}

/** Settings > "Add a person with an online account" signs in a new person. */
async function newPersonWithAccount(browser: Browser, address: string): Promise<Result> {
  const problems: string[] = [];
  const page = await browser.newPage({ viewport: { width: 740, height: 900 } });
  try {
    await open(page, address, 'ekran=settings&stan=ustawienia-wylogowany');
    await page.getByRole('button', { name: text('ustawienia.osoby.dodaj_z_kontem') }).click();
    await page.waitForTimeout(400);
    const screen = await page.evaluate(() => document.querySelector('.ekran[data-stan="aktywny"]')?.getAttribute('data-ekran'));
    if (screen !== 'account') problems.push(`ekran ${screen} zamiast account`);
    const shown = await page.locator('[data-ekran="account"]').getByText(text('konto.nowa_osoba_opis')).count();
    if (!shown) problems.push(`brak „${text('konto.nowa_osoba_opis')}” (logowanie dla wybranej osoby zamiast nowej)`);
  } catch (e) {
    problems.push(String(e));
  } finally {
    await page.close();
  }
  return { nazwa: 'interakcje-nowa-osoba-z-kontem', problemy: problems };
}

/** The report description counts characters like the core (an emoji is one character). */
async function reportLength(browser: Browser, address: string): Promise<Result> {
  const problems: string[] = [];
  const page = await browser.newPage({ viewport: { width: 740, height: 900 } });
  try {
    await open(page, address, 'ekran=report');
    await page.locator('[data-ekran="report"] textarea').fill('😀😀😀😀');
    await page.getByRole('button', { name: text('zgloszenie.wyslij') }).click();
    await page.waitForTimeout(300);
    const error = await page.locator('[data-ekran="report"] .blad[role="alert"]').first().textContent().catch(() => null);
    if (!error?.trim()) problems.push('opis z 4 emoji: brak błędu „za krótki” pod polem');
  } catch (e) {
    problems.push(String(e));
  } finally {
    await page.close();
  }
  return { nazwa: 'interakcje-zgloszenie-dlugosc', problemy: problems };
}

/** History: the eye hides a session online and a second click shows it again. */
async function hideOnline(browser: Browser, address: string): Promise<Result> {
  const problems: string[] = [];
  const page = await browser.newPage({ viewport: { width: 740, height: 900 } });
  try {
    await open(page, address, 'ekran=history');
    const eye = page.locator('[data-ekran="history"] .oko').first();
    const hidden = async () => (await eye.getAttribute('class'))?.includes('ukryta') ?? false;
    const start = await hidden();
    await eye.click();
    await page.waitForTimeout(300);
    if ((await hidden()) === start) problems.push('pierwszy klik w oko nie zmienił sesji');
    await eye.click();
    await page.waitForTimeout(300);
    if ((await hidden()) !== start) problems.push('drugi klik w oko nie cofnął zmiany');
  } catch (e) {
    problems.push(String(e));
  } finally {
    await page.close();
  }
  return { nazwa: 'interakcje-ukryj-online', problemy: problems };
}

/** Achievement dates older than 30 days: DD.MM.RRRR with a leading zero. */
async function achievementDates(browser: Browser, address: string): Promise<Result> {
  const problems: string[] = [];
  const page = await browser.newPage({ viewport: { width: 740, height: 900 } });
  try {
    await open(page, address, 'ekran=achievements');
    const body = await page.locator('[data-ekran="achievements"]').innerText();
    const bad = body.match(/(?<![\d.])\d\.\d{1,2}\.\d{4}/g);
    if (bad) problems.push(`daty bez zera wiodącego: ${[...new Set(bad)].slice(0, 5).join(', ')}`);
  } finally {
    await page.close();
  }
  return { nazwa: 'interakcje-data-osiagniecia', problemy: problems };
}

/** Badge glyphs go into {@html}: text is escaped and the wreath takes digits only. */
function badgeGlyphs(): Result {
  const problems: string[] = [];
  const text = napis(`<b>&"'`, 1);
  if (!text.includes('>&lt;b&gt;&amp;&quot;&#39;</text>')) problems.push(`napis bez escapowania: ${text}`);
  if (!znak('wieniec42').includes('>42</text>')) problems.push('wieniec42 bez liczby');
  for (const name of ['wieniec<img src=x onerror=alert(1)>', 'wieniec4a', 'wieniec', 'wieniec 1']) {
    if (znak(name) !== '') problems.push(`znak „${name}” nie jest pusty`);
  }
  return { nazwa: 'interakcje-znak-odznaki', problemy: problems };
}

/** Description and game name from another user (mock scenario tekst-html) stay plain text. */
async function htmlFromOthers(browser: Browser, address: string): Promise<Result> {
  const html = '<b>x</b><img src=x onerror=alert(1)>';
  const problems: string[] = [];
  const page = await browser.newPage({ viewport: { width: 1000, height: 800 } });
  await page.addInitScript(() => {
    const w = window as unknown as { alerts: number };
    w.alerts = 0;
    window.alert = () => void w.alerts++;
  });
  /** Elements or calls that the text would produce if it were parsed as HTML. */
  const injected = async (where: string) => {
    await page.waitForTimeout(300);
    const r = await page.evaluate(() => ({
      alerts: (window as unknown as { alerts: number }).alerts,
      img: document.querySelectorAll('img[src="x"], [onerror]').length,
      b: [...document.querySelectorAll('b')].filter((b) => b.textContent === 'x').length,
    }));
    if (r.alerts) problems.push(`${where}: alert wywołany ${r.alerts} raz(y)`);
    if (r.img) problems.push(`${where}: w DOM jest <img> z tekstu`);
    if (r.b) problems.push(`${where}: w DOM jest <b> z tekstu`);
  };
  /** Literal text of an element: whole string, no child elements, optionally the same title. */
  const literal = async (where: string, selector: string, title = false) => {
    const el = await page.evaluate((sel) => {
      const e = document.querySelector(sel);
      return e && { text: e.textContent, children: e.childElementCount, title: e.getAttribute('title') };
    }, selector);
    if (!el) problems.push(`${where}: brak elementu ${selector}`);
    else {
      if (el.text !== html) problems.push(`${where}: tekst „${el.text}”`);
      if (el.children) problems.push(`${where}: tekst ma elementy potomne`);
      if (title && el.title !== html) problems.push(`${where}: title „${el.title}”`);
    }
  };
  const rowTitle = async (where: string, id: string) => {
    const row = await page.evaluate((i) => {
      const r = document.querySelector(`.slot-panelu .wiersz[data-id="${i}"]`);
      return r && { title: r.getAttribute('title') ?? '', label: r.getAttribute('aria-label') };
    }, id);
    if (!row?.title.includes(html)) problems.push(`${where}: podpowiedź „${row?.title}” bez całego tekstu`);
    return row;
  };
  try {
    await open(page, address, 'ekran=main&stan=tekst-html&panel=pelny');
    await literal('pełny panel, gra', '.slot-panelu .wiersz[data-id="u-html"] .gra', true);
    await literal('pełny panel, opis', '.slot-panelu .wiersz[data-id="u-html-opis"] .opis');
    await rowTitle('pełny panel', 'u-html');
    await injected('pełny panel');

    await open(page, address, 'ekran=main&stan=tekst-html&panel=waski');
    const strip = await rowTitle('pasek', 'u-html');
    if (!strip?.label?.includes(html)) problems.push(`pasek: etykieta „${strip?.label}” bez całego tekstu`);
    await injected('pasek');

    await open(page, address, 'ekran=onlinePerson&stan=tekst-html');
    await literal('profil osoby, opis', '[data-ekran="onlinePerson"] .opis-znajomego');
    await literal('podział na gry', '[data-ekran="onlinePerson"] .gry .gra .nazwa', true);
    await injected('profil osoby');
  } catch (e) {
    problems.push(String(e));
  } finally {
    await page.close();
  }
  return { nazwa: 'interakcje-tekst-html', problemy: problems };
}

/**
 * Focus ring only from Tab navigation: none after a click, also when a global key (F8) follows
 * or a script moves focus, and one after Tab.
 */
async function focusRing(browser: Browser, address: string): Promise<Result> {
  const problems: string[] = [];
  const page = await browser.newPage({ viewport: { width: 1000, height: 800 } });
  /** Ring drawn around the focused element, or why it cannot be judged. */
  const ring = () =>
    page.evaluate(() => {
      const el = document.activeElement as HTMLElement | null;
      if (!el || el === document.body) return { focused: null, ring: false };
      const s = getComputedStyle(el);
      return {
        focused: el.className,
        ring: (s.outlineStyle !== 'none' && parseFloat(s.outlineWidth) > 0) || s.boxShadow !== 'none',
      };
    });
  const afterClick = async (what: string, selector: string) => {
    await page.locator(selector).first().click();
    await page.waitForTimeout(200);
    await page.keyboard.press('F8');
    const r = await ring();
    if (!r.focused) problems.push(`${what}: po kliknięciu nic nie ma fokusu`);
    else if (r.ring) problems.push(`${what}: obwódka fokusu po kliknięciu i F8`);
  };
  try {
    await open(page, address, 'ekran=main&panel=pelny');
    await afterClick('wiersz znajomego', '.slot-panelu .wiersz');
    await page.evaluate(() => document.querySelector<HTMLElement>('.slot-panelu .ib')?.focus());
    if ((await ring()).ring) problems.push('obwódka po focus() ze skryptu');
    await afterClick('„Usuń ze znajomych”', `[data-ekran="onlinePerson"] button:has-text("${text('znajomi.usun')}")`);
    await afterClick('stopka panelu', '.slot-panelu .stopka .status');
    await page.keyboard.press('Escape');
    await page.locator('.slot-panelu .wiersz').first().click();
    await page.keyboard.press('Tab');
    const tab = await ring();
    if (!tab.focused) problems.push('po Tab nic nie ma fokusu');
    else if (!tab.ring) problems.push(`po Tab brak obwódki fokusu (${tab.focused})`);
    await page.mouse.down();
    await page.mouse.up();
    if ((await ring()).ring) problems.push('obwódka zostaje po kliknięciu myszą po Tab');
  } catch (e) {
    problems.push(String(e));
  } finally {
    await page.close();
  }
  return { nazwa: 'interakcje-obwodka-fokusu', problemy: problems };
}

/**
 * Settings > Language: the title keeps the English word in every other language, the list shows
 * each language in its own name; expanding and collapsing any category leaves all of them shown.
 */
async function languageCategory(browser: Browser, address: string): Promise<Result> {
  const problems: string[] = [];
  const manifest = JSON.parse(fs.readFileSync(new URL('../locales/languages.json', import.meta.url), 'utf8')) as {
    languages: { code: string; name: string }[];
  };
  const own = new Map(manifest.languages.map((l) => [l.code, l.name]));
  const titles = () =>
    page.evaluate(() =>
      [...document.querySelectorAll('[data-ekran="settings"] .akordeon')].map((a) => {
        const r = a.getBoundingClientRect();
        const shown = a.checkVisibility({ opacityProperty: true, visibilityProperty: true }) && r.height > 0;
        return { title: a.querySelector('.tytul')?.textContent ?? '', shown, transform: getComputedStyle(a).transform };
      }),
    );
  let page = await browser.newPage({ viewport: { width: 740, height: 900 } });
  try {
    for (const [code, expected] of [['pl', `${polish('language.category.name')} · Language`], ['en', 'Language']]) {
      await open(page, address, `ekran=settings&otwarte=jezyk&jezyk=${code}`);
      const title = (await titles())[5]?.title;
      if (title !== expected) problems.push(`${code}: kategoria języka „${title}”, oczekiwano „${expected}”`);
      const rows = await page.locator('[data-ekran="settings"] li[lang]').evaluateAll((li) =>
        li.map((l) => [l.getAttribute('lang') ?? '', l.querySelector('.tytul')?.textContent?.trim() ?? '']),
      );
      for (const [lang, name] of rows) {
        if (own.has(lang) && own.get(lang) !== name) problems.push(`${code}: język ${lang} na liście jako „${name}”, nie „${own.get(lang)}”`);
      }
    }
    for (const width of [740, 420]) {
      await page.close();
      page = await browser.newPage({ viewport: { width, height: 700 } });
      await open(page, address, 'ekran=settings&otwarte=');
      const count = (await titles()).length;
      for (let i = 0; i < count; i++) {
        const header = page.locator('[data-ekran="settings"] .akordeon > .naglowek').nth(i);
        await header.click();
        await page.waitForTimeout(300);
        await header.click();
        await page.waitForTimeout(300);
        const after = await titles();
        const missing = after.filter((a) => !a.shown || a.transform !== 'none').map((a) => a.title);
        if (after.length !== count || missing.length) {
          problems.push(`${width} px, po rozwinięciu i zwinięciu kategorii ${i + 1}: ${after.length}/${count} kategorii, niewidoczne: ${missing.join(', ')}`);
        }
      }
    }
  } catch (e) {
    problems.push(String(e));
  } finally {
    await page.close();
  }
  return { nazwa: 'interakcje-kategoria-jezyka', problemy: problems };
}

export async function checkInteractions(browser: Browser, address: string): Promise<Result[]> {
  return [
    ...(await panelHeader(browser, address)),
    ...(await collapsedStrip(browser, address)),
    await failedCommand(browser, address),
    await thresholds(browser, address),
    await newPersonWithAccount(browser, address),
    await reportLength(browser, address),
    await hideOnline(browser, address),
    await achievementDates(browser, address),
    badgeGlyphs(),
    await htmlFromOthers(browser, address),
    await focusRing(browser, address),
    await languageCategory(browser, address),
  ];
}
