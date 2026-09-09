import { useEffect, useMemo, useRef, useState } from 'react';
import {
  ArrowDownToLine, ArrowRight, BookOpen, Check, CheckCircle2, ChevronDown, ChevronLeft,
  ChevronRight, ChevronUp, Clock3, Cookie, FileJson, GraduationCap, KeyRound, LayoutGrid, ListChecks,
  LoaderCircle, LogOut, Monitor, Moon, Plus, Power, QrCode, Radio, RefreshCw, Search, Settings2,
  ShieldCheck, Square, Sun, Terminal, Trash2, UserRound, X, Zap,
} from 'lucide-react';
import { api } from './bridge';
import { ActivityLog } from './ActivityLog';
import {
  buildFixedUa, credentialsFor, defaults, defaultStoredCredentials, defaultUaConfig,
  filterCourses, formatDurationSecs, formatInterval, intervalUnitLabels, intervalUnitMs,
  loginMethodLabels, mergeCreds, normalizeOrder, splitInterval, statuses, uaBrowserLabels,
  uaOsLabels, uaVersions, type Course, type Credentials, type IntervalUnit, type LoginMethod,
  type Progress, type Settings, type Snapshot, type StoredCredentials, type TaskList,
  type UaBrowser, type UaConfig, type UaMode, type UaOs,
} from './types';

type View = 'courses' | 'tasks' | 'activity' | 'settings';
type Theme = 'system' | 'light' | 'dark';
const navigation = [{ id: 'courses', title: '课程中心', icon: BookOpen }, { id: 'tasks', title: '选课任务', icon: ListChecks }, { id: 'activity', title: '运行记录', icon: Terminal }, { id: 'settings', title: '偏好设置', icon: Settings2 }] as const;
const emptyAuth: Credentials = { method: 'cas', username: '', password: '', session_id: '', route: '' };

function applyTheme(theme: Theme) {
  const dark = theme === 'dark' || (theme === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches);
  document.documentElement.dataset.theme = dark ? 'dark' : 'light';
}

export default function App() {
  const [view, setView] = useState<View>('courses');
  const [settings, setSettings] = useState<Settings>(defaults);
  const [courses, setCourses] = useState<Course[]>([]);
  const [snapshot, setSnapshot] = useState<Snapshot>({ running: false, logged_in: false, history: [], fetch_progress: null });
  const [query, setQuery] = useState('');
  const [kind, setKind] = useState('');
  const [page, setPage] = useState(0);
  const [busy, setBusy] = useState('');
  const [elapsed, setElapsed] = useState(0);
  const [notice, setNotice] = useState<{ text: string; error: boolean } | null>(null);
  const [auth, setAuth] = useState<Credentials>(emptyAuth);
  const [loginOpen, setLoginOpen] = useState(false);
  const [reviewOpen, setReviewOpen] = useState(false);
  const [reviewList, setReviewList] = useState(0);
  const [dropPickerFor, setDropPickerFor] = useState<number | null>(null);
  const [dropQuery, setDropQuery] = useState('');
  const [detail, setDetail] = useState<Course | null>(null);
  const [saved, setSaved] = useState(true);
  const [ready, setReady] = useState(false);
  const [backend, setBackend] = useState<boolean | null>(null);
  const [theme, setTheme] = useState<Theme>(() => (localStorage.getItem('hdu-theme') as Theme | null) || 'system');
  const [shutdown, setShutdown] = useState(false);
  const [qr, setQr] = useState<{ image: string; status: 'waiting' | 'expired' | 'error'; message: string } | null>(null);
  const [qrAutoMode, setQrAutoMode] = useState(false);
  const [autoNote, setAutoNote] = useState('');
  const [creds, setCreds] = useState<StoredCredentials>(defaultStoredCredentials);
  const [ua, setUa] = useState<UaConfig>(defaultUaConfig);
  const qrTimer = useRef<number | null>(null);
  const qrBusy = useRef(false);
  const qrResolve = useRef<((ok: boolean) => void) | null>(null);
  const qrAutoRef = useRef(false);
  const input = useRef<HTMLInputElement>(null);
  const offline = backend === false;
  const locked = snapshot.running || !!busy || offline;
  const toast = (text: string, error = false) => setNotice({ text, error });
  const update = (patch: Partial<Settings>) => { setSettings(s => ({ ...s, ...patch })); setSaved(false); };
  const patchUa = (patch: Partial<UaConfig>) => { setUa(u => { const next = { ...u, ...patch }; void api.saveUa(next).catch(() => {}); return next; }); };
  const active: TaskList = settings.lists[settings.active_list] ?? settings.lists[0] ?? defaults.lists[0];
  const tasks = active.tasks;
  const updateActive = (patch: Partial<TaskList>) => {
    setSettings(s => ({ ...s, lists: s.lists.map((l, i) => (i === (s.active_list < s.lists.length ? s.active_list : 0) ? { ...l, ...patch } : l)) }));
    setSaved(false);
  };
  function addList() {
    const name = `清单 ${settings.lists.length + 1}`;
    setSettings(s => ({ ...s, lists: [...s.lists, { name, mode: 'watch', tasks: [] }], active_list: s.lists.length }));
    setSaved(false);
  }
  function renameList(index: number) {
    const current = settings.lists[index]?.name || '';
    const name = window.prompt('清单名称', current);
    if (name === null) return;
    const trimmed = name.trim();
    if (!trimmed) { toast('清单名称不能为空', true); return; }
    setSettings(s => ({ ...s, lists: s.lists.map((l, i) => (i === index ? { ...l, name: trimmed } : l)) }));
    setSaved(false);
  }
  function removeList(index: number) {
    if (settings.lists.length <= 1) { toast('至少保留一个清单', true); return; }
    if (!window.confirm(`删除清单「${settings.lists[index]?.name}」？其中的任务也会一并删除。`)) return;
    setSettings(s => {
      const lists = s.lists.filter((_, i) => i !== index);
      const active_list = Math.min(s.active_list >= index ? Math.max(0, s.active_list - 1) : s.active_list, lists.length - 1);
      return { ...s, lists, active_list };
    });
    setSaved(false);
  }

  // DingTalk QR login flow: fetch the QR once, then poll the scan status.
  // In "auto" mode the promise resolves true on success / false on failure so
  // the ordered login sequence can continue to the next method.
  function clearQrTimer() {
    if (qrTimer.current !== null) { window.clearInterval(qrTimer.current); qrTimer.current = null; }
  }
  function finishQr(ok: boolean) {
    qrAutoRef.current = false;
    setQrAutoMode(false);
    clearQrTimer();
    setQr(null);
    const resolve = qrResolve.current;
    qrResolve.current = null;
    resolve?.(ok);
  }
  function cancelQr() {
    if (qrResolve.current) { finishQr(false); }
    else { clearQrTimer(); setQr(null); }
    void api.loginQrCancel().catch(() => { /* the pending session is optional state */ });
  }
  async function pollQr() {
    if (qrBusy.current) return;
    qrBusy.current = true;
    try {
      const r = await api.loginQrPoll();
      if (r.status === 'confirmed') {
        if (qrAutoRef.current) {
          finishQr(true); // the auto flow finishes the bookkeeping
        } else {
          clearQrTimer();
          setQr(null);
          setSnapshot(s => ({ ...s, logged_in: true }));
          setAuth(emptyAuth);
          setLoginOpen(false);
          toast('已通过钉钉扫码连接教务系统');
        }
      } else if (r.status === 'expired') {
        if (qrAutoRef.current) {
          finishQr(false);
        } else {
          clearQrTimer();
          setQr(q => q ? { ...q, status: 'expired', message: r.message || '二维码已过期' } : q);
        }
      } else {
        setQr(q => q && q.image ? { ...q, message: r.message || '等待扫码' } : q);
      }
    } catch (e) {
      if (qrAutoRef.current) { finishQr(false); }
      else { clearQrTimer(); setQr(q => q ? { ...q, status: 'error', message: String(e) } : q); }
    } finally {
      qrBusy.current = false;
    }
  }
  async function startQr() {
    clearQrTimer();
    setQr({ image: '', status: 'waiting', message: '正在获取二维码…' });
    try {
      const r = await api.loginQrStart(settings);
      setQr({ image: r.image, status: 'waiting', message: '请打开钉钉，扫描二维码' });
      qrTimer.current = window.setInterval(() => { void pollQr(); }, 1500);
    } catch (e) {
      if (qrAutoRef.current) { finishQr(false); }
      else { setQr({ image: '', status: 'error', message: String(e) }); }
    }
  }
  function waitQr(): Promise<boolean> {
    return new Promise(resolve => {
      qrAutoRef.current = true;
      setQrAutoMode(true);
      qrResolve.current = resolve;
      void startQr();
    });
  }
  function closeLogin() {
    cancelQr();
    setLoginOpen(false);
    setAuth(emptyAuth);
    setAutoNote('');
  }

  // -- Credentials persistence & ordered login attempts ---------------------
  function selectMethod(method: LoginMethod) {
    cancelQr();
    const c = credentialsFor(method, creds);
    setAuth(c ?? { ...emptyAuth, method });
    if (method === 'qrcode') void startQr();
  }
  async function rememberCreds(auth: Credentials) {
    const updated = mergeCreds(creds, auth);
    setCreds(updated);
    try { await api.saveCredentials(updated); } catch (e) { toast(String(e), true); }
  }
  function finishLogin() {
    setBusy('');
    setAutoNote('');
    setSnapshot(s => ({ ...s, logged_in: true }));
    setAuth(emptyAuth);
    setLoginOpen(false);
    toast('已连接教务系统');
  }
  async function autoLogin() {
    const order = normalizeOrder(creds.order);
    setBusy('自动连接中');
    for (let i = 0; i < order.length; i++) {
      const method = order[i];
      const label = loginMethodLabels[method];
      setAutoNote(`正在尝试第 ${i + 1} 种方式：${label}…`);
      if (method === 'qrcode') {
        setAutoNote(`第 ${i + 1} 种方式：钉钉扫码，请用手机钉钉扫描`);
        const ok = await waitQr();
        if (ok) { finishLogin(); return; }
        setAutoNote('钉钉扫码未完成，继续尝试下一种方式…');
        continue;
      }
      const c = credentialsFor(method, creds);
      if (!c) { setAutoNote(`跳过：${label}（未保存凭证）`); continue; }
      try {
        await api.login(c, settings);
        await rememberCreds(c);
        finishLogin();
        return;
      } catch (e) {
        setAutoNote(`${label} 失败：${String(e)}`);
      }
    }
    setBusy('');
    setAutoNote('');
    toast('所有登录方式均未成功，请检查账号、密码或网络', true);
  }
  function moveMethod(index: number, direction: -1 | 1) {
    const order = normalizeOrder(creds.order);
    const target = index + direction;
    if (target < 0 || target >= order.length) return;
    [order[index], order[target]] = [order[target], order[index]];
    const updated = { ...creds, order };
    setCreds(updated);
    void api.saveCredentials(updated).catch(e => toast(String(e), true));
  }

  // Theme: keep the document attribute in sync with the selection and the OS.
  useEffect(() => {
    applyTheme(theme);
    localStorage.setItem('hdu-theme', theme);
    if (theme !== 'system') return;
    const media = window.matchMedia('(prefers-color-scheme: dark)');
    const onChange = () => applyTheme('system');
    media.addEventListener('change', onChange);
    return () => media.removeEventListener('change', onChange);
  }, [theme]);

  // Initial load: health first (silent when the local server is down), then data.
  useEffect(() => {
    let alive = true;
    (async () => {
      try {
        await api.health();
      } catch {
        if (alive) { setBackend(false); setReady(true); }
        return;
      }
      try {
        const [s, c, r] = await Promise.all([api.loadSettings(), api.loadCourses(), api.snapshot()]);
        if (alive) { setSettings(s); setCourses(c); setSnapshot(r); setBackend(true); }
      } catch (e) {
        if (alive) { toast(String(e), true); setBackend(true); }
      } finally {
        if (alive) setReady(true);
      }
      api.loadCredentials().then(c => { if (alive) setCreds(c); }).catch(e => { if (alive) toast(String(e), true); });
      api.loadUa().then(u => {
        if (!alive) return;
        // In browser mode, always reflect the actual browser UA.
        if (u.mode === 'browser' && u.browser_ua !== navigator.userAgent) {
          const next = { ...u, browser_ua: navigator.userAgent };
          setUa(next);
          void api.saveUa(next).catch(() => {});
        } else {
          setUa(u);
        }
      }).catch(() => { /* UA 配置缺失时使用默认值 */ });
    })();
    return () => { alive = false; };
  }, []);

  // Progress polling; a failing poll means the local server went away.
  useEffect(() => {
    let alive = true, polling = false;
    const poll = async () => {
      if (polling) return;
      polling = true;
      try { const r = await api.snapshot(); if (alive) { setSnapshot(r); setBackend(true); } }
      catch { if (alive) setBackend(false); }
      finally { polling = false; }
    };
    poll();
    const timer = setInterval(poll, 900);
    return () => { alive = false; clearInterval(timer); };
  }, []);

  useEffect(() => { if (!notice || notice.error) return; const id = setTimeout(() => setNotice(null), 4500); return () => clearTimeout(id); }, [notice]);
  useEffect(() => {
    if (!busy) { setElapsed(0); return; }
    const start = Date.now();
    const id = window.setInterval(() => setElapsed(Math.floor((Date.now() - start) / 1000)), 1000);
    return () => window.clearInterval(id);
  }, [busy]);
  useEffect(() => { setPage(0); }, [query, kind, settings.year, settings.term]);
  useEffect(() => {
    const listener = (event: KeyboardEvent) => {
      if (event.key === 'Escape') { if (!busy) { cancelQr(); setLoginOpen(false); setAuth(emptyAuth); setReviewOpen(false); } setDetail(null); }
    };
    window.addEventListener('keydown', listener); return () => window.removeEventListener('keydown', listener);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [busy]);

  const filtered = useMemo(() => filterCourses(courses, settings, query, kind), [courses, settings.year, settings.term, query, kind]);
  const termCourses = useMemo(() => filterCourses(courses, settings, '', ''), [courses, settings.year, settings.term]);
  const countPages = Math.max(1, Math.ceil(filtered.length / 30));
  const displayPage = Math.min(page, countPages - 1);
  const selected = new Set(tasks.flatMap(t => [t.course?.jxbmc, ...t.drops.map(d => d.jxbmc)].filter((x): x is string => !!x)));
  const latest = useMemo(() => {
    const m = new Map<string, Progress>(); snapshot.history.forEach(e => { if (e.course_id) m.set(e.course_id, e); }); return m;
  }, [snapshot.history]);
  const completed = tasks.filter(t => t.course && latest.get(t.course.jxbmc)?.status === 'success').length;
  const failures = tasks.filter(t => t.course && ['error', 'rejected', 'unknown'].includes(latest.get(t.course.jxbmc)?.status || '')).length;

  async function perform(label: string, fn: () => Promise<void>) {
    setBusy(label);
    try { await fn(); } catch (e) { toast(String(e), true); } finally { setBusy(''); }
  }
  async function save() {
    await api.saveSettings(settings); setSaved(true);
  }
  async function importFile(file?: File) {
    if (!file) return;
    if (file.size > 30 * 1024 * 1024) { toast('文件不能超过 30 MB', true); return; }
    await perform('导入中', async () => {
      const result = await api.importCourses(await file.text());
      setCourses(result); setPage(0); toast(`已导入 ${result.length.toLocaleString()} 个教学班`);
    });
  }
  function add(course: Course) {
    if (selected.has(course.jxbmc)) return;
    updateActive({ tasks: [...tasks, { course, drops: [] }] });
  }
  function addDrop(taskIndex: number, course: Course) {
    if (selected.has(course.jxbmc)) { toast('该教学班已在清单中', true); return; }
    updateActive({ tasks: tasks.map((t, i) => (i === taskIndex ? { ...t, drops: [...t.drops, course] } : t)) });
  }
  function removeDrop(taskIndex: number, jxbmc: string) {
    updateActive({ tasks: tasks.map((t, i) => (i === taskIndex ? { ...t, drops: t.drops.filter(d => d.jxbmc !== jxbmc) } : t)) });
  }
  function addPureDropTask() {
    updateActive({ tasks: [...tasks, { course: null, drops: [] }] });
    setDropPickerFor(tasks.length);
  }
  function remove(index: number) { updateActive({ tasks: tasks.filter((_, i) => i !== index) }); }
  function moveTask(index: number, direction: -1 | 1) {
    const target = index + direction;
    if (target < 0 || target >= tasks.length) return;
    const next = [...tasks];
    [next[index], next[target]] = [next[target], next[index]];
    updateActive({ tasks: next });
  }
  const statusBadge = (id: string) => { const e = latest.get(id); return <span className={`badge ${e?.status || 'neutral'}`}>{e ? statuses[e.status] || e.status : '待开始'}</span>; };
  const reorder = (i: number) => (
    <span className="reorder">
      <button title="上移" aria-label={`上移第 ${i + 1} 个任务`} disabled={locked || i === 0} onClick={() => moveTask(i, -1)}><ChevronUp size={14}/></button>
      <button title="下移" aria-label={`下移第 ${i + 1} 个任务`} disabled={locked || i === tasks.length - 1} onClick={() => moveTask(i, 1)}><ChevronDown size={14}/></button>
    </span>
  );

  return <div className="app-shell">
    <aside className="sidebar">
      <div className="brand"><span className="brand-icon"><GraduationCap size={20}/></span><div>杭电选课<small>HDU-KillCourse NEXT</small></div></div>
      <div className="workspace-label">我的工作台</div>
      <nav aria-label="主导航">{navigation.map(n => <button key={n.id} className={`nav-item ${view === n.id ? 'active' : ''}`} onClick={() => setView(n.id)}><n.icon size={18}/>{n.title}{n.id === 'tasks' && tasks.length > 0 && <span className="nav-count">{tasks.length}</span>}</button>)}</nav>
      <div className="sidebar-bottom">
        <div className="session-card"><span className={`dot ${snapshot.logged_in ? 'green' : ''}`}/><span>{snapshot.logged_in ? '教务会话已连接' : '尚未连接教务系统'}</span></div>
        <button className="account-button" disabled={!!busy || snapshot.running} onClick={() => setLoginOpen(true)}><span className="avatar"><UserRound size={18}/></span><span><span>{snapshot.logged_in ? '管理登录' : '登录账号'}</span><small>{snapshot.logged_in ? '凭证按你的选择保存在本机' : '连接后即可查询与选课'}</small></span><ChevronRight size={15}/></button>
        <div className="version">BROWSER EDITION · v0.2.0</div>
        <button className="exit-button" disabled={!!busy} onClick={() => perform('正在退出', async () => { await api.shutdown(); setShutdown(true); })}><Power size={15}/><span>退出程序（关闭本地服务）</span></button>
      </div>
    </aside>

    <div className="main-shell">
      <header className="topbar">
        <div className="breadcrumb">工作台 <ChevronRight size={13}/><strong>{navigation.find(n => n.id === view)?.title}</strong></div>
        <div className="topbar-right">
          {backend === null ? <span className="chip"><span className="dot"/>正在连接本地服务</span> : offline ? <span className="chip red"><span className="dot"/>本地服务未连接</span> : <span className="chip green"><span className="dot"/>本地服务已连接</span>}
          <button className="icon-button" title={theme === 'dark' ? '切换到浅色' : '切换到深色'} onClick={() => setTheme(t => t === 'dark' ? 'light' : 'dark')}>{theme === 'dark' ? <Sun size={16}/> : <Moon size={16}/>}</button>
          <span className="chip"><Clock3 size={13}/>{settings.year}—{settings.year + 1} 学年 · 第 {settings.term} 学期</span>
        </div>
      </header>
      <main>
        {snapshot.log_error && <div className="offline-banner" role="alert"><strong>本地日志保存失败，已请求停止任务：</strong>{snapshot.log_error}</div>}
        {offline && <div className="offline-banner" role="alert"><ShieldCheck size={16}/><span><strong>尚未连接本地服务。</strong>请先启动 HDU-KillCourse NEXT 服务端程序，本页面的登录与选课功能暂不可用。</span></div>}
        <div className="page-heading"><div><h1>{view === 'courses' ? '把想上的课，安排好。' : view === 'tasks' ? '每一门课，进度清晰。' : view === 'activity' ? '每一步，都有记录。' : '让选课按你的节奏进行。'}</h1><p>{view === 'courses' ? '浏览教学班，建立任务清单，在一个页面里完成选课。' : view === 'tasks' ? '统一管理选退课任务，随时查看学校返回的结果。' : view === 'activity' ? '查询、提交与异常信息会在这里实时更新。' : '设置学期与执行方式，保存后用于下一次任务。'}</p></div><div className="heading-actions">{view === 'courses' && <><button className="button secondary" disabled={locked} onClick={() => input.current?.click()}><ArrowDownToLine size={15}/>导入课程</button><button className="button primary" disabled={locked} onClick={() => snapshot.logged_in ? perform('获取课程中', async () => { setCourses(await api.fetchCourses(settings)); toast('课程资料已更新'); }) : setLoginOpen(true)}>{busy === '获取课程中' ? <><LoaderCircle className="spin" size={15}/>{(() => { const fp = snapshot.fetch_progress; if (fp && fp.page > 0) return fp.total ? `第 ${fp.page} 页 · ${fp.courses}/${fp.total} 门` : `第 ${fp.page} 页 · ${fp.courses} 门`; return `获取课程中 · ${elapsed} 秒`; })()}</> : <><Radio size={15}/>从教务获取</>}</button></>}</div></div>
        <input aria-label="导入 course.json" ref={input} type="file" accept=".json,application/json" hidden onChange={e => { void importFile(e.target.files?.[0]); e.target.value = ''; }}/>
        {notice && <div role={notice.error ? 'alert' : 'status'} className={`notice ${notice.error ? 'notice-error' : ''}`}><span>{notice.text}</span><button aria-label="关闭提示" onClick={() => setNotice(null)}><X size={15}/></button></div>}

        {view !== 'settings' && <div className="stats"><Stat icon={<BookOpen size={18}/>} label="本学期教学班" value={termCourses.length.toLocaleString()} note="课程资料库"/><Stat icon={<ListChecks size={18}/>} label="已添加任务" value={String(tasks.length).padStart(2, '0')} note={`${active.mode === 'watch' ? '蹲课模式' : '单次选退课'} · ${formatInterval(settings.interval_ms)}间隔`}/><Stat icon={<CheckCircle2 size={18}/>} label="学校返回成功" value={String(completed).padStart(2, '0')} note={failures ? `${failures} 项需要关注` : '以教务已选记录为准'}/></div>}

        {view === 'courses' && <div className="course-layout">
          <section className="panel catalog">
            <div className="panel-heading"><div><h2>课程资料库 <span className="subtle-count">{filtered.length.toLocaleString()}</span></h2><p>搜索课程名称、教师或教学班编号</p></div><LayoutGrid size={17} className="muted"/></div>
            <div className="filters"><div className="search-field"><Search size={15}/><input aria-label="搜索课程" placeholder="搜索课程 / 教师 / 教学班…" value={query} onChange={e => setQuery(e.target.value)}/>{query && <button aria-label="清空搜索" onClick={() => setQuery('')}><X size={14}/></button>}</div><select aria-label="课程类型" value={kind} onChange={e => setKind(e.target.value)}><option value="">全部类型</option>{Array.from(new Set(termCourses.map(c => c.kklxmc))).filter(Boolean).map(k => <option key={k}>{k}</option>)}</select></div>
            {!ready ? <Empty icon={<LoaderCircle className="spin"/>} title="正在读取课程资料" text="稍等一下，即将准备好。"/> : !courses.length ? <Empty icon={<BookOpen size={28}/>} title="从你的第一份课程资料开始" text="导入旧版的 course.json，或登录后从教务系统获取课程。"><button className="button primary" onClick={() => input.current?.click()} disabled={locked}><FileJson size={15}/>导入 course.json</button><span className="empty-hint">兼容原 Go 项目的课程缓存</span></Empty> : !filtered.length ? <Empty icon={<Search size={26}/>} title="没有找到匹配课程" text="试试其他关键词，或在偏好设置中核对学年学期。"/> : <><div className="course-table-wrap"><table className="course-table"><thead><tr><th>课程 / 教学班</th><th>上课安排</th><th aria-label="添加任务"/></tr></thead><tbody>{filtered.slice(displayPage * 30, displayPage * 30 + 30).map(c => <tr key={c.jxbmc}><td><button className="course-title" onClick={() => setDetail(c)}>{c.kcmc || c.kch_id}</button><div className="course-id">{c.jxbmc}</div><div className="course-meta"><span>{c.kklxmc || '类型未提供'}</span>{c.jzgxx && <span>{c.jzgxx}</span>}</div></td><td><div className="schedule">{c.sksj || '时间待定'}</div>{c.jxdd && <div className="location">{c.jxdd}</div>}</td><td><button aria-label={`${selected.has(c.jxbmc) ? '已添加' : '添加'} ${c.kcmc} ${c.jxbmc}`} className={`add-course ${selected.has(c.jxbmc) ? 'added' : ''}`} disabled={locked || selected.has(c.jxbmc)} onClick={() => add(c)}>{selected.has(c.jxbmc) ? <Check size={16}/> : <Plus size={16}/>}</button></td></tr>)}</tbody></table></div><div className="pagination"><span>共 {filtered.length.toLocaleString()} 个教学班</span><div><button aria-label="上一页" disabled={displayPage === 0} onClick={() => setPage(displayPage - 1)}><ChevronLeft size={15}/></button><span>{displayPage + 1} / {countPages}</span><button aria-label="下一页" disabled={displayPage + 1 >= countPages} onClick={() => setPage(displayPage + 1)}><ChevronRight size={15}/></button></div></div></>}
          </section>
          <aside className="queue panel">
            <div className="panel-heading"><div><h2>我的任务 <span className="subtle-count">{tasks.length}</span></h2><p>{active.name}</p></div><ListChecks size={17} className="muted"/></div>
            <div className="queue-body">{!tasks.length ? <div className="queue-empty"><span className="queue-illustration"><ListChecks size={24}/></span><h3>任务清单还是空的</h3><p>点击课程右侧的 +<br/>把想上的课放进这里</p></div> : tasks.map((t, i) => <div className="queue-row" key={i}><span className="queue-number">{String(i + 1).padStart(2, '0')}</span><div>{t.course ? <strong>{t.course.kcmc}</strong> : <strong>仅退课</strong>}<small>{t.course ? t.course.jxbmc : `${t.drops.length} 门要退的课`}</small>{t.course ? statusBadge(t.course.jxbmc) : <span className="badge rejected">退课</span>}{t.drops.length > 0 && <span className="badge neutral">先退 {t.drops.length}</span>}</div>{reorder(i)}<button title="移除任务" aria-label={`移除第 ${i + 1} 个任务`} disabled={locked} onClick={() => remove(i)}><X size={14}/></button></div>)}</div>
            <div className="queue-footer"><div><span>执行方式</span><strong>{active.mode === 'watch' ? '持续蹲课' : settings.start_at ? '定时执行' : '单次执行'}</strong></div><div><span>查询间隔</span><strong>{formatInterval(settings.interval_ms)}</strong></div><button className="button primary full" disabled={!tasks.length || locked} onClick={() => setView('tasks')}>管理任务 <ArrowRight size={15}/></button><span className="queue-note"><ShieldCheck size={12}/>开始前可检查任务清单</span></div>
          </aside>
        </div>}

        {view === 'tasks' && <section className="panel">
          <div className="panel-heading"><div><h2>任务清单</h2><p>{snapshot.running ? '正在执行；停止后可编辑任务。' : '每个清单独立保存；蹲课模式先并发查询余量，单次模式按顺序提交。'}</p></div><div className="heading-actions"><button className="button secondary" disabled={locked} onClick={() => perform('保存中', async () => { await save(); toast('清单已保存'); })}>{saved ? <Check size={15}/> : <ArrowDownToLine size={15}/>}保存清单</button>{snapshot.running ? <button className="button danger" disabled={!!busy} onClick={() => perform('停止中', async () => { await api.stopTasks(); toast('已请求停止，正在等待当前请求结束'); })}><Square size={14}/>停止任务</button> : <button className="button primary" disabled={locked || !tasks.length} onClick={() => { setReviewList(settings.active_list); snapshot.logged_in ? setReviewOpen(true) : setLoginOpen(true); }}><Radio size={15}/>开始任务</button>}</div></div>
          <div className="list-bar">
            <label>当前清单<select disabled={locked} value={settings.active_list} onChange={e => update({ active_list: Number(e.target.value) })}>{settings.lists.map((l, i) => <option key={i} value={i}>{l.name}</option>)}</select></label>
            <span className="list-actions"><button className="button secondary" disabled={locked} onClick={addList}><Plus size={14}/>新建</button><button className="button secondary" disabled={locked} onClick={() => renameList(settings.active_list)}>重命名</button><button className="button secondary" disabled={locked || settings.lists.length <= 1} onClick={() => removeList(settings.active_list)}><Trash2 size={14}/>删除</button><button className="button secondary" disabled={locked || active.mode === 'watch'} title={active.mode === 'watch' ? '蹲课模式不支持退课' : '添加一个只有退课的任务'} onClick={addPureDropTask}><Square size={13}/>仅退课任务</button></span>
          </div>
          <div className="task-controls"><label>执行模式<select disabled={locked} value={active.mode} onChange={e => updateActive({ mode: e.target.value as TaskList['mode'] })}><option value="watch">蹲课 · 等待余量</option><option value="once">单次 · 选课或退课</option></select></label>{active.mode === 'watch' && <><label>查询间隔<IntervalInput ms={settings.interval_ms} disabled={locked} onChange={ms => update({ interval_ms: ms })}/></label><label>波动范围<IntervalInput ms={settings.jitter_ms} disabled={locked} onChange={ms => update({ jitter_ms: ms })}/><small>0 关闭；间隔在 ±范围 内随机</small></label><label>波动种子<input type="number" disabled={locked} value={settings.jitter_seed} onChange={e => update({ jitter_seed: Math.trunc(Number(e.target.value)) })}/></label></>}<label className="date-field">开始时间（UTC+8，留空立即）<input type="datetime-local" step="0.001" disabled={locked} value={settings.start_at} onChange={e => update({ start_at: e.target.value })}/><small>精确到毫秒，秒级可手输小数</small></label><label>提前重新登录（秒）<input type="number" min="0" max="86400" disabled={locked} value={settings.relogin_before_secs} onChange={e => update({ relogin_before_secs: Math.max(0, Number(e.target.value)) })}/><small>0 关闭；仅在设置开始时间时生效</small></label></div>
          {tasks.length ? <div className="task-list">{tasks.map((t, i) => <div className="task-row" key={i}><span className="queue-number">{String(i + 1).padStart(2, '0')}</span><div className="task-course">{t.course ? <><strong>{t.course.kcmc}</strong><small>{t.course.jxbmc}</small><p>{latest.get(t.course.jxbmc)?.message || t.course.sksj || '尚未开始'}</p></> : <><strong>仅退课</strong><small>不选新课</small><p>先退掉下列课程</p></>}</div><div className="task-drops">{t.drops.map(d => <span className="drop-chip" key={d.jxbmc}><span>{d.kcmc}<small>{d.jxbmc}</small></span><button title="移除该退课" aria-label={`移除退课 ${d.kcmc}`} disabled={locked} onClick={() => removeDrop(i, d.jxbmc)}><X size={12}/></button></span>)}{t.drops.length === 0 && <span className="drop-empty">先退的课：无</span>}<button className="drop-add" disabled={locked || active.mode === 'watch'} title={active.mode === 'watch' ? '蹲课模式不支持退课' : '添加要先退的课'} onClick={() => { setDropQuery(''); setDropPickerFor(i); }}><Plus size={13}/>先退的课</button></div>{reorder(i)}<button title="移除任务" aria-label={`移除第 ${i + 1} 个任务`} disabled={locked} onClick={() => remove(i)}><X size={15}/></button></div>)}</div> : <Empty icon={<ListChecks size={26}/>} title="还没有选课任务" text="去课程中心把想选的课加入清单，再回到这里开始。"/>}
        </section>}

        {view === 'activity' && <section className="panel">
          <ActivityLog snapshot={snapshot}/>

        </section>}

        {view === 'settings' && <div className="settings-layout">
          <div className="settings-column">
            <section className="panel">
              <div className="panel-heading"><div><h2>学期与执行偏好</h2><p>设置保存在本地数据目录，保存后用于下次任务。</p></div></div>
              <div className="settings-form">
                <div className="form-pair"><label>学年起始年份<input type="number" min="2000" max="2100" disabled={locked} value={settings.year} onChange={e => update({ year: Number(e.target.value) })}/></label><label>学期<select disabled={locked} value={settings.term} onChange={e => update({ term: Number(e.target.value) })}><option value="1">第一学期</option><option value="2">第二学期</option></select></label></div>
                <label>蹲课查询间隔<IntervalInput ms={settings.interval_ms} disabled={locked} onChange={ms => update({ interval_ms: ms })}/><small>100 毫秒～24 小时；仅蹲课模式使用，单次执行不等待。</small></label>
                <label>波动范围<IntervalInput ms={settings.jitter_ms} disabled={locked} onChange={ms => update({ jitter_ms: ms })}/><small>0 关闭；间隔在 ±范围 内随机，模拟抢课瞬间人手点。</small></label>
                <label>波动种子（MT19937）<input type="number" disabled={locked} value={settings.jitter_seed} onChange={e => update({ jitter_seed: Math.trunc(Number(e.target.value)) })}/><small>同一种子的波动序列固定；默认 1919810。</small></label>
                <div><label>外观</label><div className="segmented" role="radiogroup" aria-label="外观主题"><button className={theme === 'system' ? 'active' : ''} onClick={() => setTheme('system')}><Monitor size={14}/>跟随系统</button><button className={theme === 'light' ? 'active' : ''} onClick={() => setTheme('light')}><Sun size={14}/>浅色</button><button className={theme === 'dark' ? 'active' : ''} onClick={() => setTheme('dark')}><Moon size={14}/>深色</button></div></div>
                <button className="button primary" style={{ alignSelf: 'flex-start' }} disabled={locked} onClick={() => perform('保存中', async () => { await save(); toast('偏好设置已保存'); })}>保存设置<Check size={15}/></button>
              </div>
            </section>
            <section className="panel">
              <div className="panel-heading"><div><h2>登录顺序与凭证</h2><p>连接时按此顺序依次尝试；凭证保存在本机数据目录。</p></div><KeyRound size={17} className="muted"/></div>
              <div className="settings-form">
                <div className="method-order">
                  {normalizeOrder(creds.order).map((m, i) => <div className="method-order-row" key={m}>
                    <span className="queue-number">{String(i + 1).padStart(2, '0')}</span>
                    <span className="method-order-name">{loginMethodLabels[m]}</span>
                    <span className={`badge ${m === 'qrcode' || credentialsFor(m, creds) ? 'success' : 'neutral'}`}>{m === 'qrcode' ? '无需凭证' : credentialsFor(m, creds) ? '已保存凭证' : '未保存'}</span>
                    <span className="reorder">
                      <button title="上移" aria-label={`上移 ${loginMethodLabels[m]}`} disabled={locked || i === 0} onClick={() => moveMethod(i, -1)}><ChevronUp size={14}/></button>
                      <button title="下移" aria-label={`下移 ${loginMethodLabels[m]}`} disabled={locked || i === normalizeOrder(creds.order).length - 1} onClick={() => moveMethod(i, 1)}><ChevronDown size={14}/></button>
                    </span>
                  </div>)}
                </div>
                <p className="credential-hint"><ShieldCheck size={13}/>账号密码与 Cookie 以明文保存在程序目录的 data 文件夹（便携设计），仅用于登录学校系统；请勿在共用电脑上保存。</p>
                <button className="button secondary" style={{ alignSelf: 'flex-start' }} disabled={locked} onClick={() => perform('清除中', async () => { await api.clearCredentials(); setCreds(defaultStoredCredentials); toast('已清除保存的凭证'); })}><Trash2 size={15}/>清除已保存凭证</button>
              </div>
            </section>
            <section className="panel">
              <div className="panel-heading"><div><h2>User-Agent</h2><p>模拟普通浏览器，降低被教务识别为脚本的可能。</p></div><UserRound size={17} className="muted"/></div>
              <div className="settings-form">
                <label>模式<select disabled={locked} value={ua.mode} onChange={e => patchUa({ mode: e.target.value as UaMode })}><option value="browser">使用当前浏览器 UA</option><option value="fixed">固定 UA（自选系统/浏览器/版本）</option><option value="rotate">多 UA 轮换</option><option value="generate">随机生成（MT19937）</option></select></label>
                {ua.mode === 'browser' && <p className="ua-preview"><strong>当前 UA：</strong><span>{ua.browser_ua || navigator.userAgent}</span></p>}
                {ua.mode === 'fixed' && <>
                  <div className="form-pair">
                    <label>操作系统<select disabled={locked} value={ua.fixed_os} onChange={e => { const os = e.target.value as UaOs; const browser = (os !== 'macos' && ua.fixed_browser === 'safari') ? 'chrome' : ua.fixed_browser; patchUa({ fixed_os: os, fixed_browser: browser, fixed_version: uaVersions[browser][0] }); }}>{(Object.keys(uaOsLabels) as UaOs[]).map(o => <option key={o} value={o}>{uaOsLabels[o]}</option>)}</select></label>
                    <label>浏览器<select disabled={locked} value={ua.fixed_browser} onChange={e => { const browser = e.target.value as UaBrowser; patchUa({ fixed_browser: browser, fixed_version: uaVersions[browser][0] }); }}>{(Object.keys(uaBrowserLabels) as UaBrowser[]).map(b => <option key={b} value={b} disabled={b === 'safari' && ua.fixed_os !== 'macos'}>{uaBrowserLabels[b]}</option>)}</select></label>
                  </div>
                  <label>版本号<select disabled={locked} value={ua.fixed_version} onChange={e => patchUa({ fixed_version: e.target.value })}>{uaVersions[ua.fixed_browser].map(v => <option key={v} value={v}>{v}</option>)}</select></label>
                  <p className="ua-preview"><strong>生成的 UA：</strong><span>{buildFixedUa(ua.fixed_os, ua.fixed_browser, ua.fixed_version)}</span></p>
                </>}
                {ua.mode === 'rotate' && <>
                  <label>轮换列表（每行一个 UA）<textarea rows={5} disabled={locked} value={ua.rotate_list.join('\n')} onChange={e => patchUa({ rotate_list: e.target.value.split('\n') })} placeholder={'Mozilla/5.0 …\nMozilla/5.0 …'}/><small>最多 50 条；每次登录随机取一个，本次会话内保持不变。</small></label>
                  <p className="ua-preview"><strong>当前列表：</strong><span>{ua.rotate_list.length ? `共 ${ua.rotate_list.length} 条，每次登录随机取一条` : '列表为空'}</span></p>
                </>}
                {ua.mode === 'generate' && <>
                  <label>MT19937 种子<input type="number" disabled={locked} value={ua.generate_seed} onChange={e => patchUa({ generate_seed: Number(e.target.value) })} placeholder="114514"/><small>同一种子生成的 UA 序列固定；默认 114514。每次登录取序列里下一个真实的浏览器 UA，会话内保持不变。</small></label>
                  <p className="ua-preview"><strong>说明：</strong><span>由 MT19937（种子 {ua.generate_seed}）生成，会话内固定</span></p>
                </>}
                <p className="credential-hint"><ShieldCheck size={13}/>User-Agent 在登录与执行时生效；修改后建议重新登录以重建会话。</p>
              </div>
            </section>
          </div>
          <section className="panel info-panel">
            <ShieldCheck size={24}/>
            <h2>清晰的执行边界</h2>
            <p>学校明确拒绝的请求会显示拒绝原因，不会被记为成功。</p>
            <p>若提交超时或回复无法识别，任务会暂停，等待你核实真实选课结果。</p>
            <p>登录会按你排定的顺序依次尝试，失败的自动换下一种。</p>
            <p>本版暂不包含自动换班、排课、邮件通知与跨年级选课。</p>
            <div className="info-version">Rust 本地服务 + React 浏览器界面<br/>Browser Edition 0.2.0</div>
          </section>
        </div>}
        <footer className="page-footer"><span><span className={`dot ${snapshot.running ? 'green' : ''}`}/>{snapshot.running ? '任务正在运行' : '工作台就绪'}</span><span>{saved ? '设置已保存' : '有未保存的修改'}<span className="footer-divider">/</span>HDU-KillCourse NEXT</span></footer>
      </main>
    </div>

    {loginOpen && <Modal title={snapshot.logged_in ? '教务账号已连接' : '连接教务系统'} onClose={() => { if (!busy) closeLogin(); }}>
      <p className="modal-intro">登录用于获取课程和执行任务；成功使用的账号密码会按你的选择保存在本机。</p>
      {snapshot.logged_in ? <><div className="connected"><ShieldCheck size={30}/><strong>会话已通过学生信息校验</strong></div><button className="button secondary full" disabled={locked} onClick={() => perform('退出中', async () => { await api.logout(); setSnapshot(s => ({ ...s, logged_in: false })); toast('已退出登录'); })}><LogOut size={15}/>退出当前账号</button></> : <form onSubmit={e => { e.preventDefault(); void perform('登录中', async () => { await api.login(auth, settings); await rememberCreds(auth); finishLogin(); }); }}>
        <div className="method-cards" role="radiogroup" aria-label="登录方式">
          {normalizeOrder(creds.order).map(m => {
            const icon = m === 'cas' ? <KeyRound size={16}/> : m === 'newjw' ? <ShieldCheck size={16}/> : m === 'cookie' ? <Cookie size={16}/> : <QrCode size={16}/>;
            const desc = m === 'cas' ? 'CAS 账号密码' : m === 'newjw' ? '教务系统账号密码' : m === 'cookie' ? 'JSESSIONID + route' : '手机钉钉扫一扫登录';
            return <button type="button" key={m} className={auth.method === m ? 'active' : ''} onClick={() => selectMethod(m)}>{icon}{loginMethodLabels[m]}<small>{desc}</small></button>;
          })}
        </div>
        {auth.method === 'qrcode' || qr ? (
          <div className="qr-panel">
            <div className={`qr-frame${qr?.status === 'expired' ? ' qr-frame-expired' : ''}`}>
              {qr?.image ? <img src={`data:image/png;base64,${qr.image}`} alt="钉钉登录二维码"/> : <div className="qr-placeholder"><LoaderCircle className="spin" size={26}/></div>}
              {qr?.status === 'expired' && <div className="qr-overlay">二维码已过期</div>}
            </div>
            <p className={`qr-status${qr?.status === 'error' ? ' qr-status-error' : ''}`}>{qr?.status === 'waiting' && <span className="qr-pulse"/>}{qr?.message || '正在获取二维码…'}</p>
            {qrAutoMode ? <button type="button" className="button secondary full" onClick={cancelQr}><ArrowRight size={15}/>跳过此方式，尝试下一种</button> : qr?.status === 'expired' || qr?.status === 'error' ? <button type="button" className="button secondary full" onClick={() => void startQr()}><RefreshCw size={15}/>重新获取二维码</button> : null}
            <p className="qr-hint">使用手机钉钉扫一扫，完成学校统一身份认证</p>
          </div>
        ) : auth.method === 'cookie' ? <><label>JSESSIONID<input required type="password" autoComplete="off" value={auth.session_id} disabled={!!busy} onChange={e => setAuth({ ...auth, session_id: e.target.value })}/></label><label>route<input required type="password" autoComplete="off" value={auth.route} disabled={!!busy} onChange={e => setAuth({ ...auth, route: e.target.value })}/></label></> : <><label>账号<input required placeholder="输入学号" autoComplete="username" value={auth.username} disabled={!!busy} onChange={e => setAuth({ ...auth, username: e.target.value })}/></label><label>密码<input required type="password" autoComplete="current-password" value={auth.password} disabled={!!busy} onChange={e => setAuth({ ...auth, password: e.target.value })}/></label></>}
        {auth.method !== 'qrcode' && !qr && <button type="submit" className="button primary full" disabled={!!busy || offline}>{busy ? <LoaderCircle size={15} className="spin"/> : <ShieldCheck size={15}/>}{busy === '登录中' ? '正在连接…' : '用此方式连接'}</button>}
        <div className="auto-divider"><span>或</span></div>
        <button type="button" className="button secondary full" disabled={!!busy || offline} onClick={() => void autoLogin()}><Zap size={15}/>{busy === '自动连接中' ? '正在按顺序尝试…' : '按偏好顺序自动尝试'}</button>
        {autoNote && <p className="auto-note">{busy === '自动连接中' && <LoaderCircle size={12} className="spin"/>}{autoNote}</p>}
      </form>}
    </Modal>}

    {dropPickerFor !== null && <Modal title="添加要先退的课" onClose={() => setDropPickerFor(null)}>
      <p className="modal-intro">从课程资料库挑选已选的课程（同类课或时间冲突课）；执行时会先退掉它们再选新课。</p>
      <div className="search-field"><Search size={15}/><input aria-label="搜索要退的课" autoFocus placeholder="搜索课程 / 教学班…" value={dropQuery} onChange={e => setDropQuery(e.target.value)}/>{dropQuery && <button aria-label="清空搜索" onClick={() => setDropQuery('')}><X size={14}/></button>}</div>
      <div className="drop-picker-list">{(() => { const q = dropQuery.trim().toLowerCase().split(/\s+/).filter(Boolean); const pool = termCourses.filter(c => !selected.has(c.jxbmc) && q.every(x => `${c.kcmc} ${c.jxbmc} ${c.jzgxx}`.toLowerCase().includes(x))).slice(0, 60); return pool.length ? pool.map(c => <button key={c.jxbmc} className="drop-picker-row" onClick={() => { addDrop(dropPickerFor, c); setDropPickerFor(null); }}><span className="badge rejected">退</span><span><strong>{c.kcmc}</strong><small>{c.jxbmc}</small></span><Plus size={15}/></button>) : <div className="empty" style={{ padding: '24px 0' }}>没有找到可选课程</div>; })()}</div>
    </Modal>}

    {detail && <Modal title={detail.kcmc || '课程详情'} onClose={() => setDetail(null)}>
      <div className="detail-id">{detail.jxbmc}</div>
      <dl className="details"><dt>课程类型</dt><dd>{detail.kklxmc || '未提供'}</dd><dt>上课时间</dt><dd>{detail.sksj || '待定'}</dd><dt>授课教师</dt><dd>{detail.jzgxx || '缓存中未提供'}</dd><dt>上课地点</dt><dd>{detail.jxdd || '缓存中未提供'}</dd><dt>面向班级</dt><dd>{detail.jxbzc || '未提供'}</dd></dl>
      <button className="button primary full" disabled={locked || selected.has(detail.jxbmc)} onClick={() => { add(detail); setDetail(null); }}>{selected.has(detail.jxbmc) ? '已在任务清单中' : '加入选课任务'}<Plus size={15}/></button>
    </Modal>}

    {reviewOpen && <Modal title="确认本轮任务" onClose={() => !busy && setReviewOpen(false)}>
      <label>要执行的清单<select disabled={!!busy} value={reviewList} onChange={e => setReviewList(Number(e.target.value))}>{settings.lists.map((l, i) => <option key={i} value={i}>{l.name}（{l.tasks.length} 门 · {l.mode === 'watch' ? '蹲课' : '单次'}）</option>)}</select></label>
      {(() => { const rl = settings.lists[reviewList] ?? settings.lists[0]; const hasDrops = rl.tasks.some(t => t.drops.length > 0); return <>
        <p className="modal-intro">{rl.mode === 'watch' ? '有余量时提交选课请求。' : '每个任务先退掉所列课程，成功后再提交选课。'}以下操作会发送到学校教务系统。</p>
        <div className="review-list">{rl.tasks.map((t, i) => <div key={i}><span className={`badge ${t.course ? 'querying' : 'rejected'}`}>{t.course ? '选课' : '仅退课'}</span><span>{t.course ? t.course.kcmc : ''}{t.drops.length > 0 && <small className="review-drops">先退：{t.drops.map(d => d.kcmc).join('、')}</small>}{t.course && <small>{t.course.jxbmc}</small>}</span></div>)}</div>
        <p className="review-note">{settings.start_at ? `计划时间：${settings.start_at.replace('T', ' ')}（UTC+8）${settings.relogin_before_secs > 0 ? `，将提前 ${formatDurationSecs(settings.relogin_before_secs)}按你的顺序重新登录` : ''}` : '立即开始'}{hasDrops && '。退课后不能保证重新选回，请确认教学班。'}</p>
      </>; })()}
      <button className="button primary full" disabled={!!busy || offline} onClick={() => perform('启动中', async () => { await save(); await api.startTasks(settings, reviewList); setReviewOpen(false); setView('activity'); setSnapshot(await api.snapshot()); toast('任务已启动'); })}>{busy ? <LoaderCircle className="spin" size={15}/> : <Radio size={15}/>}确认并开始</button>
    </Modal>}

    {shutdown && <div className="shutdown-screen"><div><CheckCircle2 size={40}/><h1>本地服务已关闭</h1><p>你可以关闭这个页面了。下次使用时重新启动 HDU-KillCourse NEXT 即可。</p></div></div>}
  </div>;
}

function Stat({ icon, label, value, note }: { icon: React.ReactNode; label: string; value: string; note: string }) {
  return <div className="stat"><span className="stat-icon">{icon}</span><div><span className="stat-top">{label}</span><strong>{value}</strong><small>{note}</small></div></div>;
}
function IntervalInput({ ms, disabled, onChange }: { ms: number; disabled: boolean; onChange: (ms: number) => void }) {
  const [unit, setUnit] = useState<IntervalUnit>(() => splitInterval(ms).unit);
  const value = ms / intervalUnitMs[unit];
  return <span className="interval-input">
    <input type="number" min="0" step="any" disabled={disabled}
      value={Number.isInteger(value) ? value : +value.toFixed(3)}
      onChange={e => { const v = Number(e.target.value); if (Number.isFinite(v)) onChange(Math.max(0, Math.round(v * intervalUnitMs[unit]))); }} />
    <select disabled={disabled} value={unit} onChange={e => setUnit(e.target.value as IntervalUnit)}>
      {(Object.keys(intervalUnitLabels) as IntervalUnit[]).map(u => <option key={u} value={u}>{intervalUnitLabels[u]}</option>)}
    </select>
  </span>;
}
function Empty({ icon, title, text, children }: { icon: React.ReactNode; title: string; text: string; children?: React.ReactNode }) {
  return <div className="empty"><span className="empty-icon">{icon}</span><h3>{title}</h3><p>{text}</p>{children}</div>;
}
function Modal({ title, onClose, children }: { title: string; onClose: () => void; children: React.ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const root = ref.current!;
    root.querySelector<HTMLElement>('button,input,select')?.focus();
    const trap = (e: KeyboardEvent) => {
      if (e.key !== 'Tab') return;
      const nodes = [...root.querySelectorAll<HTMLElement>('button:not(:disabled),input:not(:disabled),select:not(:disabled),[tabindex="0"]')];
      const first = nodes[0], last = nodes[nodes.length - 1];
      if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last?.focus(); }
      else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first?.focus(); }
    };
    root.addEventListener('keydown', trap); return () => { root.removeEventListener('keydown', trap); previous?.focus(); };
  }, []);
  return <div className="modal-overlay" onMouseDown={e => e.target === e.currentTarget && onClose()}><div ref={ref} className="modal" role="dialog" aria-modal="true" aria-label={title}><div className="modal-heading"><h2>{title}</h2><button aria-label="关闭对话框" onClick={onClose}><X size={18}/></button></div>{children}</div></div>;
}
