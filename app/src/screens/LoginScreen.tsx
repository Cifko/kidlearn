import React, { useEffect, useState } from 'react';
import { Image, StyleSheet, Text, View } from 'react-native';
import AsyncStorage from '@react-native-async-storage/async-storage';
import * as AuthSession from 'expo-auth-session';
import * as WebBrowser from 'expo-web-browser';
import Logo from '../../assets/logo.png';
import { api, ApiError } from '../api';
import { useLang } from '../i18n';
import { colors } from '../theme';
import { LANGUAGES } from '../languages';
import { ChunkyButton, Dropdown, ErrorBox, Screen } from '../ui';

WebBrowser.maybeCompleteAuthSession();

const GOOGLE_CLIENT_ID = process.env.EXPO_PUBLIC_GOOGLE_CLIENT_ID ?? '';
const FB_APP_ID = process.env.EXPO_PUBLIC_FACEBOOK_APP_ID ?? '';
// Feature flag: set EXPO_PUBLIC_SHOW_EXTRA_LOGINS=false to hide the dev
// and Google buttons (e.g. for demos). Facebook always stays. Defaults
// to visible so local development is unchanged.
const SHOW_EXTRA_LOGINS = process.env.EXPO_PUBLIC_SHOW_EXTRA_LOGINS !== 'false';

// Facebook has no OIDC discovery document: static endpoints. The code is
// exchanged server-side (the app secret never leaves the backend), so the
// token endpoint below is only here to satisfy the request shape.
const FB_DISCOVERY = {
  authorizationEndpoint: 'https://www.facebook.com/v22.0/dialog/oauth',
  tokenEndpoint: 'https://graph.facebook.com/v22.0/oauth/access_token',
};

// Facebook binds the code to the dialog's redirect_uri with a trailing
// slash enforced: exchanging the identical URI without one fails with
// "Error validating verification code" (verified by hand against the
// Graph API). Normalize once so the dialog, the derived value, and the
// backend all use the slashed form. Query and fragment stay untouched.
function withTrailingSlash(uri: string): string {
  const cut = uri.search(/[?#]/);
  const head = cut === -1 ? uri : uri.slice(0, cut);
  const tail = cut === -1 ? '' : uri.slice(cut);
  return (head.endsWith('/') ? head : `${head}/`) + tail;
}

// The backend's code exchange must repeat the dialog's redirect_uri
// byte-for-byte. Derive it from the authorize URL expo actually opened,
// so expo-side normalization can't skew it.
// Falls back to the request value when unparseable.
async function dialogRedirectUri(request: AuthSession.AuthRequest): Promise<string> {
  try {
    const authUrl = await request.makeAuthUrlAsync(FB_DISCOVERY);
    const match = authUrl.match(/[?&]redirect_uri=([^&]*)/);
    if (match) {
      return decodeURIComponent(match[1]);
    }
  } catch {
    // fall through to the request value
  }
  return request.redirectUri;
}

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
  // public_profile is granted by default; email is all we must ask for.
  // No PKCE: the code is exchanged by the backend, which authenticates
  // with the app secret instead. Leaving PKCE on makes Facebook bind
  // the code to a challenge the backend never sends, and the exchange
  // fails with "no code_verifier specified".
  const [fbRequest, fbResponse, fbPromptAsync] = AuthSession.useAuthRequest(
    {
      clientId: FB_APP_ID,
      scopes: ['email'],
      responseType: AuthSession.ResponseType.Code,
      redirectUri: withTrailingSlash(AuthSession.makeRedirectUri()),
      usePKCE: false,
    },
    FB_DISCOVERY,
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

  useEffect(() => {
    if (fbResponse?.type !== 'success') {
      return;
    }
    const code = fbResponse.params.code;
    if (typeof code !== 'string' || code.length === 0 || !fbRequest) {
      setError('Facebook login failed');
      return;
    }
    setLoading(true);
    setError(null);
    dialogRedirectUri(fbRequest)
      .then((redirectUri) => api.facebookLogin(code, redirectUri))
      .then(({ token }) =>
        AsyncStorage.setItem('kidlearn.token', token).then(() => onLoggedIn(token)),
      )
      .catch((err: unknown) => {
        setError(err instanceof ApiError ? err.message : 'Facebook login failed');
        setLoading(false);
      });
  }, [fbResponse, fbRequest, onLoggedIn]);

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
  const fbReady = FB_APP_ID.length > 0 && fbRequest !== null;

  return (
    <Screen>
      <View style={styles.langRow}>
        <View style={styles.langBox}>
          <Dropdown
            options={LANGUAGES.map((entry) => ({ value: entry.code, label: entry.label }))}
            value={lang}
            onChange={setLang}
          />
        </View>
      </View>
      <Image source={Logo} style={styles.logo} resizeMode="contain" />
      <Text style={styles.title}>KidLearn</Text>
      <Text style={styles.tagline}>{t('tagline')}</Text>
      {error && <ErrorBox message={error} />}
      {SHOW_EXTRA_LOGINS && (
        <>
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
        </>
      )}
      <View style={styles.gap} />
      <ChunkyButton
        title={FB_APP_ID ? t('facebookLogin') : t('facebookSoon')}
        onPress={() => {
          void fbPromptAsync();
        }}
        color="facebook"
        disabled={!fbReady || loading}
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
    width: 200,
  },
  logo: {
    width: 140,
    height: 158,
    alignSelf: 'center',
    marginBottom: 12,
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
