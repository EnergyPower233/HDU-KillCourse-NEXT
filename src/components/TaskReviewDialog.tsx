import { LoaderCircle, Radio } from 'lucide-react';
import type * as React from 'react';
import { useApi } from '../AccountContext';
import { Modal } from '../components/ui';
import { formatDurationSecs } from '../domain/interval';
import type { View } from '../navigation';
import { type Settings, type Snapshot } from '../types';
interface Props {
  busy: string;
  setReviewOpen: React.Dispatch<React.SetStateAction<boolean>>;
  reviewList: number;
  setReviewList: React.Dispatch<React.SetStateAction<number>>;
  settings: Settings;
  offline: boolean;
  perform: (label: string, fn: () => Promise<void>) => Promise<void>;
  save: () => Promise<void>;
  setView: React.Dispatch<React.SetStateAction<View>>;
  setSnapshot: React.Dispatch<React.SetStateAction<Snapshot>>;
  toast: (text: string, error?: boolean) => void;
}
export function TaskReviewDialog({
  busy,
  setReviewOpen,
  reviewList,
  setReviewList,
  settings,
  offline,
  perform,
  save,
  setView,
  setSnapshot,
  toast,
}: Props) {
  const api = useApi();
  return (
    <Modal title="确认本轮任务" onClose={() => !busy && setReviewOpen(false)}>
      <label>
        要执行的清单
        <select
          disabled={!!busy}
          value={reviewList}
          onChange={(e) => setReviewList(Number(e.target.value))}
        >
          {settings.lists.map((l, i) => (
            <option key={i} value={i}>
              {l.name}（{l.tasks.length} 门 · {l.mode === 'watch' ? '蹲课' : '单次'}）
            </option>
          ))}
        </select>
      </label>
      {(() => {
        const rl = settings.lists[reviewList] ?? settings.lists[0];
        const hasDrops = rl.tasks.some((t) => t.drops.length > 0);
        return (
          <>
            <p className="modal-intro">
              {rl.mode === 'watch'
                ? '有余量时提交选课请求。'
                : '每个任务先退掉所列课程，成功后再提交选课。'}
              以下操作会发送到学校教务系统。
            </p>
            <div className="review-list">
              {rl.tasks.map((t, i) => (
                <div key={i}>
                  <span className={`badge ${t.course ? 'querying' : 'rejected'}`}>
                    {t.course ? '选课' : '仅退课'}
                  </span>
                  <span>
                    {t.course ? t.course.kcmc : ''}
                    {t.drops.length > 0 && (
                      <small className="review-drops">
                        先退：{t.drops.map((d) => d.kcmc).join('、')}
                      </small>
                    )}
                    {t.course && <small>{t.course.jxbmc}</small>}
                  </span>
                </div>
              ))}
            </div>
            <p className="review-note">
              {settings.start_at
                ? `计划时间：${settings.start_at.replace('T', ' ')}（UTC+8）${settings.relogin_before_secs > 0 ? `，将提前 ${formatDurationSecs(settings.relogin_before_secs)}按你的顺序重新登录` : ''}`
                : '立即开始'}
              {hasDrops && '。退课后不能保证重新选回，请确认教学班。'}
            </p>
          </>
        );
      })()}
      <button
        className="button primary full"
        disabled={!!busy || offline}
        onClick={() =>
          perform('启动中', async () => {
            await save();
            await api.startTasks(settings, reviewList);
            setReviewOpen(false);
            setView('activity');
            setSnapshot(await api.snapshot());
            toast('任务已启动');
          })
        }
      >
        {busy ? <LoaderCircle className="spin" size={15} /> : <Radio size={15} />}确认并开始
      </button>
    </Modal>
  );
}
