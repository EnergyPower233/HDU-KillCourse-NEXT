import { useWorkspace } from './hooks/useWorkspace';
import { useTheme } from './hooks/useTheme';
import { PageHeader } from './components/PageHeader';
import {
  ArrowDownToLine,
  BookOpen,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  ChevronUp,
  Clock3,
  FileJson,
  GraduationCap,
  ListChecks,
  LoaderCircle,
  Moon,
  Power,
  Radio,
  Settings2,
  ShieldCheck,
  Sun,
  Terminal,
  UserRound,
  X,
} from 'lucide-react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { ActivityLog } from './ActivityLog';
import { api } from './bridge';
import { CourseDialog } from './components/CourseDialog';
import { DropPickerDialog } from './components/DropPickerDialog';
import { LoginDialog } from './components/LoginDialog';
import { TaskReviewDialog } from './components/TaskReviewDialog';
import { Stat } from './components/ui';
import { filterCourses } from './domain/courses';
import { formatInterval } from './domain/interval';
import { statuses } from './domain/progress';
import { emptyAuth, useLogin } from './hooks/useLogin';
import { useTaskLists } from './hooks/useTaskLists';
import type { Theme, View } from './navigation';
import { CoursesPage } from './pages/CoursesPage';
import { SettingsPage } from './pages/SettingsPage';
import { TasksPage } from './pages/TasksPage';
import { type Course, type Progress, type Settings, type UaConfig } from './types';

const navigation = [
  { id: 'courses', title: '课程中心', icon: BookOpen },
  { id: 'tasks', title: '选课任务', icon: ListChecks },
  { id: 'activity', title: '运行记录', icon: Terminal },
  { id: 'settings', title: '偏好设置', icon: Settings2 },
] as const;

export default function App() {
  const [view, setView] = useState<View>('courses');

  const [query, setQuery] = useState('');
  const [kind, setKind] = useState('');
  const [page, setPage] = useState(0);
  const [busy, setBusy] = useState('');
  const [elapsed, setElapsed] = useState(0);
  const [notice, setNotice] = useState<{ text: string; error: boolean } | null>(null);

  const [reviewOpen, setReviewOpen] = useState(false);
  const [reviewList, setReviewList] = useState(0);

  const [detail, setDetail] = useState<Course | null>(null);
  const [saved, setSaved] = useState(true);

  const [theme, setTheme] = useState<Theme>(
    () => (localStorage.getItem('hdu-theme') as Theme | null) || 'system',
  );
  const [shutdown, setShutdown] = useState(false);

  const input = useRef<HTMLInputElement>(null);
  const taskInput = useRef<HTMLInputElement>(null);

  const toast = useCallback((text: string, error = false) => setNotice({ text, error }), []);
  const {
    settings,
    setSettings,
    courses,
    setCourses,
    snapshot,
    setSnapshot,
    ready,
    backend,
    ua,
    setUa,
    creds,
    setCreds,
  } = useWorkspace(toast);
  const offline = backend === false;
  const locked = snapshot.running || !!busy || offline;
  const login = useLogin({ settings, toast, setSnapshot, setBusy, creds, setCreds });
  const { setAuth, loginOpen, setLoginOpen, cancelQr } = login;
  const taskLists = useTaskLists({ settings, setSettings, setSaved, toast, perform });
  const { active, tasks, importTaskFile, exportTaskFile, moveTask, dropPickerFor } = taskLists;
  const update = (patch: Partial<Settings>) => {
    setSettings((s) => ({ ...s, ...patch }));
    setSaved(false);
  };
  const patchUa = (patch: Partial<UaConfig>) => {
    setUa((u) => {
      const next = { ...u, ...patch };
      void api.saveUa(next).catch(() => {});
      return next;
    });
  };

  useTheme(theme);

  useEffect(() => {
    if (!notice || notice.error) return;
    const id = setTimeout(() => setNotice(null), 4500);
    return () => clearTimeout(id);
  }, [notice]);
  useEffect(() => {
    if (!busy) {
      setElapsed(0);
      return;
    }
    const start = Date.now();
    const id = window.setInterval(() => setElapsed(Math.floor((Date.now() - start) / 1000)), 1000);
    return () => window.clearInterval(id);
  }, [busy]);
  useEffect(() => {
    setPage(0);
  }, [query, kind, settings.year, settings.term]);
  useEffect(() => {
    const listener = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        if (!busy) {
          cancelQr();
          setLoginOpen(false);
          setAuth(emptyAuth);
          setReviewOpen(false);
        }
        setDetail(null);
      }
    };
    window.addEventListener('keydown', listener);
    return () => window.removeEventListener('keydown', listener);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [busy]);

  const filtered = useMemo(
    () => filterCourses(courses, settings, query, kind),
    [courses, settings.year, settings.term, query, kind],
  );
  const termCourses = useMemo(
    () => filterCourses(courses, settings, '', ''),
    [courses, settings.year, settings.term],
  );
  const countPages = Math.max(1, Math.ceil(filtered.length / 30));
  const displayPage = Math.min(page, countPages - 1);

  const latest = useMemo(() => {
    const m = new Map<string, Progress>();
    snapshot.history.forEach((e) => {
      if (e.course_id) m.set(e.course_id, e);
    });
    return m;
  }, [snapshot.history]);
  const completed = tasks.filter(
    (t) => t.course && latest.get(t.course.jxbmc)?.status === 'success',
  ).length;
  const failures = tasks.filter(
    (t) =>
      t.course &&
      ['error', 'rejected', 'unknown'].includes(latest.get(t.course.jxbmc)?.status || ''),
  ).length;

  async function perform(label: string, fn: () => Promise<void>) {
    setBusy(label);
    try {
      await fn();
    } catch (e) {
      toast(String(e), true);
    } finally {
      setBusy('');
    }
  }
  async function save() {
    await api.saveSettings(settings);
    setSaved(true);
  }
  async function importFile(file?: File) {
    if (!file) return;
    if (file.size > 30 * 1024 * 1024) {
      toast('文件不能超过 30 MB', true);
      return;
    }
    await perform('导入中', async () => {
      const result = await api.importCourses(await file.text());
      setCourses(result);
      setPage(0);
      toast(`已导入 ${result.length.toLocaleString()} 个教学班`);
    });
  }

  const statusBadge = (id: string) => {
    const e = latest.get(id);
    return (
      <span className={`badge ${e?.status || 'neutral'}`}>
        {e ? statuses[e.status] || e.status : '待开始'}
      </span>
    );
  };
  const reorder = (i: number) => (
    <span className="reorder">
      <button
        title="上移"
        aria-label={`上移第 ${i + 1} 个任务`}
        disabled={locked || i === 0}
        onClick={() => moveTask(i, -1)}
      >
        <ChevronUp size={14} />
      </button>
      <button
        title="下移"
        aria-label={`下移第 ${i + 1} 个任务`}
        disabled={locked || i === tasks.length - 1}
        onClick={() => moveTask(i, 1)}
      >
        <ChevronDown size={14} />
      </button>
    </span>
  );

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-icon">
            <GraduationCap size={20} />
          </span>
          <div>
            杭电选课<small>HDU-KillCourse NEXT</small>
          </div>
        </div>
        <div className="workspace-label">我的工作台</div>
        <nav aria-label="主导航">
          {navigation.map((n) => (
            <button
              key={n.id}
              className={`nav-item ${view === n.id ? 'active' : ''}`}
              onClick={() => setView(n.id)}
            >
              <n.icon size={18} />
              {n.title}
              {n.id === 'tasks' && tasks.length > 0 && (
                <span className="nav-count">{tasks.length}</span>
              )}
            </button>
          ))}
        </nav>
        <div className="sidebar-bottom">
          <div className="session-card">
            <span className={`dot ${snapshot.logged_in ? 'green' : ''}`} />
            <span>{snapshot.logged_in ? '教务会话已连接' : '尚未连接教务系统'}</span>
          </div>
          <button
            className="account-button"
            disabled={!!busy || snapshot.running}
            onClick={() => setLoginOpen(true)}
          >
            <span className="avatar">
              <UserRound size={18} />
            </span>
            <span>
              <span>{snapshot.logged_in ? '管理登录' : '登录账号'}</span>
              <small>
                {snapshot.logged_in ? '凭证按你的选择保存在本机' : '连接后即可查询与选课'}
              </small>
            </span>
            <ChevronRight size={15} />
          </button>
          <div className="version">BROWSER EDITION · v0.3.0</div>
          <button
            className="exit-button"
            disabled={!!busy}
            onClick={() =>
              perform('正在退出', async () => {
                await api.shutdown();
                setShutdown(true);
              })
            }
          >
            <Power size={15} />
            <span>退出程序（关闭本地服务）</span>
          </button>
        </div>
      </aside>

      <div className="main-shell">
        <header className="topbar">
          <div className="breadcrumb">
            工作台 <ChevronRight size={13} />
            <strong>{navigation.find((n) => n.id === view)?.title}</strong>
          </div>
          <div className="topbar-right">
            {backend === null ? (
              <span className="chip">
                <span className="dot" />
                正在连接本地服务
              </span>
            ) : offline ? (
              <span className="chip red">
                <span className="dot" />
                本地服务未连接
              </span>
            ) : (
              <span className="chip green">
                <span className="dot" />
                本地服务已连接
              </span>
            )}
            <button
              className="icon-button"
              title={theme === 'dark' ? '切换到浅色' : '切换到深色'}
              onClick={() => setTheme((t) => (t === 'dark' ? 'light' : 'dark'))}
            >
              {theme === 'dark' ? <Sun size={16} /> : <Moon size={16} />}
            </button>
            <span className="chip">
              <Clock3 size={13} />
              {settings.year}—{settings.year + 1} 学年 · 第 {settings.term} 学期
            </span>
          </div>
        </header>
        <main>
          {snapshot.log_error && (
            <div className="offline-banner" role="alert">
              <strong>本地日志保存失败，已请求停止任务：</strong>
              {snapshot.log_error}
            </div>
          )}
          {offline && (
            <div className="offline-banner" role="alert">
              <ShieldCheck size={16} />
              <span>
                <strong>尚未连接本地服务。</strong>请先启动 HDU-KillCourse NEXT
                服务端程序，本页面的登录与选课功能暂不可用。
              </span>
            </div>
          )}
          <PageHeader view={view}>
            {view === 'tasks' && (
              <>
                <input
                  ref={taskInput}
                  aria-label="导入任务清单文件"
                  type="file"
                  accept=".json,application/json"
                  hidden
                  onChange={(event) => {
                    void importTaskFile(event.target.files?.[0]);
                    event.target.value = '';
                  }}
                />
                <button
                  className="button secondary"
                  disabled={locked}
                  onClick={() => taskInput.current?.click()}
                >
                  <FileJson size={15} />
                  导入清单 JSON
                </button>
                <button className="button secondary" disabled={locked} onClick={exportTaskFile}>
                  导出当前清单
                </button>
              </>
            )}
            {view === 'courses' && (
              <>
                <button
                  className="button secondary"
                  disabled={locked}
                  onClick={() => input.current?.click()}
                >
                  <ArrowDownToLine size={15} />
                  导入课程
                </button>
                <button
                  className="button primary"
                  disabled={locked}
                  onClick={() =>
                    snapshot.logged_in
                      ? perform('获取课程中', async () => {
                          setCourses(await api.fetchCourses(settings));
                          toast('课程资料已更新');
                        })
                      : setLoginOpen(true)
                  }
                >
                  {busy === '获取课程中' ? (
                    <>
                      <LoaderCircle className="spin" size={15} />
                      {(() => {
                        const fp = snapshot.fetch_progress;
                        if (fp && fp.page > 0)
                          return fp.total
                            ? `第 ${fp.page} 页 · ${fp.courses}/${fp.total} 门`
                            : `第 ${fp.page} 页 · ${fp.courses} 门`;
                        return `获取课程中 · ${elapsed} 秒`;
                      })()}
                    </>
                  ) : (
                    <>
                      <Radio size={15} />
                      从教务获取
                    </>
                  )}
                </button>
              </>
            )}
          </PageHeader>
          <input
            aria-label="导入 course.json"
            ref={input}
            type="file"
            accept=".json,application/json"
            hidden
            onChange={(e) => {
              void importFile(e.target.files?.[0]);
              e.target.value = '';
            }}
          />
          {notice && (
            <div
              role={notice.error ? 'alert' : 'status'}
              className={`notice ${notice.error ? 'notice-error' : ''}`}
            >
              <span>{notice.text}</span>
              <button aria-label="关闭提示" onClick={() => setNotice(null)}>
                <X size={15} />
              </button>
            </div>
          )}

          {view !== 'settings' && (
            <div className="stats">
              <Stat
                icon={<BookOpen size={18} />}
                label="本学期教学班"
                value={termCourses.length.toLocaleString()}
                note="课程资料库"
              />
              <Stat
                icon={<ListChecks size={18} />}
                label="已添加任务"
                value={String(tasks.length).padStart(2, '0')}
                note={`${active.mode === 'watch' ? '蹲课模式' : '单次选退课'} · ${formatInterval(settings.interval_ms)}间隔`}
              />
              <Stat
                icon={<CheckCircle2 size={18} />}
                label="学校返回成功"
                value={String(completed).padStart(2, '0')}
                note={failures ? `${failures} 项需要关注` : '以教务已选记录为准'}
              />
            </div>
          )}

          {view === 'courses' && (
            <CoursesPage
              filtered={filtered}
              query={query}
              setQuery={setQuery}
              kind={kind}
              setKind={setKind}
              termCourses={termCourses}
              ready={ready}
              courses={courses}
              input={input}
              locked={locked}
              displayPage={displayPage}
              setDetail={setDetail}
              setPage={setPage}
              countPages={countPages}
              statusBadge={statusBadge}
              reorder={reorder}
              settings={settings}
              setView={setView}
              taskLists={taskLists}
            />
          )}

          {view === 'tasks' && (
            <TasksPage
              snapshot={snapshot}
              locked={locked}
              perform={perform}
              save={save}
              toast={toast}
              saved={saved}
              busy={busy}
              setReviewList={setReviewList}
              settings={settings}
              setReviewOpen={setReviewOpen}
              setLoginOpen={setLoginOpen}
              update={update}
              latest={latest}
              reorder={reorder}
              taskLists={taskLists}
            />
          )}

          {view === 'activity' && (
            <section className="panel">
              <ActivityLog snapshot={snapshot} />
            </section>
          )}

          {view === 'settings' && (
            <SettingsPage
              locked={locked}
              settings={settings}
              update={update}
              theme={theme}
              setTheme={setTheme}
              perform={perform}
              save={save}
              toast={toast}
              ua={ua}
              patchUa={patchUa}
              login={login}
            />
          )}
          <footer className="page-footer">
            <span>
              <span className={`dot ${snapshot.running ? 'green' : ''}`} />
              {snapshot.running ? '任务正在运行' : '工作台就绪'}
            </span>
            <span>
              {saved ? '设置已保存' : '有未保存的修改'}
              <span className="footer-divider">/</span>HDU-KillCourse NEXT
            </span>
          </footer>
        </main>
      </div>

      {loginOpen && (
        <LoginDialog
          snapshot={snapshot}
          busy={busy}
          locked={locked}
          perform={perform}
          setSnapshot={setSnapshot}
          toast={toast}
          settings={settings}
          offline={offline}
          login={login}
        />
      )}

      {dropPickerFor !== null && (
        <DropPickerDialog termCourses={termCourses} taskLists={taskLists} />
      )}

      {detail && (
        <CourseDialog detail={detail} setDetail={setDetail} locked={locked} taskLists={taskLists} />
      )}

      {reviewOpen && (
        <TaskReviewDialog
          busy={busy}
          setReviewOpen={setReviewOpen}
          reviewList={reviewList}
          setReviewList={setReviewList}
          settings={settings}
          offline={offline}
          perform={perform}
          save={save}
          setView={setView}
          setSnapshot={setSnapshot}
          toast={toast}
        />
      )}

      {shutdown && (
        <div className="shutdown-screen">
          <div>
            <CheckCircle2 size={40} />
            <h1>本地服务已关闭</h1>
            <p>你可以关闭这个页面了。下次使用时重新启动 HDU-KillCourse NEXT 即可。</p>
          </div>
        </div>
      )}
    </div>
  );
}
