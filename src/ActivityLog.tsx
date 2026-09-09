import { useEffect, useRef, useState } from 'react';
import { api } from './bridge';
import { progressLabel, type Progress, type RunInfo, type Snapshot } from './types';

export function ActivityLog({ snapshot }: { snapshot: Snapshot }) {
  const [runs, setRuns] = useState<RunInfo[]>([]);
  const [selected, setSelected] = useState(snapshot.current_run?.id || '');
  const [events, setEvents] = useState<Progress[]>([]);
  const [before, setBefore] = useState<number | null>(null);
  const [error, setError] = useState('');
  const [warning, setWarning] = useState('');
  const [loading, setLoading] = useState(false);
  const [olderLoading, setOlderLoading] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const [reload, setReload] = useState(0);
  const selection = useRef(selected);
  selection.current = selected;
  const live = snapshot.running && selected === snapshot.current_run?.id;

  useEffect(() => {
    let alive = true, pending = false;
    const refresh = async () => {
      if (pending) return;
      pending = true;
      try {
        const result = await api.runs();
        if (alive) { setRuns(result); setSelected(id => id || result[0]?.id || ''); }
      } catch (e) { if (alive) setError(String(e)); }
      finally { pending = false; }
    };
    void refresh();
    const timer = setInterval(refresh, 3000);
    return () => { alive = false; clearInterval(timer); };
  }, []);

  useEffect(() => {
    if (snapshot.current_run?.id) { setSelected(snapshot.current_run.id); setExpanded(false); }
  }, [snapshot.current_run?.id]);

  useEffect(() => {
    if (!selected || expanded) return;
    let alive = true, pending = false;
    setLoading(true); setEvents([]); setBefore(null); setError(''); setWarning('');
    const refresh = async () => {
      if (pending) return;
      pending = true;
      try {
        const page = await api.run(selected);
        if (alive) { setEvents(page.events); setBefore(page.next_before); setWarning(page.warning || ''); setError(''); }
      } catch (e) { if (alive) setError(String(e)); }
      finally { pending = false; if (alive) setLoading(false); }
    };
    void refresh();
    const timer = live ? setInterval(refresh, 1500) : undefined;
    return () => { alive = false; if (timer) clearInterval(timer); };
  }, [selected, live, expanded, reload]);

  async function loadOlder() {
    if (before === null || olderLoading) return;
    const id = selected;
    setExpanded(true); setOlderLoading(true);
    try {
      const page = await api.run(id, before);
      if (selection.current === id) {
        setEvents(current => [...page.events, ...current]);
        setBefore(page.next_before); setWarning(page.warning || ''); setError('');
      }
    } catch (e) { if (selection.current === id) setError(String(e)); }
    finally { setOlderLoading(false); }
  }

  const choices = [...runs];
  if (snapshot.current_run && !choices.some(r => r.id === snapshot.current_run?.id)) choices.unshift(snapshot.current_run);
  return <>
    <div className="panel-heading"><div><h2>运行记录</h2><p>每次运行单独保存到本地 · 新任务不会删除历史记录</p></div><span className={`badge ${live ? 'running' : 'neutral'}`}>{live ? '本次正在运行' : '历史记录'}</span></div>
    <div className="run-log-controls">
      <label>运行批次 <select aria-label="运行批次" value={selected} onChange={e => { setExpanded(false); setSelected(e.target.value); }}>
        {!choices.length && <option value="">暂无运行记录</option>}
        {choices.map(r => <option key={r.id} value={r.id}>{new Date(r.started_at).toLocaleString()} · {r.list_name} · {r.id.slice(-6)}</option>)}
      </select></label>
      <button className="button secondary" disabled={!selected} onClick={() => { setExpanded(false); setReload(n => n + 1); }}>查看最新记录</button>
    </div>
    {error && <p role="alert" className="run-log-error">{error}</p>}
    {warning && <p role="status">{warning}</p>}
    {expanded && live && <p className="run-log-hint">正在查看较早记录，实时刷新已暂停；点击「查看最新记录」恢复。</p>}
    {loading ? <p className="run-log-hint" role="status">正在读取日志…</p> : events.length ? <div className="log-list">{events.slice().reverse().map((e, i) => <div className="log-row" key={`${selected}-${events.length - i}`}><time dateTime={e.time}>{e.time.includes("T") ? new Date(e.time).toLocaleString() : e.time}</time><span className={`badge ${e.status}`}>{progressLabel(e)}</span><div>{e.course_id && <strong className="log-course-name">{e.course_name || e.course_id}</strong>}{e.message}{e.course_id && <small>{e.course_id}{e.schedule && ` · ${e.schedule}`}</small>}</div></div>)}</div> : <p className="run-log-hint">{selected ? '这次运行暂时没有事件。' : '开始任务后，每次运行都会保存在这里，重启程序后仍可查看。'}</p>}
    {before !== null && <div className="run-log-controls"><button className="button secondary" disabled={olderLoading || loading} onClick={() => void loadOlder()}>{olderLoading ? '读取中…' : '加载更早的 500 条'}</button></div>}
  </>;
}
