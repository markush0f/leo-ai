/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_IRA_API?: string;
  readonly VITE_IRA_REALTIME?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
