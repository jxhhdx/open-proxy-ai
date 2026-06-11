import { invoke } from "@tauri-apps/api/core";
import type { AppStatus, PoolStatus, SpeedTestResult } from "../types";

export function getStatus(): Promise<AppStatus> {
  return invoke("get_status");
}

export function getModelPool(): Promise<PoolStatus> {
  return invoke("get_model_pool");
}

export function reorderPool(ids: string[]): Promise<PoolStatus> {
  return invoke("reorder_pool", { ids });
}

export function togglePoolEntry(id: string): Promise<PoolStatus> {
  return invoke("toggle_pool_entry", { id });
}

export function removePoolEntry(id: string): Promise<PoolStatus> {
  return invoke("remove_pool_entry", { id });
}

export function runSpeedTest(model: string): Promise<SpeedTestResult> {
  return invoke("run_speed_test_cmd", { req: { model } });
}

export function upsertPoolEntry(req: {
  id: string | null;
  name: string;
  base_url: string;
  api_key: string;
  model_name: string;
  priority: number;
  enabled: boolean;
  builtin: boolean;
  provider_type: string;
  api_format: string;
}): Promise<PoolStatus> {
  return invoke("upsert_pool_entry", { req });
}

export function importToTool(req: {
  model: string;
  model_name: string;
  api_key: string;
  tool: string;
}): Promise<string> {
  return invoke("import_to_tool", { req });
}
