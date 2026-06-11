export interface ApiKeyEntry {
  name: string;
  key: string;
}

export interface ModelPoolEntry {
  id: string;
  name: string;
  base_url: string;
  api_key: string;
  model_name: string;
  priority: number;
  enabled: boolean;
  builtin: boolean;
  provider_type: string;
  api_format: string;
}

export interface PoolStatus {
  pool_mode: boolean;
  entries: ModelPoolEntry[];
}

export interface AppStatus {
  running: boolean;
  port: number;
  key_count: number;
  keys: ApiKeyEntry[];
  custom_models: string[];
}

export interface SpeedTestResult {
  model: string;
  success: boolean;
  error: string | null;
  latency_ms: number;
  tokens_per_sec: number;
  total_tokens: number;
  response_preview: string;
}
