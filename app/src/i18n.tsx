import React, { createContext, ReactNode, useContext, useState } from 'react';

export type Lang = 'en' | 'sk';

const strings = {
  en: {
    tagline: 'Learn anything, one small step at a time.',
    devLogin: 'Log in as Dev',
    googleSoon: 'Google login (coming soon)',
    logout: 'Log out',
    categories: 'Categories',
    newCategory: '+ New category',
    mine: 'Mine',
    public: 'Public',
    private: 'Private',
    generating: 'Generating… this takes a few minutes.',
    failed: 'Generation failed. Try again.',
    title: 'Title',
    titlePlaceholder: 'e.g. Prvouka',
    description: 'What should I learn?',
    descriptionPlaceholder: 'e.g. everything about animals for first grade',
    language: 'Language',
    visibility: 'Visibility',
    count: 'Subcategories',
    createGenerate: 'Create & generate',
    back: 'Back',
    retry: 'Try again',
    regenerate: 'Regenerate',
    delete: 'Delete',
    deleteConfirm: 'Delete this category and all its lessons?',
    cancel: 'Cancel',
    loading: 'Loading…',
    empty: 'No categories yet. Create the first one!',
    questions: 'questions',
    locked: 'Locked',
    done: 'Done',
    next: 'Next',
    finish: 'Finish',
    correct: 'Correct!',
    wrong: 'Not quite',
    lessonComplete: 'Lesson complete!',
    score: 'Score',
    emptyLesson: 'No questions here yet.',
    exit: 'Exit',
    search: 'Search…',
    outOfHearts: 'Out of hearts!',
    tryAgain: 'Try again',
    streak: 'Streak',
  },
  sk: {
    tagline: 'Nauč sa čokoľvek, krok za krokom.',
    devLogin: 'Prihlásiť ako vývojár',
    googleSoon: 'Prihlásenie cez Google (čoskoro)',
    logout: 'Odhlásiť',
    categories: 'Okruhy',
    newCategory: '+ Nový okruh',
    mine: 'Môj',
    public: 'Verejné',
    private: 'Súkromné',
    generating: 'Generujem… trvá to pár minút.',
    failed: 'Generovanie zlyhalo. Skús znova.',
    title: 'Názov',
    titlePlaceholder: 'napr. Prvouka',
    description: 'Čo sa chcem naučiť?',
    descriptionPlaceholder: 'napr. všetko o zvieratách pre prvákov',
    language: 'Jazyk',
    visibility: 'Viditeľnosť',
    count: 'Podokruhy',
    createGenerate: 'Vytvoriť a generovať',
    back: 'Späť',
    retry: 'Skúsiť znova',
    regenerate: 'Pregenerovať',
    delete: 'Zmazať',
    deleteConfirm: 'Zmazať tento okruh aj so všetkými lekciami?',
    cancel: 'Zrušiť',
    loading: 'Načítavam…',
    empty: 'Zatiaľ tu nič nie je. Vytvor prvý okruh!',
    questions: 'otázok',
    locked: 'Zamknuté',
    done: 'Hotovo',
    next: 'Ďalej',
    finish: 'Dokončiť',
    correct: 'Správne!',
    wrong: 'Skoro!',
    lessonComplete: 'Lekcia hotová!',
    score: 'Skóre',
    emptyLesson: 'Zatiaľ tu nie sú otázky.',
    exit: 'Skončiť',
    search: 'Hľadať…',
    outOfHearts: 'Došli srdcia!',
    tryAgain: 'Skúsiť znova',
    streak: 'Séria',
  },
} as const;

export type StringKey = keyof (typeof strings)['en'];

interface LangContextValue {
  lang: Lang;
  setLang: (lang: Lang) => void;
  t: (key: StringKey) => string;
}

const LangContext = createContext<LangContextValue>({
  lang: 'en',
  setLang: () => {},
  t: (key) => key,
});

export function LangProvider({ children }: { children: ReactNode }) {
  const [lang, setLang] = useState<Lang>('en');
  const t = (key: StringKey): string => strings[lang][key];
  return <LangContext.Provider value={{ lang, setLang, t }}>{children}</LangContext.Provider>;
}

export function useLang(): LangContextValue {
  return useContext(LangContext);
}
