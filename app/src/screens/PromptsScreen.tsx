import React, { useCallback, useEffect, useState } from 'react';
import { Platform, StyleSheet, Text, TouchableOpacity, View } from 'react-native';
import { api, ApiError, LlmCall } from '../api';
import { useLang } from '../i18n';
import { colors } from '../theme';
import { ErrorBox, Screen, Segmented } from '../ui';

type KindFilter = 'all' | 'plan' | 'questions' | 'svg' | 'search';

const MONO = Platform.OS === 'ios' ? 'Menlo' : 'monospace';

export function PromptsScreen({ token, onBack }: { token: string; onBack: () => void }) {
  const { t } = useLang();
  const [calls, setCalls] = useState<LlmCall[]>([]);
  const [filter, setFilter] = useState<KindFilter>('all');
  const [expanded, setExpanded] = useState<number | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(
    async (kind: KindFilter) => {
      setLoading(true);
      setError(null);
      try {
        setCalls(await api.listLlmCalls(token, kind === 'all' ? undefined : { kind }));
      } catch (err) {
        setError(err instanceof ApiError ? err.message : 'Failed to load');
      } finally {
        setLoading(false);
      }
    },
    [token],
  );

  useEffect(() => {
    load(filter);
  }, [load, filter]);

  return (
    <Screen>
      <TouchableOpacity onPress={onBack}>
        <Text style={styles.back}>← {t('back')}</Text>
      </TouchableOpacity>
      <Text style={styles.heading}>{t('prompts')}</Text>
      <View style={styles.filter}>
        <Segmented<KindFilter>
          options={[
            { value: 'all', label: t('all') },
            { value: 'plan', label: 'plan' },
            { value: 'questions', label: 'questions' },
            { value: 'svg', label: 'svg' },
            { value: 'search', label: 'search' },
          ]}
          value={filter}
          onChange={(value) => {
            setExpanded(null);
            setFilter(value);
          }}
        />
      </View>
      {error && <ErrorBox message={error} onRetry={() => load(filter)} />}
      {loading && <Text style={styles.muted}>{t('loading')}</Text>}
      {!loading &&
        !error &&
        calls.map((call) => {
          const open = expanded === call.id;
          return (
            <TouchableOpacity
              key={call.id}
              style={styles.card}
              onPress={() => setExpanded(open ? null : call.id)}
            >
              <View style={styles.cardHeader}>
                <Text style={styles.kind}>{call.kind}</Text>
                <Text style={styles.meta}>
                  {call.created_at.slice(0, 16)} · {call.duration_ms} ms
                </Text>
              </View>
              <Text style={styles.label} numberOfLines={open ? undefined : 1}>
                {call.label || `#${call.id}`}
              </Text>
              {call.error && <Text style={styles.errorText}>{call.error}</Text>}
              {open && (
                <>
                  <Text style={styles.sectionTitle}>{t('prompt')}</Text>
                  <Text style={styles.mono} selectable>
                    {call.prompt}
                  </Text>
                  <Text style={styles.sectionTitle}>{t('response')}</Text>
                  <Text style={styles.mono} selectable>
                    {call.response ?? '—'}
                  </Text>
                </>
              )}
            </TouchableOpacity>
          );
        })}
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
    marginBottom: 16,
  },
  filter: {
    marginBottom: 16,
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
    padding: 14,
    marginBottom: 10,
  },
  cardHeader: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    gap: 8,
  },
  kind: {
    fontSize: 13,
    fontWeight: '900',
    color: colors.blueDark,
    backgroundColor: '#DDF4FF',
    paddingHorizontal: 10,
    paddingVertical: 4,
    borderRadius: 999,
    overflow: 'hidden',
  },
  meta: {
    fontSize: 12,
    fontWeight: '600',
    color: colors.muted,
  },
  label: {
    fontSize: 16,
    fontWeight: '800',
    color: colors.ink,
    marginTop: 8,
  },
  errorText: {
    fontSize: 13,
    fontWeight: '600',
    color: colors.redDark,
    marginTop: 6,
  },
  sectionTitle: {
    fontSize: 14,
    fontWeight: '800',
    color: colors.muted,
    marginTop: 14,
    marginBottom: 6,
  },
  mono: {
    fontFamily: MONO,
    fontSize: 12,
    color: colors.ink,
    backgroundColor: '#FFFFFF',
    borderRadius: 8,
    borderWidth: 1,
    borderColor: colors.border,
    padding: 10,
  },
});
