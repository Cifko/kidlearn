import React, { useState } from 'react';
import { StyleSheet, Text, TextInput, TouchableOpacity, View } from 'react-native';
import { api, ApiError } from '../api';
import { Lang, useLang } from '../i18n';
import { colors } from '../theme';
import { ChunkyButton, ErrorBox, Field, Screen, Segmented, Stepper } from '../ui';

type Phase = 'editing' | 'creating' | 'generating';

export function CreateScreen({
  token,
  onDone,
  onBack,
}: {
  token: string;
  onDone: (id: number) => void;
  onBack: () => void;
}) {
  const { lang, t } = useLang();
  const [title, setTitle] = useState('');
  const [description, setDescription] = useState('');
  const [tagsText, setTagsText] = useState('');
  const [contentLang, setContentLang] = useState<Lang>(lang);
  const [grade, setGrade] = useState<'any' | '1' | '2' | '3' | '4' | '5'>('any');
  const [modality, setModality] = useState<'auto' | 'text_only'>('auto');
  const [count, setCount] = useState(20);
  const [phase, setPhase] = useState<Phase>('editing');
  const [error, setError] = useState<string | null>(null);

  const busy = phase !== 'editing';

  async function handleSubmit() {
    if (title.trim().length === 0) {
      return;
    }
    setPhase('creating');
    setError(null);
    try {
      const tags = tagsText
        .split(',')
        .map((tag) => tag.trim())
        .filter((tag) => tag.length > 0);
      const created = await api.createSubject(token, {
        title: title.trim(),
        description: description.trim(),
        language: contentLang,
        lesson_count: count,
        tags,
        grade: grade === 'any' ? null : Number(grade),
        modality,
      });
      setPhase('generating');
      // The UI language travels with the request so lessons come back
      // in the language the child actually reads.
      await api.generateSubject(token, created.id, count);
      onDone(created.id);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : 'Creation failed');
      setPhase('editing');
    }
  }

  return (
    <Screen>
      <TouchableOpacity onPress={onBack} disabled={busy}>
        <Text style={styles.back}>← {t('back')}</Text>
      </TouchableOpacity>
      <Text style={styles.heading}>{t('newSubject')}</Text>
      {error && <ErrorBox message={error} />}
      <Field label={t('title')}>
        <TextInput
          style={styles.input}
          value={title}
          onChangeText={setTitle}
          placeholder={t('titlePlaceholder')}
          editable={!busy}
          maxLength={120}
        />
      </Field>
      <Field label={t('description')}>
        <TextInput
          style={[styles.input, styles.multiline]}
          value={description}
          onChangeText={setDescription}
          placeholder={t('descriptionPlaceholder')}
          editable={!busy}
          multiline
          numberOfLines={3}
        />
      </Field>
      <Field label={t('tags')}>
        <TextInput
          style={styles.input}
          value={tagsText}
          onChangeText={setTagsText}
          editable={!busy}
          maxLength={200}
        />
      </Field>
      <Field label={t('language')}>
        <Segmented<Lang>
          options={[
            { value: 'en', label: 'English' },
            { value: 'sk', label: 'Slovenčina' },
          ]}
          value={contentLang}
          onChange={setContentLang}
        />
      </Field>
      <Field label={t('grade')}>
        <Segmented<'any' | '1' | '2' | '3' | '4' | '5'>
          options={[
            { value: 'any', label: t('gradeAny') },
            { value: '1', label: '1' },
            { value: '2', label: '2' },
            { value: '3', label: '3' },
            { value: '4', label: '4' },
            { value: '5', label: '5+' },
          ]}
          value={grade}
          onChange={setGrade}
        />
      </Field>
      <Field label={t('pictures')}>
        <Segmented<'auto' | 'text_only'>
          options={[
            { value: 'auto', label: t('picturesAuto') },
            { value: 'text_only', label: t('picturesTextOnly') },
          ]}
          value={modality}
          onChange={setModality}
        />
      </Field>
      <Field label={t('lessons')}>
        <Stepper value={count} min={1} max={50} onChange={setCount} />
      </Field>
      <View style={styles.submit}>
        <ChunkyButton
          title={t('createGenerate')}
          onPress={handleSubmit}
          loading={busy}
          disabled={title.trim().length === 0}
        />
      </View>
    </Screen>
  );
}

const styles = StyleSheet.create({
  back: {
    fontSize: 15,
    fontWeight: '700',
    color: colors.blue,
    marginBottom: 12,
  },
  heading: {
    fontSize: 28,
    fontWeight: '900',
    color: colors.ink,
    marginBottom: 20,
  },
  input: {
    backgroundColor: '#FFFFFF',
    borderRadius: 12,
    borderWidth: 2,
    borderColor: colors.border,
    paddingHorizontal: 14,
    paddingVertical: 12,
    fontSize: 16,
    color: colors.ink,
  },
  multiline: {
    minHeight: 88,
    textAlignVertical: 'top',
  },
  submit: {
    marginTop: 8,
  },
});
