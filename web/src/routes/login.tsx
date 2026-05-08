import { useForm, useStore } from "@tanstack/react-form";
import { createFileRoute, redirect, useNavigate } from "@tanstack/react-router";
import { ArrowRight, KeyRound } from "lucide-react";
import { toast } from "sonner";
import { z } from "zod";

import { Button } from "@/components/ui/button";
import { Field, FieldError, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { ApiError, api } from "@/lib/api";
import {
  clearToken,
  hexFromPassword,
  isAuthenticated,
  setToken,
} from "@/lib/auth";
import { loginSchema } from "@/lib/schemas";
import { cn } from "@/lib/utils";

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

  const form = useForm({
    defaultValues: { password: "" },
    validators: {
      onSubmit: loginSchema,
      onSubmitAsync: async ({ value }) => {
        const token = hexFromPassword(value.password);
        setToken(token);
        try {
          await api("/api/v1/config", { skipAuthReset: true });
          return null;
        } catch (err) {
          clearToken();
          const message =
            err instanceof ApiError && err.status === 401
              ? "Wrong password"
              : "Could not reach the server";
          return { fields: { password: message } };
        }
      },
    },
    onSubmit: () => {
      toast.success("Signed in");
      navigate({ to: (search.redirect ?? "/") as "/", replace: true });
    },
  });

  const isSubmitting = useStore(form.store, (s) => s.isSubmitting);

  return (
    <div className="relative flex min-h-dvh items-center justify-center overflow-hidden bg-background p-4">
      <div
        className="bg-grid pointer-events-none absolute inset-0 opacity-[0.35]"
        aria-hidden="true"
      />
      <Crosshair className="left-4 top-4" />
      <Crosshair className="right-4 top-4" />
      <Crosshair className="bottom-4 left-4" />
      <Crosshair className="bottom-4 right-4" />

      <div className="relative w-full max-w-[400px]">
        <div className="mb-6 flex items-center gap-2">
          <img
            src="/sudoratio.png"
            alt=""
            className="size-5 shrink-0"
            width={20}
            height={20}
          />
          <span className="brand text-[14px]">sudoratio</span>
        </div>

        <div className="overflow-hidden rounded-md border bg-card">
          <div className="border-b px-5 py-4">
            <div className="eyebrow mb-1.5">Authenticate</div>
            <h1 className="text-[18px] font-semibold leading-tight">
              Restricted console
            </h1>
            <p className="mt-1.5 font-mono text-[11.5px] text-muted-foreground">
              single-password access
            </p>
          </div>

          <form
            onSubmit={(e) => {
              e.preventDefault();
              form.handleSubmit();
            }}
            className="space-y-4 p-5"
          >
            <form.Field name="password">
              {(field) => {
                const isInvalid =
                  field.state.meta.isTouched && !field.state.meta.isValid;
                return (
                  <Field data-invalid={isInvalid} className="space-y-2">
                    <FieldLabel htmlFor={field.name} className="eyebrow">
                      Password
                    </FieldLabel>
                    <div className="relative">
                      <KeyRound
                        className="absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
                        strokeWidth={1.75}
                      />
                      <Input
                        id={field.name}
                        name={field.name}
                        type="password"
                        autoComplete="current-password"
                        autoFocus
                        value={field.state.value}
                        onBlur={field.handleBlur}
                        onChange={(e) => field.handleChange(e.target.value)}
                        aria-invalid={isInvalid}
                        className="h-10 pl-9 font-mono text-[13px]"
                        placeholder="••••••••"
                      />
                    </div>
                    <div
                      aria-hidden={!isInvalid}
                      className={cn(
                        "grid transition-[grid-template-rows,opacity,margin-top] duration-200 ease-out",
                        isInvalid
                          ? "mt-1 grid-rows-[1fr] opacity-100"
                          : "mt-0 grid-rows-[0fr] opacity-0",
                      )}
                    >
                      <div className="min-h-0 overflow-hidden">
                        <FieldError
                          errors={field.state.meta.errors.map((e) =>
                            typeof e === "string" ? { message: e } : e,
                          )}
                        />
                      </div>
                    </div>
                  </Field>
                );
              }}
            </form.Field>

            <Button
              type="submit"
              className="h-10 w-full gap-2 text-[13px]"
              disabled={isSubmitting}
            >
              {isSubmitting ? "Verifying…" : "Sign in"}
              <ArrowRight className="size-4" strokeWidth={2} />
            </Button>
          </form>
        </div>

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
