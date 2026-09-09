import { X } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { intervalUnitLabels, intervalUnitMs, splitInterval } from '../domain/interval';
import { type IntervalUnit } from '../types';

export function Stat({
  icon,
  label,
  value,
  note,
}: {
  icon: React.ReactNode;
  label: string;
  value: string;
  note: string;
}) {
  return (
    <div className="stat">
      <span className="stat-icon">{icon}</span>
      <div>
        <span className="stat-top">{label}</span>
        <strong>{value}</strong>
        <small>{note}</small>
      </div>
    </div>
  );
}
export function IntervalInput({
  ms,
  disabled,
  onChange,
}: {
  ms: number;
  disabled: boolean;
  onChange: (ms: number) => void;
}) {
  const [unit, setUnit] = useState<IntervalUnit>(() => splitInterval(ms).unit);
  const value = ms / intervalUnitMs[unit];
  return (
    <span className="interval-input">
      <input
        type="number"
        min="0"
        step="any"
        disabled={disabled}
        value={Number.isInteger(value) ? value : +value.toFixed(3)}
        onChange={(e) => {
          const v = Number(e.target.value);
          if (Number.isFinite(v)) onChange(Math.max(0, Math.round(v * intervalUnitMs[unit])));
        }}
      />
      <select
        disabled={disabled}
        value={unit}
        onChange={(e) => setUnit(e.target.value as IntervalUnit)}
      >
        {(Object.keys(intervalUnitLabels) as IntervalUnit[]).map((u) => (
          <option key={u} value={u}>
            {intervalUnitLabels[u]}
          </option>
        ))}
      </select>
    </span>
  );
}
export function Empty({
  icon,
  title,
  text,
  children,
}: {
  icon: React.ReactNode;
  title: string;
  text: string;
  children?: React.ReactNode;
}) {
  return (
    <div className="empty">
      <span className="empty-icon">{icon}</span>
      <h3>{title}</h3>
      <p>{text}</p>
      {children}
    </div>
  );
}
export function Modal({
  title,
  onClose,
  children,
}: {
  title: string;
  onClose: () => void;
  children: React.ReactNode;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const root = ref.current!;
    root.querySelector<HTMLElement>('button,input,select')?.focus();
    const trap = (e: KeyboardEvent) => {
      if (e.key !== 'Tab') return;
      const nodes = [
        ...root.querySelectorAll<HTMLElement>(
          'button:not(:disabled),input:not(:disabled),select:not(:disabled),[tabindex="0"]',
        ),
      ];
      const first = nodes[0],
        last = nodes[nodes.length - 1];
      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last?.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first?.focus();
      }
    };
    root.addEventListener('keydown', trap);
    return () => {
      root.removeEventListener('keydown', trap);
      previous?.focus();
    };
  }, []);
  return (
    <div className="modal-overlay" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div ref={ref} className="modal" role="dialog" aria-modal="true" aria-label={title}>
        <div className="modal-heading">
          <h2>{title}</h2>
          <button aria-label="关闭对话框" onClick={onClose}>
            <X size={18} />
          </button>
        </div>
        {children}
      </div>
    </div>
  );
}
