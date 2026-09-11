// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import App from './App';
import { api, accountsApi, createApi } from './bridge';
import { defaultStoredCredentials } from './domain/credentials';
import { defaults } from './domain/settings';
import { defaultUaConfig } from './domain/ua';
import type { Course, TaskList } from './types';

vi.mock('./bridge', async (original) => {
  const actual = await original<typeof import('./bridge')>();
  const api = Object.fromEntries(Object.keys(actual.api).map((key) => [key, vi.fn()]));
  return {
    api,
    createApi: vi.fn(),
    accountsApi: { list: vi.fn(), create: vi.fn(), rename: vi.fn() },
  };
});
const course: Course = {
  jxbmc: '(2026-2027-1)-test',
  jxb_id: 'class-id',
  kch_id: 'course-id',
  kcmc: '测试课程',
  kklxmc: '主修课程',
  sksj: '星期一',
  jxdd: '',
  jzgxx: '',
  jxbzc: '',
};
const imported: TaskList = { name: '导入的清单', mode: 'watch', tasks: [{ course, drops: [] }] };

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(createApi).mockReturnValue(api);
  vi.mocked(accountsApi.list).mockResolvedValue([
    {
      id: 'default',
      name: '默认账号',
      running: false,
      logged_in: true,
      authenticating: false,
      log_error: null,
    },
  ]);
  localStorage.clear();
  vi.stubGlobal(
    'matchMedia',
    vi.fn(() => ({ matches: false, addEventListener: vi.fn(), removeEventListener: vi.fn() })),
  );
  vi.mocked(api.health).mockResolvedValue({ ok: true });
  vi.mocked(api.loadSettings).mockResolvedValue(structuredClone(defaults));
  vi.mocked(api.loadCourses).mockResolvedValue([course]);
  vi.mocked(api.snapshot).mockResolvedValue({
    running: false,
    logged_in: true,
    history: [],
    fetch_progress: null,
  });
  vi.mocked(api.loadCredentials).mockResolvedValue(structuredClone(defaultStoredCredentials));
  vi.mocked(api.loadUa).mockResolvedValue({ ...defaultUaConfig, browser_ua: navigator.userAgent });
  vi.mocked(api.runs).mockResolvedValue([]);
  vi.mocked(api.saveSettings).mockResolvedValue(undefined);
  vi.mocked(api.startTasks).mockResolvedValue(undefined);
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

async function openTasks() {
  await act(async () => {
    render(<App />);
  });
  expect(screen.getByRole('button', { name: '测试课程' })).toBeTruthy();
  fireEvent.click(within(screen.getByRole('navigation')).getByRole('button', { name: /选课任务/ }));
}
function importFile(text: string) {
  const file = new File([text], 'tasks.json', { type: 'application/json' });
  Object.defineProperty(file, 'text', { value: async () => text });
  fireEvent.change(screen.getByLabelText('导入任务清单文件'), { target: { files: [file] } });
}

describe('workspace interactions after module extraction', () => {
  it('switches all four pages and shows usage instructions', async () => {
    await openTasks();
    for (const name of ['偏好设置', '运行记录', '课程中心', '选课任务']) {
      fireEvent.click(
        within(screen.getByRole('navigation')).getByRole('button', { name: new RegExp(name) }),
      );
      expect(screen.getByRole('heading', { level: 1, name })).toBeTruthy();
    }
    expect(screen.queryByText('每一步，都有记录。')).toBeNull();
  });
  it('appends imported lists, keeps existing lists and waits for explicit save', async () => {
    vi.mocked(api.importTaskLists).mockResolvedValue([structuredClone(imported)]);
    await openTasks();
    importFile('{"version":1}');
    await screen.findByText('已追加 1 个清单，请核对后保存清单');
    const picker = screen.getByLabelText('当前清单') as HTMLSelectElement;
    expect(picker.options.length).toBe(2);
    expect(picker.value).toBe('1');
    expect(api.saveSettings).not.toHaveBeenCalled();
    expect(api.startTasks).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: '保存清单' }));
    await waitFor(() => expect(api.saveSettings).toHaveBeenCalledOnce());
    const saved = vi.mocked(api.saveSettings).mock.calls[0][0];
    expect(saved.lists[0]).toEqual(defaults.lists[0]);
    expect(saved.lists[1]).toEqual(imported);
  });
  it('keeps the current list when validation fails', async () => {
    vi.mocked(api.importTaskLists).mockRejectedValue(new Error('任务清单学期不一致'));
    await openTasks();
    importFile('{}');
    await screen.findByText('Error: 任务清单学期不一致');
    expect((screen.getByLabelText('当前清单') as HTMLSelectElement).options.length).toBe(1);
    expect(api.saveSettings).not.toHaveBeenCalled();
    expect(api.startTasks).not.toHaveBeenCalled();
  });
  it('adds a course to the active list and keeps it when switching pages', async () => {
    render(<App />);
    fireEvent.click(
      await screen.findByRole('button', { name: `添加 ${course.kcmc} ${course.jxbmc}` }),
    );
    fireEvent.click(
      within(screen.getByRole('navigation')).getByRole('button', { name: /选课任务/ }),
    );
    fireEvent.click(screen.getByRole('button', { name: '保存清单' }));
    await waitFor(() => expect(api.saveSettings).toHaveBeenCalledOnce());
    expect(vi.mocked(api.saveSettings).mock.calls[0][0].lists[0].tasks).toEqual([
      { course, drops: [] },
    ]);
    expect(api.startTasks).not.toHaveBeenCalled();
  });
  it('exports the selected list with version and semester, without credentials', async () => {
    vi.mocked(api.loadSettings).mockResolvedValue({
      ...structuredClone(defaults),
      lists: [structuredClone(imported)],
    });
    let content: Blob | undefined;
    vi.stubGlobal(
      'URL',
      class extends URL {
        static createObjectURL(blob: Blob) {
          content = blob;
          return 'blob:test';
        }
        static revokeObjectURL() {}
      },
    );
    const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});
    await openTasks();
    fireEvent.click(screen.getByRole('button', { name: '导出当前清单' }));
    expect(click).toHaveBeenCalledOnce();
    const text = await new Promise<string>((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result));
      reader.onerror = reject;
      reader.readAsText(content!);
    });
    expect(JSON.parse(text)).toEqual({ version: 1, year: 2026, term: 1, lists: [imported] });
    expect(api.startTasks).not.toHaveBeenCalled();
    click.mockRestore();
  });
  it('requires confirmation before starting and opens activity after success', async () => {
    vi.mocked(api.loadSettings).mockResolvedValue({
      ...structuredClone(defaults),
      lists: [structuredClone(imported)],
    });
    await openTasks();
    fireEvent.click(screen.getByRole('button', { name: '开始任务' }));
    expect(screen.getByRole('dialog', { name: '确认本轮任务' })).toBeTruthy();
    expect(api.startTasks).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: '确认并开始' }));
    await screen.findByRole('heading', { level: 1, name: '运行记录' });
    expect(api.startTasks).toHaveBeenCalledOnce();
  });
});
