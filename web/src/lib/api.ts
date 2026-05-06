// Fetch wrapper that injects Authorization: Bearer <hex> on every request.
// On 401, clears the token (auth gate redirects to /login on next render).

import { clearToken, getToken } from "@/lib/auth";

export class ApiError extends Error {
  status: number;
  code?: string;
  constructor(status: number, message: string, code?: string) {
    super(message);
    this.status = status;
    this.code = code;
  }
}

export interface ApiOptions {
  method?: string;
  body?: unknown;
  signal?: AbortSignal;
  headers?: Record<string, string>;
  /** When true, send the body as `application/octet-stream` (raw bytes). */
  rawBody?: boolean;
  /** When true, a 401 throws without clearing the stored token or redirecting. */
  skipAuthReset?: boolean;
}

export async function api<T = unknown>(
  path: string,
  opts: ApiOptions = {},
): Promise<T> {
  const token = getToken();
  const headers: Record<string, string> = {
    accept: "application/json",
    ...(opts.headers ?? {}),
  };
  if (token) headers.authorization = `Bearer ${token}`;

  let body: BodyInit | undefined;
  if (opts.body !== undefined) {
    if (opts.rawBody) {
      body = opts.body as BodyInit;
    } else {
      headers["content-type"] = headers["content-type"] ?? "application/json";
      body =
        typeof opts.body === "string" ? opts.body : JSON.stringify(opts.body);
    }
  }

  const res = await fetch(path, {
    method: opts.method ?? "GET",
    headers,
    body,
    signal: opts.signal,
  });

  if (res.status === 401) {
    if (!opts.skipAuthReset) {
      clearToken();
      window.location.replace("/login");
    }
    throw new ApiError(401, "unauthorized", "unauthorized");
  }

  const text = await res.text();
  let parsed: unknown = null;
  if (text) {
    try {
      parsed = JSON.parse(text);
    } catch {
      parsed = text;
    }
  }

  if (!res.ok) {
    const obj = (parsed as { code?: string; message?: string }) ?? {};
    throw new ApiError(
      res.status,
      obj.message ?? `HTTP ${res.status}`,
      obj.code,
    );
  }
  return parsed as T;
}
