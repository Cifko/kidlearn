import React, { useCallback, useEffect, useMemo, useState } from 'react';
import {
  Dimensions,
  Image,
  Modal,
  ScrollView,
  StyleSheet,
  Text,
  TouchableOpacity,
  View,
} from 'react-native';
import AsyncStorage from '@react-native-async-storage/async-storage';
import * as Speech from 'expo-speech';
import { api, ApiError, AnswerResult, Lesson } from '../api';
import { Lang, useLang } from '../i18n';
import { MathText, stripMathForMeasure } from '../math';
import { Owl } from '../owl';
import { PlotView } from '../plot';
import {
  matchingNarratorVoices,
  pickNarratorVoice,
  resolveSpeechLanguage,
  stripMathForSpeech,
} from '../tex';
import { OPTION_INNER, optionFontSize } from '../text-fit';
import { colors } from '../theme';
import { ChunkyButton, ErrorBox, Screen } from '../ui';

const HEARTS_PER_RUN = 3;
// Owl width plus the gap beside the question text.
const OWL_SPACE = 68;

export function LearnScreen({
  token,
  lessonId,
  onExit,
}: {
  token: string;
  lessonId: number;
  onExit: (lessonId: number) => void;
}) {
  const { t, lang } = useLang();
  const [lesson, setLesson] = useState<Lesson | null>(null);
  const [narratorEnabled, setNarratorEnabled] = useState(true);
  const [voices, setVoices] = useState<Speech.Voice[]>([]);
  const [voicesLoaded, setVoicesLoaded] = useState(false);
  const [manualVoiceId, setManualVoiceId] = useState<string | null>(null);
  const [voicePickerOpen, setVoicePickerOpen] = useState(false);
  const [images, setImages] = useState<Record<string, string>>({});
  const [backgroundUri, setBackgroundUri] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [fetchingImages, setFetchingImages] = useState(false);
  const [imagesPending, setImagesPending] = useState(0);
  const [imagesTotal, setImagesTotal] = useState(0);
  const [imagesLoaded, setImagesLoaded] = useState(0);
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
    setFetchingImages(false);
    setImagesPending(0);
    setImagesTotal(0);
    setImagesLoaded(0);
    setError(null);
    try {
      const data = await api.lesson(token, lessonId);
      // Decorative: loads alongside, never blocks or fails the lesson.
      if (data.background_url) {
        api.assetImage(token, data.background_url).then(setBackgroundUri, () => {});
      }
      const urls = [
        ...new Set(
          data.questions.flatMap((q) => [
            ...(q.image_url ? [q.image_url] : []),
            ...q.options.flatMap((o) => (o.image_url ? [o.image_url] : [])),
          ]),
        ),
      ];
      setImagesPending(data.images_pending);
      setImagesTotal(urls.length);
      if (urls.length > 0) {
        setFetchingImages(true);
      }
      const uris = await Promise.all(
        urls.map(async (url) => {
          const uri = await api.assetImage(token, url);
          setImagesLoaded((n) => n + 1);
          return uri;
        }),
      );
      setLesson(data);
      setImages(Object.fromEntries(urls.map((url, index) => [url, uris[index]])));
    } catch (err) {
      setError(err instanceof ApiError ? err.message : 'Failed to load');
    } finally {
      setLoading(false);
    }
  }, [token, lessonId]);

  useEffect(() => {
    load();
  }, [load]);

  useEffect(() => {
    AsyncStorage.getItem('kidlearn.narrator').then((value) => {
      if (value !== null) {
        setNarratorEnabled(value === '1');
      }
    });
  }, []);

  useEffect(() => {
    Speech.getAvailableVoicesAsync()
      .then((list) => setVoices(list))
      .catch(() => {})
      .finally(() => setVoicesLoaded(true));
  }, []);

  const speakQuestion = useCallback(
    (text: string, ttsLang: string, mathLang: Lang, voiceId: string | null) => {
      Speech.stop();
      Speech.speak(stripMathForSpeech(text, mathLang), {
        language: ttsLang,
        rate: 0.95,
        ...(voiceId ? { voice: voiceId } : {}),
      });
    },
    [],
  );

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
  const figureUrl = current?.image_url ?? null;
  const figure = figureUrl != null ? images[figureUrl] : undefined;
  const questionText =
    !over && !loading && !error && lesson && total > 0 ? lesson.questions[index].text : null;
  const speech = lesson ? resolveSpeechLanguage(lesson.language, lang) : null;
  // Primitives, so the effect below only re-runs when they really change.
  const ttsLang = speech?.tts ?? null;
  const mathLang = speech?.math ?? null;
  const matchingVoices = useMemo(
    () => (ttsLang ? matchingNarratorVoices(voices, ttsLang) : []),
    [voices, ttsLang],
  );
  const narratorVoice = useMemo(() => {
    const manual =
      manualVoiceId != null
        ? matchingVoices.find((voice) => voice.identifier === manualVoiceId)
        : undefined;
    if (manual) {
      return manual;
    }
    return ttsLang ? pickNarratorVoice(voices, ttsLang) : null;
  }, [manualVoiceId, matchingVoices, voices, ttsLang]);
  const narratorVoiceId = narratorVoice?.identifier ?? null;

  // Manual voice choice, remembered per language. A stored voice that is
  // gone (uninstalled) quietly falls back to automatic.
  useEffect(() => {
    if (ttsLang === null) {
      return;
    }
    const base = ttsLang.split('-')[0];
    AsyncStorage.getItem(`kidlearn.voice.${base}`).then((value) => {
      setManualVoiceId(value === null || value === '' ? null : value);
    });
  }, [ttsLang]);

  async function chooseVoice(identifier: string) {
    setVoicePickerOpen(false);
    if (ttsLang === null) {
      return;
    }
    const base = ttsLang.split('-')[0];
    if (identifier === manualVoiceId) {
      // Tapping the active manual voice drops back to automatic.
      setManualVoiceId(null);
      await AsyncStorage.setItem(`kidlearn.voice.${base}`, '');
    } else {
      setManualVoiceId(identifier);
      await AsyncStorage.setItem(`kidlearn.voice.${base}`, identifier);
    }
  }
  const showVoiceWarning =
    narratorEnabled && ttsLang !== null && voicesLoaded && narratorVoice === null;

  // The narrator reads each question aloud; moving on (or leaving) stops it.
  useEffect(() => {
    if (!narratorEnabled || questionText === null || ttsLang === null || mathLang === null) {
      if (!narratorEnabled) {
        Speech.stop();
      }
      return;
    }
    speakQuestion(questionText, ttsLang, mathLang, narratorVoiceId);
    return () => {
      Speech.stop();
    };
  }, [narratorEnabled, questionText, ttsLang, mathLang, narratorVoiceId, speakQuestion]);

  async function toggleNarrator() {
    const next = !narratorEnabled;
    setNarratorEnabled(next);
    await AsyncStorage.setItem('kidlearn.narrator', next ? '1' : '0');
  }

  function replayQuestion() {
    if (questionText === null || ttsLang === null || mathLang === null) {
      return;
    }
    if (!narratorEnabled) {
      // Tapping the sleeping owl wakes it; the effect reads the question.
      setNarratorEnabled(true);
      AsyncStorage.setItem('kidlearn.narrator', '1');
      return;
    }
    speakQuestion(questionText, ttsLang, mathLang, narratorVoiceId);
  }

  return (
    <Screen backgroundUri={backgroundUri}>
      <View style={styles.topRow}>
        <TouchableOpacity onPress={() => onExit(lessonId)}>
          <Text style={styles.exit}>✕</Text>
        </TouchableOpacity>
        <View style={styles.track}>
          <View style={[styles.fill, { width: `${Math.round(progress * 100)}%` }]} />
        </View>
        <Text style={styles.hearts}>{'♥'.repeat(Math.max(0, hearts))}</Text>
        <TouchableOpacity onPress={toggleNarrator}>
          <Text style={styles.sound}>{narratorEnabled ? '🔊' : '🔇'}</Text>
        </TouchableOpacity>
      </View>

      {loading && (
        <Text style={styles.muted}>
          {fetchingImages && imagesPending > 0
            ? `${t('generatingPictures')} (${imagesLoaded}/${imagesTotal})`
            : t('loading')}
        </Text>
      )}
      {error && <ErrorBox message={error} onRetry={load} />}

      {!loading && !error && lesson && total === 0 && (
        <>
          <Text style={styles.muted}>{t('emptyLesson')}</Text>
          <View style={styles.exitButton}>
            <ChunkyButton title={t('exit')} onPress={() => onExit(lessonId)} color="gray" />
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
              <ChunkyButton title={t('exit')} onPress={() => onExit(lessonId)} />
            )}
          </View>
          {failed && (
            <View style={styles.exitButton}>
              <ChunkyButton title={t('exit')} onPress={() => onExit(lessonId)} color="gray" />
            </View>
          )}
        </>
      )}

      {!loading && !error && lesson && total > 0 && !over && (
        <>
          <Text style={styles.subTitle}>{lesson.title}</Text>
          <View style={styles.questionRow}>
            <TouchableOpacity onPress={replayQuestion} style={styles.owlButton}>
              <Owl size={56} awake={narratorEnabled} />
            </TouchableOpacity>
            <View style={styles.questionBody}>
              <MathText
                text={lesson.questions[index].text}
                style={styles.question}
                maxWidth={Math.min(Dimensions.get('window').width, 640) - 40 - OWL_SPACE}
              />
            </View>
          </View>
          {narratorEnabled && narratorVoice !== null &&
            (matchingVoices.length > 1 ? (
              <TouchableOpacity onPress={() => setVoicePickerOpen(true)}>
                <Text style={styles.voiceLine}>{narratorVoice.name} ▾</Text>
              </TouchableOpacity>
            ) : (
              <Text style={styles.voiceLine}>{narratorVoice.name}</Text>
            ))}
          {showVoiceWarning && <Text style={styles.voiceWarning}>{t('noVoice')}</Text>}
          <Modal
            visible={voicePickerOpen}
            transparent
            animationType="fade"
            onRequestClose={() => setVoicePickerOpen(false)}
          >
            <View style={styles.voiceBackdrop}>
              <TouchableOpacity
                style={StyleSheet.absoluteFill}
                activeOpacity={1}
                onPress={() => setVoicePickerOpen(false)}
              />
              <View style={styles.voiceSheet}>
                <ScrollView keyboardShouldPersistTaps="handled">
                  {matchingVoices.map((voice) => {
                    const active = voice.identifier === narratorVoiceId;
                    return (
                      <TouchableOpacity
                        key={voice.identifier}
                        onPress={() => chooseVoice(voice.identifier)}
                        style={[styles.voiceRow, active && styles.voiceRowActive]}
                      >
                        <Text style={styles.voiceRowText}>
                          {voice.name} <Text style={styles.voiceRowLang}>{voice.language}</Text>
                        </Text>
                        {active && <Text style={styles.voiceCheck}>✓</Text>}
                      </TouchableOpacity>
                    );
                  })}
                </ScrollView>
              </View>
            </View>
          </Modal>
          {current?.plot != null && (
            <PlotView
              plot={current.plot}
              width={Math.min(Dimensions.get('window').width, 640) - 40}
            />
          )}
          {figure != null && <Image source={{ uri: figure }} style={styles.figureImage} />}
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
                    <MathText
                      text={option.text}
                      style={[
                        styles.optionText,
                        { fontSize: optionFontSize(stripMathForMeasure(option.text)) },
                      ]}
                      center
                      maxWidth={130}
                      maxHeight={OPTION_INNER}
                    />
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
  sound: {
    fontSize: 20,
    padding: 4,
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
  questionRow: {
    flexDirection: 'row',
    gap: 12,
    alignItems: 'flex-start',
  },
  owlButton: {
    paddingTop: 2,
  },
  questionBody: {
    flex: 1,
  },
  voiceWarning: {
    fontSize: 13,
    color: colors.muted,
    marginTop: -8,
    marginBottom: 16,
  },
  voiceLine: {
    fontSize: 13,
    color: colors.muted,
    marginTop: -8,
    marginBottom: 16,
  },
  voiceBackdrop: {
    flex: 1,
    backgroundColor: 'rgba(0, 0, 0, 0.35)',
    justifyContent: 'center',
    padding: 32,
  },
  voiceSheet: {
    backgroundColor: '#FFFFFF',
    borderRadius: 16,
    maxHeight: 420,
    paddingVertical: 8,
  },
  voiceRow: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingVertical: 12,
    paddingHorizontal: 16,
    gap: 12,
  },
  voiceRowActive: {
    backgroundColor: '#EEF6FF',
  },
  voiceRowText: {
    flex: 1,
    fontSize: 16,
    fontWeight: '600',
    color: colors.ink,
  },
  voiceRowLang: {
    fontSize: 13,
    fontWeight: '400',
    color: colors.muted,
  },
  voiceCheck: {
    fontSize: 16,
    fontWeight: '800',
    color: colors.blue,
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
  figureImage: {
    width: 220,
    height: 220,
    borderRadius: 16,
    backgroundColor: '#FFFFFF',
    borderWidth: 3,
    borderColor: colors.border,
    alignSelf: 'center',
    marginBottom: 16,
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
