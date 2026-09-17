/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_LEO_API?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
