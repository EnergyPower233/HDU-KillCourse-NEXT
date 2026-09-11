import {
  ArrowRight,
  Cookie,
  KeyRound,
  LoaderCircle,
  LogOut,
  QrCode,
  RefreshCw,
  ShieldCheck,
  Zap,
} from 'lucide-react';
import type * as React from 'react';
import { useApi } from '../AccountContext';
import { Modal } from '../components/ui';
import { loginMethodLabels, normalizeOrder } from '../domain/credentials';
import type { useLogin } from '../hooks/useLogin';
import { type Settings, type Snapshot } from '../types';
interface Props {
  login: ReturnType<typeof useLogin>;

  snapshot: Snapshot;
  busy: string;

  locked: boolean;
  perform: (label: string, fn: () => Promise<void>) => Promise<void>;
  setSnapshot: React.Dispatch<React.SetStateAction<Snapshot>>;
  toast: (text: string, error?: boolean) => void;

  settings: Settings;

  offline: boolean;
}
export function LoginDialog({
  snapshot,
  busy,
  locked,
  perform,
  setSnapshot,
  toast,
  settings,
  offline,
  login,
}: Props) {
  const api = useApi();
  const {
    closeLogin,
    auth,
    rememberCreds,
    finishLogin,
    creds,
    selectMethod,
    qr,
    qrAutoMode,
    cancelQr,
    startQr,
    setAuth,
    autoLogin,
    autoNote,
  } = login;
  return (
    <Modal
      title={snapshot.logged_in ? '教务账号已连接' : '连接教务系统'}
      onClose={() => {
        if (!busy) closeLogin();
      }}
    >
      <p className="modal-intro">
        登录用于获取课程和执行任务；成功使用的账号密码会按你的选择保存在本机。
      </p>
      {snapshot.logged_in ? (
        <>
          <div className="connected">
            <ShieldCheck size={30} />
            <strong>会话已通过学生信息校验</strong>
          </div>
          <button
            className="button secondary full"
            disabled={locked}
            onClick={() =>
              perform('退出中', async () => {
                await api.logout();
                setSnapshot((s) => ({ ...s, logged_in: false }));
                toast('已退出登录');
              })
            }
          >
            <LogOut size={15} />
            退出当前账号
          </button>
        </>
      ) : (
        <form
          onSubmit={(e) => {
            e.preventDefault();
            void perform('登录中', async () => {
              await api.login(auth, settings);
              await rememberCreds(auth);
              finishLogin();
            });
          }}
        >
          <div className="method-cards" role="radiogroup" aria-label="登录方式">
            {normalizeOrder(creds.order).map((m) => {
              const icon =
                m === 'cas' ? (
                  <KeyRound size={16} />
                ) : m === 'newjw' ? (
                  <ShieldCheck size={16} />
                ) : m === 'cookie' ? (
                  <Cookie size={16} />
                ) : (
                  <QrCode size={16} />
                );
              const desc =
                m === 'cas'
                  ? 'CAS 账号密码'
                  : m === 'newjw'
                    ? '教务系统账号密码'
                    : m === 'cookie'
                      ? 'JSESSIONID + route'
                      : '手机钉钉扫一扫登录';
              return (
                <button
                  type="button"
                  key={m}
                  className={auth.method === m ? 'active' : ''}
                  onClick={() => selectMethod(m)}
                >
                  {icon}
                  {loginMethodLabels[m]}
                  <small>{desc}</small>
                </button>
              );
            })}
          </div>
          {auth.method === 'qrcode' || qr ? (
            <div className="qr-panel">
              <div className={`qr-frame${qr?.status === 'expired' ? ' qr-frame-expired' : ''}`}>
                {qr?.image ? (
                  <img src={`data:image/png;base64,${qr.image}`} alt="钉钉登录二维码" />
                ) : (
                  <div className="qr-placeholder">
                    <LoaderCircle className="spin" size={26} />
                  </div>
                )}
                {qr?.status === 'expired' && <div className="qr-overlay">二维码已过期</div>}
              </div>
              <p className={`qr-status${qr?.status === 'error' ? ' qr-status-error' : ''}`}>
                {qr?.status === 'waiting' && <span className="qr-pulse" />}
                {qr?.message || '正在获取二维码…'}
              </p>
              {qrAutoMode ? (
                <button type="button" className="button secondary full" onClick={cancelQr}>
                  <ArrowRight size={15} />
                  跳过此方式，尝试下一种
                </button>
              ) : qr?.status === 'expired' || qr?.status === 'error' ? (
                <button
                  type="button"
                  className="button secondary full"
                  onClick={() => void startQr()}
                >
                  <RefreshCw size={15} />
                  重新获取二维码
                </button>
              ) : null}
              <p className="qr-hint">使用手机钉钉扫一扫，完成学校统一身份认证</p>
            </div>
          ) : auth.method === 'cookie' ? (
            <>
              <label>
                JSESSIONID
                <input
                  required
                  type="password"
                  autoComplete="off"
                  value={auth.session_id}
                  disabled={!!busy}
                  onChange={(e) => setAuth({ ...auth, session_id: e.target.value })}
                />
              </label>
              <label>
                route
                <input
                  required
                  type="password"
                  autoComplete="off"
                  value={auth.route}
                  disabled={!!busy}
                  onChange={(e) => setAuth({ ...auth, route: e.target.value })}
                />
              </label>
            </>
          ) : (
            <>
              <label>
                账号
                <input
                  required
                  placeholder="输入学号"
                  autoComplete="username"
                  value={auth.username}
                  disabled={!!busy}
                  onChange={(e) => setAuth({ ...auth, username: e.target.value })}
                />
              </label>
              <label>
                密码
                <input
                  required
                  type="password"
                  autoComplete="current-password"
                  value={auth.password}
                  disabled={!!busy}
                  onChange={(e) => setAuth({ ...auth, password: e.target.value })}
                />
              </label>
            </>
          )}
          {auth.method !== 'qrcode' && !qr && (
            <button type="submit" className="button primary full" disabled={!!busy || offline}>
              {busy ? <LoaderCircle size={15} className="spin" /> : <ShieldCheck size={15} />}
              {busy === '登录中' ? '正在连接…' : '用此方式连接'}
            </button>
          )}
          <div className="auto-divider">
            <span>或</span>
          </div>
          <button
            type="button"
            className="button secondary full"
            disabled={!!busy || offline}
            onClick={() => void autoLogin()}
          >
            <Zap size={15} />
            {busy === '自动连接中' ? '正在按顺序尝试…' : '按偏好顺序自动尝试'}
          </button>
          {autoNote && (
            <p className="auto-note">
              {busy === '自动连接中' && <LoaderCircle size={12} className="spin" />}
              {autoNote}
            </p>
          )}
        </form>
      )}
    </Modal>
  );
}
