import { createFileRoute, Outlet, redirect } from "@tanstack/react-router";

import { AppShell } from "@/components/app-shell";
import { isAuthenticated } from "@/lib/auth";

/**
 * Pathless layout that guards every nested route. `beforeLoad` runs before any data is fetched
 * (and before the route's component mounts) and throws a `redirect` to `/login` if there is no
 * stored bearer token. Children render inside the [`AppShell`].
 */
export const Route = createFileRoute("/_authed")({
  beforeLoad: ({ location }) => {
    // beforeLoad runs server-side during SSR where localStorage is unavailable.
    // Skip the check on the server; the client re-evaluates on hydration.
    if (typeof window === "undefined") return;
    if (!isAuthenticated()) {
      throw redirect({
        to: "/login",
        search: { redirect: location.href },
      });
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
