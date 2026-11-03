import React, { useCallback, useEffect, useRef, useState } from 'react';
import {
  ScrollViewInstance,
  StyleSheet,
  Text,
  TouchableOpacity,
  View,
} from 'react-native';
import { api, ApiError, SubjectDetail, LessonProgress, SubjectSummary } from '../api';
import { useLang } from '../i18n';
import { languageLabel } from '../languages';
import { colors } from '../theme';
import { ChunkyButton, ErrorBox, Screen } from '../ui';

export function DetailScreen({
  token,
  subjectId,
  focusLessonId,
  onBack,
  onDeleted,
  onLearn,
  onOpenSubject,
  onExtend,
}: {
  token: string;
  subjectId: number;
  focusLessonId?: number;
  onBack: () => void;
  onDeleted: () => void;
  onLearn: (lessonId: number) => void;
  onOpenSubject: (subjectId: number) => void;
  onExtend: (subject: SubjectSummary) => void;
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
  const scrollRef = useRef<ScrollViewInstance>(null);
  const listY = useRef(0);
  const listMeasured = useRef(false);
  const rowLayouts = useRef<Record<number, { y: number; height: number }>>({});
  const viewportHeight = useRef<number | null>(null);
  const scrolledFor = useRef<number | null>(null);
  const scrollTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

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

  // Returning from a lesson lands back where the student was: a finished
  // lesson goes to the top with the next one below it (or the next one on
  // top when it would not fit); an unfinished one simply goes to the top.
  // Fires once per arrival: polls must never yank the scroll position.
  const maybeScrollToFocus = useCallback(() => {
    if (focusLessonId == null || scrolledFor.current === focusLessonId) {
      return;
    }
    if (!detail) {
      return;
    }
    const index = detail.lessons.findIndex((sub) => sub.id === focusLessonId);
    if (index < 0) {
      return;
    }
    // Only scroll on complete measurements: scrolling while content is
    // still growing clamps to a short-lived max and sticks there.
    if (viewportHeight.current == null || !listMeasured.current) {
      return;
    }
    if (Object.keys(rowLayouts.current).length < detail.lessons.length) {
      return;
    }
    const focus = rowLayouts.current[focusLessonId];
    if (!focus) {
      return;
    }
    const focusY = listY.current + focus.y;
    let y = focusY;
    const prog = progress[focusLessonId];
    const done = prog !== undefined && prog.total > 0 && prog.answered >= prog.total;
    const next = detail.lessons[index + 1];
    const viewport = viewportHeight.current;
    if (done && next && viewport != null) {
      const nextLayout = rowLayouts.current[next.id];
      if (nextLayout && listY.current + nextLayout.y + nextLayout.height > focusY + viewport) {
        y = listY.current + nextLayout.y;
      }
    }
    scrolledFor.current = focusLessonId;
    if (scrollTimer.current) {
      clearTimeout(scrollTimer.current);
    }
    // One beat for the layout flush (e.g. buttons below the list) to
    // settle, so the offset is never clamped by a half-grown content.
    scrollTimer.current = setTimeout(() => {
      scrollRef.current?.scrollTo({ y, animated: false });
    }, 50);
  }, [detail, focusLessonId, progress]);

  useEffect(() => {
    maybeScrollToFocus();
  }, [maybeScrollToFocus]);

  useEffect(
    () => () => {
      if (scrollTimer.current) {
        clearTimeout(scrollTimer.current);
      }
    },
    [],
  );

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
    <Screen
      scrollRef={scrollRef}
      onViewportHeight={(height) => {
        viewportHeight.current = height;
        maybeScrollToFocus();
      }}
    >
      <TouchableOpacity onPress={onBack}>
        <Text style={styles.back}>← {t('back')}</Text>
      </TouchableOpacity>
      {loading && <Text style={styles.muted}>{t('loading')}</Text>}
      {error && <ErrorBox message={error} onRetry={() => load(false)} />}
      {detail && (
        <>
          <Text style={styles.heading}>{detail.title}</Text>
          <Text style={styles.meta}>
            {t('createdBy')} {detail.owner_name} · {languageLabel(detail.language)} ·{' '}
            {detail.visibility === 'public' ? t('public') : t('private')}
            {detail.grade != null && ` · ${t('grade')} ${detail.grade === 5 ? '5+' : detail.grade}`}
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
          {detail.prerequisites.length > 0 && (
            <View style={styles.tags}>
              <Text style={styles.basedOn}>{t('basedOn')}:</Text>
              {detail.prerequisites.map((prereq) => (
                <TouchableOpacity key={prereq.id} onPress={() => onOpenSubject(prereq.id)}>
                  <Text style={styles.tag}>
                    {prereq.title} · {prereq.owner_name}
                  </Text>
                </TouchableOpacity>
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
          {detail.visibility === 'public' && (
            <View style={styles.extendRow}>
              <ChunkyButton title={t('extendThis')} onPress={() => onExtend(detail)} />
            </View>
          )}
          <View
            style={styles.list}
            onLayout={(event) => {
              listY.current = event.nativeEvent.layout.y;
              listMeasured.current = true;
              maybeScrollToFocus();
            }}
          >
            {detail.lessons.map((sub) => {
              const prog = progress[sub.id];
              const locked = prog ? !prog.unlocked : false;
              const empty = prog ? prog.total === 0 : sub.question_count === 0;
              const learnable = !locked && !empty;
              const done =
                prog !== undefined && prog.total > 0 && prog.answered >= prog.total;
              // Lessons with no questions yet are still generating (or failed):
              // show that instead of a tappable-but-empty row.
              const emptyLabel =
                detail.status === 'generating'
                  ? t('generatingShort')
                  : detail.status === 'failed'
                    ? t('failedShort')
                    : null;
              return (
                <TouchableOpacity
                  key={sub.id}
                  style={[styles.subRow, !learnable && styles.subRowLocked]}
                  disabled={!learnable}
                  onPress={() => onLearn(sub.id)}
                  onLayout={(event) => {
                    const { y, height } = event.nativeEvent.layout;
                    rowLayouts.current[sub.id] = { y, height };
                    maybeScrollToFocus();
                  }}
                >
                  <Text style={[styles.subPosition, !learnable && styles.subPositionLocked]}>
                    {sub.position}
                  </Text>
                  <View style={styles.subBody}>
                    <Text style={styles.subTitle}>{sub.title}</Text>
                    {sub.summary.length > 0 && <Text style={styles.subSummary}>{sub.summary}</Text>}
                    <Text style={styles.subCount}>
                      {locked
                        ? t('locked')
                        : empty && emptyLabel
                          ? emptyLabel
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
  basedOn: {
    fontSize: 13,
    fontWeight: '800',
    color: colors.muted,
    paddingVertical: 4,
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
  extendRow: {
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
