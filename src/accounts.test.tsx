// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import App from './App';
import { defaults } from './domain/settings';
import { defaultUaConfig } from './domain/ua';
import { defaultStoredCredentials } from './domain/credentials';
import type { AccountSummary, Course, Settings } from './types';

type Call = { path: string; account: string; body: Record<string, unknown> | undefined };
let profiles: AccountSummary[];
let configs: Record<string, Settings>;
let calls: Call[];
let rejectStart: string;
let slowCourses: Promise<Response> | undefined;

function course(id: string): Course {
  return {
    jxbmc: `(2026-2027-1)-${id}`,
    jxb_id: id,
    kch_id: id,
    kcmc: `${id} 的课程`,
    kklxmc: '主修课程',
    sksj: '星期一',
    jxdd: '',
    jzgxx: '',
    jxbzc: '',
  };
}
function reply(data: unknown, status = 200) {
  return new Response(JSON.stringify(data), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

beforeEach(() => {
  localStorage.clear();
  calls = [];
  rejectStart = '';
  slowCourses = undefined;
  profiles = ['default', 'account-b'].map((id, i) => ({
    id,
    name: i ? '账号 B' : '账号 A',
    running: false,
    logged_in: true,
    authenticating: false,
    log_error: null,
  }));
  configs = Object.fromEntries(
    profiles.map((p) => [
      p.id,
      {
        ...structuredClone(defaults),
        lists: [
          { name: p.name + '清单', mode: 'once', tasks: [{ course: course(p.id), drops: [] }] },
        ],
      },
    ]),
  );
  vi.stubGlobal(
    'matchMedia',
    vi.fn(() => ({ matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() })),
  );
  vi.stubGlobal(
    'fetch',
    vi.fn(async (path: string, init: RequestInit = {}) => {
      const account = new Headers(init.headers).get('X-HDU-Account') || 'default';
      const body = init.body ? JSON.parse(String(init.body)) : undefined;
      calls.push({ path, account, body });
      if (path === '/api/accounts') {
        if (body) {
          const p = {
            id: 'account-new',
            name: body.name,
            running: false,
            logged_in: false,
            authenticating: false,
            log_error: null,
          };
          profiles.push(p);
          configs[p.id] = structuredClone(body.copy_from ? configs[body.copy_from] : defaults);
          return reply(p);
        }
        return reply(profiles);
      }
      if (path === '/api/health') return reply({ ok: true });
      if (path === '/api/settings') {
        if (body) {
          configs[account] = body.settings;
          return reply(null);
        }
        return reply(configs[account]);
      }
      if (path === '/api/courses')
        return account === 'default' && slowCourses ? slowCourses : reply([course(account)]);
      if (path === '/api/credentials')
        return reply({ ...defaultStoredCredentials, cas_username: account });
      if (path === '/api/ua') return reply({ ...defaultUaConfig, browser_ua: navigator.userAgent });
      if (path === '/api/snapshot')
        return reply({
          running: profiles.find((p) => p.id === account)?.running,
          logged_in: true,
          history: [],
          fetch_progress: null,
        });
      if (path === '/api/runs') return reply([]);
      if (path === '/api/tasks/start') {
        if (account === rejectStart) return reply({ error: '模拟登录已失效' }, 400);
        profiles.find((p) => p.id === account)!.running = true;
        return reply(null);
      }
      if (path === '/api/tasks/stop') {
        profiles.find((p) => p.id === account)!.running = false;
        return reply(null);
      }
      if (path === '/api/login/qr/cancel') return reply(null);
      throw Error(`Unhandled mock request ${path}`);
    }),
  );
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

async function openManager() {
  render(<App />);
  await screen.findByRole('button', { name: 'default 的课程' });
  fireEvent.click(screen.getByRole('button', { name: /账号与并行任务/ }));
  return screen.getByRole('dialog', { name: '账号与并行任务' });
}

it('starts accounts only after reviewing each plan and reports partial failures', async () => {
  rejectStart = 'account-b';
  const dialog = await openManager();
  fireEvent.click(within(dialog).getByLabelText('并行选择 账号 A'));
  fireEvent.click(within(dialog).getByLabelText('并行选择 账号 B'));
  fireEvent.click(within(dialog).getByRole('button', { name: '检查所选账号任务' }));
  await within(dialog).findByRole('heading', { name: '账号 A · 账号 A清单' });
  expect(within(dialog).getByRole('heading', { name: '账号 B · 账号 B清单' })).toBeTruthy();
  expect(calls.filter((c) => c.path === '/api/tasks/start')).toHaveLength(0);
  fireEvent.click(within(dialog).getByRole('button', { name: '确认并行启动 2 个账号' }));
  await within(dialog).findByText(/已启动 1 个账号；未启动：账号 B/);
  const starts = calls.filter((c) => c.path === '/api/tasks/start');
  expect(starts.map((c) => c.account).sort()).toEqual(['account-b', 'default']);
  for (const call of starts) expect(call.body?.settings).toEqual(configs[call.account]);
  expect(profiles[0].running).toBe(true);
  expect(profiles[1].running).toBe(false);
});

it('stops only the chosen account while another account is running', async () => {
  profiles.forEach((p) => (p.running = true));
  const dialog = await openManager();
  fireEvent.click(within(dialog).getByRole('button', { name: '停止 账号 B' }));
  await waitFor(() =>
    expect(calls.filter((c) => c.path === '/api/tasks/stop').map((c) => c.account)).toEqual([
      'account-b',
    ]),
  );
  expect(profiles[0].running).toBe(true);
});

it('switches accounts without accepting an older account loading response', async () => {
  let resolve!: (value: Response) => void;
  slowCourses = new Promise((r) => (resolve = r));
  render(<App />);
  const picker = await screen.findByLabelText('当前账号');
  fireEvent.change(picker, { target: { value: 'account-b' } });
  await screen.findByRole('button', { name: 'account-b 的课程' });
  await act(async () => resolve(reply([course('default')])));
  expect(screen.queryByRole('button', { name: 'default 的课程' })).toBeNull();
  expect((screen.getByLabelText('当前账号') as HTMLSelectElement).value).toBe('account-b');
  expect(calls.filter((c) => c.path === '/api/credentials').map((c) => c.account)).toContain(
    'account-b',
  );
});

it('copies task configuration into a new profile without copying login credentials', async () => {
  const dialog = await openManager();
  fireEvent.change(within(dialog).getByLabelText('账号名称（本地标识）'), {
    target: { value: '新账号' },
  });
  fireEvent.click(within(dialog).getByLabelText(/复制「账号 A」/));
  fireEvent.click(within(dialog).getByRole('button', { name: '添加并切换' }));
  await waitFor(() =>
    expect((screen.getByLabelText('当前账号') as HTMLSelectElement).value).toBe('account-new'),
  );
  const created = calls.find((c) => c.path === '/api/accounts' && c.body);
  expect(created?.body).toEqual({ name: '新账号', copy_from: 'default' });
  expect(calls.filter((c) => c.path === '/api/credentials' && c.body)).toHaveLength(0);
  expect(calls.filter((c) => c.path === '/api/tasks/start')).toHaveLength(0);
});
