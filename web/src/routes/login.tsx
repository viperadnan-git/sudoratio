import { createFileRoute, redirect, useNavigate } from "@tanstack/react-router";
import { ArrowRight, KeyRound } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { z } from "zod";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { ApiError, api } from "@/lib/api";
import {
  clearToken,
  hexFromPassword,
  isAuthenticated,
  setToken,
} from "@/lib/auth";

const searchSchema = z.object({
  redirect: z.string().optional().catch(undefined),
});

export const Route = createFileRoute("/login")({
  validateSearch: searchSchema,
  beforeLoad: ({ search }) => {
    if (typeof window === "undefined") return;
    if (isAuthenticated()) {
      throw redirect({ to: (search.redirect ?? "/") as "/" });
    }
  },
  component: LoginPage,
});

function LoginPage() {
  const navigate = useNavigate();
  const search = Route.useSearch();
  const [password, setPassword] = useState("");
  const [submitting, setSubmitting] = useState(false);

  const onSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!password) return;
    setSubmitting(true);
    const token = hexFromPassword(password);
    setToken(token);
    try {
      await api("/api/v1/health");
      toast.success("Signed in");
      const dest = (search.redirect ?? "/") as "/";
      navigate({ to: dest, replace: true });
    } catch (err) {
      clearToken();
      if (err instanceof ApiError && err.status === 401) {
        toast.error("Wrong password");
      } else {
        toast.error("Could not reach the server");
      }
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="relative flex min-h-dvh items-center justify-center overflow-hidden bg-background p-4">
      {/* Atmosphere: subtle grid */}
      <div
        className="bg-grid pointer-events-none absolute inset-0 opacity-[0.35]"
        aria-hidden="true"
      />
      {/* Atmosphere: corner crosshairs */}
      <Crosshair className="left-4 top-4" />
      <Crosshair className="right-4 top-4" />
      <Crosshair className="bottom-4 left-4" />
      <Crosshair className="bottom-4 right-4" />

      <div className="relative w-full max-w-[400px]">
        {/* Header chip */}
        <div className="mb-6 flex items-center justify-between">
          <div className="flex items-center gap-2">
            <img
              src="/sudoratio.png"
              alt=""
              className="size-5 shrink-0"
              width={20}
              height={20}
            />
            <span className="brand text-[14px]">sudoratio</span>
          </div>
          <span className="eyebrow">v0.1 · auth</span>
        </div>

        {/* Card */}
        <div className="overflow-hidden rounded-md border bg-card">
          <div className="border-b px-5 py-4">
            <div className="eyebrow mb-1.5">Authenticate</div>
            <h1 className="text-[18px] font-semibold leading-tight">
              Restricted console
            </h1>
            <p className="mt-1.5 font-mono text-[11.5px] text-muted-foreground">
              single-password access · token = hex(password)
            </p>
          </div>

          <form onSubmit={onSubmit} className="space-y-4 p-5">
            <div className="space-y-2">
              <Label htmlFor="password" className="eyebrow">
                Password
              </Label>
              <div className="relative">
                <KeyRound
                  className="absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
                  strokeWidth={1.75}
                />
                <Input
                  id="password"
                  type="password"
                  autoComplete="current-password"
                  autoFocus
                  required
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  className="h-10 pl-9 font-mono text-[13px]"
                  placeholder="••••••••"
                />
              </div>
            </div>
            <Button
              type="submit"
              className="h-10 w-full gap-2 text-[13px]"
              disabled={submitting || !password}
            >
              {submitting ? "Verifying…" : "Sign in"}
              <ArrowRight className="size-4" strokeWidth={2} />
            </Button>
          </form>

          <div className="border-t bg-card/40 px-5 py-3">
            <p className="font-mono text-[10.5px] uppercase tracking-wider text-muted-foreground">
              curl ‑H "Authorization: Bearer {`<hex>`}"
            </p>
          </div>
        </div>

        {/* Footer caption */}
        <p className="mt-6 text-center font-mono text-[10.5px] uppercase tracking-[0.14em] text-muted-foreground/70">
          tracker · announce · simulator
        </p>
      </div>
    </div>
  );
}

function Crosshair({ className }: { className?: string }) {
  return (
    <span
      className={`pointer-events-none absolute size-3 ${className ?? ""}`}
      aria-hidden="true"
    >
      <span className="absolute inset-x-0 top-1/2 h-px -translate-y-1/2 bg-foreground/30" />
      <span className="absolute inset-y-0 left-1/2 w-px -translate-x-1/2 bg-foreground/30" />
    </span>
  );
}
