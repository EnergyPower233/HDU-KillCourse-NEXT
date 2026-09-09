import { useState, useEffect } from 'react';
import { api } from '../bridge';
import { defaults } from '../domain/settings';
import { defaultUaConfig } from '../domain/ua';
import { defaultStoredCredentials } from '../domain/credentials';
import type { Settings, Course, Snapshot, UaConfig, StoredCredentials } from '../types';
export function useWorkspace(toast: (text: string, error?: boolean) => void) {
  const [settings, setSettings] = useState<Settings>(defaults);
  const [courses, setCourses] = useState<Course[]>([]);
  const [snapshot, setSnapshot] = useState<Snapshot>({
    running: false,
    logged_in: false,
    history: [],
    fetch_progress: null,
  });
  const [ready, setReady] = useState(false);
  const [backend, setBackend] = useState<boolean | null>(null);
  const [ua, setUa] = useState<UaConfig>(defaultUaConfig);
  const [creds, setCreds] = useState<StoredCredentials>(defaultStoredCredentials);
  useEffect(() => {
    let alive = true;
    (async () => {
      try {
        await api.health();
      } catch {
        if (alive) {
          setBackend(false);
          setReady(true);
        }
        return;
      }
      try {
        const [s, c, r] = await Promise.all([
          api.loadSettings(),
          api.loadCourses(),
          api.snapshot(),
        ]);
        if (alive) {
          setSettings(s);
          setCourses(c);
          setSnapshot(r);
          setBackend(true);
        }
      } catch (e) {
        if (alive) {
          toast(String(e), true);
          setBackend(true);
        }
      } finally {
        if (alive) setReady(true);
      }
      api
        .loadCredentials()
        .then((c) => {
          if (alive) setCreds(c);
        })
        .catch((e) => {
          if (alive) toast(String(e), true);
        });
      api
        .loadUa()
        .then((u) => {
          if (!alive) return;
          // In browser mode, always reflect the actual browser UA.
          if (u.mode === 'browser' && u.browser_ua !== navigator.userAgent) {
            const next = { ...u, browser_ua: navigator.userAgent };
            setUa(next);
            void api.saveUa(next).catch(() => {});
          } else {
            setUa(u);
          }
        })
        .catch(() => {
          /* UA 配置缺失时使用默认值 */
        });
    })();
    return () => {
      alive = false;
    };
  }, [toast]);
  useEffect(() => {
    let alive = true,
      polling = false;
    const poll = async () => {
      if (polling) return;
      polling = true;
      try {
        const r = await api.snapshot();
        if (alive) {
          setSnapshot(r);
          setBackend(true);
        }
      } catch {
        if (alive) setBackend(false);
      } finally {
        polling = false;
      }
    };
    poll();
    const timer = setInterval(poll, 900);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, [toast]);
  return {
    settings,
    setSettings,
    courses,
    setCourses,
    snapshot,
    setSnapshot,
    ready,
    backend,
    ua,
    setUa,
    creds,
    setCreds,
  };
}
