/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_LEO_API?: string;
  readonly VITE_LEO_REALTIME?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
