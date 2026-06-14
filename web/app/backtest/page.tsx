import type { Metadata } from "next";
import Link from "next/link";

export const metadata: Metadata = {
  title: "Backtest Results",
  description: "HERMES backtest results: M15 ICT OTE/FVG/OB scalper on XAUUSDm. H4 trend confirmation, DOW filter, partial take-profits.",
};

const results: Array<{
  symbol: string; period: string; tf: string; risk: string;
  trades: number; wr: number; pf: number; ret: number; dd: number | null;
  highlight: boolean; note?: string;
}> = [
  { symbol: "XAUUSDm", period: "50k bars (~25 mo)", tf: "M15", risk: "1%", trades: 148, wr: 54.1, pf: 3.90, ret: 209.0, dd: -9.5, highlight: true,  note: "Optimal config" },
  { symbol: "GBPUSDm", period: "50k bars",          tf: "M15", risk: "1%", trades: 72,  wr: 56.9, pf: 3.42, ret: 143.0, dd: null, highlight: false, note: "Parameter sweep" },
  { symbol: "NZDUSDm", period: "50k bars",          tf: "M15", risk: "1%", trades: 54,  wr: 53.7, pf: 2.91, ret: 87.0,  dd: null, highlight: false, note: "Parameter sweep" },
  { symbol: "EURUSDm", period: "50k bars",          tf: "M15", risk: "1%", trades: 48,  wr: 52.1, pf: 2.67, ret: 62.0,  dd: null, highlight: false, note: "Parameter sweep" },
  { symbol: "BTCUSDm", period: "50k bars",          tf: "M15", risk: "1%", trades: 31,  wr: 58.1, pf: 2.44, ret: 48.0,  dd: null, highlight: false, note: "Parameter sweep" },
];

const params = [
  ["Timeframe",          "M15"],
  ["Symbol",             "XAUUSDm"],
  ["Killzone",           "08:00–13:00, 15:00–18:00 UTC"],
  ["EMA H1 Period",      "20"],
  ["EMA H4 Confirm",     "true (period 20)"],
  ["DOW Filter",         "Skip Mon + Fri"],
  ["Min RR",             "2.5×"],
  ["Swing Period",       "2"],
  ["Partial TP 1R",      "50% at 1R"],
  ["Partial TP 2R",      "25% at 2R"],
  ["Trailing 2R",        "false (hurts PF)"],
  ["Friday Close Hour",  "21:00 UTC"],
];

// Equity progression: $5,000 start → $15,433 end (XAUUSDm, 50k M15 bars, 1% risk)
const equityPoints = [
  { label: "Start",    value: 5000  },
  { label: "Mo 3",     value: 5240  },
  { label: "Mo 6",     value: 6180  },
  { label: "Mo 9",     value: 7350  },
  { label: "Mo 12",    value: 8820  },
  { label: "Mo 15",    value: 10400 },
  { label: "Mo 18",    value: 12100 },
  { label: "Mo 21",    value: 13800 },
  { label: "Mo 24",    value: 14920 },
  { label: "End",      value: 15433 },
];

export default function BacktestPage() {
  const maxEquity = Math.max(...equityPoints.map(p => p.value));
  const minEquity = Math.min(...equityPoints.map(p => p.value));
  const h = 80;
  const w = 340;
  const pts = equityPoints.map((p, i) => {
    const x = (i / (equityPoints.length - 1)) * w;
    const y = h - ((p.value - minEquity) / (maxEquity - minEquity)) * h;
    return `${x},${y}`;
  }).join(" ");

  return (
    <>
      <section className="pt-6 pb-12">
        <p className="eyebrow mb-4">Quantitative Analysis</p>
        <h1 className="text-4xl font-semibold tracking-[-0.032em] text-ink mb-3">
          Backtest Results
        </h1>
        <p className="text-[15px] text-ink-sub max-w-xl">
          Historical simulation on real MT5 OHLC data. Includes commission ($7/lot) and slippage.
          London + NY killzone filter applied.
        </p>
      </section>

      {/* Highlight card */}
      <div className="card-featured p-10 mb-6">
        <div className="flex flex-col sm:flex-row sm:items-start gap-4 mb-8">
          <div>
            <span className="status-pill status-gold mb-3 inline-block">XAUUSDm · Best Result</span>
            <h2 className="text-2xl font-semibold tracking-tight-sm text-ink">M15 · 50k Bars · 1% Risk</h2>
            <p className="text-sm text-ink-sub mt-1">~25 months · 148 trades · $5,000 start</p>
          </div>
          <Link href="/trades" className="btn-primary sm:ml-auto shrink-0">View Live Trades →</Link>
        </div>
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-6 pt-8 border-t border-hl">
          {[
            { label: "Profit Factor", value: "3.90",    note: "> 2.0 = strong"      },
            { label: "Net Return",    value: "+209%",    note: "$5,000 → $15,433"    },
            { label: "Win Rate",      value: "54.1%",    note: "148 trades"          },
            { label: "Max Drawdown",  value: "−$476",    note: "Friction only −$131" },
          ].map(({ label, value, note }) => (
            <div key={label}>
              <p className="text-xs text-ink-sub mb-1.5">{label}</p>
              <p className="font-mono text-xl font-semibold text-ink">{value}</p>
              <p className="text-xs text-ink-ter mt-1">{note}</p>
            </div>
          ))}
        </div>
      </div>

      {/* Equity curve */}
      <div className="card mb-10 p-6">
        <div className="flex items-center justify-between mb-5">
          <div>
            <p className="eyebrow mb-1">Equity Progression</p>
            <p className="text-xs text-ink-ter">50k M15 bars · 1% risk · XAUUSDm</p>
          </div>
          <div className="text-right">
            <p className="font-mono text-lg font-semibold text-bull">$15,433</p>
            <p className="text-xs text-ink-ter">from $5,000</p>
          </div>
        </div>
        <div className="relative w-full overflow-x-auto">
          <svg viewBox={`0 0 ${w} ${h + 20}`} className="w-full" style={{ minWidth: 280 }}>
            {[0, 0.25, 0.5, 0.75, 1].map((t) => {
              const y   = h - t * h;
              const val = Math.round(minEquity + t * (maxEquity - minEquity));
              return (
                <g key={t}>
                  <line x1="0" y1={y} x2={w} y2={y} stroke="#23252a" strokeWidth="0.5" />
                  <text x={w + 2} y={y + 4} fontSize="7" fill="#62666d" textAnchor="start">
                    ${(val / 1000).toFixed(0)}k
                  </text>
                </g>
              );
            })}
            <defs>
              <linearGradient id="areaGrad" x1="0" y1="0" x2="0" y2="1">
                <stop offset="0%"   stopColor="#d4a843" stopOpacity="0.22" />
                <stop offset="100%" stopColor="#d4a843" stopOpacity="0.01" />
              </linearGradient>
            </defs>
            <polygon points={`0,${h} ${pts} ${w},${h}`} fill="url(#areaGrad)" />
            <polyline
              points={pts}
              fill="none"
              stroke="#d4a843"
              strokeWidth="1.5"
              strokeLinejoin="round"
              strokeLinecap="round"
            />
            {equityPoints.map((p, i) => {
              const x = (i / (equityPoints.length - 1)) * w;
              const y = h - ((p.value - minEquity) / (maxEquity - minEquity)) * h;
              return (
                <g key={i}>
                  <circle cx={x} cy={y} r="2.5" fill="#d4a843" />
                  <text x={x} y={h + 14} fontSize="6.5" fill="#62666d" textAnchor="middle">
                    {p.label}
                  </text>
                </g>
              );
            })}
          </svg>
        </div>
      </div>

      {/* Filter impact cards */}
      <div className="mb-10">
        <p className="eyebrow mb-5">Key Parameter Findings</p>
        <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
          <div className="card p-5">
            <div className="flex items-center gap-2 mb-3">
              <span className="text-accent text-base">◈</span>
              <p className="text-sm font-medium text-ink">H4 Trend Confirm</p>
            </div>
            <p className="text-sm text-ink-sub leading-relaxed mb-4">
              Requiring H4 EMA-20 direction to agree with H1 EMA eliminates counter-trend noise universally across all tested pairs.
            </p>
            <div className="flex gap-3 pt-3 border-t border-hl">
              <div>
                <p className="text-[10px] text-ink-ter mb-0.5">Impact</p>
                <p className="font-mono text-sm font-semibold text-bull">PF ↑ all pairs</p>
              </div>
              <div className="w-px bg-hl" />
              <div>
                <p className="text-[10px] text-ink-ter mb-0.5">Setting</p>
                <p className="font-mono text-sm font-semibold text-ink">always true</p>
              </div>
            </div>
          </div>

          <div className="card p-5">
            <div className="flex items-center gap-2 mb-3">
              <span className="text-accent text-base">⬡</span>
              <p className="text-sm font-medium text-ink">Signal Cascade</p>
            </div>
            <p className="text-sm text-ink-sub leading-relaxed mb-4">
              Three-path OTE → FVG → OB cascade increased trade frequency from ~5/month to ~6/month while maintaining high PF.
            </p>
            <div className="flex gap-3 pt-3 border-t border-hl">
              <div>
                <p className="text-[10px] text-ink-ter mb-0.5">Signals/mo</p>
                <p className="font-mono text-sm font-semibold text-bull">5 → 6+</p>
              </div>
              <div className="w-px bg-hl" />
              <div>
                <p className="text-[10px] text-ink-ter mb-0.5">PF retained</p>
                <p className="font-mono text-sm font-semibold text-bull">3.90</p>
              </div>
            </div>
          </div>

          <div className="card p-5">
            <div className="flex items-center gap-2 mb-3">
              <span className="text-accent text-base">◎</span>
              <p className="text-sm font-medium text-ink">Trailing 2R = false</p>
            </div>
            <p className="text-sm text-ink-sub leading-relaxed mb-4">
              Moving SL to 1R when 2R is hit consistently reduced PF across all 32 sweep combinations. Partial TP is superior.
            </p>
            <div className="flex gap-3 pt-3 border-t border-hl">
              <div>
                <p className="text-[10px] text-ink-ter mb-0.5">vs trailing on</p>
                <p className="font-mono text-sm font-semibold text-bull">PF higher</p>
              </div>
              <div className="w-px bg-hl" />
              <div>
                <p className="text-[10px] text-ink-ter mb-0.5">Setting</p>
                <p className="font-mono text-sm font-semibold text-ink">always false</p>
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Results table */}
      <div className="rounded-lg border border-hl overflow-hidden mb-10">
        <div className="px-6 py-4 border-b border-hl">
          <p className="eyebrow">All Runs</p>
        </div>
        <div className="overflow-x-auto">
          <table className="data-table">
            <thead>
              <tr>
                {["Symbol", "Period", "TF", "Risk", "Trades", "Win Rate", "Profit Factor", "Return", "Max DD", ""].map(h => (
                  <th key={h}>{h}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {results.map((r, i) => (
                <tr key={i} className={r.highlight ? "bg-s2" : ""}>
                  <td className="font-mono font-medium text-ink">{r.symbol}</td>
                  <td className="font-medium text-ink">{r.period}</td>
                  <td className="font-mono text-ink-sub">{r.tf}</td>
                  <td className="font-mono text-ink-sub">{r.risk}</td>
                  <td className="font-mono text-ink-md">{r.trades}</td>
                  <td className="font-mono text-ink-md">{r.wr.toFixed(1)}%</td>
                  <td className="font-mono font-medium">
                    <span className={r.pf >= 2.0 ? "text-bull" : r.pf < 1 ? "text-bear" : "text-ink-md"}>
                      {r.pf.toFixed(2)}
                    </span>
                  </td>
                  <td className="font-mono font-medium">
                    <span className={r.ret >= 0 ? "text-bull" : "text-bear"}>
                      {r.ret >= 0 ? "+" : ""}{r.ret.toFixed(1)}%
                    </span>
                  </td>
                  <td className="font-mono">
                    {r.dd != null
                      ? <span className={Math.abs(r.dd) > 25 ? "text-bear" : "text-ink-md"}>{r.dd.toFixed(1)}%</span>
                      : <span className="text-ink-ter">—</span>}
                  </td>
                  <td className="text-xs text-ink-ter">{r.note ?? ""}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>

      {/* Params + How it works */}
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-6 mb-10">
        <div className="card">
          <p className="eyebrow mb-6">Parameters</p>
          <dl className="divide-y divide-hl">
            {params.map(([k, v]) => (
              <div key={k} className="flex justify-between py-3">
                <dt className="text-sm text-ink-sub">{k}</dt>
                <dd className="font-mono text-sm font-medium text-ink">{v}</dd>
              </div>
            ))}
          </dl>
        </div>

        <div className="card">
          <p className="eyebrow mb-6">How It Works</p>
          <ol className="space-y-4">
            {[
              ["Detect Structure",    "BOS or CHoCH confirms directional bias on the rolling candle window."],
              ["Trend Alignment",     "H1 EMA-20 and H4 EMA-20 must both agree with the BOS direction."],
              ["Killzone + DOW",      "Entry only during 08–13 UTC and 15–18 UTC. Skip Monday and Friday."],
              ["Signal Cascade",      "Try OTE (Fib 61.8–78.6%) first. Fallback to FVG, then OB if no OTE."],
              ["SL / TP / Partials",  "SL at PD array boundary. TP at swing extreme. Close 50% at 1R, 25% at 2R."],
            ].map(([title, desc], i) => (
              <li key={i} className="flex gap-4">
                <span className="flex-shrink-0 w-6 h-6 rounded-full bg-s2 border border-hl text-xs font-mono text-ink-sub flex items-center justify-center">
                  {i + 1}
                </span>
                <div>
                  <p className="text-sm font-medium text-ink">{title}</p>
                  <p className="text-sm text-ink-sub mt-0.5 leading-relaxed">{desc}</p>
                </div>
              </li>
            ))}
          </ol>
        </div>
      </div>

      {/* Disclaimer */}
      <div className="rounded-lg border border-hl p-5">
        <p className="text-xs text-ink-ter leading-relaxed">
          <span className="text-ink-sub font-medium">Disclaimer — </span>
          Backtests simulate on historical OHLC data and do not account for requotes, broker restrictions, or changing market regimes.
          Past results do not guarantee future performance. For informational purposes only.
        </p>
      </div>
    </>
  );
}
