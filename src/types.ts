export type Course = {
  jxbmc: string;
  kch_id: string;
  jxb_id: string;
  kcmc: string;
  kklxmc: string;
  sksj: string;
  jxbzc: string;
  jzgxx: string;
  jxdd: string;
};

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

export type Progress = {
  course_name?: string;
  schedule?: string;
  action?: 'select' | 'cancel' | null;
  course_id: string;
  status: string;
  message: string;
  time: string;
};

export type RunInfo = { id: string; list_name: string; started_at: string };

export type RunPage = {
  run: RunInfo;
  events: Progress[];
  next_before: number | null;
  warning: string | null;
};

export type Snapshot = {
  current_run?: RunInfo | null;
  log_error?: string | null;
  running: boolean;
  logged_in: boolean;
  history: Progress[];
  fetch_progress: { page: number; courses: number; total: number | null } | null;
};

export type Credentials = {
  method: string;
  username: string;
  password: string;
  session_id: string;
  route: string;
};

export type QrPoll = { status: 'waiting' | 'expired' | 'confirmed'; message: string };

export type LoginMethod = 'cas' | 'newjw' | 'qrcode' | 'cookie';

// Query interval units. `interval_ms` is the canonical stored value; the UI
// lets the user edit it with a value + unit pair.
export type IntervalUnit = 'ms' | 's' | 'min' | 'h';

// Request identity settings. Changing a UA does not guarantee avoiding detection.
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

export type StoredCredentials = {
  cas_username: string;
  cas_password: string;
  newjw_username: string;
  newjw_password: string;
  session_id: string;
  route: string;
  order: LoginMethod[];
};
