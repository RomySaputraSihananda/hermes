"use client";
import Link from "next/link";
import { usePathname } from "next/navigation";
import clsx from "clsx";
import { GITHUB_URL } from "@/lib/github";
import HermesLogo from "./HermesLogo";

const links = [
  { href: "/",         label: "Dashboard" },
  { href: "/trades",   label: "Trades"    },
  { href: "/backtest", label: "Backtest"  },
];

export default function Nav({ version }: { version?: string | null }) {
  const path = usePathname();
  return (
    <nav className="sticky top-0 z-50" style={{ backgroundColor: "var(--c-canvas)", borderBottom: "1px solid var(--c-hl)" }}>
      <div className="max-w-5xl mx-auto px-4 sm:px-6 h-14 flex items-center gap-6">
        {/* logo + wordmark */}
        <Link href="/" className="flex items-center gap-2 group">
          <HermesLogo size={28} />
          <span className="font-mono text-sm font-semibold tracking-widest text-ink group-hover:text-accent transition-colors" style={{ letterSpacing: "0.18em" }}>
            HERMES
          </span>
          {version && (
            <span className="hidden sm:inline text-[10px] font-mono text-ink-ter bg-s2 border border-hl px-1.5 py-0.5 rounded">
              {version}
            </span>
          )}
        </Link>

        {/* nav links */}
        <div className="flex gap-1">
          {links.map(({ href, label }) => (
            <Link
              key={href}
              href={href}
              className={clsx(
                "px-3 py-1.5 rounded-md text-sm transition-colors",
                path === href
                  ? "bg-s2 text-ink"
                  : "text-ink-sub hover:text-ink hover:bg-s1"
              )}
            >
              {label}
            </Link>
          ))}
        </div>

        {/* right side */}
        <div className="ml-auto flex items-center gap-4">
          <a
            href={GITHUB_URL}
            target="_blank"
            rel="noopener noreferrer"
            className="text-ink-ter hover:text-ink transition-colors"
            aria-label="GitHub"
          >
            <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor">
              <path d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z"/>
            </svg>
          </a>
          <div className="flex items-center gap-2">
            <span className="w-1.5 h-1.5 rounded-full bg-bull pulse-dot" />
            <span className="text-xs text-ink-sub font-medium tracking-eyebrow uppercase">Live</span>
          </div>
        </div>
      </div>
    </nav>
  );
}
