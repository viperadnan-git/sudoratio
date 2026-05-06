import { useNavigate } from "@tanstack/react-router";
import { LogOut, Moon, Settings, Sun } from "lucide-react";

import { useTheme } from "@/components/theme-provider";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { clearToken } from "@/lib/auth";
import { cn } from "@/lib/utils";

/**
 * Compact session menu used in the mobile header and the md-collapsed desktop sidebar.
 * Houses the two utility actions (theme toggle, sign out) that don't deserve dedicated
 * top-level real estate but still need to be reachable on every screen.
 */
export function SessionMenu({
  align = "end",
  side = "bottom",
  className,
}: {
  align?: "start" | "end" | "center";
  side?: "top" | "right" | "bottom" | "left";
  className?: string;
}) {
  const { theme, toggle } = useTheme();
  const navigate = useNavigate();
  const isDark = theme === "dark";

  const onLogout = () => {
    clearToken();
    navigate({ to: "/login", replace: true });
  };

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          variant="ghost"
          size="icon"
          className={cn(
            "h-8 w-8 border border-border/70 bg-card/40 hover:bg-accent/60",
            className,
          )}
          aria-label="Session menu"
        >
          <Settings className="size-3.5" strokeWidth={1.75} />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent
        side={side}
        align={align}
        sideOffset={8}
        className="w-56 p-1"
      >
        <div>
          <DropdownMenuItem
            onSelect={(e) => {
              e.preventDefault();
              toggle();
            }}
            className="cursor-pointer gap-2.5 font-mono text-[12px]"
          >
            {isDark ? (
              <Sun className="size-3.5" strokeWidth={1.75} />
            ) : (
              <Moon className="size-3.5" strokeWidth={1.75} />
            )}
            <span className="flex-1">
              {isDark ? "Light theme" : "Dark theme"}
            </span>
            <span className="eyebrow text-muted-foreground/80">
              {isDark ? "DARK" : "LIGHT"}
            </span>
          </DropdownMenuItem>

          <DropdownMenuSeparator />

          <DropdownMenuItem
            onSelect={(e) => {
              e.preventDefault();
              onLogout();
            }}
            className="cursor-pointer gap-2.5 font-mono text-[12px] text-destructive focus:text-destructive"
          >
            <LogOut className="size-3.5" strokeWidth={1.75} />
            <span className="flex-1">Sign out</span>
            <span className="eyebrow text-destructive/70">EXIT</span>
          </DropdownMenuItem>
        </div>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
