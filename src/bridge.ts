import type { RunInfo, RunPage, AccountProfile, AccountSummary } from './types';
import {
  type Course,
  type Credentials,
  type QrPoll,
  type Settings,
  type Snapshot,
  type StoredCredentials,
  type TaskList,
  type UaConfig,
} from './types';

// All paths are relative to the local server (or the Vite development proxy).
async function request<T>(
  path: string,
  options: { method?: string; body?: unknown } = {},
  accountId?: string,
): Promise<T> {
  const method = options.method ?? (options.body !== undefined ? 'POST' : 'GET');
  const response = await fetch(path, {
    method,
    headers: {
      ...(options.body !== undefined ? { 'Content-Type': 'application/json' } : {}),
      ...(accountId !== undefined ? { 'X-HDU-Account': accountId } : {}),
    },
    body: options.body !== undefined ? JSON.stringify(options.body) : undefined,
  });
  const text = await response.text();
  const data: unknown = text ? JSON.parse(text) : null;
  if (!response.ok) {
    const message = (data as { error?: string } | null)?.error || `请求失败（${response.status}）`;
    throw new Error(message);
  }
  return data as T;
}

export function createApi(accountId: string) {
  const scoped = <T>(path: string, options?: { method?: string; body?: unknown }) =>
    request<T>(path, options, accountId);
  return {
    runs: () => scoped<RunInfo[]>('/api/runs'),
    run: (id: string, before?: number) =>
      scoped<RunPage>(
        `/api/runs/${encodeURIComponent(id)}${before === undefined ? '' : `?before=${before}`}`,
      ),
    health: () => scoped<{ ok: boolean }>('/api/health'),
    snapshot: () => scoped<Snapshot>('/api/snapshot'),
    loadSettings: () => scoped<Settings>('/api/settings'),
    saveSettings: (settings: Settings) => scoped<void>('/api/settings', { body: { settings } }),
    loadCourses: () => scoped<Course[]>('/api/courses'),
    importCourses: (text: string) => scoped<Course[]>('/api/courses/import', { body: { text } }),
    fetchCourses: (settings: Settings) =>
      scoped<Course[]>('/api/courses/fetch', { body: { settings } }),
    login: (auth: Credentials, settings: Settings) =>
      scoped<void>('/api/login', { body: { auth, settings } }),
    loginQrStart: (settings: Settings) =>
      scoped<{ image: string }>('/api/login/qr/start', { body: { settings } }),
    loginQrPoll: () => scoped<QrPoll>('/api/login/qr/poll', { body: {} }),
    loginQrCancel: () => scoped<void>('/api/login/qr/cancel', { body: {} }),
    logout: () => scoped<void>('/api/logout', { body: {} }),
    loadCredentials: () => scoped<StoredCredentials>('/api/credentials'),
    saveCredentials: (credentials: StoredCredentials) =>
      scoped<void>('/api/credentials', { body: { credentials } }),
    clearCredentials: () => scoped<void>('/api/credentials/clear', { body: {} }),
    loadUa: () => scoped<UaConfig>('/api/ua'),
    saveUa: (ua: UaConfig) => scoped<void>('/api/ua', { body: { ua } }),
    importTaskLists: (text: string, settings: Settings) =>
      scoped<TaskList[]>('/api/tasks/import', { body: { text, settings } }),
    startTasks: (settings: Settings, list_index: number) =>
      scoped<void>('/api/tasks/start', { body: { settings, list_index } }),
    stopTasks: () => scoped<void>('/api/tasks/stop', { body: {} }),
    shutdown: () => scoped<void>('/api/shutdown', { body: {} }),
  };
}

export const api = createApi('default');
export const accountsApi = {
  list: () => request<AccountSummary[]>('/api/accounts'),
  create: (name: string, copy_from?: string) =>
    request<AccountProfile>('/api/accounts', { body: { name, copy_from } }),
  rename: (id: string, name: string) =>
    request<void>(`/api/accounts/${encodeURIComponent(id)}`, { body: { name } }),
};
