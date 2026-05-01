export type Status = {
  pid: number;
  uptime_secs: number;
  epoch_secs: number;
  requests: number;
  buckets: number[];
};

export type Assets = {
  count: number;
  bytes_in_binary: number;
  uncompressed_bytes: number;
};

async function getJson<T>(path: string): Promise<T> {
  const res = await fetch(path);
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  return res.json();
}

export async function ping(): Promise<string> {
  const res = await fetch('/api/hello');
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  return res.text();
}

export const getStatus = () => getJson<Status>('/api/status');
export const getAssets = () => getJson<Assets>('/api/assets');

export function formatUptime(secs: number): string {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = secs % 60;
  if (h) return `${h}h ${m}m ${s}s`;
  if (m) return `${m}m ${s}s`;
  return `${s}s`;
}

export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(2)} MB`;
}
