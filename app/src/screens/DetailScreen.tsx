import React, { useCallback, useEffect, useState } from 'react';
import { StyleSheet, Text, TouchableOpacity, View } from 'react-native';
import { api, ApiError, SubjectDetail, LessonProgress } from '../api';
import { useLang } from '../i18n';
import { colors } from '../theme';
import { ChunkyButton, ErrorBox, Screen } from '../ui';

export function DetailScreen({
  token,
  subjectId,
  onBack,
  onDeleted,
  onLearn,
}: {
  token: string;
  subjectId: number;
  onBack: () => void;
  onDeleted: () => void;
  onLearn: (lessonId: number) => void;
}) {
  const { t } = useLang();
  const [detail, setDetail] = useState<SubjectDetail | null>(null);
  const [progress, setProgress] = useState<Record<number, LessonProgress>>({});
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [likeBusy, setLikeBusy] = useState(false);
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const [confirmingPublish, setConfirmingPublish] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(
    async (quiet = false) => {
      if (!quiet) {
        setLoading(true);
      }
      setError(null);
      try {
        const [data, prog] = await Promise.all([
          api.getSubject(token, subjectId),
          api.subjectProgress(token, subjectId),
        ]);
        setDetail(data);
        setProgress(Object.fromEntries(prog.lessons.map((s) => [s.lesson_id, s])));
      } catch (err) {
        setError(err instanceof ApiError ? err.message : 'Failed to load');
      } finally {
        if (!quiet) {
          setLoading(false);
        }
      }
    },
    [token, subjectId],
  );

  useEffect(() => {
    load(false);
  }, [load]);

  useEffect(() => {
    if (detail?.status !== 'generating') {
      return;
    }
    const timer = setInterval(() => load(true), 3000);
    return () => clearInterval(timer);
  }, [detail?.status, load]);

  async function handleRegenerate() {
    if (!detail) {
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await api.generateSubject(token, subjectId, detail.lesson_count);
      await load(false);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : 'Generation failed');
    } finally {
      setBusy(false);
    }
  }

  async function handleToggleLike() {
    if (!detail || likeBusy) {
      return;
    }
    setLikeBusy(true);
    setError(null);
    try {
      const updated = detail.liked_by_me
        ? await api.unlikeSubject(token, subjectId)
        : await api.likeSubject(token, subjectId);
      setDetail({ ...detail, like_count: updated.like_count, liked_by_me: updated.liked_by_me });
    } catch (err) {
      setError(err instanceof ApiError ? err.message : 'Like failed');
    } finally {
      setLikeBusy(false);
    }
  }

  async function handlePublishConfirmed() {
    if (!detail) {
      return;
    }
    setBusy(true);
    try {
      const updated = await api.publishSubject(token, subjectId);
      setDetail({ ...detail, visibility: updated.visibility });
      setConfirmingPublish(false);
    } catch (err) {
      setError(err instanceof ApiError ? err.message : 'Publish failed');
      setConfirmingPublish(false);
    } finally {
      setBusy(false);
    }
  }

  // In-app confirm: Alert.alert is a no-op on web, so it can never be
  // relied on for destructive actions.
  async function handleDeleteConfirmed() {
    setBusy(true);
    try {
      await api.deleteSubject(token, subjectId);
      onDeleted();
    } catch (err) {
      setError(err instanceof ApiError ? err.message : 'Delete failed');
      setBusy(false);
      setConfirmingDelete(false);
    }
  }

  return (
    <Screen>
      <TouchableOpacity onPress={onBack}>
        <Text style={styles.back}>← {t('back')}</Text>
      </TouchableOpacity>
      {loading && <Text style={styles.muted}>{t('loading')}</Text>}
      {error && <ErrorBox message={error} onRetry={() => load(false)} />}
      {detail && (
        <>
          <Text style={styles.heading}>{detail.title}</Text>
          <Text style={styles.meta}>
            {detail.owner_name} · {detail.language.toUpperCase()} ·{' '}
            {detail.visibility === 'public' ? t('public') : t('private')}
            {detail.grade != null && ` · ${t('grade')} ${detail.grade}`}
            {detail.modality === 'text_only' && ` · ${t('picturesTextOnly')}`}
          </Text>
          {detail.description.length > 0 && (
            <Text style={styles.description}>{detail.description}</Text>
          )}
          {detail.tags.length > 0 && (
            <View style={styles.tags}>
              {detail.tags.map((tag) => (
                <Text key={tag} style={styles.tag}>
                  #{tag}
                </Text>
              ))}
            </View>
          )}
          {detail.status === 'generating' && (
            <Text style={styles.statusGenerating}>
              {t('generatingShort')} ({detail.generated_lessons}/{detail.lesson_count})
            </Text>
          )}
          {detail.status === 'failed' && <Text style={styles.statusFailed}>{t('failed')}</Text>}
          {!detail.mine ? (
            <View style={styles.likeRow}>
              <ChunkyButton
                title={`${detail.liked_by_me ? t('unlike') : t('like')} (${detail.like_count})`}
                onPress={handleToggleLike}
                color={detail.liked_by_me ? 'gray' : 'red'}
                loading={likeBusy}
              />
            </View>
          ) : (
            <Text style={styles.likeCount}>★ {detail.like_count}</Text>
          )}
          <View style={styles.list}>
            {detail.lessons.map((sub) => {
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
          {detail.mine &&
            detail.visibility === 'private' &&
            detail.status === 'ready' &&
            !confirmingDelete &&
            !confirmingPublish && (
              <View style={styles.publishRow}>
                <ChunkyButton title={t('publish')} onPress={() => setConfirmingPublish(true)} />
              </View>
            )}
          {detail.mine && confirmingPublish && (
            <View style={styles.confirmBox}>
              <Text style={styles.confirmText}>{t('publishConfirm')}</Text>
              <View style={styles.ownerRow}>
                <View style={styles.ownerButton}>
                  <ChunkyButton
                    title={t('cancel')}
                    onPress={() => setConfirmingPublish(false)}
                    color="gray"
                  />
                </View>
                <View style={styles.ownerButton}>
                  <ChunkyButton
                    title={t('publish')}
                    onPress={handlePublishConfirmed}
                    loading={busy}
                  />
                </View>
              </View>
            </View>
          )}
          {detail.mine && detail.visibility === 'private' && !confirmingDelete && !confirmingPublish && (
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
                <ChunkyButton
                  title={t('delete')}
                  onPress={() => setConfirmingDelete(true)}
                  color="red"
                />
              </View>
            </View>
          )}
          {detail.mine && confirmingDelete && !confirmingPublish && (
            <View style={styles.confirmBox}>
              <Text style={styles.confirmText}>{t('deleteConfirm')}</Text>
              <View style={styles.ownerRow}>
                <View style={styles.ownerButton}>
                  <ChunkyButton
                    title={t('cancel')}
                    onPress={() => setConfirmingDelete(false)}
                    color="gray"
                  />
                </View>
                <View style={styles.ownerButton}>
                  <ChunkyButton
                    title={t('delete')}
                    onPress={handleDeleteConfirmed}
                    color="red"
                    loading={busy}
                  />
                </View>
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
  tags: {
    flexDirection: 'row',
    flexWrap: 'wrap',
    gap: 8,
    marginTop: 12,
  },
  tag: {
    backgroundColor: '#DDF4FF',
    color: colors.blueDark,
    fontSize: 13,
    fontWeight: '800',
    paddingHorizontal: 10,
    paddingVertical: 4,
    borderRadius: 999,
    overflow: 'hidden',
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
  likeRow: {
    marginTop: 16,
  },
  likeCount: {
    fontSize: 15,
    fontWeight: '800',
    color: colors.muted,
    marginTop: 8,
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
  publishRow: {
    marginTop: 16,
  },
  confirmBox: {
    backgroundColor: '#FFE5E5',
    borderRadius: 16,
    padding: 16,
    marginTop: 16,
  },
  confirmText: {
    fontSize: 15,
    fontWeight: '700',
    color: colors.redDark,
    marginBottom: 12,
  },
  ownerButton: {
    flex: 1,
  },
});
