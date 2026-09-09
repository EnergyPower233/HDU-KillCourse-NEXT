import type { Settings } from '../types';

export const defaults: Settings = {
  year: 2026,
  term: 1,
  interval_ms: 60000,
  jitter_ms: 0,
  jitter_seed: 1919810,
  start_at: '',
  relogin_before_secs: 0,
  lists: [{ name: '默认清单', mode: 'watch', tasks: [] }],
  active_list: 0,
};
