import type { Progress } from '../types';

export const statuses: Record<string, string> = {
  waiting: '等待中',
  querying: '查询中',
  submitting: '提交中',
  success: '学校返回成功',
  rejected: '学校拒绝',
  failed: '失败',
  unknown: '结果待核实',
  error: '异常',
  running: '运行中',
  finished: '已结束',
};

export function progressLabel(event: Progress): string {
  const status =
    event.status === 'success'
      ? '成功'
      : event.status === 'rejected'
        ? '被拒绝'
        : statuses[event.status] || event.status;
  if (event.action === 'select') return `选课${status}`;
  if (event.action === 'cancel') return `退课${status}`;
  return event.course_id ? `操作未记录 · ${status}` : statuses[event.status] || event.status;
}
