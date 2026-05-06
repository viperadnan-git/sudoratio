import { ArrowRight } from "lucide-react";

export interface DiffListItem {
  key: string;
  from: React.ReactNode;
  to: React.ReactNode;
}

export function DiffList({ items }: { items: DiffListItem[] }) {
  return (
    <ul className="max-h-[min(60vh,24rem)] divide-y overflow-y-auto rounded-md border bg-card/40 font-mono text-[11.5px]">
      {items.map((it) => (
        <li
          key={it.key}
          className="grid grid-cols-[1fr_auto] items-center gap-3 px-3 py-2"
        >
          <span className="truncate text-muted-foreground" title={it.key}>
            {it.key}
          </span>
          <span className="num flex items-center gap-2 text-right">
            <span className="text-muted-foreground/70 line-through">
              {it.from}
            </span>
            <ArrowRight
              className="size-3 text-muted-foreground/60"
              strokeWidth={2}
            />
            <span className="text-success">{it.to}</span>
          </span>
        </li>
      ))}
    </ul>
  );
}
