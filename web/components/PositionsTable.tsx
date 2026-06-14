import type { Position } from "@/lib/mt5";
import { formatCurrency } from "@/lib/format";

export default function PositionsTable({ positions, currency = "USD" }: {
  positions: Position[];
  currency?: string;
}) {
  return (
    <table className="data-table">
      <thead>
        <tr>
          {["Symbol", "Side", "Volume", "Entry", "Current", "SL", "TP", "P&L"].map(h => (
            <th key={h}>{h}</th>
          ))}
        </tr>
      </thead>
      <tbody>
        {positions.map((p) => {
          const isBuy = p.type === 0;
          return (
            <tr key={p.ticket}>
              <td className="font-mono font-medium text-ink">{p.symbol}</td>
              <td>
                <span className={`status-pill ${isBuy ? "status-bull" : "status-bear"}`}>
                  {isBuy ? "BUY" : "SELL"}
                </span>
              </td>
              <td className="font-mono text-ink-md">{p.volume}</td>
              <td className="font-mono text-ink-md">{p.price_open.toFixed(2)}</td>
              <td className="font-mono text-ink-md">{p.price_current.toFixed(2)}</td>
              <td className="font-mono text-ink-sub">{p.sl > 0 ? p.sl.toFixed(2) : "—"}</td>
              <td className="font-mono text-ink-sub">{p.tp > 0 ? p.tp.toFixed(2) : "—"}</td>
              <td className={`font-mono font-semibold ${p.profit >= 0 ? "text-bull" : "text-bear"}`}>
                {formatCurrency(p.profit, currency)}
              </td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}
