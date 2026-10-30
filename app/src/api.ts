const API_URL = process.env.EXPO_PUBLIC_API_URL ?? 'http://localhost:3000';

export interface User {
  id: number;
  email: string;
  name: string;
  created_at: string;
}

export interface CategorySummary {
  id: number;
  title: string;
  description: string;
  language: string;
  visibility: string;
  status: string;
  subcategory_count: number;
  owner_name: string;
  mine: boolean;
  created_at: string;
}

export interface Subcategory {
  id: number;
  position: number;
  title: string;
  summary: string;
  question_count: number;
}

export interface CategoryDetail extends CategorySummary {
  subcategories: Subcategory[];
}

export interface CreateCategoryInput {
  title: string;
  description?: string;
  language?: string;
  visibility?: string;
  subcategory_count?: number;
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
  me: (token: string) => request<User>('/me', { token }),
  listCategories: (token: string) => request<CategorySummary[]>('/categories', { token }),
  createCategory: (token: string, input: CreateCategoryInput) =>
    request<CategorySummary>('/categories', { method: 'POST', token, body: input }),
  getCategory: (token: string, id: number) =>
    request<CategoryDetail>(`/categories/${id}`, { token }),
  generateCategory: (token: string, id: number, subcategory_count: number) =>
    request<CategoryDetail>(`/categories/${id}/generate`, {
      method: 'POST',
      token,
      body: { subcategory_count },
    }),
  deleteCategory: (token: string, id: number) =>
    request<void>(`/categories/${id}`, { method: 'DELETE', token }),
};
