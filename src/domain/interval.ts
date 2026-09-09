import type { IntervalUnit } from '../types';

export const intervalUnitMs: Record<IntervalUnit, number> = {
  ms: 1,
  s: 1000,
  min: 60000,
  h: 3600000,
};

export const intervalUnitLabels: Record<IntervalUnit, string> = {
  ms: '毫秒',
  s: '秒',
  min: '分钟',
  h: '小时',
};

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
