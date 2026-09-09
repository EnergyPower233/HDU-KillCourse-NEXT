import { Plus } from 'lucide-react';
import type * as React from 'react';
import { Modal } from '../components/ui';
import type { useTaskLists } from '../hooks/useTaskLists';
import { type Course } from '../types';
interface Props {
  taskLists: ReturnType<typeof useTaskLists>;

  detail: Course;
  setDetail: React.Dispatch<React.SetStateAction<Course | null>>;
  locked: boolean;
}
export function CourseDialog({ detail, setDetail, locked, taskLists }: Props) {
  const { selected, add } = taskLists;
  return (
    <Modal title={detail.kcmc || '课程详情'} onClose={() => setDetail(null)}>
      <div className="detail-id">{detail.jxbmc}</div>
      <dl className="details">
        <dt>课程类型</dt>
        <dd>{detail.kklxmc || '未提供'}</dd>
        <dt>上课时间</dt>
        <dd>{detail.sksj || '待定'}</dd>
        <dt>授课教师</dt>
        <dd>{detail.jzgxx || '缓存中未提供'}</dd>
        <dt>上课地点</dt>
        <dd>{detail.jxdd || '缓存中未提供'}</dd>
        <dt>面向班级</dt>
        <dd>{detail.jxbzc || '未提供'}</dd>
      </dl>
      <button
        className="button primary full"
        disabled={locked || selected.has(detail.jxbmc)}
        onClick={() => {
          add(detail);
          setDetail(null);
        }}
      >
        {selected.has(detail.jxbmc) ? '已在任务清单中' : '加入选课任务'}
        <Plus size={15} />
      </button>
    </Modal>
  );
}
