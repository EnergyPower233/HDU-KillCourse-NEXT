import { useState, type Dispatch, type SetStateAction } from 'react';
import { api } from '../bridge';
import { defaults } from '../domain/settings';
import { type Course, type Settings, type TaskList } from '../types';
interface Options {
  settings: Settings;
  setSettings: Dispatch<SetStateAction<Settings>>;
  setSaved: Dispatch<SetStateAction<boolean>>;
  toast: (text: string, error?: boolean) => void;
  perform: (label: string, action: () => Promise<void>) => Promise<void>;
}
export function useTaskLists({ settings, setSettings, setSaved, toast, perform }: Options) {
  const [dropPickerFor, setDropPickerFor] = useState<number | null>(null);
  const [dropQuery, setDropQuery] = useState('');
  const active: TaskList =
    settings.lists[settings.active_list] ?? settings.lists[0] ?? defaults.lists[0];
  const tasks = active.tasks;
  const updateActive = (patch: Partial<TaskList>) => {
    setSettings((s) => ({
      ...s,
      lists: s.lists.map((l, i) =>
        i === (s.active_list < s.lists.length ? s.active_list : 0) ? { ...l, ...patch } : l,
      ),
    }));
    setSaved(false);
  };

  const selected = new Set(
    tasks.flatMap((t) =>
      [t.course?.jxbmc, ...t.drops.map((d) => d.jxbmc)].filter((x): x is string => !!x),
    ),
  );
  function addList() {
    const name = `清单 ${settings.lists.length + 1}`;
    setSettings((s) => ({
      ...s,
      lists: [...s.lists, { name, mode: 'watch', tasks: [] }],
      active_list: s.lists.length,
    }));
    setSaved(false);
  }
  function renameList(index: number) {
    const current = settings.lists[index]?.name || '';
    const name = window.prompt('清单名称', current);
    if (name === null) return;
    const trimmed = name.trim();
    if (!trimmed) {
      toast('清单名称不能为空', true);
      return;
    }
    setSettings((s) => ({
      ...s,
      lists: s.lists.map((l, i) => (i === index ? { ...l, name: trimmed } : l)),
    }));
    setSaved(false);
  }
  function removeList(index: number) {
    if (settings.lists.length <= 1) {
      toast('至少保留一个清单', true);
      return;
    }
    if (!window.confirm(`删除清单「${settings.lists[index]?.name}」？其中的任务也会一并删除。`))
      return;
    setSettings((s) => {
      const lists = s.lists.filter((_, i) => i !== index);
      const active_list = Math.min(
        s.active_list >= index ? Math.max(0, s.active_list - 1) : s.active_list,
        lists.length - 1,
      );
      return { ...s, lists, active_list };
    });
    setSaved(false);
  }
  async function importTaskFile(file?: File) {
    if (!file) return;
    if (file.size > 1024 * 1024) {
      toast('任务清单文件不能超过 1 MB', true);
      return;
    }
    await perform('导入任务清单中', async () => {
      const lists = await api.importTaskLists(await file.text(), settings);
      setSettings((current) => ({
        ...current,
        lists: [...current.lists, ...lists],
        active_list: current.lists.length,
      }));
      setSaved(false);
      toast(`已追加 ${lists.length} 个清单，请核对后保存清单`);
    });
  }
  function exportTaskFile() {
    const blob = new Blob(
      [
        JSON.stringify(
          { version: 1, year: settings.year, term: settings.term, lists: [active] },
          null,
          2,
        ),
      ],
      { type: 'application/json' },
    );
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = url;
    link.download = 'task-lists.json';
    link.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  }
  function add(course: Course) {
    if (selected.has(course.jxbmc)) return;
    updateActive({ tasks: [...tasks, { course, drops: [] }] });
  }
  function addDrop(taskIndex: number, course: Course) {
    if (selected.has(course.jxbmc)) {
      toast('该教学班已在清单中', true);
      return;
    }
    updateActive({
      tasks: tasks.map((t, i) => (i === taskIndex ? { ...t, drops: [...t.drops, course] } : t)),
    });
  }
  function removeDrop(taskIndex: number, jxbmc: string) {
    updateActive({
      tasks: tasks.map((t, i) =>
        i === taskIndex ? { ...t, drops: t.drops.filter((d) => d.jxbmc !== jxbmc) } : t,
      ),
    });
  }
  function addPureDropTask() {
    updateActive({ tasks: [...tasks, { course: null, drops: [] }] });
    setDropPickerFor(tasks.length);
  }
  function remove(index: number) {
    updateActive({ tasks: tasks.filter((_, i) => i !== index) });
  }
  function moveTask(index: number, direction: -1 | 1) {
    const target = index + direction;
    if (target < 0 || target >= tasks.length) return;
    const next = [...tasks];
    [next[index], next[target]] = [next[target], next[index]];
    updateActive({ tasks: next });
  }
  return {
    active,
    tasks,
    selected,
    updateActive,
    addList,
    renameList,
    removeList,
    importTaskFile,
    exportTaskFile,
    add,
    addDrop,
    removeDrop,
    addPureDropTask,
    remove,
    moveTask,
    dropPickerFor,
    setDropPickerFor,
    dropQuery,
    setDropQuery,
  };
}
