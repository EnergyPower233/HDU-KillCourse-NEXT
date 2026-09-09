import {
  ArrowRight,
  BookOpen,
  Check,
  ChevronLeft,
  ChevronRight,
  FileJson,
  LayoutGrid,
  ListChecks,
  LoaderCircle,
  Plus,
  Search,
  ShieldCheck,
  X,
} from 'lucide-react';
import type * as React from 'react';
import { Empty } from '../components/ui';
import { formatInterval } from '../domain/interval';
import type { useTaskLists } from '../hooks/useTaskLists';
import type { View } from '../navigation';
import { type Course, type Settings } from '../types';
interface Props {
  taskLists: ReturnType<typeof useTaskLists>;

  filtered: Course[];
  query: string;
  setQuery: React.Dispatch<React.SetStateAction<string>>;
  kind: string;
  setKind: React.Dispatch<React.SetStateAction<string>>;
  termCourses: Course[];
  ready: boolean;
  courses: Course[];
  input: React.RefObject<HTMLInputElement | null>;
  locked: boolean;
  displayPage: number;
  setDetail: React.Dispatch<React.SetStateAction<Course | null>>;

  setPage: React.Dispatch<React.SetStateAction<number>>;
  countPages: number;

  statusBadge: (id: string) => React.JSX.Element;
  reorder: (i: number) => React.JSX.Element;

  settings: Settings;
  setView: React.Dispatch<React.SetStateAction<View>>;
}
export function CoursesPage({
  filtered,
  query,
  setQuery,
  kind,
  setKind,
  termCourses,
  ready,
  courses,
  input,
  locked,
  displayPage,
  setDetail,
  setPage,
  countPages,
  statusBadge,
  reorder,
  settings,
  setView,
  taskLists,
}: Props) {
  const { selected, add, tasks, active, remove } = taskLists;
  return (
    <div className="course-layout">
      <section className="panel catalog">
        <div className="panel-heading">
          <div>
            <h2>
              课程资料库 <span className="subtle-count">{filtered.length.toLocaleString()}</span>
            </h2>
            <p>搜索课程名称、教师或教学班编号</p>
          </div>
          <LayoutGrid size={17} className="muted" />
        </div>
        <div className="filters">
          <div className="search-field">
            <Search size={15} />
            <input
              aria-label="搜索课程"
              placeholder="搜索课程 / 教师 / 教学班…"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
            {query && (
              <button aria-label="清空搜索" onClick={() => setQuery('')}>
                <X size={14} />
              </button>
            )}
          </div>
          <select aria-label="课程类型" value={kind} onChange={(e) => setKind(e.target.value)}>
            <option value="">全部类型</option>
            {Array.from(new Set(termCourses.map((c) => c.kklxmc)))
              .filter(Boolean)
              .map((k) => (
                <option key={k}>{k}</option>
              ))}
          </select>
        </div>
        {!ready ? (
          <Empty
            icon={<LoaderCircle className="spin" />}
            title="正在读取课程资料"
            text="稍等一下，即将准备好。"
          />
        ) : !courses.length ? (
          <Empty
            icon={<BookOpen size={28} />}
            title="从你的第一份课程资料开始"
            text="导入旧版的 course.json，或登录后从教务系统获取课程。"
          >
            <button
              className="button primary"
              onClick={() => input.current?.click()}
              disabled={locked}
            >
              <FileJson size={15} />
              导入 course.json
            </button>
            <span className="empty-hint">兼容原 Go 项目的课程缓存</span>
          </Empty>
        ) : !filtered.length ? (
          <Empty
            icon={<Search size={26} />}
            title="没有找到匹配课程"
            text="试试其他关键词，或在偏好设置中核对学年学期。"
          />
        ) : (
          <>
            <div className="course-table-wrap">
              <table className="course-table">
                <thead>
                  <tr>
                    <th>课程 / 教学班</th>
                    <th>上课安排</th>
                    <th aria-label="添加任务" />
                  </tr>
                </thead>
                <tbody>
                  {filtered.slice(displayPage * 30, displayPage * 30 + 30).map((c) => (
                    <tr key={c.jxbmc}>
                      <td>
                        <button className="course-title" onClick={() => setDetail(c)}>
                          {c.kcmc || c.kch_id}
                        </button>
                        <div className="course-id">{c.jxbmc}</div>
                        <div className="course-meta">
                          <span>{c.kklxmc || '类型未提供'}</span>
                          {c.jzgxx && <span>{c.jzgxx}</span>}
                        </div>
                      </td>
                      <td>
                        <div className="schedule">{c.sksj || '时间待定'}</div>
                        {c.jxdd && <div className="location">{c.jxdd}</div>}
                      </td>
                      <td>
                        <button
                          aria-label={`${selected.has(c.jxbmc) ? '已添加' : '添加'} ${c.kcmc} ${c.jxbmc}`}
                          className={`add-course ${selected.has(c.jxbmc) ? 'added' : ''}`}
                          disabled={locked || selected.has(c.jxbmc)}
                          onClick={() => add(c)}
                        >
                          {selected.has(c.jxbmc) ? <Check size={16} /> : <Plus size={16} />}
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <div className="pagination">
              <span>共 {filtered.length.toLocaleString()} 个教学班</span>
              <div>
                <button
                  aria-label="上一页"
                  disabled={displayPage === 0}
                  onClick={() => setPage(displayPage - 1)}
                >
                  <ChevronLeft size={15} />
                </button>
                <span>
                  {displayPage + 1} / {countPages}
                </span>
                <button
                  aria-label="下一页"
                  disabled={displayPage + 1 >= countPages}
                  onClick={() => setPage(displayPage + 1)}
                >
                  <ChevronRight size={15} />
                </button>
              </div>
            </div>
          </>
        )}
      </section>
      <aside className="queue panel">
        <div className="panel-heading">
          <div>
            <h2>
              我的任务 <span className="subtle-count">{tasks.length}</span>
            </h2>
            <p>{active.name}</p>
          </div>
          <ListChecks size={17} className="muted" />
        </div>
        <div className="queue-body">
          {!tasks.length ? (
            <div className="queue-empty">
              <span className="queue-illustration">
                <ListChecks size={24} />
              </span>
              <h3>任务清单还是空的</h3>
              <p>
                点击课程右侧的 +<br />
                把想上的课放进这里
              </p>
            </div>
          ) : (
            tasks.map((t, i) => (
              <div className="queue-row" key={i}>
                <span className="queue-number">{String(i + 1).padStart(2, '0')}</span>
                <div>
                  {t.course ? <strong>{t.course.kcmc}</strong> : <strong>仅退课</strong>}
                  <small>{t.course ? t.course.jxbmc : `${t.drops.length} 门要退的课`}</small>
                  {t.course ? (
                    statusBadge(t.course.jxbmc)
                  ) : (
                    <span className="badge rejected">退课</span>
                  )}
                  {t.drops.length > 0 && (
                    <span className="badge neutral">先退 {t.drops.length}</span>
                  )}
                </div>
                {reorder(i)}
                <button
                  title="移除任务"
                  aria-label={`移除第 ${i + 1} 个任务`}
                  disabled={locked}
                  onClick={() => remove(i)}
                >
                  <X size={14} />
                </button>
              </div>
            ))
          )}
        </div>
        <div className="queue-footer">
          <div>
            <span>执行方式</span>
            <strong>
              {active.mode === 'watch' ? '持续蹲课' : settings.start_at ? '定时执行' : '单次执行'}
            </strong>
          </div>
          <div>
            <span>查询间隔</span>
            <strong>{formatInterval(settings.interval_ms)}</strong>
          </div>
          <button
            className="button primary full"
            disabled={!tasks.length || locked}
            onClick={() => setView('tasks')}
          >
            管理任务 <ArrowRight size={15} />
          </button>
          <span className="queue-note">
            <ShieldCheck size={12} />
            开始前可检查任务清单
          </span>
        </div>
      </aside>
    </div>
  );
}
