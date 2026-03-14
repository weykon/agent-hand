export const AUTH_API = "https://auth.asymptai.com";
const TOKEN_KEY = "agent_hand_token";
const EMAIL_KEY = "agent_hand_email";
const NAME_KEY = "agent_hand_name";
const AVATAR_KEY = "agent_hand_avatar";

export function getToken(): string | null {
  if (typeof window === "undefined") return null;
  return localStorage.getItem(TOKEN_KEY);
}

export function getEmail(): string | null {
  if (typeof window === "undefined") return null;
  return localStorage.getItem(EMAIL_KEY);
}

export function getName(): string | null {
  if (typeof window === "undefined") return null;
  return localStorage.getItem(NAME_KEY);
}

export function getAvatar(): string | null {
  if (typeof window === "undefined") return null;
  return localStorage.getItem(AVATAR_KEY);
}

export function saveAuth(token: string, email: string, name?: string, avatarUrl?: string) {
  localStorage.setItem(TOKEN_KEY, token);
  localStorage.setItem(EMAIL_KEY, email);
  if (name) localStorage.setItem(NAME_KEY, name);
  if (avatarUrl) localStorage.setItem(AVATAR_KEY, avatarUrl);
}

export function logout() {
  localStorage.removeItem(TOKEN_KEY);
  localStorage.removeItem(EMAIL_KEY);
  localStorage.removeItem(NAME_KEY);
  localStorage.removeItem(AVATAR_KEY);
}

export function isLoggedIn(): boolean {
  return !!getToken();
}

export interface UserStatus {
  valid: boolean;
  email: string;
  features: string[];
  purchased_at: string;
  subscription_status: "active" | "canceled" | "expired" | null;
  subscription_end_date: string | null;
  name: string;
  avatar_url: string;
}

export async function getStatus(): Promise<UserStatus | null> {
  const token = getToken();
  if (!token) return null;

  const res = await fetch(`${AUTH_API}/auth/status`, {
    headers: { Authorization: `Bearer ${token}` },
  });

  if (!res.ok) return null;
  const status: UserStatus = await res.json();

  // Refresh local profile cache from server
  if (status.name) localStorage.setItem(NAME_KEY, status.name);
  if (status.avatar_url) localStorage.setItem(AVATAR_KEY, status.avatar_url);

  return status;
}

// ── Device management ────────────────────────────────────────────────────────

export interface DeviceInfo {
  device_id: string;
  hostname: string;
  os_arch: string;
  first_seen: string;
  last_seen: string;
}

export interface DevicesResponse {
  devices: DeviceInfo[];
  device_limit: number;
  active_count: number;
}

export async function getDevices(): Promise<DevicesResponse | null> {
  const token = getToken();
  if (!token) return null;

  const res = await fetch(`${AUTH_API}/api/devices`, {
    headers: { Authorization: `Bearer ${token}` },
  });

  if (!res.ok) return null;
  return res.json();
}

export async function unbindDevice(deviceId: string): Promise<boolean> {
  const token = getToken();
  if (!token) return false;

  const res = await fetch(`${AUTH_API}/api/devices/${encodeURIComponent(deviceId)}`, {
    method: "DELETE",
    headers: { Authorization: `Bearer ${token}` },
  });

  return res.ok;
}

export function isMax(status: UserStatus | null): boolean {
  return status?.features?.includes("max") ?? false;
}

export function isPro(status: UserStatus | null): boolean {
  return status?.features?.includes("upgrade") ?? false;
}

export function getPlanName(status: UserStatus | null): "Free" | "Pro" | "Max" {
  if (isMax(status)) return "Max";
  if (isPro(status)) return "Pro";
  return "Free";
}

// ── Admin ────────────────────────────────────────────────────────────────────

export const ADMIN_EMAIL = "weykonkong@gmail.com";

export function isAdmin(): boolean {
  return getEmail() === ADMIN_EMAIL;
}
