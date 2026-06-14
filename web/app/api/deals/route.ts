import { NextResponse } from "next/server";
import { getDeals } from "@/lib/mt5";

export const dynamic = "force-dynamic";

export async function GET(req: Request) {
  const { searchParams } = new URL(req.url);
  const dateFrom = searchParams.get("date_from") ?? "";
  const dateTo   = searchParams.get("date_to")   ?? "";
  const symbol   = searchParams.get("symbol") ?? undefined;
  try {
    const data = await getDeals(dateFrom, dateTo, symbol);
    return NextResponse.json(data);
  } catch (e) {
    return NextResponse.json({ error: String(e) }, { status: 502 });
  }
}
