import { useState } from 'react';
import { accountsApi, createApi } from '../bridge';
import type { AccountProfile, AccountSummary, Settings } from '../types';
import { Modal } from './ui';

interface Props {
  accounts: AccountSummary[];
  current: AccountProfile;
  onClose: () => void;
  onSelect: (id: string) => Promise<void>;
  refresh: () => Promise<void>;
  saveCurrent: () => Promise<void>;
}
type Plan = { account: AccountSummary; settings: Settings };

export function AccountsDialog({
  accounts,
  current,
  onClose,
  onSelect,
  refresh,
  saveCurrent,
}: Props) {
  const [name, setName] = useState('');
  const [copy, setCopy] = useState(false);
  const [selected, setSelected] = useState<string[]>([]);
  const [plans, setPlans] = useState<Plan[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const [rename, setRename] = useState(current.name);
  async function perform(action: () => Promise<void>) {
    setBusy(true);
    setMessage('');
    try {
      await action();
    } catch (e) {
      setMessage(String(e));
    } finally {
      setBusy(false);
    }
  }
  async function prepare() {
    await saveCurrent();
    const targets = accounts.filter((a) => selected.includes(a.id));
    if (!targets.length || targets.some((a) => a.running || !a.logged_in || a.authenticating))
      throw Error('请选择已登录且未运行的账号');
    const next = await Promise.all(
      targets.map(async (account) => ({
        account,
        settings: await createApi(account.id).loadSettings(),
      })),
    );
    for (const { account, settings } of next) {
      if (!settings.lists[settings.active_list]?.tasks.length)
        throw Error(`账号「${account.name}」的当前清单为空，请先配置任务`);
    }
    setPlans(next);
  }
  async function start() {
    if (!plans) return;
    const results = await Promise.allSettled(
      plans.map(({ account, settings }) =>
        createApi(account.id).startTasks(settings, settings.active_list),
      ),
    );
    const failed = results.flatMap((r, i) =>
      r.status === 'rejected' ? [`${plans[i].account.name}：${String(r.reason)}`] : [],
    );
    const count = results.filter((r) => r.status === 'fulfilled').length;
    setPlans(null);
    setSelected([]);
    const summary = `已启动 ${count} 个账号${failed.length ? `；未启动：${failed.join('；')}` : '，可切换账号查看各自日志'}`;
    setMessage(summary);
    try {
      await refresh();
    } catch (e) {
      setMessage(`${summary}；状态刷新失败：${String(e)}`);
    }
  }
  return (
    <Modal title="账号与并行任务" onClose={() => !busy && onClose()}>
      <p className="modal-intro">
        各账号独立登录、保存清单并运行任务。切换账号不会停止已经启动的任务；并行启动会执行各账号已保存的当前清单。
      </p>
      {message && (
        <div role="status" className="notice">
          {message}
        </div>
      )}
      {plans ? (
        <>
          <h3>确认并行启动</h3>
          {plans.map(({ account, settings }) => {
            const list = settings.lists[settings.active_list];
            return (
              <section className="account-plan" key={account.id}>
                <h4>
                  {account.name} · {list.name}
                </h4>
                <p>
                  {settings.year} 学年 · 第 {settings.term} 学期 ·{' '}
                  {list.mode === 'watch' ? '蹲课' : '单次选退课'} ·{' '}
                  {settings.start_at || '立即开始'}
                </p>
                <ul>
                  {list.tasks.map((task, i) => (
                    <li key={i}>
                      {task.drops.length > 0 && (
                        <div>
                          先退：
                          {task.drops.map((c) => `${c.kcmc}（${c.jxbmc} · ${c.sksj}）`).join('、')}
                        </div>
                      )}
                      {task.course && (
                        <div>
                          选课：{task.course.kcmc}（{task.course.jxbmc} · {task.course.sksj}）
                        </div>
                      )}
                    </li>
                  ))}
                </ul>
              </section>
            );
          })}
          <p className="review-note">
            确认后会向各账号的教务会话提交任务。先退后选不能保证选回；部分账号启动失败不会停止其他已启动账号。
          </p>
          <div className="account-actions">
            <button className="button secondary" disabled={busy} onClick={() => setPlans(null)}>
              返回修改
            </button>
            <button className="button primary" disabled={busy} onClick={() => void perform(start)}>
              确认并行启动 {plans.length} 个账号
            </button>
          </div>
        </>
      ) : (
        <>
          <div className="account-list">
            {accounts.map((account) => (
              <div className="account-row" key={account.id}>
                <label>
                  <input
                    type="checkbox"
                    aria-label={`并行选择 ${account.name}`}
                    checked={selected.includes(account.id)}
                    disabled={
                      busy || account.running || !account.logged_in || account.authenticating
                    }
                    onChange={(e) =>
                      setSelected((ids) =>
                        e.target.checked
                          ? [...ids, account.id]
                          : ids.filter((id) => id !== account.id),
                      )
                    }
                  />
                  <span>
                    {account.name}
                    <small>
                      {account.running
                        ? '运行中'
                        : account.authenticating
                          ? '登录中'
                          : account.logged_in
                            ? '已登录'
                            : '未登录'}
                      {account.log_error ? ` · 日志错误：${account.log_error}` : ''}
                    </small>
                  </span>
                </label>
                <button
                  className="button secondary"
                  disabled={busy}
                  onClick={() =>
                    void perform(async () => {
                      await onSelect(account.id);
                      onClose();
                    })
                  }
                >
                  打开 {account.name}
                </button>
                {account.running && (
                  <button
                    className="button danger"
                    disabled={busy}
                    onClick={() =>
                      void perform(async () => {
                        await createApi(account.id).stopTasks();
                        await refresh();
                      })
                    }
                  >
                    停止 {account.name}
                  </button>
                )}
              </div>
            ))}
          </div>
          <button
            className="button primary"
            disabled={busy || !selected.length}
            onClick={() => void perform(prepare)}
          >
            检查所选账号任务
          </button>
          <form
            className="account-form"
            onSubmit={(e) => {
              e.preventDefault();
              void perform(async () => {
                if (copy) await saveCurrent();
                const profile = await accountsApi.create(name, copy ? current.id : undefined);
                await refresh();
                await onSelect(profile.id);
                onClose();
              });
            }}
          >
            <h3>添加账号</h3>
            <label>
              账号名称（本地标识）
              <input
                required
                maxLength={60}
                value={name}
                disabled={busy}
                onChange={(e) => setName(e.target.value)}
              />
            </label>
            <label className="account-copy">
              <input
                type="checkbox"
                checked={copy}
                disabled={busy}
                onChange={(e) => setCopy(e.target.checked)}
              />
              复制「{current.name}」的清单和执行参数（不复制凭据）
            </label>
            <button className="button secondary" disabled={busy || !name.trim()}>
              添加并切换
            </button>
          </form>
          <form
            className="account-form"
            onSubmit={(e) => {
              e.preventDefault();
              void perform(async () => {
                await accountsApi.rename(current.id, rename);
                await refresh();
              });
            }}
          >
            <label>
              当前账号名称
              <input
                required
                maxLength={60}
                value={rename}
                disabled={busy}
                onChange={(e) => setRename(e.target.value)}
              />
            </label>
            <button className="button secondary" disabled={busy || !rename.trim()}>
              保存账号名称
            </button>
          </form>
        </>
      )}
    </Modal>
  );
}
