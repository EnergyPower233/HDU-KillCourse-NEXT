import type { ReactNode } from 'react';
import type { View } from '../navigation';
const instructions: Record<View, { title: string; description: string }> = {
  courses: {
    title: '课程中心',
    description: '获取或导入课程资料，按课程名、教师或教学班编号搜索，点击 + 加入当前清单。',
  },
  tasks: {
    title: '选课任务',
    description: '编辑或导入任务清单，指定先退课程与执行顺序；保存后点击开始任务，核对计划再执行。',
  },
  activity: {
    title: '运行记录',
    description: '选择运行批次查看选退课结果；点击「加载更早的 500 条」查看历史记录。',
  },
  settings: {
    title: '偏好设置',
    description: '设置学年学期、查询间隔与登录顺序，管理本机保存的凭据和请求标识。',
  },
};
export function PageHeader({ view, children }: { view: View; children?: ReactNode }) {
  const { title, description } = instructions[view];
  return (
    <div className="page-heading">
      <div>
        <h1>{title}</h1>
        <p>{description}</p>
      </div>
      <div className="heading-actions">{children}</div>
    </div>
  );
}
