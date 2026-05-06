import { Link, useLocation, useNavigate } from "@tanstack/react-router";
import {
  ArrowDownToLine,
  ArrowUpFromLine,
  Boxes,
  LogOut,
  Moon,
  Settings,
  Sun,
  UserSquare2,
} from "lucide-react";

import { SessionMenu } from "@/components/session-menu";
import { useTheme } from "@/components/theme-provider";
import { Button } from "@/components/ui/button";
import { clearToken } from "@/lib/auth";
import { useProfiles, useStats } from "@/lib/queries";
import { cn } from "@/lib/utils";

interface NavItem {
  to: "/" | "/config" | "/clients";
  label: string;
  short: string;
  Icon: typeof Boxes;
}

const NAV: NavItem[] = [
  { to: "/", label: "Torrents", short: "TRN", Icon: Boxes },
  { to: "/config", label: "Config", short: "CFG", Icon: Settings },
  { to: "/clients", label: "Clients", short: "CLI", Icon: UserSquare2 },
];

function fmtRate(bps: number | undefined): string {
  if (!bps || bps === 0) return "0";
  const units = ["B", "K", "M", "G"];
  let v = bps;
  let i = 0;
  while (v >= 1000 && i < units.length - 1) {
    v /= 1000;
    i++;
  }
  return `${v.toFixed(v >= 100 || i === 0 ? 0 : 1)}${units[i]}`;
}

export function AppShell({ children }: { children: React.ReactNode }) {
  const location = useLocation();
  const stats = useStats();
  const profiles = useProfiles();
  const activeClient = profiles.data?.find((p) => p.active);

  return (
    <div className="min-h-dvh bg-background text-foreground">
      {/* ═════════════ DESKTOP SIDEBAR ═════════════ */}
      <aside
        className={cn(
          "fixed inset-y-0 left-0 z-30 hidden w-[60px] flex-col border-r bg-sidebar transition-[width] duration-300 ease-out md:flex",
          "lg:w-[200px]",
        )}
      >
        <Link
          to="/"
          className="flex h-14 items-center gap-2.5 border-b px-4"
          aria-label="sudoratio home"
        >
          <img
            src="/sudoratio.png"
            alt=""
            className="size-6 shrink-0"
            width={24}
            height={24}
          />
          <span className="brand hidden text-sm tracking-tight lg:inline">
            sudoratio
          </span>
        </Link>

        <nav className="flex-1 px-2 py-4">
          <ul className="flex flex-col gap-0.5">
            {NAV.map(({ to, label, Icon }) => {
              const active =
                to === "/"
                  ? location.pathname === "/"
                  : location.pathname.startsWith(to);
              return (
                <li key={to}>
                  <Link
                    to={to}
                    className={cn(
                      "group flex items-center gap-3 rounded-md px-3 py-2 text-[13px] font-medium transition-colors",
                      active
                        ? "bg-accent text-foreground"
                        : "text-muted-foreground hover:bg-accent/60 hover:text-foreground",
                    )}
                  >
                    <span className="relative flex size-5 shrink-0 items-center justify-center">
                      <Icon className="size-4" strokeWidth={1.75} />
                      {active && (
                        <span
                          className="absolute -left-3 h-3.5 w-px bg-signal"
                          aria-hidden="true"
                        />
                      )}
                    </span>
                    <span className="hidden lg:inline">{label}</span>
                  </Link>
                </li>
              );
            })}
          </ul>
        </nav>

        {/* ── SIDEBAR FOOTER ── */}
        {/* lg ≥1024: rich session card. md 768–1023: just the SessionMenu trigger. */}
        <div className="border-t">
          <SidebarSessionLg className="hidden lg:block" />
          <div className="flex justify-center p-2 lg:hidden">
            <SessionMenu side="right" align="end" />
          </div>
        </div>
      </aside>

      {/* ═════════════ MAIN COLUMN ═════════════ */}
      <div className="md:ml-[60px] lg:ml-[200px]">
        {/* Telemetry strip — visible everywhere; sticks to top */}
        <header className="sticky top-0 z-20 border-b bg-background/85 backdrop-blur supports-[backdrop-filter]:bg-background/70">
          <div className="flex h-12 items-center gap-2 px-3 md:gap-3 md:px-6">
            {/* mobile brand */}
            <Link
              to="/"
              className="flex items-center gap-2 md:hidden"
              aria-label="sudoratio home"
            >
              <img
                src="/sudoratio.png"
                alt=""
                className="size-5 shrink-0"
                width={20}
                height={20}
              />
              <span className="brand text-sm">sudoratio</span>
            </Link>
            {/* breadcrumb / page title (desktop only) */}
            <div className="hidden items-center gap-2 md:flex">
              <span className="eyebrow">{currentLabel(location.pathname)}</span>
            </div>

            {/* live telemetry badges + active client (right side) */}
            <div className="ml-auto flex min-w-0 items-center gap-1.5 md:gap-2">
              <div className="hidden items-center gap-1.5 md:flex md:gap-2">
                <TelemetryPill
                  label="ACTIVE"
                  value={
                    stats.data
                      ? `${stats.data.active_torrents}/${stats.data.max_active_torrents}`
                      : "—"
                  }
                />
                <TelemetryPill
                  icon={<ArrowUpFromLine className="size-3" strokeWidth={2} />}
                  value={`${fmtRate(stats.data?.upload_speed)}B/s`}
                />
                <TelemetryPill
                  icon={<ArrowDownToLine className="size-3" strokeWidth={2} />}
                  value={`${fmtRate(stats.data?.download_speed)}B/s`}
                />
              </div>

              {activeClient && (
                <Link
                  to="/clients"
                  title={`Active client · ${activeClient.id}`}
                  className="group inline-flex h-7 min-w-0 items-center gap-1.5 rounded-md border border-border/70 px-2 transition-colors hover:border-foreground/30 hover:bg-accent/40"
                >
                  <UserSquare2
                    className="size-3 shrink-0 text-muted-foreground group-hover:text-foreground"
                    strokeWidth={1.75}
                  />
                  <span className="num truncate text-[11px] font-medium leading-none">
                    {activeClient.client}
                  </span>
                  <span className="num shrink-0 text-[10px] leading-none text-muted-foreground">
                    v{activeClient.version}
                  </span>
                </Link>
              )}

              {/* Mobile-only session menu — keeps theme + logout reachable */}
              <div className="md:hidden">
                <SessionMenu side="bottom" align="end" />
              </div>
            </div>
          </div>
        </header>

        <main className="pb-20 md:pb-12">{children}</main>
      </div>

      {/* ═════════════ MOBILE BOTTOM NAV ═════════════ */}
      <nav className="fixed inset-x-0 bottom-0 z-30 grid grid-cols-3 border-t bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/80 md:hidden">
        {NAV.map(({ to, label, Icon }) => {
          const active =
            to === "/"
              ? location.pathname === "/"
              : location.pathname.startsWith(to);
          return (
            <Link
              key={to}
              to={to}
              className={cn(
                "relative flex flex-col items-center justify-center gap-1 py-2.5 text-[10px] font-medium uppercase tracking-[0.14em] transition-colors",
                active
                  ? "text-foreground"
                  : "text-muted-foreground hover:text-foreground",
              )}
            >
              {active && (
                <span
                  className="absolute inset-x-6 top-0 h-px bg-signal"
                  aria-hidden="true"
                />
              )}
              <Icon
                className={cn("size-[18px]", active && "text-signal")}
                strokeWidth={1.75}
              />
              <span className="font-mono">{label}</span>
            </Link>
          );
        })}
      </nav>
    </div>
  );
}

/* ─────────────────────── SIDEBAR SESSION FOOTER (LG) ─────────────────────── */

function SidebarSessionLg({ className }: { className?: string }) {
  const { theme, toggle } = useTheme();
  const navigate = useNavigate();
  const isDark = theme === "dark";
  const onLogout = () => {
    clearToken();
    navigate({ to: "/login", replace: true });
  };

  return (
    <div className={cn("p-2.5", className)}>
      <div className="grid grid-cols-2 gap-1">
        <Button
          variant="ghost"
          size="sm"
          onClick={toggle}
          aria-label={isDark ? "Switch to light theme" : "Switch to dark theme"}
          title={isDark ? "Light theme" : "Dark theme"}
          className="h-8 gap-1.5 px-2 font-mono text-[10.5px] uppercase tracking-[0.12em]"
        >
          {isDark ? (
            <Sun className="size-3.5" strokeWidth={1.75} />
          ) : (
            <Moon className="size-3.5" strokeWidth={1.75} />
          )}
          <span>{isDark ? "Light" : "Dark"}</span>
        </Button>
        <Button
          variant="ghost"
          size="sm"
          onClick={onLogout}
          aria-label="Sign out"
          className="h-8 gap-1.5 px-2 font-mono text-[10.5px] uppercase tracking-[0.12em] text-destructive hover:bg-destructive/10 hover:text-destructive"
        >
          <LogOut className="size-3.5" strokeWidth={1.75} />
          <span>Exit</span>
        </Button>
      </div>
    </div>
  );
}

function currentLabel(path: string): string {
  if (path === "/") return "Torrents";
  if (path.startsWith("/config")) return "Configuration";
  if (path.startsWith("/clients")) return "Clients";
  return "—";
}

function TelemetryPill({
  label,
  value,
  icon,
  live,
}: {
  label?: string;
  value: string;
  icon?: React.ReactNode;
  live?: boolean;
}) {
  return (
    <span className="inline-flex h-7 items-center gap-1.5 rounded-md border border-border/70 px-2 font-mono text-[11px]">
      {live ? (
        <span className="text-success">
          <span className="dot-live" aria-hidden="true" />
        </span>
      ) : icon ? (
        <span className="text-muted-foreground" aria-hidden="true">
          {icon}
        </span>
      ) : null}
      {label && (
        <span className="hidden text-[10px] uppercase tracking-[0.14em] text-muted-foreground sm:inline">
          {label}
        </span>
      )}
      <span className="num font-medium tracking-tight">{value}</span>
    </span>
  );
}
