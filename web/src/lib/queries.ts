// TanStack Query hooks for every /api/v1/* route. Live polls are 2s for high-frequency lists,
// and config/profiles are fetched on-demand (invalidated after mutations).

import {
  type QueryClient,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";

import { api } from "@/lib/api";
import type {
  ClientProfileSummary,
  ConfigBody,
  ConfigUpdate,
  ConnectivityResponse,
  HealthStatus,
  SeedingStatus,
  Torrent,
} from "@/lib/types";

export const qk = {
  health: ["health"] as const,
  config: ["config"] as const,
  configDefaults: ["config", "defaults"] as const,
  profiles: ["profiles"] as const,
  stats: ["stats"] as const,
  torrents: (withAnnounces = false) => ["torrents", { withAnnounces }] as const,
  torrent: (infoHash: string) => ["torrents", infoHash] as const,
  announces: (infoHash: string, limit: number, offset: number) =>
    ["torrents", infoHash, "announces", limit, offset] as const,
};

export function invalidateTorrents(qc: QueryClient) {
  return qc.invalidateQueries({ queryKey: ["torrents"] });
}

export function useHealth() {
  return useQuery({
    queryKey: qk.health,
    queryFn: () => api<HealthStatus>("/api/v1/health"),
    staleTime: 30_000,
  });
}

export function useConfig() {
  return useQuery({
    queryKey: qk.config,
    queryFn: () => api<ConfigBody>("/api/v1/config"),
  });
}

export function fetchConfigDefaults(qc: QueryClient) {
  return qc.fetchQuery({
    queryKey: qk.configDefaults,
    queryFn: () => api<ConfigBody>("/api/v1/config/defaults"),
    staleTime: Number.POSITIVE_INFINITY,
  });
}

export function useCheckConnectivity() {
  return useMutation({
    mutationFn: (port?: number) =>
      api<ConnectivityResponse>("/api/v1/diagnostics/connectivity", {
        method: "POST",
        body: port != null ? { port } : {},
      }),
  });
}

export function useUpdateConfig() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (patch: ConfigUpdate) =>
      api<ConfigBody>("/api/v1/config", { method: "PATCH", body: patch }),
    onSuccess: (data) => qc.setQueryData(qk.config, data),
  });
}

export function useStats() {
  return useQuery({
    queryKey: qk.stats,
    queryFn: () => api<SeedingStatus>("/api/v1/stats"),
    refetchInterval: 2_000,
  });
}

export function useTorrents(withAnnounces = false) {
  return useQuery({
    queryKey: qk.torrents(withAnnounces),
    queryFn: () =>
      api<Torrent[]>(
        withAnnounces ? "/api/v1/torrents?with=announces" : "/api/v1/torrents",
      ),
    refetchInterval: 2_000,
  });
}

export function useTorrent(infoHash: string | undefined) {
  return useQuery({
    queryKey: infoHash ? qk.torrent(infoHash) : ["torrents", "__none__"],
    queryFn: () => api<Torrent>(`/api/v1/torrents/${infoHash}`),
    enabled: !!infoHash,
    refetchInterval: 2_000,
  });
}

export function useTorrentAnnounces(
  infoHash: string | undefined,
  limit = 25,
  offset = 0,
) {
  return useQuery({
    queryKey: infoHash
      ? qk.announces(infoHash, limit, offset)
      : ["announces", "__none__"],
    queryFn: () => {
      const params = new URLSearchParams();
      params.set("limit", String(limit));
      params.set("offset", String(offset));
      return api<import("@/lib/types").AnnouncesPage>(
        `/api/v1/torrents/${infoHash}/announces?${params.toString()}`,
      );
    },
    enabled: !!infoHash,
    placeholderData: (prev) => prev,
    // Refetch only while the sheet is mounted (the hook is only used there).
    // Pause when the tab isn't focused to avoid background traffic.
    refetchInterval: 5_000,
    refetchIntervalInBackground: false,
  });
}

export function useAddTorrent() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({
      file,
      downloadBeforeSeed,
    }: {
      file: File | Blob;
      downloadBeforeSeed: boolean;
    }) => {
      const form = new FormData();
      form.append(
        "file",
        file,
        file instanceof File ? file.name : "torrent.torrent",
      );
      form.append(
        "download_before_seed",
        downloadBeforeSeed ? "true" : "false",
      );
      return api<{ info_hash: string }>("/api/v1/torrents", {
        method: "POST",
        body: form,
        rawBody: true,
      });
    },
    onSuccess: () => invalidateTorrents(qc),
  });
}

export function usePauseTorrent() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (infoHash: string) =>
      api(`/api/v1/torrents/${infoHash}/pause`, { method: "POST" }),
    onSuccess: () => invalidateTorrents(qc),
  });
}

export function useResumeTorrent() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (infoHash: string) =>
      api(`/api/v1/torrents/${infoHash}/resume`, { method: "POST" }),
    onSuccess: () => invalidateTorrents(qc),
  });
}

export function useDeleteTorrent() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (infoHash: string) =>
      api(`/api/v1/torrents/${infoHash}`, { method: "DELETE" }),
    onSuccess: () => invalidateTorrents(qc),
  });
}

export function useAnnounceTorrent() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({
      infoHash,
      event,
    }: {
      infoHash: string;
      event: "none" | "started" | "stopped" | "completed";
    }) =>
      api(`/api/v1/torrents/${infoHash}/announce`, {
        method: "POST",
        body: { event },
      }),
    onSuccess: () => invalidateTorrents(qc),
  });
}

export function useProfiles() {
  return useQuery({
    queryKey: qk.profiles,
    queryFn: () => api<ClientProfileSummary[]>("/api/v1/clients"),
  });
}

export interface RegisterClientResponse {
  client: string;
  ids: string[];
}

export function useRegisterClient() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (toml: string) =>
      api<RegisterClientResponse>("/api/v1/clients", {
        method: "POST",
        body: { toml },
      }),
    onSuccess: () => qc.invalidateQueries({ queryKey: qk.profiles }),
  });
}

export function useActivateVariant() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) =>
      api(`/api/v1/clients/variants/${encodeURIComponent(id)}/activate`, {
        method: "POST",
      }),
    onSuccess: () => qc.invalidateQueries({ queryKey: qk.profiles }),
  });
}

export interface ClientSource {
  client: string;
  editable: boolean;
  toml: string;
}

/** Doc TOML for a client family (shared by every variant). */
export function useClientSource(client: string | null) {
  return useQuery({
    queryKey: client
      ? (["clients", client, "source"] as const)
      : ["clients", "__none__"],
    queryFn: () =>
      api<ClientSource>(
        `/api/v1/clients/${encodeURIComponent(client ?? "")}/source`,
      ),
    enabled: !!client,
    staleTime: 60_000,
  });
}

export function useDeleteClient() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (client: string) =>
      api(`/api/v1/clients/${encodeURIComponent(client)}`, {
        method: "DELETE",
      }),
    onSuccess: () => qc.invalidateQueries({ queryKey: qk.profiles }),
  });
}
