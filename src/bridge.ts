import type { RunInfo, RunPage } from './types';
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
): Promise<T> {
  const method = options.method ?? (options.body !== undefined ? 'POST' : 'GET');
  const response = await fetch(path, {
    method,
    headers: options.body !== undefined ? { 'Content-Type': 'application/json' } : undefined,
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

export const api = {
  runs: () => request<RunInfo[]>('/api/runs'),
  run: (id: string, before?: number) =>
    request<RunPage>(
      `/api/runs/${encodeURIComponent(id)}${before === undefined ? '' : `?before=${before}`}`,
    ),
  health: () => request<{ ok: boolean }>('/api/health'),
  snapshot: () => request<Snapshot>('/api/snapshot'),
  loadSettings: () => request<Settings>('/api/settings'),
  saveSettings: (settings: Settings) => request<void>('/api/settings', { body: { settings } }),
  loadCourses: () => request<Course[]>('/api/courses'),
  importCourses: (text: string) => request<Course[]>('/api/courses/import', { body: { text } }),
  fetchCourses: (settings: Settings) =>
    request<Course[]>('/api/courses/fetch', { body: { settings } }),
  login: (auth: Credentials, settings: Settings) =>
    request<void>('/api/login', { body: { auth, settings } }),
  loginQrStart: (settings: Settings) =>
    request<{ image: string }>('/api/login/qr/start', { body: { settings } }),
  loginQrPoll: () => request<QrPoll>('/api/login/qr/poll', { body: {} }),
  loginQrCancel: () => request<void>('/api/login/qr/cancel', { body: {} }),
  logout: () => request<void>('/api/logout', { body: {} }),
  loadCredentials: () => request<StoredCredentials>('/api/credentials'),
  saveCredentials: (credentials: StoredCredentials) =>
    request<void>('/api/credentials', { body: { credentials } }),
  clearCredentials: () => request<void>('/api/credentials/clear', { body: {} }),
  loadUa: () => request<UaConfig>('/api/ua'),
  saveUa: (ua: UaConfig) => request<void>('/api/ua', { body: { ua } }),
  importTaskLists: (text: string, settings: Settings) =>
    request<TaskList[]>('/api/tasks/import', { body: { text, settings } }),
  startTasks: (settings: Settings, list_index: number) =>
    request<void>('/api/tasks/start', { body: { settings, list_index } }),
  stopTasks: () => request<void>('/api/tasks/stop', { body: {} }),
  shutdown: () => request<void>('/api/shutdown', { body: {} }),
};
