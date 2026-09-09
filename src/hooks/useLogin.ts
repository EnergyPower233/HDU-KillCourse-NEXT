import { useRef, useState, type Dispatch, type SetStateAction } from 'react';
import { api } from '../bridge';
import {
  credentialsFor,
  loginMethodLabels,
  mergeCreds,
  normalizeOrder,
} from '../domain/credentials';
import {
  type Credentials,
  type LoginMethod,
  type Settings,
  type Snapshot,
  type StoredCredentials,
} from '../types';
export const emptyAuth: Credentials = {
  method: 'cas',
  username: '',
  password: '',
  session_id: '',
  route: '',
};
interface Options {
  creds: StoredCredentials;
  setCreds: Dispatch<SetStateAction<StoredCredentials>>;
  settings: Settings;
  toast: (text: string, error?: boolean) => void;
  setSnapshot: Dispatch<SetStateAction<Snapshot>>;
  setBusy: Dispatch<SetStateAction<string>>;
}
export function useLogin({ settings, toast, setSnapshot, setBusy, creds, setCreds }: Options) {
  const [auth, setAuth] = useState<Credentials>(emptyAuth);
  const [loginOpen, setLoginOpen] = useState(false);
  const [qr, setQr] = useState<{
    image: string;
    status: 'waiting' | 'expired' | 'error';
    message: string;
  } | null>(null);
  const [qrAutoMode, setQrAutoMode] = useState(false);
  const [autoNote, setAutoNote] = useState('');
  const qrTimer = useRef<number | null>(null);
  const qrBusy = useRef(false);
  const qrResolve = useRef<((ok: boolean) => void) | null>(null);
  const qrAutoRef = useRef(false);
  // DingTalk QR login flow: fetch the QR once, then poll the scan status.
  // In "auto" mode the promise resolves true on success / false on failure so
  // the ordered login sequence can continue to the next method.
  function clearQrTimer() {
    if (qrTimer.current !== null) {
      window.clearInterval(qrTimer.current);
      qrTimer.current = null;
    }
  }
  function finishQr(ok: boolean) {
    qrAutoRef.current = false;
    setQrAutoMode(false);
    clearQrTimer();
    setQr(null);
    const resolve = qrResolve.current;
    qrResolve.current = null;
    resolve?.(ok);
  }
  function cancelQr() {
    if (qrResolve.current) {
      finishQr(false);
    } else {
      clearQrTimer();
      setQr(null);
    }
    void api.loginQrCancel().catch(() => {
      /* the pending session is optional state */
    });
  }
  async function pollQr() {
    if (qrBusy.current) return;
    qrBusy.current = true;
    try {
      const r = await api.loginQrPoll();
      if (r.status === 'confirmed') {
        if (qrAutoRef.current) {
          finishQr(true); // the auto flow finishes the bookkeeping
        } else {
          clearQrTimer();
          setQr(null);
          setSnapshot((s) => ({ ...s, logged_in: true }));
          setAuth(emptyAuth);
          setLoginOpen(false);
          toast('已通过钉钉扫码连接教务系统');
        }
      } else if (r.status === 'expired') {
        if (qrAutoRef.current) {
          finishQr(false);
        } else {
          clearQrTimer();
          setQr((q) => (q ? { ...q, status: 'expired', message: r.message || '二维码已过期' } : q));
        }
      } else {
        setQr((q) => (q && q.image ? { ...q, message: r.message || '等待扫码' } : q));
      }
    } catch (e) {
      if (qrAutoRef.current) {
        finishQr(false);
      } else {
        clearQrTimer();
        setQr((q) => (q ? { ...q, status: 'error', message: String(e) } : q));
      }
    } finally {
      qrBusy.current = false;
    }
  }
  async function startQr() {
    clearQrTimer();
    setQr({ image: '', status: 'waiting', message: '正在获取二维码…' });
    try {
      const r = await api.loginQrStart(settings);
      setQr({ image: r.image, status: 'waiting', message: '请打开钉钉，扫描二维码' });
      qrTimer.current = window.setInterval(() => {
        void pollQr();
      }, 1500);
    } catch (e) {
      if (qrAutoRef.current) {
        finishQr(false);
      } else {
        setQr({ image: '', status: 'error', message: String(e) });
      }
    }
  }
  function waitQr(): Promise<boolean> {
    return new Promise((resolve) => {
      qrAutoRef.current = true;
      setQrAutoMode(true);
      qrResolve.current = resolve;
      void startQr();
    });
  }
  function closeLogin() {
    cancelQr();
    setLoginOpen(false);
    setAuth(emptyAuth);
    setAutoNote('');
  }

  // -- Credentials persistence & ordered login attempts ---------------------
  function selectMethod(method: LoginMethod) {
    cancelQr();
    const c = credentialsFor(method, creds);
    setAuth(c ?? { ...emptyAuth, method });
    if (method === 'qrcode') void startQr();
  }
  async function rememberCreds(auth: Credentials) {
    const updated = mergeCreds(creds, auth);
    setCreds(updated);
    try {
      await api.saveCredentials(updated);
    } catch (e) {
      toast(String(e), true);
    }
  }
  function finishLogin() {
    setBusy('');
    setAutoNote('');
    setSnapshot((s) => ({ ...s, logged_in: true }));
    setAuth(emptyAuth);
    setLoginOpen(false);
    toast('已连接教务系统');
  }
  async function autoLogin() {
    const order = normalizeOrder(creds.order);
    setBusy('自动连接中');
    for (let i = 0; i < order.length; i++) {
      const method = order[i];
      const label = loginMethodLabels[method];
      setAutoNote(`正在尝试第 ${i + 1} 种方式：${label}…`);
      if (method === 'qrcode') {
        setAutoNote(`第 ${i + 1} 种方式：钉钉扫码，请用手机钉钉扫描`);
        const ok = await waitQr();
        if (ok) {
          finishLogin();
          return;
        }
        setAutoNote('钉钉扫码未完成，继续尝试下一种方式…');
        continue;
      }
      const c = credentialsFor(method, creds);
      if (!c) {
        setAutoNote(`跳过：${label}（未保存凭证）`);
        continue;
      }
      try {
        await api.login(c, settings);
        await rememberCreds(c);
        finishLogin();
        return;
      } catch (e) {
        setAutoNote(`${label} 失败：${String(e)}`);
      }
    }
    setBusy('');
    setAutoNote('');
    toast('所有登录方式均未成功，请检查账号、密码或网络', true);
  }
  function moveMethod(index: number, direction: -1 | 1) {
    const order = normalizeOrder(creds.order);
    const target = index + direction;
    if (target < 0 || target >= order.length) return;
    [order[index], order[target]] = [order[target], order[index]];
    const updated = { ...creds, order };
    setCreds(updated);
    void api.saveCredentials(updated).catch((e) => toast(String(e), true));
  }

  return {
    auth,
    setAuth,
    loginOpen,
    setLoginOpen,
    qr,
    qrAutoMode,
    autoNote,
    creds,
    setCreds,
    cancelQr,
    closeLogin,
    selectMethod,
    rememberCreds,
    finishLogin,
    autoLogin,
    moveMethod,
    startQr,
  };
}
