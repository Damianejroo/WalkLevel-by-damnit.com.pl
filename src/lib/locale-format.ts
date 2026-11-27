// Numbers and plural categories of a language: Intl.NumberFormat and Intl.PluralRules (CLDR).
// Rust does the same with ICU4X (crates/core/src/texts/locale.rs); the generator writes samples
// of both functions to a fixture that the Rust tests compare. No imports: Node scripts use it.

/** Exceptions to CLDR number formatting of a language (locales/languages.json `number`). */
export interface NumberStyle {
  /** "always": group also 4-digit numbers (CLDR pl groups from 5 digits) */
  grouping?: 'always' | 'auto';
  /** group separator instead of the CLDR one */
  groupSeparator?: string;
}

export interface LocaleFormat {
  readonly code: string;
  /** `places` digits after the decimal separator, rounded */
  number(value: number, places?: number): string;
  /** a number shown by `number` (or typed the same way) back as a value and its visible digits */
  parse(text: string): { value: number; fractionDigits: number } | null;
  /** CLDR cardinal category of a value with `fractionDigits` visible digits */
  plural(value: number, fractionDigits: number): string;
}

export function localeFormat(code: string, style: NumberStyle = {}): LocaleFormat {
  const formats = new Map<number, Intl.NumberFormat>();
  const formatter = (places: number) => {
    let f = formats.get(places);
    if (!f) {
      f = new Intl.NumberFormat(code, {
        minimumFractionDigits: places,
        maximumFractionDigits: places,
        useGrouping: style.grouping === 'always' ? 'always' : 'auto',
      });
      formats.set(places, f);
    }
    return f;
  };
  const sample = new Intl.NumberFormat(code, { minimumFractionDigits: 1, useGrouping: 'always' }).formatToParts(12345.5);
  const decimal = sample.find((p) => p.type === 'decimal')?.value ?? '.';
  const group = style.groupSeparator ?? sample.find((p) => p.type === 'group')?.value ?? ',';
  const rules = new Map<number, Intl.PluralRules>();
  return {
    code,
    number(value, places = 0) {
      if (!Number.isFinite(value)) return '–';
      // -0.04 rounded to one place is 0.0, without a minus
      const rounded = Number(value.toFixed(places)) === 0 ? 0 : value;
      return formatter(places)
        .formatToParts(rounded)
        .map((p) => (p.type === 'group' ? group : p.value))
        .join('');
    },
    parse(text) {
      let clean = '';
      for (const c of text.trim()) {
        if (c === decimal) clean += '.';
        else if (c === group || /\s/.test(c) || c === ' ' || c === ' ') continue;
        else if (c === '−') clean += '-';
        else clean += c;
      }
      if (!/^-?\d+(\.\d+)?$/.test(clean)) return null;
      return { value: Number(clean), fractionDigits: clean.includes('.') ? clean.length - clean.indexOf('.') - 1 : 0 };
    },
    plural(value, fractionDigits) {
      let r = rules.get(fractionDigits);
      if (!r) {
        r = new Intl.PluralRules(code, { minimumFractionDigits: fractionDigits, maximumFractionDigits: fractionDigits });
        rules.set(fractionDigits, r);
      }
      return r.select(value);
    },
  };
}

/** Plural categories of a language in CLDR order (zero, one, two, few, many, other). */
export function pluralCategories(code: string): string[] {
  const order = ['zero', 'one', 'two', 'few', 'many', 'other'];
  const found = new Intl.PluralRules(code).resolvedOptions().pluralCategories as string[];
  return order.filter((c) => found.includes(c));
}
