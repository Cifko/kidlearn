import React, { useCallback, useEffect, useState } from 'react';
import { StyleSheet, Text, TextInput, TouchableOpacity, View } from 'react-native';
import AsyncStorage from '@react-native-async-storage/async-storage';
import { api, ApiError, CategorySummary, User } from '../api';
import { useLang } from '../i18n';
import { colors } from '../theme';
import { ChunkyButton, ErrorBox, Screen, Segmented } from '../ui';

export function HomeScreen({
  token,
  user,
  onOpen,
  onCreate,
  onLogout,
}: {
  token: string;
  user: User;
  onOpen: (id: number) => void;
  onCreate: () => void;
  onLogout: () => void;
}) {
  const { lang, setLang, t } = useLang();
  const [categories, setCategories] = useState<CategorySummary[]>([]);
  const [query, setQuery] = useState('');
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(
    async (q: string) => {
      setLoading(true);
      setError(null);
      try {
        setCategories(await api.listCategories(token, q.trim() || undefined));
      } catch (err) {
        setError(err instanceof ApiError ? err.message : 'Failed to load');
      } finally {
        setLoading(false);
      }
    },
    [token],
  );

  useEffect(() => {
    load('');
  }, [load]);

  async function handleLogout() {
    await AsyncStorage.removeItem('kidlearn.token');
    onLogout();
  }

  return (
    <Screen>
      <View style={styles.header}>
        <View>
          <Text style={styles.heading}>{t('categories')}</Text>
          <Text style={styles.user}>{user.name}</Text>
        </View>
        <TouchableOpacity onPress={handleLogout}>
          <Text style={styles.logout}>{t('logout')}</Text>
        </TouchableOpacity>
      </View>
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
      <TextInput
        style={styles.search}
        value={query}
        onChangeText={setQuery}
        onSubmitEditing={() => load(query)}
        placeholder={t('search')}
        returnKeyType="search"
      />
      <View style={styles.newButton}>
        <ChunkyButton title={t('newCategory')} onPress={onCreate} color="blue" />
      </View>
      {error && <ErrorBox message={error} onRetry={() => load(query)} />}
      {loading && <Text style={styles.muted}>{t('loading')}</Text>}
      {!loading && !error && categories.length === 0 && (
        <Text style={styles.muted}>{t('empty')}</Text>
      )}
      {categories.map((category) => (
        <TouchableOpacity key={category.id} style={styles.card} onPress={() => onOpen(category.id)}>
          <View style={styles.cardHeader}>
            <Text style={styles.cardTitle}>{category.title}</Text>
            <Text style={[styles.badge, category.mine ? styles.badgeMine : styles.badgePublic]}>
              {category.mine ? t('mine') : category.owner_name}
            </Text>
          </View>
          <Text style={styles.cardMeta}>
            {category.language.toUpperCase()} ·{' '}
            {category.visibility === 'public' ? t('public') : t('private')} ·{' '}
            {category.subcategory_count}
          </Text>
          {category.status === 'generating' && (
            <Text style={styles.statusGenerating}>{t('generating')}</Text>
          )}
          {category.status === 'failed' && <Text style={styles.statusFailed}>{t('failed')}</Text>}
        </TouchableOpacity>
      ))}
    </Screen>
  );
}

const styles = StyleSheet.create({
  header: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'flex-start',
    marginBottom: 16,
  },
  heading: {
    fontSize: 28,
    fontWeight: '900',
    color: colors.ink,
  },
  user: {
    fontSize: 15,
    color: colors.muted,
    marginTop: 2,
  },
  logout: {
    fontSize: 15,
    fontWeight: '700',
    color: colors.blue,
    paddingTop: 6,
  },
  langBox: {
    marginBottom: 16,
  },
  search: {
    backgroundColor: '#FFFFFF',
    borderRadius: 12,
    borderWidth: 2,
    borderColor: colors.border,
    paddingHorizontal: 14,
    paddingVertical: 12,
    fontSize: 16,
    color: colors.ink,
    marginBottom: 12,
  },
  newButton: {
    marginBottom: 20,
  },
  muted: {
    fontSize: 15,
    color: colors.muted,
    textAlign: 'center',
    marginTop: 24,
  },
  card: {
    backgroundColor: colors.card,
    borderRadius: 16,
    borderWidth: 2,
    borderColor: colors.border,
    padding: 16,
    marginBottom: 12,
  },
  cardHeader: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    gap: 8,
  },
  cardTitle: {
    flex: 1,
    fontSize: 19,
    fontWeight: '800',
    color: colors.ink,
  },
  badge: {
    fontSize: 13,
    fontWeight: '800',
    paddingHorizontal: 10,
    paddingVertical: 4,
    borderRadius: 999,
    overflow: 'hidden',
  },
  badgeMine: {
    backgroundColor: '#D7FFB8',
    color: colors.greenDark,
  },
  badgePublic: {
    backgroundColor: '#DDF4FF',
    color: colors.blueDark,
  },
  cardMeta: {
    fontSize: 14,
    color: colors.muted,
    marginTop: 6,
    fontWeight: '600',
  },
  statusGenerating: {
    fontSize: 14,
    fontWeight: '700',
    color: colors.orange,
    marginTop: 6,
  },
  statusFailed: {
    fontSize: 14,
    fontWeight: '700',
    color: colors.red,
    marginTop: 6,
  },
});
