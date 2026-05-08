// Single source of truth for form-shaped types. All types are defined here and inferred in types.ts.

import { z } from "zod";

/* ─────────────────────────── PresetPolicy ────────────────────────── */

export const presetPolicySchema = z.object({
  min_upload_speed: z.number().int("must be whole").min(0),
  max_upload_speed: z.number().int("must be whole").min(0),
  min_download_speed: z.number().int("must be whole").min(0),
  max_download_speed: z.number().int("must be whole").min(0),
  max_active_torrents: z.number().int("must be whole").min(1, "min 1"),
  upload_ratio_target: z.number(),
  pause_torrent_with_zero_leechers: z.boolean(),
  pause_torrent_with_zero_leechers_grace: z
    .number()
    .int("must be whole")
    .min(0),
  min_swarm_seeders_to_seed: z.number().int("must be whole").min(0),
  max_announce_jitter: z.number().int("must be whole").min(0),
  client_profile_id: z.string().nullable(),
});
export type PresetPolicy = z.infer<typeof presetPolicySchema>;

export const DEFAULT_POLICY: PresetPolicy = {
  min_upload_speed: 27,
  max_upload_speed: 183,
  min_download_speed: 800,
  max_download_speed: 1200,
  max_active_torrents: 5,
  upload_ratio_target: 3.0,
  pause_torrent_with_zero_leechers: false,
  pause_torrent_with_zero_leechers_grace: 10800,
  min_swarm_seeders_to_seed: 0,
  max_announce_jitter: 8,
  client_profile_id: null,
};

/* ─────────────────────────── Preset (form) ───────────────────────── */

const HEX_RE = /^#[0-9a-fA-F]{6}$/;

export const presetFormSchema = z.object({
  name: z.string().trim().min(1, "Name is required").max(64, "Name too long"),
  color: z.string().regex(HEX_RE, "Use #rrggbb"),
  policy: presetPolicySchema,
});
export type PresetForm = z.infer<typeof presetFormSchema>;

/* ─────────────────────────── EngineConfig ────────────────────────── */

export const configBodySchema = z.object({
  announce_port: z.number().int("must be whole").min(1).max(65535).nullable(),
  bandwidth_tick_ms: z.number().int("must be whole").min(1),
  max_concurrent_announces: z.number().int("must be whole").min(0),
  http_tracker_connect_timeout_secs: z
    .number()
    .int("must be whole")
    .min(0)
    .nullable(),
  http_tracker_request_timeout_secs: z
    .number()
    .int("must be whole")
    .min(0)
    .nullable(),
  http_tracker_max_idle_per_host: z
    .number()
    .int("must be whole")
    .min(0)
    .nullable(),
  http_tracker_max_redirects: z.number().int("must be whole").min(0).nullable(),
  http_tracker_tcp_keepalive_secs: z
    .number()
    .int("must be whole")
    .min(0)
    .nullable(),
  http_tracker_pool_idle_timeout_secs: z
    .number()
    .int("must be whole")
    .min(0)
    .nullable(),
});
export type ConfigBody = z.infer<typeof configBodySchema>;
export type ConfigUpdate = Partial<ConfigBody>;

/* ─────────────────────── Preset wire payloads ───────────────────── */

export type PresetPolicyUpdate = Partial<PresetPolicy>;

export interface PresetUpdateBody {
  name?: string;
  color?: string;
  policy?: PresetPolicyUpdate;
}

export interface PresetCreateBody {
  id?: string;
  name: string;
  color: string;
  policy?: PresetPolicy;
}

/* ─────────────────────────── Login ──────────────────────────────── */

export const loginSchema = z.object({
  password: z.string().min(1, "Password is required"),
});
export type LoginForm = z.infer<typeof loginSchema>;
