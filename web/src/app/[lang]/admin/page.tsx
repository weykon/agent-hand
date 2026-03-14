"use client";

import { useEffect, useState, useCallback } from "react";
import { isLoggedIn, isAdmin } from "@/lib/auth";
import { adminFetch } from "@/lib/admin-api";

// ── Types ────────────────────────────────────────────────────────────────────

interface Overview {
  users: { total: number; free: number; pro: number; max: number; recent_7d: number };
  devices: { total_active: number };
  analytics: { total_pv: number; total_uv: number };
}

interface AdminUser {
  email: string;
  name: string | null;
  avatar_url: string | null;
  features: string[];
  purchased_at: string | null;
  subscription_status: string | null;
  subscription_end_date: string | null;
  created_at: string;
  device_count: string;
}

interface AdminDevice {
  id: number;
  user_email: string;
  device_id: string;
  hostname: string;
  os_arch: string;
  first_seen: string;
  last_seen: string;
  is_active: boolean;
}

interface UserDetail {
  user: AdminUser;
  devices: AdminDevice[];
}

interface DailyStats {
  date: string;
  pv: string;
  uv: string;
}

interface Analytics {
  daily: DailyStats[];
  top_pages: { page: string; pv: string; uv: string }[];
  top_referrers: { referrer: string; count: string }[];
  totals: { pv: string; uv: string };
}

type Tab = "overview" | "users" | "devices" | "analytics";

// ── Helpers ──────────────────────────────────────────────────────────────────

function timeAgo(dateStr: string): string {
  const diff = Date.now() - new Date(dateStr).getTime();
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return "just now";
  if (mins < 60) return `${mins}m ago`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days}d ago`;
  return new Date(dateStr).toLocaleDateString();
}

function getTier(features: string[]): "Free" | "Pro" | "Max" {
  if (features.includes("max")) return "Max";
  if (features.includes("upgrade")) return "Pro";
  return "Free";
}

const TIER_COLORS: Record<string, string> = {
  Free: "bg-[#334155] text-[#94a3b8]",
  Pro: "bg-[#7c3aed]/20 text-[#a78bfa]",
  Max: "bg-[#f59e0b]/20 text-[#fbbf24]",
};

// ── Components ───────────────────────────────────────────────────────────────

function StatCard({ label, value, sub }: { label: string; value: string | number; sub?: string }) {
  return (
    <div className="rounded-xl border border-[#1e293b] bg-[#1a1a2e] p-5">
      <p className="text-xs text-[#64748b] uppercase tracking-wide">{label}</p>
      <p className="mt-1 text-2xl font-bold text-white">{value}</p>
      {sub && <p className="mt-0.5 text-xs text-[#94a3b8]">{sub}</p>}
    </div>
  );
}

function OverviewSection({ data }: { data: Overview | null }) {
  if (!data) return <p className="text-[#64748b]">Loading...</p>;
  return (
    <div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
      <StatCard label="Total Users" value={data.users.total} sub={`Free ${data.users.free} / Pro ${data.users.pro} / Max ${data.users.max}`} />
      <StatCard label="New (7d)" value={data.users.recent_7d} />
      <StatCard label="Active Devices" value={data.devices.total_active} />
      <StatCard label="PV / UV (30d)" value={`${data.analytics.total_pv} / ${data.analytics.total_uv}`} />
    </div>
  );
}

function UsersSection() {
  const [users, setUsers] = useState<AdminUser[]>([]);
  const [total, setTotal] = useState(0);
  const [search, setSearch] = useState("");
  const [searchInput, setSearchInput] = useState("");
  const [offset, setOffset] = useState(0);
  const [expanded, setExpanded] = useState<string | null>(null);
  const [detail, setDetail] = useState<UserDetail | null>(null);
  const [featureInput, setFeatureInput] = useState("");
  const [loading, setLoading] = useState(false);

  const load = useCallback(async (s: string, off: number, append: boolean) => {
    setLoading(true);
    try {
      const data = await adminFetch<{ users: AdminUser[]; total: number }>(
        `/api/admin/users?search=${encodeURIComponent(s)}&limit=50&offset=${off}`
      );
      setUsers(prev => append ? [...prev, ...data.users] : data.users);
      setTotal(data.total);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { load(search, 0, false); }, [search, load]);

  // Debounced search
  useEffect(() => {
    const t = setTimeout(() => { setSearch(searchInput); setOffset(0); }, 300);
    return () => clearTimeout(t);
  }, [searchInput]);

  async function expandUser(email: string) {
    if (expanded === email) { setExpanded(null); return; }
    setExpanded(email);
    const d = await adminFetch<UserDetail>(`/api/admin/users/${encodeURIComponent(email)}`);
    setDetail(d);
  }

  async function grantFeature(email: string) {
    if (!featureInput.trim()) return;
    await adminFetch(`/api/admin/users/${encodeURIComponent(email)}/features`, {
      method: "POST",
      body: JSON.stringify({ action: "grant", feature: featureInput.trim() }),
    });
    setFeatureInput("");
    await expandUser(email);
    load(search, 0, false);
  }

  async function revokeFeature(email: string, feature: string) {
    await adminFetch(`/api/admin/users/${encodeURIComponent(email)}/features`, {
      method: "POST",
      body: JSON.stringify({ action: "revoke", feature }),
    });
    await expandUser(email);
    load(search, 0, false);
  }

  async function updateSubscription(email: string, status: string) {
    await adminFetch(`/api/admin/users/${encodeURIComponent(email)}/subscription`, {
      method: "POST",
      body: JSON.stringify({ status }),
    });
    await expandUser(email);
    load(search, 0, false);
  }

  return (
    <div>
      <input
        type="text"
        placeholder="Search by email or name..."
        value={searchInput}
        onChange={e => setSearchInput(e.target.value)}
        className="mb-4 w-full rounded-lg border border-[#333] bg-[#0f0f1a] px-4 py-2.5 text-sm text-white placeholder-[#64748b] outline-none focus:border-[#6366f1]"
      />
      <div className="rounded-xl border border-[#1e293b] bg-[#1a1a2e] overflow-hidden">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-[#1e293b] text-left text-xs text-[#64748b] uppercase">
              <th className="px-4 py-3">Email</th>
              <th className="px-4 py-3 hidden md:table-cell">Name</th>
              <th className="px-4 py-3">Tier</th>
              <th className="px-4 py-3 hidden lg:table-cell">Status</th>
              <th className="px-4 py-3 hidden sm:table-cell">Devices</th>
              <th className="px-4 py-3 hidden lg:table-cell">Created</th>
            </tr>
          </thead>
          <tbody>
            {users.map(u => {
              const tier = getTier(u.features);
              const isExpanded = expanded === u.email;
              return (
                <tr key={u.email} className="group">
                  <td colSpan={6} className="p-0">
                    <div
                      onClick={() => expandUser(u.email)}
                      className="grid cursor-pointer px-4 py-3 hover:bg-[#252547] transition-colors"
                      style={{ gridTemplateColumns: "1fr auto auto auto auto auto" }}
                    >
                      <span className="text-white truncate max-w-[200px]">{u.email}</span>
                      <span className="hidden md:block px-4 text-[#94a3b8] truncate max-w-[120px]">{u.name ?? "—"}</span>
                      <span className="px-4">
                        <span className={`rounded-full px-2 py-0.5 text-xs font-medium ${TIER_COLORS[tier]}`}>{tier}</span>
                      </span>
                      <span className="hidden lg:block px-4 text-[#94a3b8]">{u.subscription_status ?? "—"}</span>
                      <span className="hidden sm:block px-4 text-[#94a3b8]">{u.device_count}</span>
                      <span className="hidden lg:block px-4 text-[#64748b]">{timeAgo(u.created_at)}</span>
                    </div>
                    {isExpanded && detail && detail.user.email === u.email && (
                      <div className="border-t border-[#1e293b] bg-[#12122a] px-6 py-4 space-y-4">
                        {/* Features */}
                        <div>
                          <p className="text-xs text-[#64748b] uppercase mb-2">Features</p>
                          <div className="flex flex-wrap gap-2">
                            {detail.user.features.map(f => (
                              <span key={f} className="flex items-center gap-1 rounded-full bg-[#334155] px-2.5 py-1 text-xs text-[#94a3b8]">
                                {f}
                                <button onClick={() => revokeFeature(u.email, f)} className="ml-1 text-red-400 hover:text-red-300">x</button>
                              </span>
                            ))}
                            <form onSubmit={e => { e.preventDefault(); grantFeature(u.email); }} className="flex items-center gap-1">
                              <input
                                value={featureInput}
                                onChange={e => setFeatureInput(e.target.value)}
                                placeholder="add..."
                                className="w-24 rounded border border-[#333] bg-[#0f0f1a] px-2 py-1 text-xs text-white outline-none focus:border-[#6366f1]"
                              />
                              <button type="submit" className="rounded bg-[#6366f1] px-2 py-1 text-xs text-white hover:bg-[#818cf8]">+</button>
                            </form>
                          </div>
                        </div>
                        {/* Subscription */}
                        <div>
                          <p className="text-xs text-[#64748b] uppercase mb-2">Subscription</p>
                          <div className="flex items-center gap-3 text-sm">
                            <span className="text-[#94a3b8]">{detail.user.subscription_status ?? "none"}</span>
                            {detail.user.subscription_end_date && (
                              <span className="text-[#64748b]">ends {new Date(detail.user.subscription_end_date).toLocaleDateString()}</span>
                            )}
                            <div className="flex gap-2 ml-auto">
                              {["active", "canceled", "expired"].map(s => (
                                <button
                                  key={s}
                                  onClick={() => updateSubscription(u.email, s)}
                                  className={`rounded px-2 py-1 text-xs ${detail.user.subscription_status === s ? "bg-[#6366f1] text-white" : "bg-[#1e293b] text-[#94a3b8] hover:bg-[#333]"}`}
                                >
                                  {s}
                                </button>
                              ))}
                            </div>
                          </div>
                        </div>
                        {/* Devices */}
                        {detail.devices.length > 0 && (
                          <div>
                            <p className="text-xs text-[#64748b] uppercase mb-2">Devices ({detail.devices.length})</p>
                            <div className="space-y-1">
                              {detail.devices.map(d => (
                                <div key={d.id} className="flex items-center gap-3 rounded bg-[#1a1a2e] px-3 py-2 text-xs">
                                  <span className={`h-2 w-2 rounded-full ${d.is_active ? "bg-green-400" : "bg-[#64748b]"}`} />
                                  <span className="text-white font-mono">{d.device_id.slice(0, 12)}...</span>
                                  <span className="text-[#94a3b8]">{d.hostname || "—"}</span>
                                  <span className="text-[#64748b]">{d.os_arch}</span>
                                  <span className="ml-auto text-[#64748b]">{timeAgo(d.last_seen)}</span>
                                </div>
                              ))}
                            </div>
                          </div>
                        )}
                      </div>
                    )}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      <div className="mt-4 flex items-center justify-between text-sm text-[#64748b]">
        <span>{total} users total</span>
        {users.length < total && (
          <button
            onClick={() => { const next = offset + 50; setOffset(next); load(search, next, true); }}
            disabled={loading}
            className="rounded bg-[#1e293b] px-4 py-2 text-[#94a3b8] hover:bg-[#333] disabled:opacity-50"
          >
            Load More
          </button>
        )}
      </div>
    </div>
  );
}

function DevicesSection() {
  const [devices, setDevices] = useState<AdminDevice[]>([]);
  const [total, setTotal] = useState(0);
  const [userFilter, setUserFilter] = useState("");
  const [stale, setStale] = useState(false);
  const [offset, setOffset] = useState(0);
  const [loading, setLoading] = useState(false);

  const load = useCallback(async (user: string, staleOnly: boolean, off: number, append: boolean) => {
    setLoading(true);
    try {
      const data = await adminFetch<{ devices: AdminDevice[]; total: number }>(
        `/api/admin/devices?user=${encodeURIComponent(user)}&stale=${staleOnly}&limit=50&offset=${off}`
      );
      setDevices(prev => append ? [...prev, ...data.devices] : data.devices);
      setTotal(data.total);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { load(userFilter, stale, 0, false); }, [userFilter, stale, load]);

  async function deactivate(id: number) {
    await adminFetch(`/api/admin/devices/${id}/deactivate`, { method: "POST" });
    load(userFilter, stale, 0, false);
  }

  return (
    <div>
      <div className="mb-4 flex gap-3">
        <input
          type="text"
          placeholder="Filter by user email..."
          value={userFilter}
          onChange={e => { setUserFilter(e.target.value); setOffset(0); }}
          className="flex-1 rounded-lg border border-[#333] bg-[#0f0f1a] px-4 py-2.5 text-sm text-white placeholder-[#64748b] outline-none focus:border-[#6366f1]"
        />
        <button
          onClick={() => { setStale(!stale); setOffset(0); }}
          className={`rounded-lg px-4 py-2.5 text-sm ${stale ? "bg-[#f59e0b]/20 text-[#fbbf24]" : "bg-[#1e293b] text-[#94a3b8]"}`}
        >
          Stale only
        </button>
      </div>
      <div className="rounded-xl border border-[#1e293b] bg-[#1a1a2e] overflow-hidden">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-[#1e293b] text-left text-xs text-[#64748b] uppercase">
              <th className="px-4 py-3">User</th>
              <th className="px-4 py-3 hidden sm:table-cell">Device ID</th>
              <th className="px-4 py-3 hidden md:table-cell">Hostname</th>
              <th className="px-4 py-3 hidden lg:table-cell">OS</th>
              <th className="px-4 py-3">Last Seen</th>
              <th className="px-4 py-3 w-20"></th>
            </tr>
          </thead>
          <tbody>
            {devices.map(d => (
              <tr key={d.id} className="border-b border-[#1e293b]/50 hover:bg-[#252547] transition-colors">
                <td className="px-4 py-3 text-white truncate max-w-[180px]">{d.user_email}</td>
                <td className="px-4 py-3 hidden sm:table-cell font-mono text-[#94a3b8]">{d.device_id.slice(0, 12)}</td>
                <td className="px-4 py-3 hidden md:table-cell text-[#94a3b8]">{d.hostname || "—"}</td>
                <td className="px-4 py-3 hidden lg:table-cell text-[#64748b]">{d.os_arch || "—"}</td>
                <td className="px-4 py-3 text-[#64748b]">{timeAgo(d.last_seen)}</td>
                <td className="px-4 py-3">
                  <button
                    onClick={() => deactivate(d.id)}
                    className="rounded bg-red-500/10 px-2.5 py-1 text-xs text-red-400 hover:bg-red-500/20"
                  >
                    Deactivate
                  </button>
                </td>
              </tr>
            ))}
            {devices.length === 0 && (
              <tr><td colSpan={6} className="px-4 py-8 text-center text-[#64748b]">No devices found</td></tr>
            )}
          </tbody>
        </table>
      </div>
      <div className="mt-4 flex items-center justify-between text-sm text-[#64748b]">
        <span>{total} devices total</span>
        {devices.length < total && (
          <button
            onClick={() => { const next = offset + 50; setOffset(next); load(userFilter, stale, next, true); }}
            disabled={loading}
            className="rounded bg-[#1e293b] px-4 py-2 text-[#94a3b8] hover:bg-[#333] disabled:opacity-50"
          >
            Load More
          </button>
        )}
      </div>
    </div>
  );
}

function AnalyticsSection() {
  const [data, setData] = useState<Analytics | null>(null);

  useEffect(() => {
    adminFetch<Analytics>("/api/admin/analytics?days=30").then(setData);
  }, []);

  if (!data) return <p className="text-[#64748b]">Loading analytics...</p>;

  const maxPV = Math.max(...data.daily.map(d => parseInt(d.pv, 10)), 1);

  return (
    <div className="space-y-6">
      {/* Daily chart */}
      <div className="rounded-xl border border-[#1e293b] bg-[#1a1a2e] p-5">
        <h3 className="mb-4 text-sm font-medium text-white">Daily Traffic (30d)</h3>
        <div className="flex items-end gap-[2px]" style={{ height: 160 }}>
          {data.daily.map(d => {
            const pv = parseInt(d.pv, 10);
            const uv = parseInt(d.uv, 10);
            const pvH = (pv / maxPV) * 140;
            const uvH = (uv / maxPV) * 140;
            return (
              <div key={d.date} className="group relative flex-1 flex flex-col items-center justify-end" style={{ minWidth: 0 }}>
                <div className="absolute bottom-full mb-1 hidden group-hover:block z-10 rounded bg-[#0f0f1a] border border-[#333] px-2 py-1 text-xs text-white whitespace-nowrap">
                  {d.date}: {pv} PV / {uv} UV
                </div>
                <div className="w-full rounded-t" style={{ height: pvH, backgroundColor: "#6366f1", opacity: 0.6 }} />
                <div className="w-full rounded-t absolute bottom-0" style={{ height: uvH, backgroundColor: "#4ade80", opacity: 0.7 }} />
              </div>
            );
          })}
        </div>
        <div className="mt-2 flex items-center gap-4 text-xs text-[#64748b]">
          <span className="flex items-center gap-1"><span className="inline-block h-2 w-2 rounded bg-[#6366f1]" /> PV</span>
          <span className="flex items-center gap-1"><span className="inline-block h-2 w-2 rounded bg-[#4ade80]" /> UV</span>
          <span className="ml-auto">Total: {data.totals.pv} PV / {data.totals.uv} UV</span>
        </div>
      </div>

      {/* Top pages + referrers side-by-side */}
      <div className="grid gap-4 md:grid-cols-2">
        <div className="rounded-xl border border-[#1e293b] bg-[#1a1a2e] p-5">
          <h3 className="mb-3 text-sm font-medium text-white">Top Pages</h3>
          <div className="space-y-1.5">
            {data.top_pages.map(p => (
              <div key={p.page} className="flex items-center justify-between text-xs">
                <span className="text-[#94a3b8] truncate max-w-[60%]">{p.page}</span>
                <span className="text-[#64748b]">{p.pv} / {p.uv}</span>
              </div>
            ))}
          </div>
        </div>
        <div className="rounded-xl border border-[#1e293b] bg-[#1a1a2e] p-5">
          <h3 className="mb-3 text-sm font-medium text-white">Top Referrers</h3>
          <div className="space-y-1.5">
            {data.top_referrers.map(r => (
              <div key={r.referrer} className="flex items-center justify-between text-xs">
                <span className="text-[#94a3b8] truncate max-w-[60%]">{r.referrer}</span>
                <span className="text-[#64748b]">{r.count}</span>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}

// ── Main Page ────────────────────────────────────────────────────────────────

const TABS: { key: Tab; label: string }[] = [
  { key: "overview", label: "Overview" },
  { key: "users", label: "Users" },
  { key: "devices", label: "Devices" },
  { key: "analytics", label: "Analytics" },
];

export default function AdminPage() {
  const [authorized, setAuthorized] = useState<boolean | null>(null);
  const [tab, setTab] = useState<Tab>("overview");
  const [overview, setOverview] = useState<Overview | null>(null);

  useEffect(() => {
    if (!isLoggedIn() || !isAdmin()) {
      setAuthorized(false);
      return;
    }
    setAuthorized(true);
    adminFetch<Overview>("/api/admin/overview").then(setOverview).catch(() => setAuthorized(false));
  }, []);

  if (authorized === null) {
    return (
      <div className="flex min-h-[60vh] items-center justify-center">
        <p className="text-[#64748b]">Checking authorization...</p>
      </div>
    );
  }

  if (!authorized) {
    return (
      <div className="flex min-h-[60vh] items-center justify-center">
        <div className="rounded-xl border border-[#1e293b] bg-[#1a1a2e] p-8 text-center">
          <p className="text-lg font-medium text-white">Not Authorized</p>
          <p className="mt-2 text-sm text-[#64748b]">Admin access is restricted.</p>
        </div>
      </div>
    );
  }

  return (
    <div className="mx-auto max-w-6xl px-6 py-8">
      <h1 className="text-2xl font-bold text-white">Admin Dashboard</h1>

      {/* Tabs */}
      <div className="mt-6 flex gap-1 rounded-lg bg-[#0f0f1a] border border-[#1e293b] p-1">
        {TABS.map(t => (
          <button
            key={t.key}
            onClick={() => setTab(t.key)}
            className={`flex-1 rounded-md px-4 py-2 text-sm font-medium transition-colors ${
              tab === t.key
                ? "bg-[#1a1a2e] text-white"
                : "text-[#64748b] hover:text-[#94a3b8]"
            }`}
          >
            {t.label}
          </button>
        ))}
      </div>

      {/* Content */}
      <div className="mt-6">
        {tab === "overview" && <OverviewSection data={overview} />}
        {tab === "users" && <UsersSection />}
        {tab === "devices" && <DevicesSection />}
        {tab === "analytics" && <AnalyticsSection />}
      </div>
    </div>
  );
}
