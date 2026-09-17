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
  auto_deploy: boolean;
}

export interface ImportResult {
  imported: string[];
  skipped: string[];
}

export interface DeployResult {
  deployed: number;
  errors: string[];
}

// --- Bases (save editing) ---

export interface SaveFileInfo {
  name: string;
  size_bytes: number;
  size_display: string;
  modified: string;
  mtime_ms: number;
}

export interface BaseSummary {
  idx: number;
  name: string;
  display_name: string;
  base_type: string;
  objects: number;
  owner_uid: string;
}

export interface TypeCounts {
  ship: number;
  planet: number;
  freighter: number;
  space: number;
  total_objs: number;
}

export interface DecompressResult {
  bases: BaseSummary[];
  counts: TypeCounts;
  backup_path: string;
}

export interface ExportResult {
  path: string;
  content: string;
}

export interface BaseImportResult {
  idx: number;
  objects: number;
  backup_path: string;
}

export interface BackupInfo {
  name: string;
  size_display: string;
  modified: string;
  path: string;
}
