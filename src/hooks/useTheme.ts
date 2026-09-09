import { useEffect } from 'react';
import type { Theme } from '../navigation';
function applyTheme(theme: Theme) {
  const dark =
    theme === 'dark' ||
    (theme === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches);
  document.documentElement.dataset.theme = dark ? 'dark' : 'light';
}
export function useTheme(theme: Theme) {
  // Theme: keep the document attribute in sync with the selection and the OS.
  useEffect(() => {
    applyTheme(theme);
    localStorage.setItem('hdu-theme', theme);
    if (theme !== 'system') return;
    const media = window.matchMedia('(prefers-color-scheme: dark)');
    const onChange = () => applyTheme('system');
    media.addEventListener('change', onChange);
    return () => media.removeEventListener('change', onChange);
  }, [theme]);
}
