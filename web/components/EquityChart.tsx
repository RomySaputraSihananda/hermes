"use client";
import { useState } from "react";
import {
  AreaChart, Area, XAxis, YAxis, CartesianGrid, Tooltip,
  ReferenceLine, ResponsiveContainer,
} from "recharts";
import { formatCurrency } from "@/lib/format";

interface Deal { time: string; profit: number; swap: number; commission: number; }

type Period = "1D" | "7D" | "30D" | "ALL";
const PERIODS: Period[] = ["1D", "7D", "30D", "ALL"];

function periodStart(p: Period): Date {
  const now = new Date();
  if (p === "1D")  return new Date(now.getTime() - 86_400_000);
  if (p === "7D")  return new Date(now.getTime() - 7  * 86_400_000);
  if (p === "30D") return new Date(now.getTime() - 30 * 86_400_000);
  return new Date("2000-01-01");
}

function fmtTick(iso: string, p: Period) {
  const d = new Date(iso);
  if (p === "1D") return d.toLocaleTimeString("en-US", { hour: "2-digit", minute: "2-digit", hour12: false });
  return d.toLocaleDateString("en-US", { month: "short", day: "numeric" });
}

export default function EquityChart({ deals, currency = "USD" }: {
  deals: Deal[];
  currency?: string;
}) {
  const [period, setPeriod] = useState<Period>("ALL");
  const since  = periodStart(period);
  const sorted = deals
    .filter(d => new Date(d.time) >= since)
    .sort((a, b) => +new Date(a.time) - +new Date(b.time));

  let cum = 0;
  const pts = [
    { label: "Start", pnl: 0 },
    ...sorted.map(d => {
      cum += d.profit + d.swap + d.commission;
      return { label: fmtTick(d.time, period), pnl: parseFloat(cum.toFixed(2)) };
    }),
  ];

  const netPnl = pts[pts.length - 1]?.pnl ?? 0;
  const isPos  = netPnl >= 0;
  const minPnl = Math.min(0, ...pts.map(d => d.pnl));
  const maxPnl = Math.max(0, ...pts.map(d => d.pnl));
  const range  = maxPnl - minPnl || 1;
  const zeroPct = `${((maxPnl / range) * 100).toFixed(1)}%`;

  if (pts.length < 2) {
    return (
      <div className="flex items-center justify-center h-[240px] text-ink-sub text-sm">
        No trades in this period.
      </div>
    );
  }

  return (
    <div>
      <div className="flex items-center justify-between mb-5">
        <div className="flex gap-1">
          {PERIODS.map(p => (
            <button
              key={p}
              onClick={() => setPeriod(p)}
              style={period === p
                ? { background: "var(--c-s3)", border: "1px solid var(--c-hl-strong)", color: "var(--c-ink)" }
                : { color: "var(--c-ink-sub)", border: "1px solid transparent" }
              }
              className="px-3 py-1 rounded-md text-xs font-mono font-medium transition-colors hover:text-ink"
            >
              {p}
            </button>
          ))}
        </div>
        <span className={`font-mono text-sm font-semibold ${isPos ? "text-bull" : "text-bear"}`}>
          {isPos ? "+" : ""}{formatCurrency(netPnl, currency)}
        </span>
      </div>

      <ResponsiveContainer width="100%" height={240}>
        <AreaChart data={pts} margin={{ top: 4, right: 4, left: 0, bottom: 0 }}>
          <defs>
            <linearGradient id="pnlFill" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%"      stopColor="#27a644" stopOpacity={0.18} />
              <stop offset={zeroPct} stopColor="#27a644" stopOpacity={0.04} />
              <stop offset={zeroPct} stopColor="#e5484d" stopOpacity={0.04} />
              <stop offset="100%"   stopColor="#e5484d"  stopOpacity={0.18} />
            </linearGradient>
            <linearGradient id="pnlLine" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%"      stopColor="#27a644" />
              <stop offset={zeroPct} stopColor="#27a644" />
              <stop offset={zeroPct} stopColor="#e5484d" />
              <stop offset="100%"   stopColor="#e5484d"  />
            </linearGradient>
          </defs>

          <CartesianGrid strokeDasharray="4 4" stroke="#23252a" vertical={false} />
          <ReferenceLine y={0} stroke="#34343a" strokeDasharray="3 3" />

          <XAxis
            dataKey="label"
            tick={{ fontSize: 11, fill: "#62666d", fontFamily: "JetBrains Mono, monospace" }}
            tickLine={false} axisLine={false}
            interval="preserveStartEnd"
          />
          <YAxis
            domain={[minPnl - Math.abs(minPnl) * 0.08 - 1, maxPnl + maxPnl * 0.08 + 1]}
            tick={{ fontSize: 11, fill: "#62666d", fontFamily: "JetBrains Mono, monospace" }}
            tickLine={false} axisLine={false}
            tickFormatter={v => `$${v.toFixed(0)}`}
            width={60}
          />
          <Tooltip
            formatter={(v: number) => [`${v >= 0 ? "+" : ""}${formatCurrency(v, currency)}`, "P&L"]}
            contentStyle={{
              background: "#141516", border: "1px solid #34343a",
              borderRadius: "8px", fontSize: "12px",
              fontFamily: "'JetBrains Mono', monospace", color: "#f7f8f8",
            }}
            labelStyle={{ color: "#8a8f98", marginBottom: 4 }}
            cursor={{ stroke: "#34343a", strokeWidth: 1 }}
          />
          <Area
            type="monotone"
            dataKey="pnl"
            stroke="url(#pnlLine)"
            strokeWidth={1.5}
            fill="url(#pnlFill)"
            dot={false}
            activeDot={{ r: 3, strokeWidth: 0, fill: "#d4a843" }}
          />
        </AreaChart>
      </ResponsiveContainer>
    </div>
  );
}
