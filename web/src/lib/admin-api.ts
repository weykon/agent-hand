import { getToken, AUTH_API } from "./auth";

export async function adminFetch<T>(path: string, options?: RequestInit): Promise<T> {
  const token = getToken();
  const res = await fetch(`${AUTH_API}${path}`, {
    ...options,
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${token}`,
      ...options?.headers,
    },
  });
  if (res.status === 403) throw new Error("Not authorized as admin");
  if (!res.ok) throw new Error(`Admin API error: ${res.status}`);
  return res.json();
}
