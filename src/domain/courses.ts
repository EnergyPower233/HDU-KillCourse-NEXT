import type { Course, Settings } from '../types';

export function filterCourses(
  courses: Course[],
  settings: Settings,
  query: string,
  kind: string,
): Course[] {
  const prefix = `(${settings.year}-${settings.year + 1}-${settings.term})-`;
  const terms = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
  return courses.filter(
    (c) =>
      c.jxbmc.startsWith(prefix) &&
      (!kind || c.kklxmc === kind) &&
      terms.every((q) => `${c.kcmc} ${c.jxbmc} ${c.jzgxx} ${c.sksj}`.toLowerCase().includes(q)),
  );
}
