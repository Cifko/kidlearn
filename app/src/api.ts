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
  generated_lessons: number;
  grade: number | null;
  modality: string;
  created_at: string;
}

export interface LessonSummary {
  id: number;
  position: number;
  title: string;
  summary: string;
  question_count: number;
}

export interface PrerequisiteRef {
  id: number;
  title: string;
  owner_name: string;
}

export interface SubjectDetail extends SubjectSummary {
  lessons: LessonSummary[];
  prerequisites: PrerequisiteRef[];
}

export interface CreateSubjectInput {
  title: string;
  description?: string;
  language?: string;
  lesson_count?: number;
  tags?: string[];
  grade?: number | null;
  modality?: string;
  prerequisite_ids?: number[];
}

export interface LessonOption {
  asset_key: string | null;
  image_url: string | null;
  text: string | null;
}

export interface PlotFunction {
  expr: string;
  label: string;
}

export interface PlotPoint {
  x: number;
  y: number;
  label: string;
}

export interface PlotSpec {
  functions: PlotFunction[];
  x_min: number;
  x_max: number;
  y_min: number;
  y_max: number;
  points: PlotPoint[];
}

export interface LessonQuestion {
  id: number;
  position: number;
  qid: string;
  text: string;
  qtype: string;
  image_url: string | null;
  plot: PlotSpec | null;
  options: LessonOption[];
}

export interface Lesson {
  id: number;
  subject_id: number;
  position: number;
  title: string;
  summary: string;
  questions: LessonQuestion[];
  images_pending: number;
  language: string;
}

export interface AnswerResult {
  correct: boolean;
  correct_asset_key: string;
  correct_text: string;
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

export interface LlmCall {
  id: number;
  created_at: string;
  kind: string;
  subject_id: number | null;
  lesson_id: number | null;
  asset_id: number | null;
  label: string;
  model: string;
  duration_ms: number;
  error: string | null;
  prompt: string;
  response: string | null;
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
  devLogin: (who?: string) =>
    request<{ token: string; user: User }>('/auth/dev-login', {
      method: 'POST',
      body: who ? { who } : undefined,
    }),
  publishSubject: (token: string, id: number) =>
    request<SubjectSummary>(`/subjects/${id}/publish`, { method: 'POST', token }),
  googleLogin: (idToken: string) =>
    request<{ token: string; user: User }>('/auth/google', {
      method: 'POST',
      body: { id_token: idToken },
    }),
  me: (token: string) => request<User>('/me', { token }),
  listSubjects: (token: string, q?: string, lang?: string, visibility?: string) => {
    const search = new URLSearchParams();
    if (q) {
      search.set('q', q);
    }
    if (lang) {
      search.set('lang', lang);
    }
    if (visibility) {
      search.set('visibility', visibility);
    }
    const query = search.toString();
    return request<SubjectSummary[]>(`/subjects${query ? `?${query}` : ''}`, { token });
  },
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
  answer: (
    token: string,
    lessonId: number,
    question_id: number,
    picked: { asset_key: string } | { text: string },
  ) =>
    request<AnswerResult>(`/lessons/${lessonId}/answer`, {
      method: 'POST',
      token,
      body: { question_id, ...picked },
    }),
  subjectProgress: (token: string, id: number) =>
    request<SubjectProgress>(`/subjects/${id}/progress`, { token }),
  stats: (token: string) => request<Stats>('/stats', { token }),
  aiSearch: (token: string, q: string, lang?: string) => {
    const search = new URLSearchParams({ q });
    if (lang) {
      search.set('lang', lang);
    }
    return request<SubjectSummary[]>(`/subjects/ai-search?${search.toString()}`, { token });
  },
  listLlmCalls: (token: string, params?: { subject_id?: number; kind?: string }) => {
    const search = new URLSearchParams();
    if (params?.subject_id !== undefined) {
      search.set('subject_id', String(params.subject_id));
    }
    if (params?.kind !== undefined) {
      search.set('kind', params.kind);
    }
    const query = search.toString();
    return request<LlmCall[]>(`/debug/llm-calls${query ? `?${query}` : ''}`, { token });
  },
  assetImage: async (token: string, path: string): Promise<string> => {
    const response = await fetch(`${API_URL}${path}`, {
      headers: { authorization: `Bearer ${token}` },
    });
    if (!response.ok) {
      throw new ApiError(response.status, `Image failed (${response.status})`);
    }
    // Images need the auth header, so they cannot be hot-linked: fetch the
    // bytes and hand the UI a data URI. Works on web and native.
    const blob = await response.blob();
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onloadend = () => {
        if (typeof reader.result === 'string') {
          resolve(reader.result);
        } else {
          reject(new ApiError(response.status, 'Image failed'));
        }
      };
      reader.onerror = () => reject(new ApiError(response.status, 'Image failed'));
      reader.readAsDataURL(blob);
    });
  },
};
