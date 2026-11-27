// ICU MessageFormat subset used by the language packs (docs/i18n.md): text, `{name}`,
// `{name, plural, [offset:n] =0 {...} one {...} other {...}}` with `#`, and
// `{name, select, key {...} other {...}}`. Apostrophes follow ICU (DOUBLE_OPTIONAL): `''` is one
// apostrophe, `'{` starts quoted text, any other apostrophe is literal. The same subset is parsed
// in Rust (crates/core/src/texts/message.rs). No imports: Node scripts use this file too.

export type Part = string | Argument | Pound;

export interface Argument {
  kind: 'argument';
  name: string;
  /** plain value, or a choice of sub-messages */
  type: 'simple' | 'plural' | 'select';
  offset: number;
  options: Map<string, Part[]>;
  /** source text, shown when the value is missing */
  raw: string;
}

export interface Pound {
  kind: 'pound';
}

export class MessageError extends Error {}

const isSpace = (c: string) => c === ' ' || c === '\t' || c === '\n' || c === '\r';

class Parser {
  i = 0;
  readonly text: string;
  constructor(text: string) {
    this.text = text;
  }

  fail(what: string): never {
    throw new MessageError(`${what} at ${this.i}: ${this.text}`);
  }

  spaces() {
    while (this.i < this.text.length && isSpace(this.text[this.i])) this.i++;
  }

  word(): string {
    const start = this.i;
    while (this.i < this.text.length && !/[\s{},#']/.test(this.text[this.i])) this.i++;
    if (this.i === start) this.fail('name expected');
    return this.text.slice(start, this.i);
  }

  message(inPlural: boolean, nested: boolean): Part[] {
    const parts: Part[] = [];
    let text = '';
    const flush = () => {
      if (text) parts.push(text);
      text = '';
    };
    while (this.i < this.text.length) {
      const c = this.text[this.i];
      if (c === "'") {
        const next = this.text[this.i + 1];
        if (next === "'") {
          text += "'";
          this.i += 2;
        } else if (next === '{' || next === '}' || (inPlural && next === '#')) {
          this.i++;
          while (this.i < this.text.length) {
            if (this.text[this.i] === "'") {
              if (this.text[this.i + 1] === "'") {
                text += "'";
                this.i += 2;
                continue;
              }
              this.i++;
              break;
            }
            text += this.text[this.i++];
          }
        } else {
          text += c;
          this.i++;
        }
      } else if (c === '{') {
        flush();
        parts.push(this.argument());
      } else if (c === '}') {
        if (!nested) this.fail('unmatched }');
        break;
      } else if (c === '#' && inPlural) {
        flush();
        parts.push({ kind: 'pound' });
        this.i++;
      } else {
        text += c;
        this.i++;
      }
    }
    flush();
    return parts;
  }

  argument(): Argument {
    const start = this.i;
    this.i++; // {
    this.spaces();
    const name = this.word();
    this.spaces();
    const options = new Map<string, Part[]>();
    let type: Argument['type'] = 'simple';
    let offset = 0;
    if (this.text[this.i] === ',') {
      this.i++;
      this.spaces();
      const kind = this.word();
      if (kind !== 'plural' && kind !== 'select') this.fail(`unsupported argument type ${kind}`);
      type = kind;
      this.spaces();
      if (this.text[this.i] !== ',') this.fail(', expected');
      this.i++;
      this.spaces();
      if (type === 'plural' && this.text.startsWith('offset:', this.i)) {
        this.i += 'offset:'.length;
        this.spaces();
        const m = /^\d+/.exec(this.text.slice(this.i));
        if (!m) this.fail('offset number expected');
        offset = Number(m[0]);
        this.i += m[0].length;
        this.spaces();
      }
      while (this.i < this.text.length && this.text[this.i] !== '}') {
        const selector = this.word();
        if (options.has(selector)) this.fail(`repeated option ${selector}`);
        this.spaces();
        if (this.text[this.i] !== '{') this.fail('{ expected');
        this.i++;
        options.set(selector, this.message(type === 'plural', true));
        if (this.text[this.i] !== '}') this.fail('} expected');
        this.i++;
        this.spaces();
      }
      if (!options.has('other')) this.fail(`${name}: option "other" is required`);
    }
    if (this.text[this.i] !== '}') this.fail('} expected');
    this.i++;
    return { kind: 'argument', name, type, offset, options, raw: this.text.slice(start, this.i) };
  }
}

export function parseMessage(text: string): Part[] {
  const p = new Parser(text);
  const parts = p.message(false, false);
  if (p.i < text.length) p.fail('unexpected }');
  return parts;
}

/** Arguments of a message: name -> its types and choice options (nested messages included). */
export function argumentsOf(parts: Part[], out = new Map<string, { types: Set<string>; options: Set<string>[] }>()) {
  for (const part of parts) {
    if (typeof part === 'string' || part.kind === 'pound') continue;
    const entry = out.get(part.name) ?? { types: new Set<string>(), options: [] };
    entry.types.add(part.type);
    if (part.type !== 'simple') entry.options.push(new Set(part.options.keys()));
    out.set(part.name, entry);
    for (const sub of part.options.values()) argumentsOf(sub, out);
  }
  return out;
}

export type Values = Record<string, string | number>;

export interface FormatContext {
  /** CLDR plural category of a value (number or text already formatted), null - not a number */
  plural(value: string | number, offset: number): { category: string; value: number } | null;
  /** a number variable shown as text */
  number(value: number): string;
}

export function formatMessage(parts: Part[], values: Values, ctx: FormatContext, pound?: string): string {
  let out = '';
  for (const part of parts) {
    if (typeof part === 'string') {
      out += part;
      continue;
    }
    if (part.kind === 'pound') {
      out += pound ?? '#';
      continue;
    }
    const value = values[part.name];
    if (value === undefined) {
      out += part.raw;
      continue;
    }
    if (part.type === 'simple') {
      out += typeof value === 'number' ? ctx.number(value) : value;
      continue;
    }
    if (part.type === 'select') {
      out += formatMessage(part.options.get(String(value)) ?? part.options.get('other')!, values, ctx, pound);
      continue;
    }
    const chosen = ctx.plural(value, part.offset);
    if (chosen === null) {
      out += part.raw;
      continue;
    }
    const exact = part.options.get(`=${typeof value === 'number' ? value : chosen.value + part.offset}`);
    const sub = exact ?? part.options.get(chosen.category) ?? part.options.get('other')!;
    const shown = typeof value === 'number' ? ctx.number(value - part.offset) : part.offset ? ctx.number(chosen.value) : value;
    out += formatMessage(sub, values, ctx, shown);
  }
  return out;
}
