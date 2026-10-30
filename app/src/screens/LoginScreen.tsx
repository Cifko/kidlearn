import React, { useState } from 'react';
import { StyleSheet, Text, View } from 'react-native';
import AsyncStorage from '@react-native-async-storage/async-storage';
import { api, ApiError } from '../api';
import { useLang } from '../i18n';
import { colors } from '../theme';
import { ChunkyButton, ErrorBox, Screen, Segmented } from '../ui';

export function LoginScreen({ onLoggedIn }: { onLoggedIn: (token: string) => void }) {
  const { lang, setLang, t } = useLang();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleDevLogin() {
    setLoading(true);
    setError(null);
    try {
      const { token } = await api.devLogin();
      await AsyncStorage.setItem('kidlearn.token', token);
      onLoggedIn(token);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : 'Login failed');
      setLoading(false);
    }
  }

  return (
    <Screen>
      <View style={styles.langRow}>
        <View style={styles.langBox}>
          <Segmented
            options={[
              { value: 'en', label: 'EN' },
              { value: 'sk', label: 'SK' },
            ]}
            value={lang}
            onChange={setLang}
          />
        </View>
      </View>
      <Text style={styles.title}>KidLearn</Text>
      <Text style={styles.tagline}>{t('tagline')}</Text>
      {error && <ErrorBox message={error} />}
      <View style={styles.gap}>
        <ChunkyButton title={t('devLogin')} onPress={handleDevLogin} loading={loading} />
      </View>
      <ChunkyButton title={t('googleSoon')} onPress={() => {}} color="gray" disabled />
    </Screen>
  );
}

const styles = StyleSheet.create({
  langRow: {
    flexDirection: 'row',
    justifyContent: 'flex-end',
    marginBottom: 48,
  },
  langBox: {
    width: 160,
  },
  title: {
    fontSize: 44,
    fontWeight: '900',
    color: colors.green,
    textAlign: 'center',
  },
  tagline: {
    fontSize: 17,
    color: colors.muted,
    textAlign: 'center',
    marginTop: 8,
    marginBottom: 40,
  },
  gap: {
    marginBottom: 12,
  },
});
