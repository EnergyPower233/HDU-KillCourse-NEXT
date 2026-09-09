import { Plus, Search, X } from 'lucide-react';
import { Modal } from '../components/ui';
import type { useTaskLists } from '../hooks/useTaskLists';
import { type Course } from '../types';
interface Props {
  taskLists: ReturnType<typeof useTaskLists>;

  termCourses: Course[];
}
export function DropPickerDialog({ termCourses, taskLists }: Props) {
  const { setDropPickerFor, dropQuery, setDropQuery, selected, addDrop, dropPickerFor } = taskLists;
  if (dropPickerFor === null) return null;
  return (
    <Modal title="添加要先退的课" onClose={() => setDropPickerFor(null)}>
      <p className="modal-intro">
        从课程资料库挑选已选的课程（同类课或时间冲突课）；执行时会先退掉它们再选新课。
      </p>
      <div className="search-field">
        <Search size={15} />
        <input
          aria-label="搜索要退的课"
          autoFocus
          placeholder="搜索课程 / 教学班…"
          value={dropQuery}
          onChange={(e) => setDropQuery(e.target.value)}
        />
        {dropQuery && (
          <button aria-label="清空搜索" onClick={() => setDropQuery('')}>
            <X size={14} />
          </button>
        )}
      </div>
      <div className="drop-picker-list">
        {(() => {
          const q = dropQuery.trim().toLowerCase().split(/\s+/).filter(Boolean);
          const pool = termCourses
            .filter(
              (c) =>
                !selected.has(c.jxbmc) &&
                q.every((x) => `${c.kcmc} ${c.jxbmc} ${c.jzgxx}`.toLowerCase().includes(x)),
            )
            .slice(0, 60);
          return pool.length ? (
            pool.map((c) => (
              <button
                key={c.jxbmc}
                className="drop-picker-row"
                onClick={() => {
                  addDrop(dropPickerFor, c);
                  setDropPickerFor(null);
                }}
              >
                <span className="badge rejected">退</span>
                <span>
                  <strong>{c.kcmc}</strong>
                  <small>{c.jxbmc}</small>
                </span>
                <Plus size={15} />
              </button>
            ))
          ) : (
            <div className="empty" style={{ padding: '24px 0' }}>
              没有找到可选课程
            </div>
          );
        })()}
      </div>
    </Modal>
  );
}
