export interface Mod {
  id: string;
  display_name: string;
  type: string; // "pak" | "folder"
  source_path: string;
  size_bytes: number;
  has_lua: boolean;
  has_pak: boolean;
}

export interface Profile {
  name: string;
  mod_order: string[];
  enabled: Record<string, boolean>;
}

export interface AppConfig {
  game_path: string | null;
  deploy_mode: string;
  active_profile: string;
  global_disable: boolean;
  theme: string;
  font: string;
}

export interface ImportResult {
  imported: string[];
  skipped: string[];
}

export interface DeployResult {
  deployed: number;
  errors: string[];
}
