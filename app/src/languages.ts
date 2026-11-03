/**
 * App languages: one selection drives the UI, quiz generation, subject
 * filtering and narration. Labels are native names. When adding a code
 * here, add its English name to `language_name` in backend/src/prompts.rs
 * and a UI string table in src/i18n.tsx too.
 */
export const LANGUAGES = [
  { code: 'en', label: 'English', tts: 'en-US' },
  { code: 'sk', label: 'Slovenčina', tts: 'sk-SK' },
  { code: 'ar', label: 'العربية', tts: 'ar-SA' },
  { code: 'bg', label: 'Български', tts: 'bg-BG' },
  { code: 'cs', label: 'Čeština', tts: 'cs-CZ' },
  { code: 'da', label: 'Dansk', tts: 'da-DK' },
  { code: 'de', label: 'Deutsch', tts: 'de-DE' },
  { code: 'el', label: 'Ελληνικά', tts: 'el-GR' },
  { code: 'es', label: 'Español', tts: 'es-ES' },
  { code: 'fi', label: 'Suomi', tts: 'fi-FI' },
  { code: 'fr', label: 'Français', tts: 'fr-FR' },
  { code: 'hi', label: 'हिन्दी', tts: 'hi-IN' },
  { code: 'hr', label: 'Hrvatski', tts: 'hr-HR' },
  { code: 'hu', label: 'Magyar', tts: 'hu-HU' },
  { code: 'it', label: 'Italiano', tts: 'it-IT' },
  { code: 'ja', label: '日本語', tts: 'ja-JP' },
  { code: 'nl', label: 'Nederlands', tts: 'nl-NL' },
  { code: 'no', label: 'Norsk', tts: 'nb-NO' },
  { code: 'pl', label: 'Polski', tts: 'pl-PL' },
  { code: 'pt', label: 'Português', tts: 'pt-PT' },
  { code: 'ro', label: 'Română', tts: 'ro-RO' },
  { code: 'ru', label: 'Русский', tts: 'ru-RU' },
  { code: 'sl', label: 'Slovenščina', tts: 'sl-SI' },
  { code: 'sr', label: 'Српски', tts: 'sr-RS' },
  { code: 'sv', label: 'Svenska', tts: 'sv-SE' },
  { code: 'tr', label: 'Türkçe', tts: 'tr-TR' },
  { code: 'uk', label: 'Українська', tts: 'uk-UA' },
  { code: 'zh', label: '中文', tts: 'zh-CN' },
] as const;

/** Native label for a language code, falling back to the bare code. */
export function languageLabel(code: string): string {
  return LANGUAGES.find((entry) => entry.code === code)?.label ?? code.toUpperCase();
}
