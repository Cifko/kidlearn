import React, { useCallback, useEffect, useState } from 'react';
import { Alert, StyleSheet, Text, TouchableOpacity, View } from 'react-native';
import { api, ApiError, CategoryDetail, SubProgress } from '../api';
import { useLang } from '../i18n';
import { colors } from '../theme';
import { ChunkyButton, ErrorBox, Screen } from '../ui';

export function DetailScreen({
  token,
  categoryId,
  onBack,
  onDeleted,
  onLearn,
}: {
  token: string;
  categoryId: number;
  onBack: () => void;
  onDeleted: () => void;
  onLearn: (subId: number) => void;
}) {
  const { t } = useLang();
  const [detail, setDetail] = useState<CategoryDetail | null>(null);
  const [progress, setProgress] = useState<Record<number, SubProgress>>({});
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [data, prog] = await Promise.all([
        api.getCategory(token, categoryId),
        api.categoryProgress(token, categoryId),
      ]);
      setDetail(data);
      setProgress(Object.fromEntries(prog.subcategories.map((s) => [s.subcategory_id, s])));
    } catch (err) {
      setError(err instanceof ApiError ? err.message : 'Failed to load');
    } finally {
      setLoading(false);
    }
  }, [token, categoryId]);

  useEffect(() => {
    load();
  }, [load]);

  async function handleRegenerate() {
    if (!detail) {
      return;
    }
    setBusy(true);
    setError(null);
    try {
      setDetail(await api.generateCategory(token, categoryId, detail.subcategory_count));
      const prog = await api.categoryProgress(token, categoryId);
      setProgress(Object.fromEntries(prog.subcategories.map((s) => [s.subcategory_id, s])));
    } catch (err) {
      setError(err instanceof ApiError ? err.message : 'Generation failed');
    } finally {
      setBusy(false);
    }
  }

  function handleDelete() {
    Alert.alert(t('delete'), t('deleteConfirm'), [
      { text: t('cancel'), style: 'cancel' },
      {
        text: t('delete'),
        style: 'destructive',
        onPress: async () => {
          setBusy(true);
          try {
            await api.deleteCategory(token, categoryId);
            onDeleted();
          } catch (err) {
            setError(err instanceof ApiError ? err.message : 'Delete failed');
            setBusy(false);
          }
        },
      },
    ]);
  }

  return (
    <Screen>
      <TouchableOpacity onPress={onBack}>
        <Text style={styles.back}>← {t('back')}</Text>
      </TouchableOpacity>
      {loading && <Text style={styles.muted}>{t('loading')}</Text>}
      {error && <ErrorBox message={error} onRetry={load} />}
      {detail && (
        <>
          <Text style={styles.heading}>{detail.title}</Text>
          <Text style={styles.meta}>
            {detail.owner_name} · {detail.language.toUpperCase()} ·{' '}
            {detail.visibility === 'public' ? t('public') : t('private')}
          </Text>
          {detail.description.length > 0 && (
            <Text style={styles.description}>{detail.description}</Text>
          )}
          {detail.status === 'generating' && (
            <Text style={styles.statusGenerating}>{t('generating')}</Text>
          )}
          {detail.status === 'failed' && <Text style={styles.statusFailed}>{t('failed')}</Text>}
          <View style={styles.list}>
            {detail.subcategories.map((sub) => {
              const prog = progress[sub.id];
              const locked = prog ? !prog.unlocked : false;
              const done =
                prog !== undefined && prog.total > 0 && prog.answered >= prog.total;
              return (
                <TouchableOpacity
                  key={sub.id}
                  style={[styles.subRow, locked && styles.subRowLocked]}
                  disabled={locked}
                  onPress={() => onLearn(sub.id)}
                >
                  <Text style={[styles.subPosition, locked && styles.subPositionLocked]}>
                    {sub.position}
                  </Text>
                  <View style={styles.subBody}>
                    <Text style={styles.subTitle}>{sub.title}</Text>
                    {sub.summary.length > 0 && <Text style={styles.subSummary}>{sub.summary}</Text>}
                    <Text style={styles.subCount}>
                      {locked
                        ? t('locked')
                        : prog
                          ? `${prog.answered}/${prog.total} ${t('questions')}${done ? ` · ${t('done')}` : ''}`
                          : `${sub.question_count} ${t('questions')}`}
                    </Text>
                  </View>
                </TouchableOpacity>
              );
            })}
          </View>
          {detail.mine && (
            <View style={styles.ownerRow}>
              <View style={styles.ownerButton}>
                <ChunkyButton
                  title={t('regenerate')}
                  onPress={handleRegenerate}
                  color="blue"
                  loading={busy}
                />
              </View>
              <View style={styles.ownerButton}>
                <ChunkyButton title={t('delete')} onPress={handleDelete} color="red" />
              </View>
            </View>
          )}
        </>
      )}
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
  },
  meta: {
    fontSize: 14,
    fontWeight: '600',
    color: colors.muted,
    marginTop: 6,
  },
  description: {
    fontSize: 16,
    color: colors.ink,
    marginTop: 12,
  },
  muted: {
    fontSize: 15,
    color: colors.muted,
    textAlign: 'center',
    marginTop: 24,
  },
  statusGenerating: {
    fontSize: 15,
    fontWeight: '700',
    color: colors.orange,
    marginTop: 12,
  },
  statusFailed: {
    fontSize: 15,
    fontWeight: '700',
    color: colors.red,
    marginTop: 12,
  },
  list: {
    marginTop: 20,
  },
  subRow: {
    flexDirection: 'row',
    gap: 12,
    backgroundColor: colors.card,
    borderRadius: 16,
    borderWidth: 2,
    borderColor: colors.border,
    padding: 14,
    marginBottom: 10,
  },
  subRowLocked: {
    opacity: 0.55,
  },
  subPosition: {
    width: 32,
    height: 32,
    borderRadius: 16,
    backgroundColor: colors.green,
    color: '#FFFFFF',
    fontSize: 16,
    fontWeight: '900',
    textAlign: 'center',
    lineHeight: 32,
    overflow: 'hidden',
  },
  subPositionLocked: {
    backgroundColor: colors.grayDark,
  },
  subBody: {
    flex: 1,
  },
  subTitle: {
    fontSize: 17,
    fontWeight: '800',
    color: colors.ink,
  },
  subSummary: {
    fontSize: 14,
    color: colors.muted,
    marginTop: 2,
  },
  subCount: {
    fontSize: 13,
    fontWeight: '700',
    color: colors.blueDark,
    marginTop: 4,
  },
  ownerRow: {
    flexDirection: 'row',
    gap: 12,
    marginTop: 16,
  },
  ownerButton: {
    flex: 1,
  },
});
