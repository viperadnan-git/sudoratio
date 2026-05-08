// All types are defined in schemas.ts and inferred here, no new types should be added here.

export type TorrentState = "downloading" | "seeding" | "queued" | "stopped";
export type StopReason =
  | "user"
  | "upload_ratio"
  | "no_leechers"
  | "tiny_swarm"
  | "tracker_failed";
export type AnnounceEvent = "none" | "started" | "stopped" | "completed";

export interface TrackersHttp {
  tiers: string[][];
}

export interface AnnounceHttpHeader {
  name: string;
  value: string;
}

export interface AnnounceRequestParams {
  port: number;
  uploaded: number;
  downloaded: number;
  left: number;
  event: AnnounceEvent;
}

export interface AnnounceRequestTrace {
  method: string;
  protocol: string;
  url: string;
  headers: AnnounceHttpHeader[];
  params: AnnounceRequestParams;
}

export interface AnnounceResponseTrace {
  status: number;
  headers: AnnounceHttpHeader[];
  body: unknown;
}

export interface AnnounceTrace {
  tracker_index: number;
  event: AnnounceEvent;
  announced_at: number;
  success: boolean;
  request: AnnounceRequestTrace;
  response: AnnounceResponseTrace;
  error_code?: string | null;
  error_message?: string | null;
}

export interface AnnouncesPage {
  items: AnnounceTrace[];
  total: number;
  limit: number;
  offset: number;
}

export interface Torrent {
  id: string;
  info_hash?: string | null;
  preset_id: string;
  name: string;
  size?: number | null;
  downloaded?: number | null;
  uploaded?: number | null;
  left?: number | null;
  download_speed?: number | null;
  upload_speed?: number | null;
  seeders?: number | null;
  leechers?: number | null;
  state: TorrentState;
  reason?: StopReason | null;
  download_before_seed: boolean;
  trackers: TrackersHttp;
  announce_interval?: number | null;
  min_announce_interval?: number | null;
  last_announced_at?: number | null;
  announces?: AnnounceTrace[] | null;
  queue_position: number;
}

export interface TorrentsPage {
  items: Torrent[];
  total: number;
  offset: number;
  limit: number;
}

export interface ClientProfileSummary {
  id: string;
  client: string;
  version: string;
  active: boolean;
  name: string;
  editable: boolean;
}

export interface SeedingStatus {
  running: boolean;
  upload_speed: number;
  download_speed: number;
  max_active_torrents: number;
  active_torrents: number;
  waiting_torrents: number;
  tracked_metainfo_torrents: number;
}

// Form-shaped types (PresetPolicy, ConfigBody, etc.) are sourced from schemas.ts.
export type {
  ConfigBody,
  ConfigUpdate,
  PresetCreateBody,
  PresetPolicy,
  PresetPolicyUpdate,
  PresetUpdateBody,
} from "@/lib/schemas";

import type { PresetPolicy } from "@/lib/schemas";

export interface PresetRollup {
  torrent_count: number;
  active_count: number;
  queued_count: number;
  upload_speed_bps: number;
  download_speed_bps: number;
}

export interface Preset {
  id: string;
  name: string;
  color: string;
  is_default: boolean;
  policy: PresetPolicy;
  created_at_ms: number;
  updated_at_ms: number;
  /** Embedded rollup (lives next to the preset in /presets responses). */
  rollup: PresetRollup;
}

export interface ConnectivityFamily {
  reachable: boolean;
  public_ip: string | null;
  error: string | null;
}

export interface ConnectivityResponse {
  port: number;
  checked_at_ms: number;
  ipv4: ConnectivityFamily;
  ipv6: ConnectivityFamily;
  via: string;
}

export interface HealthStatus {
  ok: boolean;
  version: string;
}
