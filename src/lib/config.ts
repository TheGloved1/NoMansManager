import { LazyStore } from '@tauri-apps/plugin-store';
import type { AppConfig } from './types';

const store = new LazyStore('settings.json');

const DEFAULTS: AppConfig = {
  game_path: null,
  deploy_mode: 'auto',
  active_profile: 'default',
  global_disable: false,
  theme: 'default',
  font: 'inter',
  auto_deploy: false,
};

export async function loadConfigNative(): Promise<AppConfig> {
  const game_path = await store.get<string | null>('game_path');
  const deploy_mode = await store.get<string>('deploy_mode');
  const active_profile = await store.get<string>('active_profile');
  const global_disable = await store.get<boolean>('global_disable');
  const auto_deploy = await store.get<boolean>('auto_deploy');
  const theme = await store.get<string>('theme');
  const font = await store.get<string>('font');

  return {
    game_path: game_path ?? DEFAULTS.game_path,
    deploy_mode: deploy_mode ?? DEFAULTS.deploy_mode,
    active_profile: active_profile ?? DEFAULTS.active_profile,
    global_disable: global_disable ?? DEFAULTS.global_disable,
    auto_deploy: auto_deploy ?? DEFAULTS.auto_deploy,
    theme: theme ?? DEFAULTS.theme,
    font: font ?? DEFAULTS.font,
  };
}

export async function saveConfigNative(cfg: AppConfig): Promise<void> {
  await store.set('game_path', cfg.game_path);
  await store.set('deploy_mode', cfg.deploy_mode);
  await store.set('active_profile', cfg.active_profile);
  await store.set('global_disable', cfg.global_disable);
  await store.set('auto_deploy', cfg.auto_deploy);
  await store.set('theme', cfg.theme);
  await store.set('font', cfg.font);
  await store.save();
}

// Also expose raw store for advanced use (e.g., profile active tracking)
export { store as settingsStore };
