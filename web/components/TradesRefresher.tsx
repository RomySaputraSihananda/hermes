"use client";
import { useRouter } from "next/navigation";
import { useEffect } from "react";

export default function TradesRefresher() {
  const router = useRouter();
  useEffect(() => {
    const id = setInterval(() => router.refresh(), 60_000);
    return () => clearInterval(id);
  }, [router]);
  return (
    <span className="text-xs text-ink-sub">
      Auto-refreshes every 60s
    </span>
  );
}
