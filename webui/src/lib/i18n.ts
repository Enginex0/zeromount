import { createSignal } from 'solid-js';
import en from '../locales/en.json';

type Strings = Record<string, string>;

const cache: Record<string, Strings> = { en };
const [locale, setLocaleSignal] = createSignal('en');
const [strings, setStrings] = createSignal<Strings>(en);

export { locale };

export function t(key: string, params?: Record<string, string | number>): string {
  let str = strings()[key] ?? (en as Strings)[key] ?? key;
  if (params) {
    for (const [k, v] of Object.entries(params))
      str = str.replaceAll(`{${k}}`, String(v));
  }
  return str;
}

function getInlinedLocale(code: string): Strings | null {
  const locales = (window as any).__ZM_LOCALES__;
  if (locales && typeof locales === 'object' && locales[code]) return locales[code];
  return null;
}

export async function loadLocale(code: string) {
  if (code === 'en') {
    setStrings(en);
    setLocaleSignal('en');
    return;
  }
  if (!cache[code]) {
    // Check boot-time inlined locales first (zero I/O)
    const inlined = getInlinedLocale(code);
    if (inlined) {
      cache[code] = inlined;
    } else {
      try {
        cache[code] = (await import(`../locales/${code}.json`)).default;
      } catch {
        setStrings(en);
        setLocaleSignal('en');
        return;
      }
    }
  }
  setStrings({ ...(en as Strings), ...cache[code] });
  setLocaleSignal(code);
}

export function detectLocale(available: string[]): string {
  const stored = localStorage.getItem('zeromount-language');
  if (stored && available.includes(stored)) return stored;

  const hint = (window as any).__ZM_LOCALE_CODE__;
  if (typeof hint === 'string' && available.includes(hint)) return hint;

  const raw = navigator.language || 'en';
  if (available.includes(raw)) return raw;

  const normalized = raw.replace(/_/g, '-');
  if (available.includes(normalized)) return normalized;

  const lower = normalized.toLowerCase();
  if (lower.startsWith('zh-hans') || lower.startsWith('zh-cn')) {
    if (available.includes('zh-CN')) return 'zh-CN';
  }
  if (lower.startsWith('zh-hant') || lower.startsWith('zh-tw')) {
    if (available.includes('zh-TW')) return 'zh-TW';
  }

  const base = raw.split('-')[0];
  if (available.includes(base)) return base;

  const partial = available.find(c => c.split('-')[0] === base);
  if (partial) return partial;

  return 'en';
}

export const RTL_LOCALES = new Set(['ar', 'fa', 'he']);

export const LANGUAGES = [
  { code: 'en', name: 'English' },
  { code: 'af', name: 'Afrikaans' },
  { code: 'ar', name: 'العربية' },
  { code: 'bg', name: 'Български' },
  { code: 'bn', name: 'বাংলা' },
  { code: 'ca', name: 'Català' },
  { code: 'cs', name: 'Čeština' },
  { code: 'da', name: 'Dansk' },
  { code: 'de', name: 'Deutsch' },
  { code: 'el', name: 'Ελληνικά' },
  { code: 'es', name: 'Español' },
  { code: 'fa', name: 'فارسی' },
  { code: 'fi', name: 'Suomi' },
  { code: 'fr', name: 'Français' },
  { code: 'he', name: 'עברית' },
  { code: 'hi', name: 'हिन्दी' },
  { code: 'hu', name: 'Magyar' },
  { code: 'id', name: 'Bahasa Indonesia' },
  { code: 'it', name: 'Italiano' },
  { code: 'ja', name: '日本語' },
  { code: 'ko', name: '한국어' },
  { code: 'nl', name: 'Nederlands' },
  { code: 'no', name: 'Norsk' },
  { code: 'pl', name: 'Polski' },
  { code: 'pt-BR', name: 'Português (BR)' },
  { code: 'pt-PT', name: 'Português (PT)' },
  { code: 'ro', name: 'Română' },
  { code: 'ru', name: 'Русский' },
  { code: 'sr', name: 'Српски' },
  { code: 'sv', name: 'Svenska' },
  { code: 'th', name: 'ไทย' },
  { code: 'tr', name: 'Türkçe' },
  { code: 'uk', name: 'Українська' },
  { code: 'vi', name: 'Tiếng Việt' },
  { code: 'zh-CN', name: '简体中文' },
  { code: 'zh-TW', name: '繁體中文' },
] as const;
