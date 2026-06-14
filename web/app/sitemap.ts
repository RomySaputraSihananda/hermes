import type { MetadataRoute } from "next";

export default function sitemap(): MetadataRoute.Sitemap {
  const base = "https://hermes.romys.my.id";
  return [
    { url: base,             lastModified: new Date(), changeFrequency: "daily",   priority: 1 },
    { url: `${base}/trades`, lastModified: new Date(), changeFrequency: "hourly",  priority: 0.8 },
    { url: `${base}/backtest`, lastModified: new Date(), changeFrequency: "weekly", priority: 0.7 },
  ];
}
