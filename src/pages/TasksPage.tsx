import { ArrowDownToLine, Check, ListChecks, Plus, Radio, Square, Trash2, X } from 'lucide-react';
import type * as React from 'react';
import { useApi } from '../AccountContext';
import { Empty, IntervalInput } from '../components/ui';
import type { useTaskLists } from '../hooks/useTaskLists';
import { type Progress, type Settings, type Snapshot, type TaskList } from '../types';
interface Props {
  taskLists: ReturnType<typeof useTaskLists>;

  snapshot: Snapshot;
  locked: boolean;
  perform: (label: string, fn: () => Promise<void>) => Promise<void>;
  save: () => Promise<void>;
  toast: (text: string, error?: boolean) => void;
  saved: boolean;
  busy: string;

  setReviewList: React.Dispatch<React.SetStateAction<number>>;
  settings: Settings;
  setReviewOpen: React.Dispatch<React.SetStateAction<boolean>>;
  setLoginOpen: React.Dispatch<React.SetStateAction<boolean>>;
  update: (patch: Partial<Settings>) => void;

  latest: Map<string, Progress>;

  reorder: (i: number) => React.JSX.Element;
}
export function TasksPage({
  snapshot,
  locked,
  perform,
  save,
  toast,
  saved,
  busy,
  setReviewList,
  settings,
  setReviewOpen,
  setLoginOpen,
  update,
  latest,
  reorder,
  taskLists,
}: Props) {
  const api = useApi();
  const {
    tasks,
    addList,
    renameList,
    removeList,
    active,
    addPureDropTask,
    updateActive,
    removeDrop,
    setDropQuery,
    setDropPickerFor,
    remove,
  } = taskLists;
  return (
    <section className="panel">
      <div className="panel-heading">
        <div>
          <h2>任务清单</h2>
          <p>
            {snapshot.running
              ? '正在执行；停止后可编辑任务。'
              : '每个清单独立保存；蹲课模式先并发查询余量，单次模式按顺序提交。'}
          </p>
        </div>
        <div className="heading-actions">
          <button
            className="button secondary"
            disabled={locked}
            onClick={() =>
              perform('保存中', async () => {
                await save();
                toast('清单已保存');
              })
            }
          >
            {saved ? <Check size={15} /> : <ArrowDownToLine size={15} />}保存清单
          </button>
          {snapshot.running ? (
            <button
              className="button danger"
              disabled={!!busy}
              onClick={() =>
                perform('停止中', async () => {
                  await api.stopTasks();
                  toast('已请求停止，正在等待当前请求结束');
                })
              }
            >
              <Square size={14} />
              停止任务
            </button>
          ) : (
            <button
              className="button primary"
              disabled={locked || !tasks.length}
              onClick={() => {
                setReviewList(settings.active_list);
                snapshot.logged_in ? setReviewOpen(true) : setLoginOpen(true);
              }}
            >
              <Radio size={15} />
              开始任务
            </button>
          )}
        </div>
      </div>
      <div className="list-bar">
        <label>
          当前清单
          <select
            disabled={locked}
            value={settings.active_list}
            onChange={(e) => update({ active_list: Number(e.target.value) })}
          >
            {settings.lists.map((l, i) => (
              <option key={i} value={i}>
                {l.name}
              </option>
            ))}
          </select>
        </label>
        <span className="list-actions">
          <button className="button secondary" disabled={locked} onClick={addList}>
            <Plus size={14} />
            新建
          </button>
          <button
            className="button secondary"
            disabled={locked}
            onClick={() => renameList(settings.active_list)}
          >
            重命名
          </button>
          <button
            className="button secondary"
            disabled={locked || settings.lists.length <= 1}
            onClick={() => removeList(settings.active_list)}
          >
            <Trash2 size={14} />
            删除
          </button>
          <button
            className="button secondary"
            disabled={locked || active.mode === 'watch'}
            title={active.mode === 'watch' ? '蹲课模式不支持退课' : '添加一个只有退课的任务'}
            onClick={addPureDropTask}
          >
            <Square size={13} />
            仅退课任务
          </button>
        </span>
      </div>
      <div className="task-controls">
        <label>
          执行模式
          <select
            disabled={locked}
            value={active.mode}
            onChange={(e) => updateActive({ mode: e.target.value as TaskList['mode'] })}
          >
            <option value="watch">蹲课 · 等待余量</option>
            <option value="once">单次 · 选课或退课</option>
          </select>
        </label>
        {active.mode === 'watch' && (
          <>
            <label>
              查询间隔
              <IntervalInput
                ms={settings.interval_ms}
                disabled={locked}
                onChange={(ms) => update({ interval_ms: ms })}
              />
            </label>
            <label>
              波动范围
              <IntervalInput
                ms={settings.jitter_ms}
                disabled={locked}
                onChange={(ms) => update({ jitter_ms: ms })}
              />
              <small>0 关闭；间隔在 ±范围 内随机</small>
            </label>
            <label>
              波动种子
              <input
                type="number"
                disabled={locked}
                value={settings.jitter_seed}
                onChange={(e) => update({ jitter_seed: Math.trunc(Number(e.target.value)) })}
              />
            </label>
          </>
        )}
        <label className="date-field">
          开始时间（UTC+8，留空立即）
          <input
            type="datetime-local"
            step="0.001"
            disabled={locked}
            value={settings.start_at}
            onChange={(e) => update({ start_at: e.target.value })}
          />
          <small>精确到毫秒，秒级可手输小数</small>
        </label>
        <label>
          提前重新登录（秒）
          <input
            type="number"
            min="0"
            max="86400"
            disabled={locked}
            value={settings.relogin_before_secs}
            onChange={(e) => update({ relogin_before_secs: Math.max(0, Number(e.target.value)) })}
          />
          <small>0 关闭；仅在设置开始时间时生效</small>
        </label>
      </div>
      {tasks.length ? (
        <div className="task-list">
          {tasks.map((t, i) => (
            <div className="task-row" key={i}>
              <span className="queue-number">{String(i + 1).padStart(2, '0')}</span>
              <div className="task-course">
                {t.course ? (
                  <>
                    <strong>{t.course.kcmc}</strong>
                    <small>{t.course.jxbmc}</small>
                    <p>{latest.get(t.course.jxbmc)?.message || t.course.sksj || '尚未开始'}</p>
                  </>
                ) : (
                  <>
                    <strong>仅退课</strong>
                    <small>不选新课</small>
                    <p>先退掉下列课程</p>
                  </>
                )}
              </div>
              <div className="task-drops">
                {t.drops.map((d) => (
                  <span className="drop-chip" key={d.jxbmc}>
                    <span>
                      {d.kcmc}
                      <small>{d.jxbmc}</small>
                    </span>
                    <button
                      title="移除该退课"
                      aria-label={`移除退课 ${d.kcmc}`}
                      disabled={locked}
                      onClick={() => removeDrop(i, d.jxbmc)}
                    >
                      <X size={12} />
                    </button>
                  </span>
                ))}
                {t.drops.length === 0 && <span className="drop-empty">先退的课：无</span>}
                <button
                  className="drop-add"
                  disabled={locked || active.mode === 'watch'}
                  title={active.mode === 'watch' ? '蹲课模式不支持退课' : '添加要先退的课'}
                  onClick={() => {
                    setDropQuery('');
                    setDropPickerFor(i);
                  }}
                >
                  <Plus size={13} />
                  先退的课
                </button>
              </div>
              {reorder(i)}
              <button
                title="移除任务"
                aria-label={`移除第 ${i + 1} 个任务`}
                disabled={locked}
                onClick={() => remove(i)}
              >
                <X size={15} />
              </button>
            </div>
          ))}
        </div>
      ) : (
        <Empty
          icon={<ListChecks size={26} />}
          title="还没有选课任务"
          text="去课程中心把想选的课加入清单，再回到这里开始。"
        />
      )}
    </section>
  );
}
