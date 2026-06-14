import { NextResponse } from "next/server";
import { getPositions } from "@/lib/mt5";

export const dynamic = "force-dynamic";

export async function GET() {
  try {
    const data = await getPositions();
    return NextResponse.json(data);
  } catch (e) {
    return NextResponse.json({ error: String(e) }, { status: 502 });
  }
}
