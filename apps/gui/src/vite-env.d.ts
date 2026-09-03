/// <reference types="vite/client" />

declare const __APP_VERSION__: string;

interface ImportMetaEnv {
  readonly VITE_MOCK_IPC?: string;
  readonly VITE_MOCK_UPDATE?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
