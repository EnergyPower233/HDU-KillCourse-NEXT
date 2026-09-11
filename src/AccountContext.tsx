import { createContext, useContext, useMemo, type ReactNode } from 'react';
import { api, createApi } from './bridge';
const ApiContext = createContext(api);
export function AccountProvider({ id, children }: { id: string; children: ReactNode }) {
  const client = useMemo(() => createApi(id), [id]);
  return <ApiContext.Provider value={client}>{children}</ApiContext.Provider>;
}
export function useApi() {
  return useContext(ApiContext);
}
