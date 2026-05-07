// Wire types matching `sudoratio-core` shapes.

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

export interface PresetPolicy {
  min_upload_speed: number;
  max_upload_speed: number;
  min_download_speed: number;
  max_download_speed: number;
  max_active_torrents: number;
  upload_ratio_target: number;
  pause_torrent_with_zero_leechers: boolean;
  pause_torrent_with_zero_leechers_grace: number;
  min_swarm_seeders_to_seed: number;
  max_announce_jitter: number;
  /** `null` = inherit engine default. Otherwise a `client@version` variant id. */
  client_profile_id: string | null;
}

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

/** Engine infra config (per-tracker policy lives in presets). */
export interface ConfigBody {
  announce_port: number | null;
  bandwidth_tick_ms: number;
  max_concurrent_announces: number;
  http_tracker_connect_timeout_secs: number | null;
  http_tracker_request_timeout_secs: number | null;
  http_tracker_max_idle_per_host: number | null;
  http_tracker_max_redirects: number | null;
  http_tracker_tcp_keepalive_secs: number | null;
  http_tracker_pool_idle_timeout_secs: number | null;
}

export type ConfigUpdate = Partial<ConfigBody>;

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
