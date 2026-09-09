import type { Credentials, LoginMethod, StoredCredentials } from '../types';

export const loginMethodLabels: Record<LoginMethod, string> = {
  cas: '统一身份认证',
  newjw: '教务账号',
  qrcode: '钉钉扫码',
  cookie: '已有 Cookie',
};

export const defaultStoredCredentials: StoredCredentials = {
  cas_username: '',
  cas_password: '',
  newjw_username: '',
  newjw_password: '',
  session_id: '',
  route: '',
  order: ['cas', 'newjw', 'qrcode', 'cookie'],
};

/** Keep the order list valid: drop duplicates/unknowns, append missing methods. */
export function normalizeOrder(order: LoginMethod[]): LoginMethod[] {
  const all: LoginMethod[] = ['cas', 'newjw', 'qrcode', 'cookie'];
  const out = [...new Set(order)].filter((m) => all.includes(m));
  for (const m of all) if (!out.includes(m)) out.push(m);
  return out;
}

/** Request-shaped credentials for a method, or null when not fully saved (QR is interactive). */
export function credentialsFor(method: LoginMethod, creds: StoredCredentials): Credentials | null {
  if (method === 'cas' && creds.cas_username && creds.cas_password)
    return {
      method: 'cas',
      username: creds.cas_username,
      password: creds.cas_password,
      session_id: '',
      route: '',
    };
  if (method === 'newjw' && creds.newjw_username && creds.newjw_password)
    return {
      method: 'newjw',
      username: creds.newjw_username,
      password: creds.newjw_password,
      session_id: '',
      route: '',
    };
  if (method === 'cookie' && creds.session_id && creds.route)
    return {
      method: 'cookie',
      username: '',
      password: '',
      session_id: creds.session_id,
      route: creds.route,
    };
  return null;
}

/** Merge credentials that just logged in successfully back into storage. */
export function mergeCreds(creds: StoredCredentials, auth: Credentials): StoredCredentials {
  const c = { ...creds };
  if (auth.method === 'cas') {
    c.cas_username = auth.username;
    c.cas_password = auth.password;
  } else if (auth.method === 'newjw') {
    c.newjw_username = auth.username;
    c.newjw_password = auth.password;
  } else if (auth.method === 'cookie') {
    c.session_id = auth.session_id;
    c.route = auth.route;
  }
  return c;
}
