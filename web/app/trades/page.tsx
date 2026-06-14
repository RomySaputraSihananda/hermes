import type { Metadata } from "next";
import { getAccount, getAllDeals, type Deal } from "@/lib/mt5";
import { formatCurrency, formatDate } from "@/lib/format";
import EquityChart from "@/components/EquityChart";
import TradesRefresher from "@/components/TradesRefresher";

export const metadata: Metadata = {
  title: "Trade History",
  description: "Live closed trades and equity curve for HERMES — ICT OTE/FVG/OB scalper on XAUUSDm.",
};

export const dynamic = "force-dynamic";
const MAGIC = 19731;

export default async function TradesPage() {
  let deals: Deal[] = [];
  let account = null;
  let error = false;

  try {
    [account, deals] = await Promise.all([getAccount(), getAllDeals()]);
  } catch { error = true; }

  const hermesDeals = deals.filter(d => d.magic === MAGIC && d.type !== 2);
  const closed      = hermesDeals.filter(d => d.entry === 1);
  const wins        = closed.filter(d => d.profit > 0);
  const losses      = closed.filter(d => d.profit < 0);
  const netPnl      = closed.reduce((s, d) => s + d.profit + d.swap + d.commission, 0);
  const totalFriction = closed.reduce((s, d) => s + d.swap + d.commission, 0);
  const grossW      = wins.reduce((s, d) => s + d.profit, 0);
  const grossL      = Math.abs(losses.reduce((s, d) => s + d.profit, 0));
  const pf          = grossL > 0 ? grossW / grossL : 0;
  const wr          = closed.length > 0 ? (wins.length / closed.length) * 100 : 0;
  const avgWin      = wins.length   > 0 ? grossW / wins.length   : 0;
  const avgLoss     = losses.length > 0 ? grossL / losses.length : 0;
  const currency    = account?.currency ?? "USD";

  const bestTrade  = closed.reduce<Deal | null>((b, d) => !b || d.profit > b.profit ? d : b, null);
  const worstTrade = closed.reduce<Deal | null>((b, d) => !b || d.profit < b.profit ? d : b, null);

  const streak = (() => {
    if (closed.length === 0) return { count: 0, type: "none" as const };
    const sorted = [...closed].sort((a, b) => +new Date(a.time) - +new Date(b.time));
    const last   = sorted[sorted.length - 1];
    const isWin  = last.profit > 0;
    let count = 0;
    for (let i = sorted.length - 1; i >= 0; i--) {
      if ((sorted[i].profit > 0) === isWin) count++;
      else break;
    }
    return { count, type: isWin ? "win" as const : "loss" as const };
  })();

  return (
    <>
      <section className="pt-6 pb-12">
        <p className="eyebrow mb-4">Live Forward Test</p>
        <h1 className="text-4xl font-semibold tracking-[-0.032em] text-ink mb-3">
          Trade History
        </h1>
        <p className="text-[15px] text-ink-sub">
          All closed trades from HERMES · magic {MAGIC}
        </p>
        <div className="mt-3 flex items-center gap-2">
          <span className="w-1.5 h-1.5 rounded-full bg-bull pulse-dot" />
          <TradesRefresher />
        </div>
      </section>

      {error ? (
        <div className="card text-center text-ink-sub text-sm mb-12 py-10">
          Unable to fetch trade data.
        </div>
      ) : (
        <>
          {/* Stats */}
          <div className="grid grid-cols-2 sm:grid-cols-4 gap-3 mb-4">
            {[
              { label: "Total Trades",  value: closed.length.toString(),           mono: true },
              { label: "Win Rate",      value: `${wr.toFixed(1)}%`,                mono: true },
              { label: "Profit Factor", value: pf > 0 ? pf.toFixed(2) : "—",      mono: true },
              { label: "Net P&L",       value: formatCurrency(netPnl, currency),   mono: true, colored: netPnl !== 0 ? netPnl > 0 : undefined },
            ].map(({ label, value, mono, colored }) => (
              <div key={label} className="stat-tile">
                <p className="text-xs text-ink-sub mb-2">{label}</p>
                <p className={`${mono ? "font-mono" : ""} text-xl font-semibold ${
                  colored === true ? "text-bull" : colored === false ? "text-bear" : "text-ink"
                }`}>{value}</p>
              </div>
            ))}
          </div>

          {/* Secondary stats */}
          <div className="grid grid-cols-2 sm:grid-cols-4 gap-3 mb-8">
            <div className="stat-tile">
              <p className="text-xs text-ink-sub mb-2">Avg Win</p>
              <p className="font-mono text-lg font-semibold text-bull">
                {avgWin > 0 ? `+${formatCurrency(avgWin, currency)}` : "—"}
              </p>
            </div>
            <div className="stat-tile">
              <p className="text-xs text-ink-sub mb-2">Avg Loss</p>
              <p className="font-mono text-lg font-semibold text-bear">
                {avgLoss > 0 ? `−${formatCurrency(avgLoss, currency)}` : "—"}
              </p>
            </div>
            <div className="stat-tile">
              <p className="text-xs text-ink-sub mb-2">Total Friction</p>
              <p className="font-mono text-lg font-semibold text-ink-sub">
                {formatCurrency(totalFriction, currency)}
              </p>
            </div>
            <div className="stat-tile">
              <p className="text-xs text-ink-sub mb-2">Current Streak</p>
              {streak.count === 0 ? (
                <p className="font-mono text-lg font-semibold text-ink-ter">—</p>
              ) : (
                <p className={`font-mono text-lg font-semibold ${streak.type === "win" ? "text-bull" : "text-bear"}`}>
                  {streak.count}× {streak.type === "win" ? "W" : "L"}
                </p>
              )}
            </div>
          </div>

          {/* Win/Loss bar */}
          {closed.length > 0 && (
            <div className="card mb-6 p-5">
              <div className="flex items-center justify-between mb-3">
                <p className="text-xs text-ink-sub">Win / Loss Distribution</p>
                <p className="text-xs font-mono text-ink-ter">{wins.length}W · {losses.length}L</p>
              </div>
              <div className="flex rounded-full overflow-hidden h-2.5 bg-s3 gap-px">
                <div className="bg-bull h-full transition-all" style={{ width: `${wr}%` }} />
                <div className="bg-bear h-full transition-all flex-1" />
              </div>
              <div className="flex justify-between mt-2">
                <span className="text-[11px] font-mono text-bull">{wr.toFixed(1)}% wins</span>
                <span className="text-[11px] font-mono text-bear">{(100 - wr).toFixed(1)}% losses</span>
              </div>
            </div>
          )}

          {/* Best / Worst */}
          {(bestTrade || worstTrade) && (
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 mb-8">
              {bestTrade && (
                <div className="card p-5" style={{ borderColor: "rgba(39,166,68,0.3)" }}>
                  <p className="text-xs text-ink-ter mb-3">Best Trade</p>
                  <div className="flex items-end justify-between">
                    <div>
                      <p className="font-mono text-2xl font-semibold text-bull">
                        +{formatCurrency(bestTrade.profit, currency)}
                      </p>
                      <p className="text-xs text-ink-ter mt-1.5">
                        {bestTrade.symbol} · {formatDate(bestTrade.time)}
                      </p>
                    </div>
                    <span className={`status-pill ${bestTrade.type === 0 ? "status-bull" : "status-bear"}`}>
                      {bestTrade.type === 0 ? "BUY" : "SELL"}
                    </span>
                  </div>
                </div>
              )}
              {worstTrade && (
                <div className="card p-5" style={{ borderColor: "rgba(229,72,77,0.3)" }}>
                  <p className="text-xs text-ink-ter mb-3">Worst Trade</p>
                  <div className="flex items-end justify-between">
                    <div>
                      <p className="font-mono text-2xl font-semibold text-bear">
                        {formatCurrency(worstTrade.profit, currency)}
                      </p>
                      <p className="text-xs text-ink-ter mt-1.5">
                        {worstTrade.symbol} · {formatDate(worstTrade.time)}
                      </p>
                    </div>
                    <span className={`status-pill ${worstTrade.type === 0 ? "status-bull" : "status-bear"}`}>
                      {worstTrade.type === 0 ? "BUY" : "SELL"}
                    </span>
                  </div>
                </div>
              )}
            </div>
          )}

          {/* Equity curve */}
          <div className="card mb-10">
            <p className="eyebrow mb-6">Equity Curve</p>
            <EquityChart deals={closed} currency={currency} />
          </div>

          {/* Trade table */}
          <div className="rounded-lg border border-hl overflow-hidden">
            <div className="px-6 py-4 border-b border-hl flex items-center justify-between">
              <p className="eyebrow">Closed Trades</p>
              <p className="text-xs text-ink-ter font-mono">{closed.length} trades</p>
            </div>
            {closed.length === 0 ? (
              <div className="text-center text-ink-sub text-sm py-12">No closed trades yet.</div>
            ) : (
              <div className="overflow-x-auto">
                <table className="data-table">
                  <thead>
                    <tr>
                      {["Time", "Symbol", "Side", "Volume", "Price", "P&L"].map(h => (
                        <th key={h}>{h}</th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {closed.slice().reverse().map((d) => (
                      <tr key={d.ticket}>
                        <td className="text-ink-sub text-xs whitespace-nowrap">{formatDate(d.time)}</td>
                        <td className="font-mono font-medium text-ink">{d.symbol || "—"}</td>
                        <td>
                          <span className={`status-pill ${d.type === 0 ? "status-bull" : "status-bear"}`}>
                            {d.type === 0 ? "BUY" : "SELL"}
                          </span>
                        </td>
                        <td className="font-mono text-ink-md">{d.volume}</td>
                        <td className="font-mono text-ink-md">{d.price.toFixed(2)}</td>
                        <td>
                          <span className={`font-mono font-semibold ${d.profit >= 0 ? "text-bull" : "text-bear"}`}>
                            {formatCurrency(d.profit, currency)}
                          </span>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </div>
        </>
      )}
    </>
  );
}
