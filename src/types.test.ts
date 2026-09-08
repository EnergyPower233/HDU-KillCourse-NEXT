import { describe, expect, it } from 'vitest';
import { credentialsFor, defaults, defaultStoredCredentials, filterCourses, formatDurationSecs, formatInterval, mergeCreds, normalizeOrder, parseCourseFile, splitInterval } from './types';
const rows = parseCourseFile(JSON.stringify({ items: [
  { jxbmc: '(2026-2027-1)-A-01', jxb_id: '1', kch_id: 'A', kcmc: '数据结构', sksj: '星期一', kklxmc: '主修课程' },
  { jxbmc: '(2025-2026-1)-A-01', jxb_id: '2', kch_id: 'A', kcmc: '数据结构' }
]}));
describe('legacy course import and search', () => {
  it('merges duplicate teaching rows while preserving all sessions', () => {
    const c = rows[0];
    const merged = parseCourseFile(JSON.stringify([c, {...c,sksj:'星期三'}]));
    expect(merged).toHaveLength(1);
    expect(merged[0].sksj).toBe('星期一;星期三');
    expect(() => parseCourseFile(JSON.stringify([c,{...c,jxb_id:'different'}]))).toThrow();
  });
  it('isolates terms and supports multi-keyword search', () => {
    expect(filterCourses(rows, defaults, '数据 星期一', '主修课程')).toHaveLength(1);
    expect(filterCourses(rows, defaults, '数据 星期二', '')).toHaveLength(0);
    expect(filterCourses(rows, defaults, '', '')).toHaveLength(1);
  });
  it('refuses incomplete data instead of creating unexecutable tasks', () => {
    expect(() => parseCourseFile('{"items":[{"kcmc":"test"}]}')).toThrow();
    expect(() => parseCourseFile('{}')).toThrow();
  });
  it('normalizes login order and maps saved credentials', () => {
    expect(normalizeOrder(['cookie', 'cas', 'cas'])).toEqual(['cookie', 'cas', 'newjw', 'qrcode']);
    expect(normalizeOrder([])).toEqual(['cas', 'newjw', 'qrcode', 'cookie']);
    const creds = { ...defaultStoredCredentials, cas_username: 'u', cas_password: 'p' };
    expect(credentialsFor('cas', creds)?.username).toBe('u');
    expect(credentialsFor('newjw', creds)).toBeNull();
    expect(credentialsFor('qrcode', creds)).toBeNull();
    expect(credentialsFor('cookie', creds)).toBeNull();
    const merged = mergeCreds(defaultStoredCredentials, { method: 'cookie', username: '', password: '', session_id: 's', route: 'r' });
    expect(merged.session_id).toBe('s');
    expect(merged.route).toBe('r');
    expect(merged.cas_username).toBe('');
  });
  it('splits and formats the query interval and durations', () => {
    expect(splitInterval(60000)).toEqual({ value: 1, unit: 'min' });
    expect(splitInterval(3600000)).toEqual({ value: 1, unit: 'h' });
    expect(splitInterval(1000)).toEqual({ value: 1, unit: 's' });
    expect(splitInterval(500)).toEqual({ value: 500, unit: 'ms' });
    expect(formatInterval(120000)).toBe('2 分钟');
    expect(formatInterval(500)).toBe('500 毫秒');
    expect(formatDurationSecs(120)).toBe('2 分钟');
    expect(formatDurationSecs(30)).toBe('30 秒');
  });
});
