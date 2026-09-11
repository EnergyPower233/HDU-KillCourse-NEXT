import { useState, useEffect } from 'react';
import { useApi } from '../AccountContext';
import { defaults } from '../domain/settings';
import { defaultUaConfig } from '../domain/ua';
import { defaultStoredCredentials } from '../domain/credentials';
import type { Settings, Course, Snapshot, UaConfig, StoredCredentials } from '../types';
export function useWorkspace(toast: (text: string, error?: boolean) => void) {
  const api = useApi();
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
        const [s, c, r, credentials, userAgent] = await Promise.all([
          api.loadSettings(),
          api.loadCourses(),
          api.snapshot(),
          api.loadCredentials(),
          api.loadUa(),
        ]);
        if (!alive) return;
        setSettings(s);
        setCourses(c);
        setSnapshot(r);
        setCreds(credentials);
        const nextUa =
          userAgent.mode === 'browser'
            ? { ...userAgent, browser_ua: navigator.userAgent }
            : userAgent;
        setUa(nextUa);
        setBackend(true);
        setReady(true);
        if (userAgent.mode === 'browser' && userAgent.browser_ua !== navigator.userAgent) {
          void api.saveUa(nextUa).catch((e) => {
            if (alive) toast(String(e), true);
          });
        }
      } catch (e) {
        if (alive) {
          toast(String(e), true);
          setBackend(true);
        }
      }
    })();
    return () => {
      alive = false;
    };
  }, [toast, api]);
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
  }, [toast, api]);
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
