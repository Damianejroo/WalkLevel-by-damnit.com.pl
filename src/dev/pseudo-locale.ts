// Pseudo-language en-XA for the view tests (docs/i18n.md): every text of the built-in pack with
// accented letters, about 40% longer and in ⟦ ⟧. Cut texts and texts typed straight into the
// code (without t()) stand out. Only letters of the plain text change; variables, plural and
// select options keep their syntax.
import { parseMessage, type Part } from '../lib/message-format';

const ACCENTS: Record<string, string> = {
  a: 'á', b: 'ƀ', c: 'ç', d: 'ð', e: 'é', f: 'ƒ', g: 'ĝ', h: 'ĥ', i: 'í', j: 'ĵ', k: 'ķ', l: 'ļ', m: 'ɱ',
  n: 'ñ', o: 'ó', p: 'þ', q: 'ǫ', r: 'ŕ', s: 'š', t: 'ţ', u: 'ú', v: 'ṽ', w: 'ŵ', x: 'ẋ', y: 'ý', z: 'ž',
  A: 'Á', B: 'Ɓ', C: 'Ç', D: 'Ð', E: 'É', F: 'Ƒ', G: 'Ĝ', H: 'Ĥ', I: 'Í', J: 'Ĵ', K: 'Ķ', L: 'Ļ', M: 'Ṁ',
  N: 'Ñ', O: 'Ó', P: 'Þ', Q: 'Ǫ', R: 'Ŕ', S: 'Š', T: 'Ţ', U: 'Ú', V: 'Ṽ', W: 'Ŵ', X: 'Ẋ', Y: 'Ý', Z: 'Ž',
};

/** Text with accents, every word longer by ~40% (vowels doubled). */
function stretch(text: string): string {
  return text.replace(/[A-Za-z]+/g, (word) => {
    let out = '';
    let extra = Math.ceil(word.length * 0.4);
    for (const c of word) {
      out += ACCENTS[c] ?? c;
      if (extra > 0 && /[aeiouAEIOU]/.test(c)) {
        out += ACCENTS[c.toLowerCase()];
        extra--;
      }
    }
    return out + '~'.repeat(Math.max(0, extra));
  });
}

/** ICU apostrophe quoting of plain text (the message is parsed again by the app). */
const quote = (text: string, inPlural: boolean) =>
  text.replace(/'/g, "''").replace(inPlural ? /[{}#]/g : /[{}]/g, (c) => `'${c}'`);

function write(parts: Part[], inPlural: boolean): string {
  return parts
    .map((p) => {
      if (typeof p === 'string') return quote(stretch(p), inPlural);
      if (p.kind === 'pound') return '#';
      if (p.type === 'simple') return `{${p.name}}`;
      const options = [...p.options].map(([k, sub]) => `${k} {${write(sub, p.type === 'plural')}}`).join(' ');
      return `{${p.name}, ${p.type}, ${p.offset ? `offset:${p.offset} ` : ''}${options}}`;
    })
    .join('');
}

/** Pseudo-translation of one message (" / " separators of split texts stay as they are). */
export function pseudoMessage(text: string): string {
  return `⟦${write(parseMessage(text), false)}⟧`;
}

/** Every text except names that are the same in every language (`WalkLevel`). */
export function pseudoTexts(texts: Record<string, string>, keep: string[] = []): Record<string, string> {
  const same = new Set(keep);
  return Object.fromEntries(Object.entries(texts).map(([k, v]) => [k, same.has(k) ? v : pseudoMessage(v)]));
}
