const API_URL = process.env.EXPO_PUBLIC_API_URL ?? 'http://localhost:3000';

export interface User {
  id: number;
  email: string;
  name: string;
  created_at: string;
}

export interface SubjectSummary {
  id: number;
  title: string;
  description: string;
  language: string;
  visibility: string;
  status: string;
  lesson_count: number;
  owner_name: string;
  mine: boolean;
  like_count: number;
  liked_by_me: boolean;
  tags: string[];
  created_at: string;
}

export interface LessonSummary {
  id: number;
  position: number;
  title: string;
  summary: string;
  question_count: number;
}

export interface SubjectDetail extends SubjectSummary {
  lessons: LessonSummary[];
}

export interface CreateSubjectInput {
  title: string;
  description?: string;
  language?: string;
  visibility?: string;
  lesson_count?: number;
  tags?: string[];
}

export interface LessonOption {
  asset_key: string;
  svg_url: string;
}

export interface LessonQuestion {
  id: number;
  position: number;
  qid: string;
  text: string;
  qtype: string;
  options: LessonOption[];
}

export interface Lesson {
  id: number;
  subject_id: number;
  position: number;
  title: string;
  summary: string;
  questions: LessonQuestion[];
}

export interface AnswerResult {
  correct: boolean;
  correct_asset_key: string;
  xp_earned: number;
  total_xp: number;
}

export interface LessonProgress {
  lesson_id: number;
  position: number;
  total: number;
  answered: number;
  correct: number;
  unlocked: boolean;
}

export interface SubjectProgress {
  subject_id: number;
  lessons: LessonProgress[];
}

export interface Stats {
  xp: number;
  streak_days: number;
}

export class ApiError extends Error {
  status: number;
  constructor(status: number, message: string) {
    super(message);
    this.status = status;
  }
}

async function request<T>(
  path: string,
  options: { method?: string; token?: string; body?: unknown } = {},
): Promise<T> {
  const response = await fetch(`${API_URL}${path}`, {
    method: options.method ?? 'GET',
    headers: {
      ...(options.token ? { authorization: `Bearer ${options.token}` } : {}),
      ...(options.body !== undefined ? { 'content-type': 'application/json' } : {}),
    },
    body: options.body !== undefined ? JSON.stringify(options.body) : undefined,
  });
  if (response.status === 204) {
    return undefined as T;
  }
  const data = (await response.json().catch(() => ({}))) as { error?: string };
  if (!response.ok) {
    throw new ApiError(response.status, data.error ?? `Request failed (${response.status})`);
  }
  return data as T;
}

export const api = {
  devLogin: () => request<{ token: string; user: User }>('/auth/dev-login', { method: 'POST' }),
  googleLogin: (idToken: string) =>
    request<{ token: string; user: User }>('/auth/google', {
      method: 'POST',
      body: { id_token: idToken },
    }),
  me: (token: string) => request<User>('/me', { token }),
  listSubjects: (token: string, q?: string) =>
    request<SubjectSummary[]>(
      q ? `/subjects?q=${encodeURIComponent(q)}` : '/subjects',
      { token },
    ),
  createSubject: (token: string, input: CreateSubjectInput) =>
    request<SubjectSummary>('/subjects', { method: 'POST', token, body: input }),
  getSubject: (token: string, id: number) =>
    request<SubjectDetail>(`/subjects/${id}`, { token }),
  generateSubject: (token: string, id: number, lesson_count: number) =>
    request<SubjectDetail>(`/subjects/${id}/generate`, {
      method: 'POST',
      token,
      body: { lesson_count },
    }),
  deleteSubject: (token: string, id: number) =>
    request<void>(`/subjects/${id}`, { method: 'DELETE', token }),
  likeSubject: (token: string, id: number) =>
    request<SubjectSummary>(`/subjects/${id}/like`, { method: 'POST', token }),
  unlikeSubject: (token: string, id: number) =>
    request<SubjectSummary>(`/subjects/${id}/like`, { method: 'DELETE', token }),
  lesson: (token: string, lessonId: number) =>
    request<Lesson>(`/lessons/${lessonId}`, { token }),
  answer: (token: string, lessonId: number, question_id: number, asset_key: string) =>
    request<AnswerResult>(`/lessons/${lessonId}/answer`, {
      method: 'POST',
      token,
      body: { question_id, asset_key },
    }),
  subjectProgress: (token: string, id: number) =>
    request<SubjectProgress>(`/subjects/${id}/progress`, { token }),
  stats: (token: string) => request<Stats>('/stats', { token }),
  aiSearch: (token: string, q: string) =>
    request<SubjectSummary[]>(`/subjects/ai-search?q=${encodeURIComponent(q)}`, { token }),
  assetSvg: async (token: string, path: string): Promise<string> => {
    const response = await fetch(`${API_URL}${path}`, {
      headers: { authorization: `Bearer ${token}` },
    });
    if (!response.ok) {
      throw new ApiError(response.status, `Image failed (${response.status})`);
    }
    return response.text();
  },
};
