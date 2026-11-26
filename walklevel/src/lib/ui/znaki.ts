// Znaki na odznakach (bieznia/gui/odznaki.py glyph) jako proste kształty SVG w układzie -1..1.
// Rysowane w kolorze currentColor: obrys 0,16 z zaokrąglonymi końcami (domyślnie),
// klasa „f” - wypełnienie bez obrysu, „fs” - wypełnienie z obrysem, „d” - wycięcie (kolor tła
// odznaki), „c” - cieńszy obrys, „lawa” - wyjątek w danych (wulkan).

const n = (x: number) => +x.toFixed(3);
const pt = (x: number, y: number) => `${n(x)} ${n(y)}`;
const rad = (st: number) => (st * Math.PI) / 180;

const linia = (x1: number, y1: number, x2: number, y2: number, k = '') =>
  `<path${k ? ` class="${k}"` : ''} d="M${pt(x1, y1)}L${pt(x2, y2)}"/>`;
const wielokat = (pkt: number[][], k = 'fs') => `<path class="${k}" d="M${pkt.map(([x, y]) => pt(x, y)).join('L')}Z"/>`;
const kolo = (x: number, y: number, r: number, k = '') => `<circle${k ? ` class="${k}"` : ''} cx="${n(x)}" cy="${n(y)}" r="${n(r)}"/>`;
const elipsa = (x: number, y: number, rx: number, ry: number, k = '') =>
  `<ellipse${k ? ` class="${k}"` : ''} cx="${n(x)}" cy="${n(y)}" rx="${n(rx)}" ry="${n(ry)}"/>`;
const prost = (x: number, y: number, w: number, h: number, k = 'f', rr = 0) =>
  `<rect class="${k}" x="${n(x)}" y="${n(y)}" width="${n(w)}" height="${n(h)}"${rr ? ` rx="${n(rr)}"` : ''}/>`;
const sciezka = (d: string, k = '') => `<path${k ? ` class="${k}"` : ''} d="${d}"/>`;

/** Łuk jak QPainter.drawArc: prostokąt (x, y, w, h), kąty w stopniach przeciwnie do zegara od godz. 3. */
function luk(x: number, y: number, w: number, h: number, start: number, rozpietosc: number, k = '') {
  const cx = x + w / 2;
  const cy = y + h / 2;
  const rx = w / 2;
  const ry = h / 2;
  const a = rad(start);
  const b = rad(start + rozpietosc);
  const duzy = Math.abs(rozpietosc) > 180 ? 1 : 0;
  const kierunek = rozpietosc > 0 ? 0 : 1;
  return sciezka(`M${pt(cx + rx * Math.cos(a), cy - ry * Math.sin(a))}A${n(rx)} ${n(ry)} 0 ${duzy} ${kierunek} ${pt(cx + rx * Math.cos(b), cy - ry * Math.sin(b))}`, k);
}

const napis = (tekst: string, wys: number) =>
  `<text class="tx" x="0" y="0" font-size="${n(wys)}" text-anchor="middle" dominant-baseline="central">${tekst}</text>`;

function wieniec(liczba: string) {
  let s = luk(-0.75, -0.75, 1.5, 1.5, -200, 170) + luk(-0.75, -0.75, 1.5, 1.5, 30, 170);
  for (const strona of [-1, 1]) {
    for (let k = 0; k < 4; k++) {
      const a = rad(strona < 0 ? 200 - k * 38 : -20 + k * 38);
      s += kolo(0.75 * Math.cos(a), -0.75 * Math.sin(a), 0.13, 'f');
    }
  }
  return s + napis(liczba, 0.75);
}

const ZNAKI: Record<string, () => string> = {
  stopy: () => {
    let s = '';
    for (const [dx, dy] of [[-0.38, 0.25], [0.38, -0.3]]) {
      s += elipsa(dx, dy + 0.11, 0.22, 0.31, 'f');
      [[-0.17, -0.36], [-0.02, -0.44], [0.14, -0.4]].forEach(([tx, ty], i) => (s += kolo(dx + tx, dy + ty, 0.085 - i * 0.008, 'f')));
    }
    return s;
  },
  kopula: () =>
    linia(-0.95, 0.55, 0.95, 0.55) + luk(-0.8, -0.85, 1.6, 2.8, 0, 180) + luk(-0.45, -0.85, 0.9, 2.8, 0, 180, 'c') +
    wielokat([[-0.05, -0.15], [0.12, -0.15], [-0.02, 0.12], [0.1, 0.12], [-0.12, 0.42], [-0.03, 0.16], [-0.14, 0.16]]),
  helm: () =>
    sciezka('M-0.55 0.45C-0.6 -0.45 0.6 -0.45 0.55 0.45Z', 'f') +
    [-1, 1].map((z) => sciezka(`M${pt(z * 0.45, 0.05)}C${pt(z * 0.85, 0)} ${pt(z * 0.95, -0.45)} ${pt(z * 0.8, -0.85)}C${pt(z * 0.75, -0.45)} ${pt(z * 0.6, -0.25)} ${pt(z * 0.4, -0.2)}Z`, 'f')).join('') +
    prost(-0.07, -0.05, 0.14, 0.55, 'd') + prost(-0.62, 0.42, 1.24, 0.14),
  miecz: () =>
    `<g transform="rotate(45)">${wielokat([[-0.1, 0.3], [-0.1, -0.75], [0, -0.95], [0.1, -0.75], [0.1, 0.3]])}${linia(-0.35, 0.32, 0.35, 0.32)}${linia(0, 0.35, 0, 0.72)}${kolo(0, 0.82, 0.1, 'fs')}</g>`,
  dycha: () => napis('10', 1.2),
  licznik: () => {
    let s = luk(-0.85, -0.7, 1.7, 1.7, 0, 180);
    for (let k = 0; k < 7; k++) {
      const a = rad(180 - k * 30);
      s += linia(0.85 * Math.cos(a), 0.15 - 0.85 * Math.sin(a), 0.62 * Math.cos(a), 0.15 - 0.62 * Math.sin(a));
    }
    return s + linia(0, 0.15, 0.5, -0.38) + kolo(0, 0.15, 0.14, 'fs') + linia(-0.6, 0.62, 0.6, 0.62);
  },
  luki: () =>
    [-0.45, 0, 0.45].map((dx) => linia(dx, 0.85, dx, -0.55) + wielokat([[dx - 0.18, -0.45], [dx, -0.9], [dx + 0.18, -0.45]]) + linia(dx, 0.85, dx - 0.15, 0.6) + linia(dx, 0.85, dx + 0.15, 0.6)).join(''),
  mur: () => {
    let s = '';
    for (let rz = 0; rz < 4; rz++) {
      const y = -0.7 + rz * 0.38;
      let x = -0.9 - (rz % 2 === 0 ? 0 : 0.25);
      while (x < 0.9) {
        const x0 = Math.max(x, -0.9);
        const x1 = Math.min(x + 0.46, 0.9);
        if (x1 - x0 > 0.08) s += prost(x0 + 0.02, y, x1 - x0 - 0.04, 0.32);
        x += 0.5;
      }
    }
    return s + prost(-0.9, -0.95, 0.3, 0.22) + prost(-0.15, -0.95, 0.3, 0.22) + prost(0.6, -0.95, 0.3, 0.22);
  },
  lisc: () => {
    let s = sciezka('M0 -0.95C0.75 -0.45 0.6 0.45 0 0.75C-0.6 0.45 -0.75 -0.45 0 -0.95Z', 'f') + linia(0, -0.7, 0, 0.95, 'dl');
    for (let k = 0; k < 3; k++) {
      const y = -0.35 + k * 0.3;
      s += linia(0, y + 0.12, -0.3, y - 0.05, 'dl') + linia(0, y + 0.12, 0.3, y - 0.05, 'dl');
    }
    return s + linia(0, 0.7, 0, 0.95, 'c');
  },
  gora: () =>
    wielokat([[-0.95, 0.75], [-0.15, -0.45], [0.15, -0.1], [0.35, -0.3], [0.95, 0.75]]) +
    sciezka('M-0.45 -0.7Q-0.2 -0.95 0 -0.72Q0.2 -0.95 0.45 -0.7', 'c'),
  wulkan: () =>
    wielokat([[-0.95, 0.8], [-0.3, -0.35], [0.3, -0.35], [0.95, 0.8]]) +
    wielokat([[-0.3, -0.35], [0.3, -0.35], [0.12, 0.05], [0, -0.1], [-0.15, 0.2]], 'lawa') +
    kolo(-0.2, -0.65, 0.11, 'f') + kolo(0.15, -0.8, 0.09, 'f') + kolo(0.35, -0.55, 0.07, 'f'),
  pudlo: () =>
    wielokat([[-0.7, -0.35], [0, -0.7], [0.7, -0.35], [0.7, 0.45], [0, 0.8], [-0.7, 0.45]], '') +
    linia(-0.7, -0.35, 0, 0) + linia(0.7, -0.35, 0, 0) + linia(0, 0, 0, 0.8) + linia(-0.35, -0.52, 0.35, -0.17),
  wglab: () =>
    kolo(0, 0, 0.85) + luk(-0.85, -0.3, 1.7, 0.6, 0, 180) + kolo(0, 0.15, 0.25, 'fs') + linia(0, -0.95, 0, -0.25) +
    wielokat([[-0.15, -0.4], [0, -0.2], [0.15, -0.4]]),
  kostka: () =>
    wielokat([[0, -0.9], [0.8, -0.45], [0, 0], [-0.8, -0.45]], 'f') +
    `<path class="f" fill-opacity="0.67" d="M-0.8 -0.45L0 0L0 0.9L-0.8 0.45Z"/><path class="f" fill-opacity="0.43" d="M0.8 -0.45L0 0L0 0.9L0.8 0.45Z"/>`,
  biegacz: () =>
    sciezka('M-0.9 0.6C-0.4 -0.4 0.2 0.7 0.85 -0.55', 'kreski') + kolo(-0.9, 0.6, 0.14, 'f') + wielokat([[0.6, -0.75], [0.98, -0.65], [0.75, -0.3]], 'f'),
  glob: () =>
    kolo(0, 0, 0.88) + elipsa(0, 0, 0.38, 0.88) + linia(-0.88, 0, 0.88, 0) +
    luk(-0.8, -0.75, 1.6, 0.7, 180, 180) + luk(-0.8, 0.05, 1.6, 0.7, 0, 180),
  plomien: () =>
    sciezka('M0 -0.95C0.2 -0.45 0.8 -0.2 0.6 0.35C0.5 0.75 0.2 0.9 0 0.9C-0.4 0.9 -0.7 0.6 -0.65 0.2C-0.6 -0.1 -0.3 -0.2 -0.25 -0.5C0.05 -0.3 0.05 -0.6 0 -0.95Z', 'f') +
    elipsa(0, 0.45, 0.25, 0.35, 'd'),
  pad: () =>
    prost(-0.9, -0.45, 1.8, 0.85, 'f', 0.35) + elipsa(-0.6, 0.35, 0.32, 0.42, 'f') + elipsa(0.6, 0.35, 0.32, 0.42, 'f') +
    prost(-0.62, -0.13, 0.36, 0.1, 'd') + prost(-0.49, -0.26, 0.1, 0.36, 'd') +
    [[0.45, -0.08], [0.62, -0.22], [0.62, 0.06], [0.79, -0.08]].map(([x, y]) => kolo(x, y, 0.07, 'd')).join(''),
  portale: () => kolo(-0.38, 0.22, 0.46) + kolo(0.38, 0.22, 0.46) + kolo(0, -0.42, 0.46),
  dom: () => wielokat([[-0.85, 0], [0, -0.8], [0.85, 0]]) + prost(-0.6, 0, 1.2, 0.8, '') + prost(-0.15, 0.35, 0.3, 0.45, 'fs'),
  ksiezyc: () => `<path class="f" fill-rule="evenodd" d="M-0.8 0A0.8 0.8 0 1 0 0.8 0A0.8 0.8 0 1 0 -0.8 0Z"/>${kolo(0.38, -0.25, 0.68, 'd')}`,
  slonce: () => {
    let s = kolo(0, 0.25, 0.38, 'fs');
    for (let k = 0; k < 5; k++) {
      const a = rad(180 - k * 45);
      s += linia(0.58 * Math.cos(a), 0.25 - 0.58 * Math.sin(a), 0.88 * Math.cos(a), 0.25 - 0.88 * Math.sin(a));
    }
    return s + linia(-0.95, 0.72, 0.95, 0.72);
  },
  druzyna: () =>
    ([[-0.42, 0.8], [0.42, 0.8], [0, 1]] as const).map(([x, sc]) => kolo(x, -0.35 * sc, 0.22 * sc, 'f') + prost(x - 0.36 * sc, 0, 0.72 * sc, 0.75 * sc, 'f', 0.3 * sc)).join(''),
  fala: () =>
    [0, 1, 2].map((k) => {
      const y = -0.5 + k * 0.5;
      return sciezka(`M-0.9 ${n(y)}C-0.6 ${n(y - 0.3)} -0.3 ${n(y + 0.3)} 0 ${n(y)}C0.3 ${n(y - 0.3)} 0.6 ${n(y + 0.3)} 0.9 ${n(y)}`);
    }).join(''),
  portal: () =>
    prost(-0.7, -0.95, 0.2, 1.9) + prost(0.5, -0.95, 0.2, 1.9) + prost(-0.7, -0.95, 1.4, 0.2) + prost(-0.7, 0.75, 1.4, 0.2) +
    `<rect class="f" fill-opacity="0.45" x="-0.5" y="-0.75" width="1" height="1.5"/>` + luk(-0.3, -0.45, 0.6, 0.9, 30, 260, 'c'),
  brama: () => {
    const gwiazda = Array.from({ length: 10 }, (_, k) => {
      const r = k % 2 === 0 ? 0.24 : 0.1;
      const a = rad(-90 + 36 * k);
      return [r * Math.cos(a), -0.3 + r * Math.sin(a)];
    });
    return sciezka('M-0.6 0.85L-0.6 -0.15A0.6 0.6 0 0 1 0.6 -0.15L0.6 0.85') + linia(0, -0.15, 0, 0.85) + wielokat(gwiazda, 'f');
  },
  drzwi: () => kolo(0, 0, 0.78) + kolo(0, 0, 0.55, 'f') + kolo(0, 0, 0.12, 'd'),
  celownik: () =>
    kolo(0, 0, 0.6) + linia(0, -0.95, 0, -0.3) + linia(0, 0.3, 0, 0.95) + linia(-0.95, 0, -0.3, 0) + linia(0.3, 0, 0.95, 0) + kolo(0, 0, 0.08, 'fs'),
  cytadela: () => {
    let s = kolo(0, -0.1, 0.25);
    for (let k = 0; k < 5; k++) {
      const a = rad(90 + k * 72);
      s += linia(0.25 * Math.cos(a), -0.1 + 0.25 * Math.sin(a), 0.92 * Math.cos(a), -0.1 + 0.92 * Math.sin(a));
    }
    return s;
  },
  gwiazda: () => kolo(0, 0, 0.85, 'f') + kolo(-0.32, -0.3, 0.24, 'd') + prost(-0.85, -0.04, 1.7, 0.08, 'd') + kolo(-0.32, -0.3, 0.08, 'd'),
  dysk: () => elipsa(0, 0.05, 0.95, 0.3) + elipsa(0, -0.325, 0.35, 0.225, 'fs') + [-0.5, 0, 0.5].map((x) => linia(x, 0.4, x * 0.9, 0.85)).join(''),
  pierscien: () => elipsa(0, 0, 0.9, 0.45, 'gr') + elipsa(0, 0, 0.55, 0.25, 'c'),
  planeta: () => kolo(0, 0, 0.8) + kolo(-0.3, -0.25, 0.16, 'fs') + kolo(0.3, 0.1, 0.22, 'fs') + kolo(-0.15, 0.4, 0.1, 'fs'),
  wieza: () =>
    wielokat([[-0.45, 0.9], [-0.45, -0.5], [0.45, -0.5], [0.45, 0.9]]) + [-0.45, -0.1, 0.25].map((x) => prost(x, -0.8, 0.2, 0.3, 'fs')).join('') +
    prost(-0.15, 0.35, 0.3, 0.55, 'd', 0.15),
  drzewo: () => kolo(0, -0.4, 0.42, 'f') + kolo(-0.4, -0.05, 0.35, 'f') + kolo(0.4, -0.05, 0.35, 'f') + prost(-0.1, 0.1, 0.2, 0.8),
  kolo: () => {
    let s = kolo(0, 0, 0.82);
    for (let k = 0; k < 8; k++) s += linia(0, 0, 0.82 * Math.cos(rad(k * 45)), 0.82 * Math.sin(rad(k * 45)));
    return s + kolo(0, 0, 0.16, 'fs');
  },
  czerw: () => {
    let s = kolo(0, 0, 0.85);
    for (let k = 0; k < 12; k++) {
      const a = rad(k * 30);
      s += wielokat([[0.35 * Math.cos(a), 0.35 * Math.sin(a)], [0.78 * Math.cos(a - 0.18), 0.78 * Math.sin(a - 0.18)], [0.78 * Math.cos(a + 0.18), 0.78 * Math.sin(a + 0.18)]], 'f');
    }
    return s;
  },
  radiacja: () => {
    let s = '';
    for (let k = 0; k < 3; k++) {
      const a = 60 + k * 120;
      const z = (st: number, r: number) => pt(r * Math.cos(rad(st)), -r * Math.sin(rad(st)));
      s += sciezka(`M${z(a, 0.3)}L${z(a, 0.9)}A0.9 0.9 0 0 0 ${z(a + 60, 0.9)}L${z(a + 60, 0.3)}A0.3 0.3 0 0 1 ${z(a, 0.3)}Z`, 'f');
    }
    return s + kolo(0, 0, 0.18, 'f');
  },
  korona: () => wielokat([[-0.85, 0.5], [-0.85, -0.4], [-0.42, 0], [0, -0.7], [0.42, 0], [0.85, -0.4], [0.85, 0.5]]) + linia(-0.85, 0.72, 0.85, 0.72),
};

const pamiec = new Map<string, string>();

/** Treść SVG znaku (układ -1..1) albo pusty napis dla nieznanej nazwy. */
export function znak(nazwa: string): string {
  let s = pamiec.get(nazwa);
  if (s === undefined) {
    s = nazwa.startsWith('wieniec') ? wieniec(nazwa.slice(7)) : (ZNAKI[nazwa]?.() ?? '');
    pamiec.set(nazwa, s);
  }
  return s;
}

/** Nazwy wszystkich znaków (wzornik). */
export const NAZWY_ZNAKOW = [...Object.keys(ZNAKI), 'wieniec42'];
