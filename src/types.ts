export type Course = { jxbmc: string; kch_id: string; jxb_id: string; kcmc: string; kklxmc: string; sksj: string; jxbzc: string; jzgxx: string; jxdd: string };
/** A task: optionally select `course` after first dropping every course in
 * `drops` (manually chosen by the user). `course` null + drops = pure drop. */
export type CourseTask = { course: Course | null; drops: Course[] };
export type TaskList = { name: string; mode: 'once' | 'watch'; tasks: CourseTask[] };
export type Settings = {
  year: number;
  term: number;
  interval_ms: number;
  jitter_ms: number;
  jitter_seed: number;
  start_at: string;
  relogin_before_secs: number;
  lists: TaskList[];
  active_list: number;
};
export type Progress = { course_id: string; status: string; message: string; time: string };
export type Snapshot = { running: boolean; logged_in: boolean; history: Progress[]; fetch_progress: { page: number; courses: number; total: number | null; finished: boolean } | null };
export type Credentials = { method: string; username: string; password: string; session_id: string; route: string };
export type QrPoll = { status: 'waiting' | 'expired' | 'confirmed'; message: string };
export type LoginMethod = 'cas' | 'newjw' | 'qrcode' | 'cookie';

// Query interval units. `interval_ms` is the canonical stored value; the UI
// lets the user edit it with a value + unit pair.
export type IntervalUnit = 'ms' | 's' | 'min' | 'h';
export const intervalUnitMs: Record<IntervalUnit, number> = { ms: 1, s: 1000, min: 60000, h: 3600000 };
export const intervalUnitLabels: Record<IntervalUnit, string> = { ms: '毫秒', s: '秒', min: '分钟', h: '小时' };
export function splitInterval(ms: number): { value: number; unit: IntervalUnit } {
  if (ms > 0 && ms % 3600000 === 0) return { value: ms / 3600000, unit: 'h' };
  if (ms > 0 && ms % 60000 === 0) return { value: ms / 60000, unit: 'min' };
  if (ms > 0 && ms % 1000 === 0) return { value: ms / 1000, unit: 's' };
  return { value: ms, unit: 'ms' };
}
export function formatInterval(ms: number): string {
  const { value, unit } = splitInterval(ms);
  return `${value} ${intervalUnitLabels[unit]}`;
}
export function formatDurationSecs(secs: number): string {
  if (secs >= 60 && secs % 60 === 0) return `${secs / 60} 分钟`;
  return `${secs} 秒`;
}

// User-Agent customization (anti-script detection).
export type UaMode = 'browser' | 'fixed' | 'rotate' | 'generate';
export type UaOs = 'windows' | 'macos' | 'linux';
export type UaBrowser = 'chrome' | 'edge' | 'firefox' | 'opera' | 'safari';
export type UaConfig = {
  mode: UaMode;
  browser_ua: string;
  fixed_os: UaOs;
  fixed_browser: UaBrowser;
  fixed_version: string;
  rotate_list: string[];
  generate_seed: number;
};
export const defaultUaConfig: UaConfig = {
  mode: 'browser', browser_ua: '', fixed_os: 'windows', fixed_browser: 'chrome',
  fixed_version: '143.0.7467.120', rotate_list: [], generate_seed: 114514,
};
export const uaOsLabels: Record<UaOs, string> = { windows: 'Windows', macos: 'macOS', linux: 'Linux' };
export const uaBrowserLabels: Record<UaBrowser, string> = { chrome: 'Chrome', edge: 'Edge', firefox: 'Firefox', opera: 'Opera', safari: 'Safari' };
export const uaVersions: Record<UaBrowser, string[]> = {
  chrome: ['143.0.7467.120', '142.0.7444.110', '141.0.7218.87', '140.0.7339.208', '139.0.7258.114'],
  edge: ['143.0.3270.55', '142.0.3236.48', '141.0.3175.102', '140.0.3124.54'],
  firefox: ['143.0', '142.0', '141.0', '140.0', '139.0', '138.0'],
  opera: ['116.0.5366.45', '115.0.5322.109', '114.0.5282.115', '113.0.5230.132'],
  safari: ['18.5', '18.4', '17.6', '17.5', '16.6', '16.5'],
};
export function buildFixedUa(os: UaOs, browser: UaBrowser, version: string): string {
  const platform = (firefox: boolean) => {
    if (firefox) {
      const base = os === 'macos' ? 'Macintosh; Intel Mac OS X 10.15' : os === 'linux' ? 'X11; Linux x86_64' : 'Windows NT 10.0; Win64; x64';
      return `${base}; rv:${version}`;
    }
    if (os === 'macos') return 'Macintosh; Intel Mac OS X 10_15_7';
    return os === 'linux' ? 'X11; Linux x86_64' : 'Windows NT 10.0; Win64; x64';
  };
  switch (browser) {
    case 'firefox': return `Mozilla/5.0 (${platform(true)}) Gecko/20100101 Firefox/${version}`;
    case 'safari': return `Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/${version} Safari/605.1.15`;
    case 'edge': return `Mozilla/5.0 (${platform(false)}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/${version} Safari/537.36 Edg/${version}`;
    case 'opera': return `Mozilla/5.0 (${platform(false)}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.6723.124 Safari/537.36 OPR/${version}`;
    default: return `Mozilla/5.0 (${platform(false)}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/${version} Safari/537.36`;
  }
}
export type StoredCredentials = {
  cas_username: string;
  cas_password: string;
  newjw_username: string;
  newjw_password: string;
  session_id: string;
  route: string;
  order: LoginMethod[];
};
export const loginMethodLabels: Record<LoginMethod, string> = { cas: '统一身份认证', newjw: '教务账号', qrcode: '钉钉扫码', cookie: '已有 Cookie' };
export const defaultStoredCredentials: StoredCredentials = {
  cas_username: '', cas_password: '', newjw_username: '', newjw_password: '',
  session_id: '', route: '', order: ['cas', 'newjw', 'qrcode', 'cookie'],
};
/** Keep the order list valid: drop duplicates/unknowns, append missing methods. */
export function normalizeOrder(order: LoginMethod[]): LoginMethod[] {
  const all: LoginMethod[] = ['cas', 'newjw', 'qrcode', 'cookie'];
  const out = [...new Set(order)].filter(m => all.includes(m));
  for (const m of all) if (!out.includes(m)) out.push(m);
  return out;
}
/** Request-shaped credentials for a method, or null when not fully saved (QR is interactive). */
export function credentialsFor(method: LoginMethod, creds: StoredCredentials): Credentials | null {
  if (method === 'cas' && creds.cas_username && creds.cas_password) return { method: 'cas', username: creds.cas_username, password: creds.cas_password, session_id: '', route: '' };
  if (method === 'newjw' && creds.newjw_username && creds.newjw_password) return { method: 'newjw', username: creds.newjw_username, password: creds.newjw_password, session_id: '', route: '' };
  if (method === 'cookie' && creds.session_id && creds.route) return { method: 'cookie', username: '', password: '', session_id: creds.session_id, route: creds.route };
  return null;
}
/** Merge credentials that just logged in successfully back into storage. */
export function mergeCreds(creds: StoredCredentials, auth: Credentials): StoredCredentials {
  const c = { ...creds };
  if (auth.method === 'cas') { c.cas_username = auth.username; c.cas_password = auth.password; }
  else if (auth.method === 'newjw') { c.newjw_username = auth.username; c.newjw_password = auth.password; }
  else if (auth.method === 'cookie') { c.session_id = auth.session_id; c.route = auth.route; }
  return c;
}
export const defaults: Settings = {
  year: 2026, term: 1, interval_ms: 60000, jitter_ms: 0, jitter_seed: 1919810,
  start_at: '', relogin_before_secs: 0,
  lists: [{ name: '默认清单', mode: 'watch', tasks: [] }], active_list: 0,
};
export const statuses: Record<string, string> = { waiting: '等待中', querying: '查询中', submitting: '提交中', success: '学校返回成功', rejected: '学校拒绝', unknown: '结果待核实', error: '异常', running: '运行中', finished: '已结束' };
export function filterCourses(courses: Course[], settings: Settings, query: string, kind: string): Course[] {
  const prefix = `(${settings.year}-${settings.year + 1}-${settings.term})-`;
  const terms = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
  return courses.filter(c => c.jxbmc.startsWith(prefix) && (!kind || c.kklxmc === kind) && terms.every(q => `${c.kcmc} ${c.jxbmc} ${c.jzgxx} ${c.sksj}`.toLowerCase().includes(q)));
}
export function parseCourseFile(text: string): Course[] {
  const data = JSON.parse(text);
  const rows: unknown = Array.isArray(data) ? data : data.items;
  if (!Array.isArray(rows) || !rows.length) throw new Error('文件中没有课程资料');
  const normalized = rows.map((row: unknown) => {
    if (!row || typeof row !== 'object') throw new Error('课程格式错误');
    const v = row as Record<string, unknown>;
    if (!['jxbmc', 'jxb_id', 'kch_id'].every(k => typeof v[k] === 'string' && v[k])) throw new Error('课程缺少教学班名称或内部编号');
    return Object.fromEntries(['jxbmc','jxb_id','kch_id','kcmc','kklxmc','sksj','jxbzc','jzgxx','jxdd'].map(k => [k, typeof v[k] === 'string' ? v[k] : ''])) as Course;
  });
  const unique = new Map<string, Course>();
  for (const course of normalized) {
    const previous = unique.get(course.jxbmc);
    if (!previous) { unique.set(course.jxbmc, course); continue; }
    if (previous.jxb_id !== course.jxb_id || previous.kch_id !== course.kch_id) throw new Error(`同一教学班存在不同内部编号：${course.jxbmc}`);
    for (const key of ['sksj','jxbzc','jzgxx','jxdd'] as const) previous[key] = [...new Set(`${previous[key]};${course[key]}`.split(';').filter(Boolean))].join(';');
  }
  return [...unique.values()];
}
