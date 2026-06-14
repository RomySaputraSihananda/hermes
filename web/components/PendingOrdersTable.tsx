import type { PendingOrder } from "@/lib/mt5";

const ORDER_LABEL: Record<number, { label: string; bull: boolean }> = {
  2: { label: "BUY LIMIT",  bull: true  },
  3: { label: "SELL LIMIT", bull: false },
  4: { label: "BUY STOP",   bull: true  },
  5: { label: "SELL STOP",  bull: false },
};

export default function PendingOrdersTable({ orders }: { orders: PendingOrder[] }) {
  return (
    <table className="data-table">
      <thead>
        <tr>
          {["Symbol", "Type", "Vol", "Entry", "Current", "SL", "TP", "Comment"].map(h => (
            <th key={h}>{h}</th>
          ))}
        </tr>
      </thead>
      <tbody>
        {orders.map((o) => {
          const kind = ORDER_LABEL[o.type] ?? { label: `TYPE ${o.type}`, bull: true };
          return (
            <tr key={o.ticket}>
              <td className="font-mono font-medium text-ink">{o.symbol}</td>
              <td>
                <span className={`status-pill ${kind.bull ? "status-bull" : "status-bear"}`}>
                  {kind.label}
                </span>
              </td>
              <td className="font-mono text-ink-md">{o.volume_current}</td>
              <td className="font-mono text-ink-md">{o.price_open.toFixed(2)}</td>
              <td className="font-mono text-ink-md">{o.price_current.toFixed(2)}</td>
              <td className="font-mono text-ink-sub">{o.sl > 0 ? o.sl.toFixed(2) : "—"}</td>
              <td className="font-mono text-ink-sub">{o.tp > 0 ? o.tp.toFixed(2) : "—"}</td>
              <td className="text-xs text-ink-ter">{o.comment || "—"}</td>
            </tr>
          );
        })}
      </tbody>
    </table>
  );
}
