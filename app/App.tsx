import React, { useEffect, useState } from 'react';
import { Text } from 'react-native';
import AsyncStorage from '@react-native-async-storage/async-storage';
import { api, SubjectSummary, User } from './src/api';
import { LangProvider, useLang } from './src/i18n';
import { CreateScreen } from './src/screens/CreateScreen';
import { DetailScreen } from './src/screens/DetailScreen';
import { HomeScreen } from './src/screens/HomeScreen';
import { LearnScreen } from './src/screens/LearnScreen';
import { LoginScreen } from './src/screens/LoginScreen';
import { PromptsScreen } from './src/screens/PromptsScreen';
import { Screen } from './src/ui';

type Route =
  | { name: 'login' }
  | { name: 'home' }
  | { name: 'create'; basedOn?: SubjectSummary }
  | { name: 'detail'; id: number; focusLessonId?: number }
  | { name: 'learn'; subjectId: number; lessonId: number }
  | { name: 'prompts' };

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
    // Extending a subject returns to it on Back; plain creation to home.
    const backRoute: Route = route.basedOn
      ? { name: 'detail', id: route.basedOn.id }
      : { name: 'home' };
    return (
      <CreateScreen
        token={token}
        basedOn={route.basedOn}
        onDone={(id) => setRoute({ name: 'detail', id })}
        onBack={() => setRoute(backRoute)}
      />
    );
  }

  if (route.name === 'learn') {
    return (
      <LearnScreen
        token={token}
        lessonId={route.lessonId}
        onExit={(lessonId) => setRoute({ name: 'detail', id: route.subjectId, focusLessonId: lessonId })}
      />
    );
  }

  if (route.name === 'detail') {
    return (
      <DetailScreen
        key={route.id}
        token={token}
        subjectId={route.id}
        focusLessonId={route.focusLessonId}
        onBack={() => setRoute({ name: 'home' })}
        onDeleted={() => setRoute({ name: 'home' })}
        onLearn={(lessonId) => setRoute({ name: 'learn', subjectId: route.id, lessonId })}
        onOpenSubject={(id) => setRoute({ name: 'detail', id })}
        onExtend={(subject) => setRoute({ name: 'create', basedOn: subject })}
      />
    );
  }

  if (route.name === 'prompts') {
    return <PromptsScreen token={token} onBack={() => setRoute({ name: 'home' })} />;
  }

  return (
    <HomeScreen
      token={token}
      user={user}
      onOpen={(id) => setRoute({ name: 'detail', id })}
      onCreate={() => setRoute({ name: 'create' })}
      onLogout={handleLogout}
      onPrompts={() => setRoute({ name: 'prompts' })}
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
