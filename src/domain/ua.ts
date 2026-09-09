import type { UaBrowser, UaConfig, UaOs } from '../types';

export const defaultUaConfig: UaConfig = {
  mode: 'browser',
  browser_ua: '',
  fixed_os: 'windows',
  fixed_browser: 'chrome',
  fixed_version: '143.0.7467.120',
  rotate_list: [],
  generate_seed: 114514,
};

export const uaOsLabels: Record<UaOs, string> = {
  windows: 'Windows',
  macos: 'macOS',
  linux: 'Linux',
};

export const uaBrowserLabels: Record<UaBrowser, string> = {
  chrome: 'Chrome',
  edge: 'Edge',
  firefox: 'Firefox',
  opera: 'Opera',
  safari: 'Safari',
};

export const uaVersions: Record<UaBrowser, string[]> = {
  chrome: ['143.0.7467.120', '142.0.7444.110', '141.0.7218.87', '140.0.7339.208', '139.0.7258.114'],
  edge: ['143.0.3270.55', '142.0.3236.48', '141.0.3175.102', '140.0.3124.54'],
  firefox: ['143.0', '142.0', '141.0', '140.0', '139.0', '138.0'],
  opera: ['116.0.5366.45', '115.0.5322.109', '114.0.5282.115', '113.0.5230.132'],
  safari: ['18.5', '18.4', '17.6', '17.5', '16.6', '16.5'],
};

export function buildFixedUa(os: UaOs, browser: UaBrowser, version: string): string {
  const platform = (firefox: boolean) => {
    if (firefox) {
      const base =
        os === 'macos'
          ? 'Macintosh; Intel Mac OS X 10.15'
          : os === 'linux'
            ? 'X11; Linux x86_64'
            : 'Windows NT 10.0; Win64; x64';
      return `${base}; rv:${version}`;
    }
    if (os === 'macos') return 'Macintosh; Intel Mac OS X 10_15_7';
    return os === 'linux' ? 'X11; Linux x86_64' : 'Windows NT 10.0; Win64; x64';
  };
  switch (browser) {
    case 'firefox':
      return `Mozilla/5.0 (${platform(true)}) Gecko/20100101 Firefox/${version}`;
    case 'safari':
      return `Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/${version} Safari/605.1.15`;
    case 'edge':
      return `Mozilla/5.0 (${platform(false)}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/${version} Safari/537.36 Edg/${version}`;
    case 'opera':
      return `Mozilla/5.0 (${platform(false)}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.6723.124 Safari/537.36 OPR/${version}`;
    default:
      return `Mozilla/5.0 (${platform(false)}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/${version} Safari/537.36`;
  }
}
