const REPO = "romysaputrasihananda/hermes";

export async function getLatestVersion(): Promise<string | null> {
  try {
    const res = await fetch(
      `https://api.github.com/repos/${REPO}/releases/latest`,
      {
        headers: { Accept: "application/vnd.github+json" },
        next: { revalidate: 3600 },
      }
    );
    if (!res.ok) return null;
    const data = await res.json();
    return (data.tag_name as string) ?? null;
  } catch {
    return null;
  }
}

export const GITHUB_URL  = `https://github.com/${REPO}`;
export const GITHUB_REPO = REPO;
