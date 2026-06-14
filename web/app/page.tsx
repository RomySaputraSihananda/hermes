import Link from "next/link";
import LiveDashboard from "@/components/LiveDashboard";

export default function DashboardPage() {
  return (
    <>
      {/* Hero */}
      <section className="pt-6 pb-16">
        <p className="eyebrow mb-5">Live Forward Test · Demo Account</p>
        <h1 className="text-5xl font-semibold tracking-[-0.032em] text-ink leading-[1.10] mb-5">
          HERMES Trading Bot
        </h1>
        <p className="text-[18px] text-ink-md leading-relaxed max-w-2xl mb-8">
          Open-source ICT algorithmic trading system built in Rust. Multi-path signal cascade
          (OTE → FVG → OB) with H4 trend confirmation, DOW filter, partial take-profits,
          automatic position sizing, and live MT5 execution.
        </p>
        <div className="flex gap-3 flex-wrap">
          <Link href="/trades"   className="btn-primary">View Trades</Link>
          <Link href="/backtest" className="btn-secondary">Backtest Results</Link>
        </div>
      </section>

      {/* Live data */}
      <LiveDashboard />

      {/* Strategy */}
      <section className="mb-16">
        <p className="eyebrow mb-6">Strategy</p>
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
          <FeatureCard
            icon="◈"
            title="BOS / CHoCH"
            body="Break of Structure or Change of Character confirms directional bias before any entry is considered."
          />
          <FeatureCard
            icon="◎"
            title="OTE → FVG → OB"
            body="Three-path signal cascade. OTE (61.8–78.6% Fibonacci) is primary. FVG and Order Block are fallbacks."
          />
          <FeatureCard
            icon="⬡"
            title="H1 + H4 Trend Filter"
            body="EMA-20 on H1 and H4 must agree on direction. Eliminates counter-trend noise and low-quality setups."
          />
          <FeatureCard
            icon="◉"
            title="Risk Management"
            body="Fixed % risk per trade. Min 2.5× RR. Partial TP at 1R and 2R. DOW filter skips Monday and Friday."
          />
        </div>
      </section>

      {/* Backtest CTA */}
      <section className="card-featured p-10 mb-16">
        <div className="flex flex-col sm:flex-row sm:items-center gap-6 mb-8">
          <div>
            <p className="eyebrow mb-2">Backtest Highlight · XAUUSDm</p>
            <h2 className="text-2xl font-semibold tracking-tight-sm text-ink">
              M15 · 50k Bars · 1% Risk
            </h2>
            <p className="text-sm text-ink-sub mt-1">XAUUSDm · ~25 months · 148 trades</p>
          </div>
          <Link href="/backtest" className="btn-primary sm:ml-auto shrink-0">
            Full Results →
          </Link>
        </div>
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-6 pt-6 border-t border-hl">
          {[
            { label: "Profit Factor", value: "3.90",   sub: "> 2.0 = strong"        },
            { label: "Net Return",    value: "+209%",   sub: "$5k → $15,433"         },
            { label: "Win Rate",      value: "54.1%",   sub: "148 trades"            },
            { label: "Max Drawdown",  value: "−$476",   sub: "Low friction: −$131"   },
          ].map(({ label, value, sub }) => (
            <div key={label}>
              <p className="text-xs text-ink-sub mb-1.5">{label}</p>
              <p className="font-mono text-xl font-semibold tracking-tight-md">{value}</p>
              <p className="text-xs text-ink-ter mt-1">{sub}</p>
            </div>
          ))}
        </div>
      </section>
    </>
  );
}

function FeatureCard({ icon, title, body }: { icon: string; title: string; body: string }) {
  return (
    <div className="card">
      <span className="text-accent text-lg mb-4 block">{icon}</span>
      <p className="font-medium text-[15px] text-ink mb-2 tracking-tight">{title}</p>
      <p className="text-sm text-ink-sub leading-relaxed">{body}</p>
    </div>
  );
}
