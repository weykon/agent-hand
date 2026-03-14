#!/bin/bash
# Deploy webhook fix + upgrade weykonkong@gmail.com to Pro
# Run this on the server: ssh usa && bash deploy-webhook-fix.sh
set -e

cd /root/asympt-auth

echo "=== Step 1: Backing up current index.ts ==="
cp app/src/index.ts app/src/index.ts.backup.$(date +%Y%m%d_%H%M%S)

echo "=== Step 2: Writing fixed index.ts ==="
cat > app/src/index.ts << 'ENDOFTS'
import { Hono } from "hono";
import { serve } from "@hono/node-server";
import { Pool } from "pg";
import * as crypto from "crypto";

// ── DB ────────────────────────────────────────────────────────────────────────

const pool = new Pool({ connectionString: process.env.DATABASE_URL });

async function initDb() {
  await pool.query(`
    CREATE SCHEMA IF NOT EXISTS agent_hand;

    CREATE TABLE IF NOT EXISTS agent_hand.users (
      id           SERIAL PRIMARY KEY,
      email        TEXT UNIQUE NOT NULL,
      features     TEXT[] DEFAULT '{}',
      purchased_at TIMESTAMPTZ,
      created_at   TIMESTAMPTZ DEFAULT NOW()
    );

    CREATE TABLE IF NOT EXISTS agent_hand.device_codes (
      code       TEXT PRIMARY KEY,
      email      TEXT,
      status     TEXT DEFAULT 'pending',
      expires_at TIMESTAMPTZ NOT NULL,
      created_at TIMESTAMPTZ DEFAULT NOW()
    );

    CREATE TABLE IF NOT EXISTS agent_hand.tokens (
      token      TEXT PRIMARY KEY,
      email      TEXT NOT NULL,
      features   TEXT[] DEFAULT '{}',
      expires_at TIMESTAMPTZ NOT NULL,
      created_at TIMESTAMPTZ DEFAULT NOW()
    );
  `);
}

// ── helpers ───────────────────────────────────────────────────────────────────

function randomCode(): string {
  const chars = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
  let code = "";
  const bytes = crypto.randomBytes(8);
  for (let i = 0; i < 8; i++) {
    if (i === 4) code += "-";
    code += chars[bytes[i] % chars.length];
  }
  return code;
}

function signJwt(payload: object, secret: string): string {
  const header = Buffer.from(JSON.stringify({ alg: "HS256", typ: "JWT" })).toString("base64url");
  const body = Buffer.from(JSON.stringify(payload)).toString("base64url");
  const data = `${header}.${body}`;
  const sig = crypto.createHmac("sha256", secret).update(data).digest("base64url");
  return `${data}.${sig}`;
}

function verifyJwt(token: string, secret: string): Record<string, unknown> | null {
  const parts = token.split(".");
  if (parts.length !== 3) return null;
  const data = `${parts[0]}.${parts[1]}`;
  const expected = crypto.createHmac("sha256", secret).update(data).digest("base64url");
  if (expected !== parts[2]) return null;
  try {
    return JSON.parse(Buffer.from(parts[1], "base64url").toString());
  } catch {
    return null;
  }
}

// ── config ────────────────────────────────────────────────────────────────────

const BASE_URL = process.env.BASE_URL ?? "https://auth.asymptai.com";
const FRONTEND_URL = process.env.FRONTEND_URL ?? "https://agent-hand.dev";
const JWT_SECRET = process.env.JWT_SECRET ?? "";
const GOOGLE_CLIENT_ID = process.env.GOOGLE_CLIENT_ID ?? "";
const GOOGLE_CLIENT_SECRET = process.env.GOOGLE_CLIENT_SECRET ?? "";
const CREEM_WEBHOOK_SECRET = process.env.CREEM_WEBHOOK_SECRET ?? "";
const GITHUB_CLIENT_ID = process.env.GITHUB_CLIENT_ID ?? "";
const GITHUB_CLIENT_SECRET = process.env.GITHUB_CLIENT_SECRET ?? "";

// ── app ───────────────────────────────────────────────────────────────────────

const app = new Hono();

// CORS middleware
app.use("*", async (c, next) => {
  if (c.req.method === "OPTIONS") {
    return new Response(null, {
      status: 204,
      headers: {
        "Access-Control-Allow-Origin": "*",
        "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
        "Access-Control-Allow-Headers": "Content-Type, Authorization",
      },
    });
  }
  await next();
  c.res.headers.set("Access-Control-Allow-Origin", "*");
  c.res.headers.set("Access-Control-Allow-Methods", "GET, POST, OPTIONS");
  c.res.headers.set("Access-Control-Allow-Headers", "Content-Type, Authorization");
});

// Health check
app.get("/health", (c) => c.json({ ok: true }));

// POST /device/code — issue a new device code
app.post("/device/code", async (c) => {
  const code = randomCode();
  const expiresAt = new Date(Date.now() + 5 * 60 * 1000);
  const url = `${FRONTEND_URL}/activate?code=${code}`;

  await pool.query(
    `INSERT INTO agent_hand.device_codes (code, status, expires_at) VALUES ($1, 'pending', $2)`,
    [code, expiresAt]
  );

  return c.json({ code, url, interval: 3 });
});

// GET /device/token — poll for authorization result
app.get("/device/token", async (c) => {
  const code = c.req.query("code");
  if (!code) return c.json({ error: "missing code" }, 400);

  const { rows } = await pool.query(
    `SELECT * FROM agent_hand.device_codes WHERE code = $1`,
    [code]
  );

  if (rows.length === 0) return c.json({ status: "expired" });

  const entry = rows[0];
  if (new Date() > new Date(entry.expires_at)) {
    await pool.query(`DELETE FROM agent_hand.device_codes WHERE code = $1`, [code]);
    return c.json({ status: "expired" });
  }

  if (entry.status !== "authorized" || !entry.email) {
    return c.json({ status: "pending" });
  }

  const { rows: userRows } = await pool.query(
    `SELECT features, purchased_at FROM agent_hand.users WHERE email = $1`,
    [entry.email]
  );
  const user = userRows[0] ?? { features: [], purchased_at: null };

  const token = signJwt(
    { email: entry.email, features: user.features, iat: Math.floor(Date.now() / 1000) },
    JWT_SECRET
  );

  const tokenExpires = new Date(Date.now() + 365 * 24 * 3600 * 1000);
  await pool.query(
    `INSERT INTO agent_hand.tokens (token, email, features, expires_at)
     VALUES ($1, $2, $3, $4) ON CONFLICT (token) DO NOTHING`,
    [token, entry.email, user.features, tokenExpires]
  );

  await pool.query(`DELETE FROM agent_hand.device_codes WHERE code = $1`, [code]);

  return c.json({
    status: "authorized",
    access_token: token,
    email: entry.email,
    features: user.features,
    purchased_at: user.purchased_at ?? "",
  });
});

// GET /auth/google — redirect to Google OAuth
app.get("/auth/google", (c) => {
  const deviceCode = c.req.query("code") ?? "";
  const state = Buffer.from(JSON.stringify({ deviceCode })).toString("base64url");

  const params = new URLSearchParams({
    client_id: GOOGLE_CLIENT_ID,
    redirect_uri: `${BASE_URL}/auth/google/callback`,
    response_type: "code",
    scope: "openid email",
    state,
  });

  return c.redirect(`https://accounts.google.com/o/oauth2/v2/auth?${params}`);
});

// GET /auth/google/callback — handle Google OAuth callback
app.get("/auth/google/callback", async (c) => {
  const googleCode = c.req.query("code");
  const stateRaw = c.req.query("state") ?? "";

  if (!googleCode) {
    return c.html("<h1>Error: missing code from Google</h1>", 400);
  }

  let deviceCode = "";
  try {
    const state = JSON.parse(Buffer.from(stateRaw, "base64url").toString());
    deviceCode = state.deviceCode ?? "";
  } catch {
    return c.html("<h1>Error: invalid state</h1>", 400);
  }

  // Exchange google code for access token
  const tokenRes = await fetch("https://oauth2.googleapis.com/token", {
    method: "POST",
    headers: { "Content-Type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams({
      code: googleCode,
      client_id: GOOGLE_CLIENT_ID,
      client_secret: GOOGLE_CLIENT_SECRET,
      redirect_uri: `${BASE_URL}/auth/google/callback`,
      grant_type: "authorization_code",
    }),
  });

  if (!tokenRes.ok) {
    return c.html("<h1>Error: failed to exchange code with Google</h1>", 500);
  }

  const tokenData = (await tokenRes.json()) as { access_token?: string };

  // Get user email from Google userinfo
  const userRes = await fetch("https://www.googleapis.com/oauth2/v3/userinfo", {
    headers: { Authorization: `Bearer ${tokenData.access_token}` },
  });

  if (!userRes.ok) {
    return c.html("<h1>Error: failed to get user info from Google</h1>", 500);
  }

  const userInfo = (await userRes.json()) as { email?: string };
  const email = userInfo.email;

  if (!email) {
    return c.html("<h1>Error: no email returned from Google</h1>", 400);
  }

  // Ensure user row exists
  await pool.query(
    `INSERT INTO agent_hand.users (email) VALUES ($1) ON CONFLICT (email) DO NOTHING`,
    [email]
  );

  if (deviceCode) {
    const { rowCount } = await pool.query(
      `UPDATE agent_hand.device_codes SET email = $1, status = 'authorized'
       WHERE code = $2 AND expires_at > NOW()`,
      [email, deviceCode]
    );

    if ((rowCount ?? 0) === 0) {
      return c.html(
        `<html><body style="font-family:sans-serif;text-align:center;padding:4rem;background:#0f0f1a;color:#e2e8f0;">
          <h2>Code expired or not found</h2>
          <p>Please run <code>agent-hand login</code> again.</p>
        </body></html>`,
        410
      );
    }
  }

  // Generate a short-lived browser JWT and redirect to frontend
  const browserJwt = signJwt(
    { email, iat: Math.floor(Date.now() / 1000), exp: Math.floor(Date.now() / 1000) + 3600 },
    JWT_SECRET
  );

  return c.redirect(
    `${FRONTEND_URL}/auth/callback?token=${encodeURIComponent(browserJwt)}&email=${encodeURIComponent(email)}`
  );
});

// GET /auth/github — redirect to GitHub OAuth
app.get("/auth/github", (c) => {
  const deviceCode = c.req.query("code") ?? "";
  const state = Buffer.from(JSON.stringify({ deviceCode })).toString("base64url");

  const params = new URLSearchParams({
    client_id: GITHUB_CLIENT_ID,
    redirect_uri: `${BASE_URL}/auth/github/callback`,
    scope: "user:email",
    state,
  });

  return c.redirect(`https://github.com/login/oauth/authorize?${params}`);
});

// GET /auth/github/callback — handle GitHub OAuth callback
app.get("/auth/github/callback", async (c) => {
  const githubCode = c.req.query("code");
  const stateRaw = c.req.query("state") ?? "";

  if (!githubCode) {
    return c.html("<h1>Error: missing code from GitHub</h1>", 400);
  }

  let deviceCode = "";
  try {
    const state = JSON.parse(Buffer.from(stateRaw, "base64url").toString());
    deviceCode = state.deviceCode ?? "";
  } catch {
    return c.html("<h1>Error: invalid state</h1>", 400);
  }

  // Exchange code for access token
  const tokenRes = await fetch("https://github.com/login/oauth/access_token", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      Accept: "application/json",
    },
    body: JSON.stringify({
      client_id: GITHUB_CLIENT_ID,
      client_secret: GITHUB_CLIENT_SECRET,
      code: githubCode,
      redirect_uri: `${BASE_URL}/auth/github/callback`,
    }),
  });

  if (!tokenRes.ok) {
    return c.html("<h1>Error: failed to exchange code with GitHub</h1>", 500);
  }

  const tokenData = (await tokenRes.json()) as { access_token?: string; error?: string };
  if (tokenData.error || !tokenData.access_token) {
    return c.html(`<h1>Error: ${tokenData.error ?? "no access token"}</h1>`, 500);
  }

  // Get user email from GitHub
  const userRes = await fetch("https://api.github.com/user/emails", {
    headers: {
      Authorization: `Bearer ${tokenData.access_token}`,
      "User-Agent": "agent-hand-auth",
    },
  });

  if (!userRes.ok) {
    return c.html("<h1>Error: failed to get emails from GitHub</h1>", 500);
  }

  const emails = (await userRes.json()) as Array<{ email: string; primary: boolean; verified: boolean }>;
  const primary = emails.find((e) => e.primary && e.verified);
  const email = primary?.email ?? emails.find((e) => e.verified)?.email;

  if (!email) {
    return c.html("<h1>Error: no verified email found on GitHub account</h1>", 400);
  }

  // Ensure user row exists
  await pool.query(
    `INSERT INTO agent_hand.users (email) VALUES ($1) ON CONFLICT (email) DO NOTHING`,
    [email]
  );

  if (deviceCode) {
    const { rowCount } = await pool.query(
      `UPDATE agent_hand.device_codes SET email = $1, status = 'authorized'
       WHERE code = $2 AND expires_at > NOW()`,
      [email, deviceCode]
    );

    if ((rowCount ?? 0) === 0) {
      return c.html(
        `<html><body style="font-family:sans-serif;text-align:center;padding:4rem;background:#0f0f1a;color:#e2e8f0;">
          <h2>Code expired or not found</h2>
          <p>Please run <code>agent-hand login</code> again.</p>
        </body></html>`,
        410
      );
    }
  }

  // Generate browser JWT and redirect to frontend
  const browserJwt = signJwt(
    { email, iat: Math.floor(Date.now() / 1000), exp: Math.floor(Date.now() / 1000) + 3600 },
    JWT_SECRET
  );

  return c.redirect(
    `${FRONTEND_URL}/auth/callback?token=${encodeURIComponent(browserJwt)}&email=${encodeURIComponent(email)}`
  );
});

// POST /creem/webhook — handle creem.io payment events
app.post("/creem/webhook", async (c) => {
  const body = await c.req.text();
  const signature = c.req.header("creem-signature") ?? "";

  if (!signature) {
    console.error("[webhook] Missing creem-signature header");
    return c.json({ error: "missing signature" }, 401);
  }

  const expected = crypto
    .createHmac("sha256", CREEM_WEBHOOK_SECRET)
    .update(body)
    .digest("hex");

  // Timing-safe signature comparison to prevent timing attacks
  try {
    if (!crypto.timingSafeEqual(Buffer.from(expected, "hex"), Buffer.from(signature, "hex"))) {
      console.error("[webhook] Signature mismatch");
      return c.json({ error: "invalid signature" }, 401);
    }
  } catch {
    console.error("[webhook] Signature verification error (length mismatch)");
    return c.json({ error: "invalid signature" }, 401);
  }

  const event = JSON.parse(body) as {
    id?: string;
    eventType?: string;
    object?: { customer?: { email?: string; name?: string } };
  };

  console.log(`[webhook] Received event: ${event.eventType} (${event.id})`);

  if (event.eventType === "checkout.completed") {
    const email = event.object?.customer?.email;
    if (email) {
      await pool.query(
        `INSERT INTO agent_hand.users (email, features, purchased_at)
         VALUES ($1, ARRAY['upgrade'], NOW())
         ON CONFLICT (email) DO UPDATE
         SET features = array(
               SELECT DISTINCT unnest(agent_hand.users.features || ARRAY['upgrade'])
             ),
             purchased_at = COALESCE(agent_hand.users.purchased_at, NOW())`,
        [email]
      );
      console.log(`[webhook] Upgraded user: ${email}`);
    } else {
      console.error("[webhook] checkout.completed but no customer email found");
    }
  }

  return c.json({ ok: true });
});

// GET /auth/verify — validate a JWT token
app.get("/auth/verify", (c) => {
  const auth = c.req.header("Authorization") ?? "";
  const token = auth.replace(/^Bearer\s+/i, "");
  if (!token) return c.json({ valid: false }, 401);

  const payload = verifyJwt(token, JWT_SECRET);
  if (!payload) return c.json({ valid: false }, 401);

  return c.json({ valid: true, email: payload.email, features: payload.features });
});

// GET /auth/status — return current user status from DB (not JWT cache)
app.get("/auth/status", async (c) => {
  const auth = c.req.header("Authorization") ?? "";
  const token = auth.replace(/^Bearer\s+/i, "");
  if (!token) return c.json({ valid: false }, 401);

  const payload = verifyJwt(token, JWT_SECRET);
  if (!payload) return c.json({ valid: false }, 401);

  const email = payload.email as string;
  const { rows } = await pool.query(
    `SELECT features, purchased_at FROM agent_hand.users WHERE email = $1`,
    [email]
  );

  if (rows.length === 0) {
    return c.json({ valid: true, email, features: [], purchased_at: "" });
  }

  const user = rows[0];
  return c.json({
    valid: true,
    email,
    features: user.features ?? [],
    purchased_at: user.purchased_at ?? "",
  });
});

// ── start ─────────────────────────────────────────────────────────────────────

initDb()
  .then(() => {
    serve({ fetch: app.fetch, port: 3100 }, (info) => {
      console.log(`Auth server running on port ${info.port}`);
    });
  })
  .catch((err) => {
    console.error("Failed to initialize DB:", err);
    process.exit(1);
  });
ENDOFTS

echo "=== Step 3: Rebuilding auth container ==="
docker-compose stop auth
docker-compose rm -f auth
docker-compose up -d --build auth

echo "=== Step 4: Waiting for service to start ==="
sleep 8

echo "=== Step 5: Health check ==="
curl -sf http://127.0.0.1:3100/health && echo " OK" || echo " FAILED"

echo "=== Step 6: Upgrading weykonkong@gmail.com to Pro ==="
docker-compose exec -T db psql -U asympt -d asympt -c "
INSERT INTO agent_hand.users (email, features, purchased_at)
VALUES ('weykonkong@gmail.com', ARRAY['upgrade'], NOW())
ON CONFLICT (email) DO UPDATE
SET features = array(SELECT DISTINCT unnest(agent_hand.users.features || ARRAY['upgrade'])),
    purchased_at = COALESCE(agent_hand.users.purchased_at, NOW());
"

echo "=== Step 7: Verifying user status ==="
docker-compose exec -T db psql -U asympt -d asympt -c "
SELECT email, features, purchased_at FROM agent_hand.users WHERE email = 'weykonkong@gmail.com';
"

echo ""
echo "=== DONE ==="
echo "Webhook fix deployed. weykonkong@gmail.com upgraded to Pro."
echo "Now run on your Mac: agent-hand account --refresh"
