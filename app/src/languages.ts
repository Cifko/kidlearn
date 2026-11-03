export interface ContentLanguage {
  code: string;
  label: string;
}

/**
 * Quiz content languages: the AI generates in any of these. Labels are
 * native names, so they need no UI translation. When adding a code here,
 * add its English name to `language_name` in backend/src/prompts.rs too.
 */
export const CONTENT_LANGUAGES: ContentLanguage[] = [
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
];

/** Native label for a content code, falling back to the bare code. */
export function contentLanguageLabel(code: string): string {
  return CONTENT_LANGUAGES.find((entry) => entry.code === code)?.label ?? code.toUpperCase();
}
