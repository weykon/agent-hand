const AUTH_API = "https://auth.asymptai.com";
const TOKEN_KEY = "agent_hand_token";
const EMAIL_KEY = "agent_hand_email";

export function getToken(): string | null {
  if (typeof window === "undefined") return null;
  return localStorage.getItem(TOKEN_KEY);
}

export function getEmail(): string | null {
  if (typeof window === "undefined") return null;
  return localStorage.getItem(EMAIL_KEY);
}

export function saveAuth(token: string, email: string) {
  localStorage.setItem(TOKEN_KEY, token);
  localStorage.setItem(EMAIL_KEY, email);
}

export function logout() {
  localStorage.removeItem(TOKEN_KEY);
  localStorage.removeItem(EMAIL_KEY);
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
}

export async function getStatus(): Promise<UserStatus | null> {
  const token = getToken();
  if (!token) return null;

  const res = await fetch(`${AUTH_API}/auth/status`, {
    headers: { Authorization: `Bearer ${token}` },
  });

  if (!res.ok) return null;
  return res.json();
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
