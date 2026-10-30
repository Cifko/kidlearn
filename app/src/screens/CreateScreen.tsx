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
  const [contentLang, setContentLang] = useState<Lang>(lang);
  const [visibility, setVisibility] = useState<'private' | 'public'>('private');
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
      const created = await api.createCategory(token, {
        title: title.trim(),
        description: description.trim(),
        language: contentLang,
        visibility,
        subcategory_count: count,
      });
      setPhase('generating');
      // The UI language travels with the request so lessons come back
      // in the language the child actually reads.
      await api.generateCategory(token, created.id, count);
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
      <Text style={styles.heading}>{t('newCategory')}</Text>
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
      <Field label={t('visibility')}>
        <Segmented<'private' | 'public'>
          options={[
            { value: 'private', label: t('private') },
            { value: 'public', label: t('public') },
          ]}
          value={visibility}
          onChange={setVisibility}
        />
      </Field>
      <Field label={t('count')}>
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
