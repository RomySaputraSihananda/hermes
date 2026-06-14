"use client";
import { useEffect, useState } from "react";
import type { AccountInfo, Position, PendingOrder } from "@/lib/mt5";
import { formatCurrency } from "@/lib/format";
import PositionsTable from "./PositionsTable";
import PendingOrdersTable from "./PendingOrdersTable";

const MAGIC   = 19731;
const POLL_MS = 30_000;

interface State {
  account:   AccountInfo | null;
  positions: Position[];
  orders:    PendingOrder[];
  loading:   boolean;
  error:     string | null;
  ts:        Date | null;
}

export default function LiveDashboard() {
  const [state, setState] = useState<State>({
    account: null, positions: [], orders: [], loading: true, error: null, ts: null,
  });

  async function fetchData() {
    try {
      const [aRes, pRes, oRes] = await Promise.all([
        fetch("/api/account"), fetch("/api/positions"), fetch("/api/orders"),
      ]);
      if (!aRes.ok) throw new Error("bridge error");
      const [account, positions, orders] = await Promise.all([
        aRes.json() as Promise<AccountInfo>,
        pRes.json() as Promise<Position[]>,
        oRes.json() as Promise<PendingOrder[]>,
      ]);
      setState({
        account, positions,
        orders: Array.isArray(orders) ? orders : [],
        loading: false, error: null, ts: new Date(),
      });
    } catch (e) {
      setState(prev => ({ ...prev, loading: false, error: String(e) }));
    }
  }

  useEffect(() => {
    fetchData();
    const id = setInterval(fetchData, POLL_MS);
    return () => clearInterval(id);
  }, []);

  const hermesPos = state.positions.filter(p => p.magic === MAGIC);
  const hermesOrd = state.orders.filter(o => o.magic === MAGIC);
  const openPnl   = hermesPos.reduce((s, p) => s + p.profit, 0);
  const currency  = state.account?.currency ?? "USD";

  if (state.loading) return <LoadingSkeleton />;
  if (state.error) {
    return (
      <div className="card mb-16 text-center text-ink-sub text-sm py-10">
        Unable to connect to MT5 bridge.
      </div>
    );
  }
  const { account } = state;
  if (!account) return null;

  return (
    <>
      {/* Account stats */}
      <section className="mb-10">
        <div className="flex items-center justify-between mb-5">
          <p className="eyebrow">Account Overview</p>
          {state.ts && (
            <span className="text-xs text-ink-ter font-mono">
              {state.ts.toLocaleTimeString()}
            </span>
          )}
        </div>
        <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
          <StatTile label="Balance"     value={formatCurrency(account.balance, currency)} />
          <StatTile
            label="Equity"
            value={formatCurrency(account.equity, currency)}
            diff={account.equity !== account.balance
              ? { val: account.equity - account.balance, currency }
              : undefined}
          />
          <StatTile
            label="Open P&L"
            value={formatCurrency(openPnl, currency)}
            colored={openPnl !== 0 ? openPnl > 0 : undefined}
          />
          <StatTile label="Free Margin" value={formatCurrency(account.margin_free, currency)} />
        </div>
      </section>

      {/* Open positions */}
      <section className="mb-10">
        <div className="flex items-center gap-3 mb-5">
          <p className="eyebrow">Open Positions</p>
          {hermesPos.length > 0 && (
            <span className="status-pill status-gold">{hermesPos.length}</span>
          )}
        </div>
        {hermesPos.length === 0 ? (
          <Empty>No open positions right now.</Empty>
        ) : (
          <div className="rounded-lg border border-hl overflow-hidden">
            <PositionsTable positions={hermesPos} currency={currency} />
          </div>
        )}
      </section>

      {/* Pending orders */}
      <section className="mb-16">
        <div className="flex items-center gap-3 mb-5">
          <p className="eyebrow">Pending Orders</p>
          {hermesOrd.length > 0 && (
            <span className="status-pill status-muted">{hermesOrd.length}</span>
          )}
        </div>
        {hermesOrd.length === 0 ? (
          <Empty>No pending orders right now.</Empty>
        ) : (
          <div className="rounded-lg border border-hl overflow-hidden">
            <PendingOrdersTable orders={hermesOrd} />
          </div>
        )}
      </section>
    </>
  );
}

function StatTile({ label, value, diff, colored }: {
  label: string;
  value: string;
  diff?: { val: number; currency: string };
  colored?: boolean;
}) {
  return (
    <div className="stat-tile">
      <p className="text-xs text-ink-sub mb-2">{label}</p>
      <p className={`font-mono text-xl font-semibold ${
        colored === true ? "text-bull" : colored === false ? "text-bear" : "text-ink"
      }`}>{value}</p>
      {diff && (
        <p className={`text-xs font-mono mt-1 ${diff.val >= 0 ? "text-bull" : "text-bear"}`}>
          {diff.val >= 0 ? "+" : ""}{formatCurrency(diff.val, diff.currency)}
        </p>
      )}
    </div>
  );
}

function Empty({ children }: { children: React.ReactNode }) {
  return (
    <div className="card text-center text-ink-sub text-sm py-10">{children}</div>
  );
}

function LoadingSkeleton() {
  return (
    <div className="mb-16 space-y-10 animate-pulse">
      <div>
        <div className="h-3 w-32 bg-s2 rounded mb-5" />
        <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
          {[...Array(4)].map((_, i) => <div key={i} className="stat-tile h-20 bg-s2" />)}
        </div>
      </div>
      <div>
        <div className="h-3 w-32 bg-s2 rounded mb-5" />
        <div className="card h-24 bg-s2" />
      </div>
    </div>
  );
}
