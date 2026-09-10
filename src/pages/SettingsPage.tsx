import {
  Check,
  ChevronDown,
  ChevronUp,
  KeyRound,
  Monitor,
  Moon,
  ShieldCheck,
  Sun,
  Trash2,
  UserRound,
} from 'lucide-react';
import type * as React from 'react';
import { api } from '../bridge';
import { IntervalInput } from '../components/ui';
import {
  credentialsFor,
  defaultStoredCredentials,
  loginMethodLabels,
  normalizeOrder,
} from '../domain/credentials';
import { buildFixedUa, uaBrowserLabels, uaOsLabels, uaVersions } from '../domain/ua';
import type { useLogin } from '../hooks/useLogin';
import type { Theme } from '../navigation';
import { type Settings, type UaBrowser, type UaConfig, type UaMode, type UaOs } from '../types';
interface Props {
  login: ReturnType<typeof useLogin>;

  locked: boolean;
  settings: Settings;
  update: (patch: Partial<Settings>) => void;
  theme: Theme;
  setTheme: React.Dispatch<React.SetStateAction<Theme>>;
  perform: (label: string, fn: () => Promise<void>) => Promise<void>;
  save: () => Promise<void>;
  toast: (text: string, error?: boolean) => void;

  ua: UaConfig;
  patchUa: (patch: Partial<UaConfig>) => void;
}
export function SettingsPage({
  locked,
  settings,
  update,
  theme,
  setTheme,
  perform,
  save,
  toast,
  ua,
  patchUa,
  login,
}: Props) {
  const { creds, moveMethod, setCreds } = login;
  return (
    <div className="settings-layout">
      <div className="settings-column">
        <section className="panel">
          <div className="panel-heading">
            <div>
              <h2>学期与执行偏好</h2>
              <p>设置保存在本地数据目录，保存后用于下次任务。</p>
            </div>
          </div>
          <div className="settings-form">
            <div className="form-pair">
              <label>
                学年起始年份
                <input
                  type="number"
                  min="2000"
                  max="2100"
                  disabled={locked}
                  value={settings.year}
                  onChange={(e) => update({ year: Number(e.target.value) })}
                />
              </label>
              <label>
                学期
                <select
                  disabled={locked}
                  value={settings.term}
                  onChange={(e) => update({ term: Number(e.target.value) })}
                >
                  <option value="1">第一学期</option>
                  <option value="2">第二学期</option>
                </select>
              </label>
            </div>
            <label>
              蹲课查询间隔
              <IntervalInput
                ms={settings.interval_ms}
                disabled={locked}
                onChange={(ms) => update({ interval_ms: ms })}
              />
              <small>100 毫秒～24 小时；仅蹲课模式使用，单次执行不等待。</small>
            </label>
            <label>
              波动范围
              <IntervalInput
                ms={settings.jitter_ms}
                disabled={locked}
                onChange={(ms) => update({ jitter_ms: ms })}
              />
              <small>0 关闭；间隔在 ±范围 内随机，模拟抢课瞬间人手点。</small>
            </label>
            <label>
              波动种子（MT19937）
              <input
                type="number"
                disabled={locked}
                value={settings.jitter_seed}
                onChange={(e) => update({ jitter_seed: Math.trunc(Number(e.target.value)) })}
              />
              <small>同一种子的波动序列固定；默认 1919810。</small>
            </label>
            <div>
              <label>外观</label>
              <div className="segmented" role="radiogroup" aria-label="外观主题">
                <button
                  className={theme === 'system' ? 'active' : ''}
                  onClick={() => setTheme('system')}
                >
                  <Monitor size={14} />
                  跟随系统
                </button>
                <button
                  className={theme === 'light' ? 'active' : ''}
                  onClick={() => setTheme('light')}
                >
                  <Sun size={14} />
                  浅色
                </button>
                <button
                  className={theme === 'dark' ? 'active' : ''}
                  onClick={() => setTheme('dark')}
                >
                  <Moon size={14} />
                  深色
                </button>
              </div>
            </div>
            <button
              className="button primary"
              style={{ alignSelf: 'flex-start' }}
              disabled={locked}
              onClick={() =>
                perform('保存中', async () => {
                  await save();
                  toast('偏好设置已保存');
                })
              }
            >
              保存设置
              <Check size={15} />
            </button>
          </div>
        </section>
        <section className="panel">
          <div className="panel-heading">
            <div>
              <h2>登录顺序与凭证</h2>
              <p>连接时按此顺序依次尝试；凭证保存在本机数据目录。</p>
            </div>
            <KeyRound size={17} className="muted" />
          </div>
          <div className="settings-form">
            <div className="method-order">
              {normalizeOrder(creds.order).map((m, i) => (
                <div className="method-order-row" key={m}>
                  <span className="queue-number">{String(i + 1).padStart(2, '0')}</span>
                  <span className="method-order-name">{loginMethodLabels[m]}</span>
                  <span
                    className={`badge ${m === 'qrcode' || credentialsFor(m, creds) ? 'success' : 'neutral'}`}
                  >
                    {m === 'qrcode'
                      ? '无需凭证'
                      : credentialsFor(m, creds)
                        ? '已保存凭证'
                        : '未保存'}
                  </span>
                  <span className="reorder">
                    <button
                      title="上移"
                      aria-label={`上移 ${loginMethodLabels[m]}`}
                      disabled={locked || i === 0}
                      onClick={() => moveMethod(i, -1)}
                    >
                      <ChevronUp size={14} />
                    </button>
                    <button
                      title="下移"
                      aria-label={`下移 ${loginMethodLabels[m]}`}
                      disabled={locked || i === normalizeOrder(creds.order).length - 1}
                      onClick={() => moveMethod(i, 1)}
                    >
                      <ChevronDown size={14} />
                    </button>
                  </span>
                </div>
              ))}
            </div>
            <p className="credential-hint">
              <ShieldCheck size={13} />
              账号密码与 Cookie 以明文保存在程序目录的 data
              文件夹（便携设计），仅用于登录学校系统；请勿在共用电脑上保存。
            </p>
            <button
              className="button secondary"
              style={{ alignSelf: 'flex-start' }}
              disabled={locked}
              onClick={() =>
                perform('清除中', async () => {
                  await api.clearCredentials();
                  setCreds(defaultStoredCredentials);
                  toast('已清除保存的凭证');
                })
              }
            >
              <Trash2 size={15} />
              清除已保存凭证
            </button>
          </div>
        </section>
        <section className="panel">
          <div className="panel-heading">
            <div>
              <h2>User-Agent</h2>
              <p>设置请求中的浏览器标识；修改后重新登录生效。</p>
            </div>
            <UserRound size={17} className="muted" />
          </div>
          <div className="settings-form">
            <label>
              模式
              <select
                disabled={locked}
                value={ua.mode}
                onChange={(e) => patchUa({ mode: e.target.value as UaMode })}
              >
                <option value="browser">使用当前浏览器 UA</option>
                <option value="fixed">固定 UA（自选系统/浏览器/版本）</option>
                <option value="rotate">多 UA 轮换</option>
                <option value="generate">随机生成（MT19937）</option>
              </select>
            </label>
            {ua.mode === 'browser' && (
              <p className="ua-preview">
                <strong>当前 UA：</strong>
                <span>{ua.browser_ua || navigator.userAgent}</span>
              </p>
            )}
            {ua.mode === 'fixed' && (
              <>
                <div className="form-pair">
                  <label>
                    操作系统
                    <select
                      disabled={locked}
                      value={ua.fixed_os}
                      onChange={(e) => {
                        const os = e.target.value as UaOs;
                        const browser =
                          os !== 'macos' && ua.fixed_browser === 'safari'
                            ? 'chrome'
                            : ua.fixed_browser;
                        patchUa({
                          fixed_os: os,
                          fixed_browser: browser,
                          fixed_version: uaVersions[browser][0],
                        });
                      }}
                    >
                      {(Object.keys(uaOsLabels) as UaOs[]).map((o) => (
                        <option key={o} value={o}>
                          {uaOsLabels[o]}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label>
                    浏览器
                    <select
                      disabled={locked}
                      value={ua.fixed_browser}
                      onChange={(e) => {
                        const browser = e.target.value as UaBrowser;
                        patchUa({ fixed_browser: browser, fixed_version: uaVersions[browser][0] });
                      }}
                    >
                      {(Object.keys(uaBrowserLabels) as UaBrowser[]).map((b) => (
                        <option
                          key={b}
                          value={b}
                          disabled={b === 'safari' && ua.fixed_os !== 'macos'}
                        >
                          {uaBrowserLabels[b]}
                        </option>
                      ))}
                    </select>
                  </label>
                </div>
                <label>
                  版本号
                  <select
                    disabled={locked}
                    value={ua.fixed_version}
                    onChange={(e) => patchUa({ fixed_version: e.target.value })}
                  >
                    {uaVersions[ua.fixed_browser].map((v) => (
                      <option key={v} value={v}>
                        {v}
                      </option>
                    ))}
                  </select>
                </label>
                <p className="ua-preview">
                  <strong>生成的 UA：</strong>
                  <span>{buildFixedUa(ua.fixed_os, ua.fixed_browser, ua.fixed_version)}</span>
                </p>
              </>
            )}
            {ua.mode === 'rotate' && (
              <>
                <label>
                  轮换列表（每行一个 UA）
                  <textarea
                    rows={5}
                    disabled={locked}
                    value={ua.rotate_list.join('\n')}
                    onChange={(e) => patchUa({ rotate_list: e.target.value.split('\n') })}
                    placeholder={'Mozilla/5.0 …\nMozilla/5.0 …'}
                  />
                  <small>最多 50 条；每次登录随机取一个，本次会话内保持不变。</small>
                </label>
                <p className="ua-preview">
                  <strong>当前列表：</strong>
                  <span>
                    {ua.rotate_list.length
                      ? `共 ${ua.rotate_list.length} 条，每次登录随机取一条`
                      : '列表为空'}
                  </span>
                </p>
              </>
            )}
            {ua.mode === 'generate' && (
              <>
                <label>
                  MT19937 种子
                  <input
                    type="number"
                    disabled={locked}
                    value={ua.generate_seed}
                    onChange={(e) => patchUa({ generate_seed: Number(e.target.value) })}
                    placeholder="114514"
                  />
                  <small>
                    同一种子生成的 UA 序列固定；默认 114514。每次登录取序列里下一个浏览器格式的
                    UA，会话内保持不变。
                  </small>
                </label>
                <p className="ua-preview">
                  <strong>说明：</strong>
                  <span>由 MT19937（种子 {ua.generate_seed}）生成，会话内固定</span>
                </p>
              </>
            )}
            <p className="credential-hint">
              <ShieldCheck size={13} />
              User-Agent 在登录与执行时生效；修改后建议重新登录以重建会话。
            </p>
          </div>
        </section>
      </div>
      <section className="panel info-panel">
        <ShieldCheck size={24} />
        <h2>清晰的执行边界</h2>
        <p>学校明确拒绝的请求会显示拒绝原因，不会被记为成功。</p>
        <p>若提交超时或回复无法识别，任务会暂停，等待你核实真实选课结果。</p>
        <p>登录会按你排定的顺序依次尝试，失败的自动换下一种。</p>
        <p>本版暂不包含自动换班、排课、邮件通知与跨年级选课。</p>
        <div className="info-version">
          Rust 本地服务 + React 浏览器界面
          <br />
          Browser Edition 0.3.1
        </div>
      </section>
    </div>
  );
}
