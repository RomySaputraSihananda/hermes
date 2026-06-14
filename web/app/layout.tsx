import type { Metadata } from "next";
import "./globals.css";
import Nav from "@/components/Nav";
import HermesLogo from "@/components/HermesLogo";
import { getLatestVersion, GITHUB_URL } from "@/lib/github";

const SITE_URL = "https://hermes.romys.my.id";

export const metadata: Metadata = {
  metadataBase: new URL(SITE_URL),
  title: {
    default: "HERMES — ICT Algorithmic Trading System",
    template: "%s · HERMES",
  },
  description:
    "Open-source ICT algorithmic trading system built in Rust. OTE/FVG/OB signal cascade with H4 trend confirmation, automated risk management, and live forward-test results.",
  keywords: [
    "algorithmic trading", "trading bot", "open source", "XAUUSD",
    "ICT trading", "inner circle trader", "Rust trading bot",
    "OTE entry", "FVG trading", "order block", "MT5 expert advisor",
  ],
  authors: [{ name: "Romy Saputra Sihananda" }],
  creator: "Romy Saputra Sihananda",
  robots: {
    index: true,
    follow: true,
    googleBot: { index: true, follow: true, "max-snippet": -1, "max-image-preview": "large" },
  },
  openGraph: {
    type: "website",
    url: SITE_URL,
    siteName: "HERMES Trading Bot",
    title: "HERMES — ICT Algorithmic Trading System",
    description: "Open-source ICT OTE/FVG/OB scalper built in Rust · Live forward-test · MT5",
    locale: "en_US",
  },
  twitter: {
    card: "summary_large_image",
    title: "HERMES — ICT Algorithmic Trading System",
    description: "Open-source ICT OTE/FVG/OB scalper built in Rust · Live forward-test · MT5",
  },
  icons: {
    icon: "/favicon.svg",
    shortcut: "/favicon.svg",
  },
};

export default async function RootLayout({ children }: { children: React.ReactNode }) {
  const version = await getLatestVersion();

  return (
    <html lang="en">
      <body className="min-h-screen antialiased">
        <div className="relative z-10">
          <Nav version={version} />
          <main className="max-w-5xl mx-auto px-4 sm:px-6 py-12">
            {children}
          </main>
          <footer style={{ borderTop: "1px solid var(--c-hl)" }} className="mt-24">
            <div className="max-w-5xl mx-auto px-6 py-16 grid grid-cols-1 sm:grid-cols-3 gap-10">
              <div>
                <div className="flex items-center gap-2 mb-3">
                  <HermesLogo size={24} />
                  <span className="font-mono text-sm font-semibold" style={{ letterSpacing: "0.18em", color: "var(--c-ink)" }}>HERMES</span>
                  {version && (
                    <span className="text-[10px] font-mono text-ink-ter bg-s2 border border-hl px-1.5 py-0.5 rounded">
                      {version}
                    </span>
                  )}
                </div>
                <p className="text-sm leading-relaxed mb-3" style={{ color: "var(--c-ink-sub)" }}>
                  Open-source ICT algorithmic trading system<br />built in Rust · M15 OTE / FVG / OB
                </p>
                <a
                  href={GITHUB_URL}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="inline-flex items-center gap-1.5 text-xs text-ink-sub hover:text-ink transition-colors"
                >
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
                    <path d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z"/>
                  </svg>
                  View on GitHub
                </a>
              </div>
              <div>
                <p className="eyebrow mb-4">Navigation</p>
                <ul className="space-y-2 text-sm" style={{ color: "var(--c-ink-sub)" }}>
                  <li><a href="/"         className="hover:text-ink transition-colors">Dashboard</a></li>
                  <li><a href="/trades"   className="hover:text-ink transition-colors">Trades</a></li>
                  <li><a href="/backtest" className="hover:text-ink transition-colors">Backtest</a></li>
                  <li>
                    <a href={GITHUB_URL} target="_blank" rel="noopener noreferrer" className="hover:text-ink transition-colors">
                      GitHub ↗
                    </a>
                  </li>
                </ul>
              </div>
              <div>
                <p className="eyebrow mb-4">Disclaimer</p>
                <p className="text-xs leading-relaxed" style={{ color: "var(--c-ink-ter)" }}>
                  Past performance does not guarantee future results. Demo account forward test. Not financial advice.
                </p>
              </div>
            </div>
          </footer>
        </div>
      </body>
    </html>
  );
}
