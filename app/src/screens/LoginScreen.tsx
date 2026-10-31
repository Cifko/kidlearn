import React, { useEffect, useState } from 'react';
import { StyleSheet, Text, View } from 'react-native';
import AsyncStorage from '@react-native-async-storage/async-storage';
import * as AuthSession from 'expo-auth-session';
import * as WebBrowser from 'expo-web-browser';
import { api, ApiError } from '../api';
import { useLang } from '../i18n';
import { colors } from '../theme';
import { ChunkyButton, ErrorBox, Screen, Segmented } from '../ui';

WebBrowser.maybeCompleteAuthSession();

const GOOGLE_CLIENT_ID = process.env.EXPO_PUBLIC_GOOGLE_CLIENT_ID ?? '';

export function LoginScreen({ onLoggedIn }: { onLoggedIn: (token: string) => void }) {
  const { lang, setLang, t } = useLang();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const discovery = AuthSession.useAutoDiscovery('https://accounts.google.com');
  const [request, response, promptAsync] = AuthSession.useAuthRequest(
    {
      clientId: GOOGLE_CLIENT_ID,
      scopes: ['openid', 'profile', 'email'],
      responseType: AuthSession.ResponseType.IdToken,
      redirectUri: AuthSession.makeRedirectUri(),
    },
    discovery,
  );

  useEffect(() => {
    if (response?.type !== 'success') {
      return;
    }
    const idToken = response.params.id_token;
    if (typeof idToken !== 'string' || idToken.length === 0) {
      setError('Google login failed');
      return;
    }
    setLoading(true);
    setError(null);
    api
      .googleLogin(idToken)
      .then(({ token }) =>
        AsyncStorage.setItem('kidlearn.token', token).then(() => onLoggedIn(token)),
      )
      .catch((err: unknown) => {
        setError(err instanceof ApiError ? err.message : 'Google login failed');
        setLoading(false);
      });
  }, [response, onLoggedIn]);

  async function handleDevLogin(who?: string) {
    setLoading(true);
    setError(null);
    try {
      const { token } = await api.devLogin(who);
      await AsyncStorage.setItem('kidlearn.token', token);
      onLoggedIn(token);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : 'Login failed');
      setLoading(false);
    }
  }

  const googleReady = GOOGLE_CLIENT_ID.length > 0 && request !== null;

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
        <ChunkyButton
          title={t('devLogin')}
          onPress={() => handleDevLogin()}
          loading={loading}
        />
      </View>
      <View style={styles.gap}>
        <ChunkyButton
          title={t('devLogin2')}
          onPress={() => handleDevLogin('dev2')}
          color="blue"
          loading={loading}
        />
      </View>
      <ChunkyButton
        title={GOOGLE_CLIENT_ID ? t('googleLogin') : t('googleSoon')}
        onPress={() => {
          void promptAsync();
        }}
        color="blue"
        disabled={!googleReady || loading}
      />
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
