/**
 * App languages: one selection drives the UI, quiz generation, subject
 * filtering and narration. Labels are native names. When adding a code
 * here, add its English name to `language_name` in backend/src/prompts.rs
 * and a UI string table in src/i18n.tsx too.
 */
export const LANGUAGES = [
  { code: 'en', label: 'English' },
  { code: 'sk', label: 'Slovenčina' },
  { code: 'ar', label: 'العربية' },
  { code: 'bg', label: 'Български' },
  { code: 'cs', label: 'Čeština' },
  { code: 'da', label: 'Dansk' },
  { code: 'de', label: 'Deutsch' },
  { code: 'el', label: 'Ελληνικά' },
  { code: 'es', label: 'Español' },
  { code: 'fi', label: 'Suomi' },
  { code: 'fr', label: 'Français' },
  { code: 'hi', label: 'हिन्दी' },
  { code: 'hr', label: 'Hrvatski' },
  { code: 'hu', label: 'Magyar' },
  { code: 'it', label: 'Italiano' },
  { code: 'ja', label: '日本語' },
  { code: 'nl', label: 'Nederlands' },
  { code: 'no', label: 'Norsk' },
  { code: 'pl', label: 'Polski' },
  { code: 'pt', label: 'Português' },
  { code: 'ro', label: 'Română' },
  { code: 'ru', label: 'Русский' },
  { code: 'sl', label: 'Slovenščina' },
  { code: 'sr', label: 'Српски' },
  { code: 'sv', label: 'Svenska' },
  { code: 'tr', label: 'Türkçe' },
  { code: 'uk', label: 'Українська' },
  { code: 'zh', label: '中文' },
] as const;

/** Native label for a language code, falling back to the bare code. */
export function languageLabel(code: string): string {
  return LANGUAGES.find((entry) => entry.code === code)?.label ?? code.toUpperCase();
}
