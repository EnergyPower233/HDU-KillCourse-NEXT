import { useCallback, useEffect, useRef, useState } from 'react';
import { accountsApi } from '../bridge';
import type { AccountSummary } from '../types';

export function useAccounts() {
  const [accounts, setAccounts] = useState<AccountSummary[]>([]);
  const [selected, setSelected] = useState(() => localStorage.getItem('hdu-account') || 'default');
  const [error, setError] = useState('');
  const mounted = useRef(false);
  const generation = useRef(0);
  const refresh = useCallback(async () => {
    const current = ++generation.current;
    try {
      const next = await accountsApi.list();
      if (mounted.current && current === generation.current) {
        setAccounts(next);
        setError('');
      }
    } catch (e) {
      if (mounted.current && current === generation.current) setError(String(e));
      throw e;
    }
  }, []);
  useEffect(() => {
    mounted.current = true;
    let pending = false;
    const poll = async () => {
      if (pending) return;
      pending = true;
      try {
        await refresh();
      } catch {
        /* shown above the workspace */
      } finally {
        pending = false;
      }
    };
    void poll();
    const timer = setInterval(poll, 1500);
    return () => {
      mounted.current = false;
      generation.current++;
      clearInterval(timer);
    };
  }, [refresh]);
  const select = (id: string) => {
    localStorage.setItem('hdu-account', id);
    setSelected(id);
  };
  return {
    accounts,
    current: accounts.find((a) => a.id === selected) ?? accounts[0],
    error,
    refresh,
    select,
  };
}
