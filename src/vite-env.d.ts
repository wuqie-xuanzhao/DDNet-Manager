/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_DDNET_MANAGER_LOCAL_SMOKE?: string;
  readonly VITE_DDNET_MANAGER_LOCAL_SMOKE_CLIENT_INSTALL_DIR?: string;
  readonly VITE_DDNET_MANAGER_LOCAL_SMOKE_MANIFEST_URL?: string;
  readonly VITE_DDNET_MANAGER_LOCAL_SMOKE_CLOSE_WINDOW_ON_FINISH?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
