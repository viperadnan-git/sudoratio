import { createFileRoute, Outlet, redirect } from "@tanstack/react-router";

import { AppShell } from "@/components/app-shell";
import { ApiError, api } from "@/lib/api";
import { clearToken, isAuthenticated } from "@/lib/auth";

export const Route = createFileRoute("/_authed")({
  // /health is unauthenticated by design — use a gated endpoint to verify
  // the token. 401 → clear + bounce; other errors fall through.
  beforeLoad: async ({ location }) => {
    if (typeof window === "undefined") return;
    if (!isAuthenticated()) {
      throw redirect({ to: "/login", search: { redirect: location.href } });
    }
    try {
      await api("/api/v1/config", { skipAuthReset: true });
    } catch (err) {
      if (err instanceof ApiError && err.status === 401) {
        clearToken();
        throw redirect({ to: "/login", search: { redirect: location.href } });
      }
    }
  },
  component: AuthedLayout,
});

function AuthedLayout() {
  return (
    <AppShell>
      <Outlet />
    </AppShell>
  );
}
