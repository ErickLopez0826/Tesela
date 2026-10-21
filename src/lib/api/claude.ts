import { invoke } from "@tauri-apps/api/core";

/** Identificador con el que la tarjeta del uso se abre en la ventana emergente compartida. */
export const USAGE_CARD_KEY = "claude-usage";

export type ClaudeActivityKind = "idle" | "working" | "waiting";

/** Algo que Claude tiene en marcha ahora: una sesión de Claude Code o uno de sus subagentes. */
export interface ClaudeAgent {
  /** La carpeta del proyecto (sesión) o el tipo de agente (subagente). */
  name: string;
  /** De dónde viene: «Claude Code · VS Code», «Subagente»… */
  source: string;
  state: Exclude<ClaudeActivityKind, "idle">;
  /** Es un subagente de la sesión que va justo antes. */
  sub: boolean;
}

export interface ClaudeActivity {
  activity: ClaudeActivityKind;
  /** Sesiones de Claude Code con avisos recientes. */
  sessions: number;
  /** Lo que está trabajando o esperando ahora. */
  agents?: ClaudeAgent[];
}

export interface HooksStatus {
  /** Los hooks de Tesela están en todos sus eventos. */
  installed: boolean;
  /** Están solo en algunos. */
  partial: boolean;
  settings_path: string;
  script_path: string;
  /** `settings.json` existe y es JSON válido (si no, no se puede instalar). */
  settings_ok: boolean;
}

export type UsageLevel = "ok" | "warn" | "danger";

export interface UsageWindow {
  /** `five_hour`, `seven_day`, `seven_day_opus`, `seven_day_sonnet`. */
  id: string;
  label: string;
  /** 0 a 100, gastado. */
  percent: number;
  /** Segundos desde 1970. */
  resets_at: number | null;
  level: UsageLevel;
}

export interface UsageShare {
  name: string;
  percent: number;
}

/** `off`: apagado; `waiting`: encendido y sin lectura aún; `ok`: al día; `stale`: hay una lectura vieja porque el último intento falló. */
export type UsageStatus = "off" | "waiting" | "ok" | "stale";

export type UsageProblem = "signed_out" | "expired" | "rejected" | "forbidden" | "rate_limited" | "offline" | "unexpected";

export interface UsageView {
  enabled: boolean;
  status: UsageStatus;
  windows: UsageWindow[];
  breakdown: UsageShare[];
  /** Segundos desde 1970 de la última lectura buena. */
  fetched_at: number | null;
  problem: UsageProblem | null;
  message: string | null;
}

export interface ClaudeApi {
  hooksStatus: () => Promise<HooksStatus>;
  hooksInstall: () => Promise<HooksStatus>;
  hooksUninstall: () => Promise<HooksStatus>;
  usageGet: () => Promise<UsageView>;
  usageSetEnabled: (enabled: boolean) => Promise<UsageView>;
  usageRefresh: () => Promise<UsageView>;
}

export const claudeApi: ClaudeApi & { activity: () => Promise<ClaudeActivity> } = {
  activity: () => invoke<ClaudeActivity>("claude_activity_get"),
  hooksStatus: () => invoke<HooksStatus>("claude_hooks_status"),
  hooksInstall: () => invoke<HooksStatus>("claude_hooks_install"),
  hooksUninstall: () => invoke<HooksStatus>("claude_hooks_uninstall"),
  usageGet: () => invoke<UsageView>("claude_usage_get"),
  usageSetEnabled: (enabled: boolean) => invoke<UsageView>("claude_usage_set_enabled", { enabled }),
  usageRefresh: () => invoke<UsageView>("claude_usage_refresh"),
};
