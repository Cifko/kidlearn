import React, { useEffect, useState } from 'react';
import { Text } from 'react-native';
import AsyncStorage from '@react-native-async-storage/async-storage';
import { api, User } from './src/api';
import { LangProvider, useLang } from './src/i18n';
import { CreateScreen } from './src/screens/CreateScreen';
import { DetailScreen } from './src/screens/DetailScreen';
import { HomeScreen } from './src/screens/HomeScreen';
import { LoginScreen } from './src/screens/LoginScreen';
import { Screen } from './src/ui';

type Route = { name: 'login' } | { name: 'home' } | { name: 'create' } | { name: 'detail'; id: number };

function Shell() {
  const { t } = useLang();
  const [token, setToken] = useState<string | null>(null);
  const [user, setUser] = useState<User | null>(null);
  const [route, setRoute] = useState<Route>({ name: 'login' });
  const [booting, setBooting] = useState(true);

  useEffect(() => {
    AsyncStorage.getItem('kidlearn.token')
      .then(async (stored) => {
        if (!stored) {
          return;
        }
        try {
          const me = await api.me(stored);
          setToken(stored);
          setUser(me);
          setRoute({ name: 'home' });
        } catch {
          await AsyncStorage.removeItem('kidlearn.token');
        }
      })
      .finally(() => setBooting(false));
  }, []);

  async function handleLoggedIn(newToken: string) {
    const me = await api.me(newToken);
    setToken(newToken);
    setUser(me);
    setRoute({ name: 'home' });
  }

  function handleLogout() {
    setToken(null);
    setUser(null);
    setRoute({ name: 'login' });
  }

  if (booting) {
    return (
      <Screen>
        <Text>{t('loading')}</Text>
      </Screen>
    );
  }

  if (!token || !user) {
    return <LoginScreen onLoggedIn={handleLoggedIn} />;
  }

  if (route.name === 'create') {
    return (
      <CreateScreen
        token={token}
        onDone={(id) => setRoute({ name: 'detail', id })}
        onBack={() => setRoute({ name: 'home' })}
      />
    );
  }

  if (route.name === 'detail') {
    return (
      <DetailScreen
        token={token}
        categoryId={route.id}
        onBack={() => setRoute({ name: 'home' })}
        onDeleted={() => setRoute({ name: 'home' })}
      />
    );
  }

  return (
    <HomeScreen
      token={token}
      user={user}
      onOpen={(id) => setRoute({ name: 'detail', id })}
      onCreate={() => setRoute({ name: 'create' })}
      onLogout={handleLogout}
    />
  );
}

export default function App() {
  return (
    <LangProvider>
      <Shell />
    </LangProvider>
  );
}
