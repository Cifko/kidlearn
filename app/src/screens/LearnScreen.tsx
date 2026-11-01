import React, { useCallback, useEffect, useState } from 'react';
import { Image, StyleSheet, Text, TouchableOpacity, View } from 'react-native';
import { api, ApiError, AnswerResult, Lesson } from '../api';
import { useLang } from '../i18n';
import { colors } from '../theme';
import { ChunkyButton, ErrorBox, Screen } from '../ui';

const HEARTS_PER_RUN = 3;

export function LearnScreen({
  token,
  lessonId,
  onExit,
}: {
  token: string;
  lessonId: number;
  onExit: () => void;
}) {
  const { t } = useLang();
  const [lesson, setLesson] = useState<Lesson | null>(null);
  const [images, setImages] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [index, setIndex] = useState(0);
  const [picked, setPicked] = useState<string | null>(null);
  const [result, setResult] = useState<AnswerResult | null>(null);
  const [checking, setChecking] = useState(false);
  const [score, setScore] = useState(0);
  const [runXp, setRunXp] = useState(0);
  const [hearts, setHearts] = useState(HEARTS_PER_RUN);
  const [finished, setFinished] = useState(false);
  const [failed, setFailed] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await api.lesson(token, lessonId);
      const urls = [
        ...new Set(
          data.questions.flatMap((q) =>
            q.options.flatMap((o) => (o.image_url ? [o.image_url] : [])),
          ),
        ),
      ];
      const entries = await Promise.all(
        urls.map(async (url) => [url, await api.assetImage(token, url)] as const),
      );
      setLesson(data);
      setImages(Object.fromEntries(entries));
    } catch (err) {
      setError(err instanceof ApiError ? err.message : 'Failed to load');
    } finally {
      setLoading(false);
    }
  }, [token, lessonId]);

  useEffect(() => {
    load();
  }, [load]);

  function resetRun() {
    setIndex(0);
    setPicked(null);
    setResult(null);
    setScore(0);
    setRunXp(0);
    setHearts(HEARTS_PER_RUN);
    setFinished(false);
    setFailed(false);
    setError(null);
  }

  async function pick(value: string) {
    if (picked || checking || !lesson) {
      return;
    }
    const question = lesson.questions[index];
    const isText = question.options[0]?.text != null;
    setPicked(value);
    setChecking(true);
    try {
      const answer = await api.answer(
        token,
        lessonId,
        question.id,
        isText ? { text: value } : { asset_key: value },
      );
      setResult(answer);
      if (answer.correct) {
        setScore((s) => s + 1);
        setRunXp((xp) => xp + answer.xp_earned);
      } else {
        setHearts((h) => h - 1);
      }
    } catch (err) {
      setError(err instanceof ApiError ? err.message : 'Answer failed');
      setPicked(null);
    } finally {
      setChecking(false);
    }
  }

  function next() {
    if (!lesson) {
      return;
    }
    if (result && !result.correct && hearts <= 0) {
      setFailed(true);
      return;
    }
    if (index + 1 >= lesson.questions.length) {
      setFinished(true);
    } else {
      setIndex((i) => i + 1);
      setPicked(null);
      setResult(null);
    }
  }

  const total = lesson?.questions.length ?? 0;
  const progress = total === 0 ? 0 : (finished || failed ? total : index) / total;
  const over = finished || failed;
  const current = lesson?.questions[index];
  // The server guarantees one kind per question; the first option tells which.
  const isTextQuestion = current != null && current.options[0]?.text != null;

  return (
    <Screen>
      <View style={styles.topRow}>
        <TouchableOpacity onPress={onExit}>
          <Text style={styles.exit}>✕</Text>
        </TouchableOpacity>
        <View style={styles.track}>
          <View style={[styles.fill, { width: `${Math.round(progress * 100)}%` }]} />
        </View>
        <Text style={styles.hearts}>{'♥'.repeat(Math.max(0, hearts))}</Text>
      </View>

      {loading && <Text style={styles.muted}>{t('loading')}</Text>}
      {error && <ErrorBox message={error} onRetry={load} />}

      {!loading && !error && lesson && total === 0 && (
        <>
          <Text style={styles.muted}>{t('emptyLesson')}</Text>
          <View style={styles.exitButton}>
            <ChunkyButton title={t('exit')} onPress={onExit} color="gray" />
          </View>
        </>
      )}

      {!loading && !error && lesson && total > 0 && over && (
        <>
          <Text style={styles.title}>{failed ? t('outOfHearts') : t('lessonComplete')}</Text>
          <Text style={styles.score}>
            {t('score')}: {score} / {total}
          </Text>
          <Text style={styles.xp}>+{runXp} XP</Text>
          <View style={styles.exitButton}>
            {failed ? (
              <ChunkyButton title={t('tryAgain')} onPress={resetRun} color="blue" />
            ) : (
              <ChunkyButton title={t('exit')} onPress={onExit} />
            )}
          </View>
          {failed && (
            <View style={styles.exitButton}>
              <ChunkyButton title={t('exit')} onPress={onExit} color="gray" />
            </View>
          )}
        </>
      )}

      {!loading && !error && lesson && total > 0 && !over && (
        <>
          <Text style={styles.subTitle}>{lesson.title}</Text>
          <Text style={styles.question}>{lesson.questions[index].text}</Text>
          <View style={styles.options}>
            {lesson.questions[index].options.map((option, optionIndex) => {
              const value = option.text ?? option.asset_key ?? '';
              const correctValue = isTextQuestion
                ? result?.correct_text
                : result?.correct_asset_key;
              const isPicked = picked === value;
              const isCorrect = result != null && correctValue === value;
              return (
                <TouchableOpacity
                  key={option.text ?? option.asset_key ?? optionIndex}
                  onPress={() => pick(value)}
                  disabled={picked !== null}
                  style={[
                    styles.option,
                    isPicked && result?.correct && styles.optionCorrect,
                    isPicked && result && !result.correct && styles.optionWrong,
                    result && !isPicked && isCorrect && styles.optionCorrect,
                  ]}
                >
                  {option.text != null ? (
                    <Text
                      style={styles.optionText}
                      numberOfLines={1}
                      adjustsFontSizeToFit
                      minimumFontScale={0.4}
                    >
                      {option.text}
                    </Text>
                  ) : option.image_url != null && images[option.image_url] ? (
                    <Image
                      source={{ uri: images[option.image_url] }}
                      style={styles.optionImage}
                    />
                  ) : (
                    <View style={styles.optionImage} />
                  )}
                </TouchableOpacity>
              );
            })}
          </View>
          {result && (
            <View style={[styles.feedback, result.correct ? styles.feedbackGood : styles.feedbackBad]}>
              <Text
                style={[
                  styles.feedbackText,
                  result.correct ? styles.feedbackTextGood : styles.feedbackTextBad,
                ]}
              >
                {result.correct
                  ? `${t('correct')}${result.xp_earned > 0 ? ` +${result.xp_earned} XP` : ''}`
                  : t('wrong')}
              </Text>
              <View style={styles.nextButton}>
                <ChunkyButton
                  title={index + 1 >= total ? t('finish') : t('next')}
                  onPress={next}
                  color={result.correct ? 'green' : 'red'}
                />
              </View>
            </View>
          )}
        </>
      )}
    </Screen>
  );
}

const styles = StyleSheet.create({
  topRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 12,
    marginBottom: 20,
  },
  exit: {
    fontSize: 20,
    fontWeight: '800',
    color: colors.muted,
    padding: 4,
  },
  track: {
    flex: 1,
    height: 14,
    borderRadius: 7,
    backgroundColor: colors.card,
    overflow: 'hidden',
  },
  fill: {
    height: 14,
    borderRadius: 7,
    backgroundColor: colors.green,
  },
  hearts: {
    fontSize: 18,
    fontWeight: '900',
    color: colors.red,
    minWidth: 54,
  },
  muted: {
    fontSize: 15,
    color: colors.muted,
    textAlign: 'center',
    marginTop: 24,
  },
  title: {
    fontSize: 30,
    fontWeight: '900',
    color: colors.ink,
    textAlign: 'center',
    marginTop: 32,
  },
  score: {
    fontSize: 22,
    fontWeight: '800',
    color: colors.greenDark,
    textAlign: 'center',
    marginTop: 12,
  },
  xp: {
    fontSize: 18,
    fontWeight: '800',
    color: colors.orange,
    textAlign: 'center',
    marginTop: 4,
  },
  exitButton: {
    marginTop: 16,
  },
  subTitle: {
    fontSize: 14,
    fontWeight: '800',
    color: colors.muted,
    letterSpacing: 0.5,
  },
  question: {
    fontSize: 24,
    fontWeight: '800',
    color: colors.ink,
    marginTop: 6,
    marginBottom: 20,
  },
  options: {
    flexDirection: 'row',
    flexWrap: 'wrap',
    gap: 12,
    justifyContent: 'center',
  },
  option: {
    width: 150,
    height: 150,
    borderRadius: 16,
    backgroundColor: '#FFFFFF',
    borderWidth: 3,
    borderColor: colors.border,
    alignItems: 'center',
    justifyContent: 'center',
  },
  optionCorrect: {
    borderColor: colors.green,
    backgroundColor: '#EAFBDD',
  },
  optionWrong: {
    borderColor: colors.red,
    backgroundColor: '#FFE5E5',
  },
  optionImage: {
    width: 110,
    height: 110,
    borderRadius: 8,
    backgroundColor: colors.card,
  },
  optionText: {
    width: '100%',
    fontSize: 24,
    fontWeight: '900',
    color: colors.ink,
    textAlign: 'center',
    paddingHorizontal: 8,
  },
  feedback: {
    borderRadius: 16,
    padding: 16,
    marginTop: 24,
  },
  feedbackGood: {
    backgroundColor: '#EAFBDD',
  },
  feedbackBad: {
    backgroundColor: '#FFE5E5',
  },
  feedbackText: {
    fontSize: 20,
    fontWeight: '900',
    marginBottom: 12,
  },
  feedbackTextGood: {
    color: colors.greenDark,
  },
  feedbackTextBad: {
    color: colors.redDark,
  },
  nextButton: {
    marginTop: 4,
  },
});
