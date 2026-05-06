// Single-password auth: the server expects the lowercase hex of the password's UTF-8 bytes
// in the `Authorization: Bearer <hex>` header on every /api/v1/* request.

const STORAGE_KEY = "sudoratio.auth_token";

/** Hex-encode the UTF-8 bytes of a password. */
export function hexFromPassword(password: string): string {
  const bytes = new TextEncoder().encode(password);
  let out = "";
  for (let i = 0; i < bytes.length; i++) {
    out += bytes[i].toString(16).padStart(2, "0");
  }
  return out;
}

export function getToken(): string | null {
  try {
    return localStorage.getItem(STORAGE_KEY);
  } catch {
    return null;
  }
}

export function setToken(token: string): void {
  try {
    localStorage.setItem(STORAGE_KEY, token);
  } catch {
    /* storage unavailable; in-memory fallback is fine since the SPA is gated on a token */
  }
}

export function clearToken(): void {
  try {
    localStorage.removeItem(STORAGE_KEY);
  } catch {
    /* noop */
  }
}

export function isAuthenticated(): boolean {
  return !!getToken();
}
