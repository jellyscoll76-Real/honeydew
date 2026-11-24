import { createClient } from "npm:@supabase/supabase-js@2";

const BUCKET = "project-shares";
const MAX_BYTES = 128 * 1024 * 1024;
const SHARE_DAYS = 30;
const SIGNED_DOWNLOAD_SECONDS = 5 * 60;
const corsHeaders = {
  "access-control-allow-origin": "*",
  "access-control-allow-headers": "authorization, apikey, content-type, x-banaforge-cleanup",
  "access-control-allow-methods": "POST, OPTIONS",
  "cache-control": "no-store",
};

const json = (status: number, value: Record<string, unknown>) =>
  new Response(JSON.stringify(value), {
    status,
    headers: { ...corsHeaders, "content-type": "application/json; charset=utf-8" },
  });

const token = () => {
  const bytes = crypto.getRandomValues(new Uint8Array(32));
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/, "");
};

async function sha256(value: string): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(value));
  return [...new Uint8Array(digest)].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

function safeName(value: unknown): string {
  if (typeof value !== "string") return "Shared project";
  const name = value
    .replace(/[\u0000-\u001f\u007f]/g, " ")
    .replace(/[\/\\?%*:|"<>]/g, "_")
    .trim()
    .slice(0, 96);
  return name || "Shared project";
}

function shareId(value: unknown): value is string {
  return typeof value === "string" && /^[A-Za-z0-9_-]{40,64}$/.test(value);
}

function storageUrl(projectUrl: string, value: string): string {
  if (/^https?:\/\//i.test(value)) return value;
  if (value.startsWith("/storage/v1/")) return `${projectUrl}${value}`;
  return `${projectUrl}/storage/v1${value.startsWith("/") ? value : `/${value}`}`;
}

Deno.serve(async (request) => {
  if (request.method === "OPTIONS") return new Response("ok", { headers: corsHeaders });
  if (request.method !== "POST") return json(405, { error: "Use POST." });

  const projectUrl = Deno.env.get("SUPABASE_URL");
  const serviceKey = Deno.env.get("SUPABASE_SERVICE_ROLE_KEY");
  const rateSecret = Deno.env.get("BANAFORGE_SHARE_RATE_SECRET");
  if (!projectUrl || !serviceKey || !rateSecret) {
    return json(503, { error: "Project sharing is not configured." });
  }

  const length = Number(request.headers.get("content-length") || 0);
  if (length > 8192) return json(413, { error: "Request is too large." });
  let body: Record<string, unknown>;
  try {
    body = await request.json();
  } catch {
    return json(400, { error: "Invalid request." });
  }

  const admin = createClient(projectUrl, serviceKey, {
    auth: { autoRefreshToken: false, persistSession: false },
  });

  // Opportunistically prune expired records on link creation. The daily cleanup action can drain larger backlogs.
  const cleanup = async (limit: number) => {
    const now = new Date().toISOString();
    const { data: expired } = await admin
      .from("project_shares")
      .select("share_hash, object_path")
      .lt("expires_at", now)
      .limit(limit);
    if (!expired?.length) return;
    const paths = expired.map((entry) => entry.object_path);
    await admin.storage.from(BUCKET).remove(paths);
    await admin.from("project_shares").delete().in("share_hash", expired.map((entry) => entry.share_hash));
  };

  const action = body.action;
  if (action === "init") {
    const size = Number(body.size);
    if (!Number.isSafeInteger(size) || size < 1 || size > MAX_BYTES) {
      return json(413, { error: "Project archives must be between 1 byte and 128 MiB." });
    }
    const forwarded = request.headers.get("x-forwarded-for")?.split(",")[0]?.trim()
      || request.headers.get("x-real-ip")
      || "unknown";
    const ipHash = await sha256(`${rateSecret}:${forwarded}`);
    const { data: allowed, error: rateError } = await admin.rpc("consume_project_share_quota", {
      p_ip_hash: ipHash,
    });
    if (rateError) return json(503, { error: "Share creation is temporarily unavailable." });
    if (!allowed) return json(429, { error: "Too many share links from this network. Try again in an hour." });

    await cleanup(50);
    const id = token();
    const ownerKey = token();
    const objectPath = `${crypto.randomUUID()}.zip`;
    const expiresAt = new Date(Date.now() + SHARE_DAYS * 24 * 60 * 60 * 1000).toISOString();
    const projectName = safeName(body.name);
    const { data: upload, error: uploadError } = await admin.storage
      .from(BUCKET)
      .createSignedUploadUrl(objectPath, { upsert: false });
    if (uploadError || !upload?.signedUrl) {
      return json(503, { error: "Could not prepare a secure project upload." });
    }

    const { error: insertError } = await admin.from("project_shares").insert({
      share_hash: await sha256(id),
      owner_hash: await sha256(ownerKey),
      object_path: objectPath,
      project_name: projectName,
      expected_size: size,
      uploaded: false,
      expires_at: expiresAt,
    });
    if (insertError) return json(503, { error: "Could not register the project link." });

    return json(201, {
      id,
      ownerKey,
      name: projectName,
      expiresAt,
      uploadUrl: storageUrl(projectUrl.replace(/\/$/, ""), upload.signedUrl),
    });
  }

  if (action === "finalize") {
    if (!shareId(body.id)) return json(400, { error: "Invalid project link." });
    const hash = await sha256(body.id);
    const { data: item, error } = await admin.from("project_shares").select("*").eq("share_hash", hash).maybeSingle();
    if (error || !item || new Date(item.expires_at).getTime() <= Date.now()) {
      return json(404, { error: "Project upload expired. Create a new share link." });
    }
    const pathParts = String(item.object_path).split("/");
    const leaf = pathParts.pop() || "";
    const folder = pathParts.join("/");
    const { data: listed, error: listError } = await admin.storage.from(BUCKET).list(folder, {
      limit: 20,
      search: leaf,
    });
    const uploaded = listed?.find((entry) => entry.name === leaf);
    const actualSize = Number(uploaded?.metadata?.size ?? uploaded?.metadata?.contentLength ?? 0);
    if (listError || !uploaded || actualSize !== Number(item.expected_size)) {
      await admin.storage.from(BUCKET).remove([item.object_path]);
      return json(400, { error: "The uploaded project is missing or its size did not match." });
    }
    const { error: updateError } = await admin
      .from("project_shares")
      .update({ uploaded: true })
      .eq("share_hash", hash);
    if (updateError) return json(503, { error: "Could not finish the project upload." });
    return json(200, { ok: true });
  }

  if (action === "open") {
    if (!shareId(body.id)) return json(404, { error: "This project link is invalid, expired, or revoked." });
    const { data: item, error } = await admin
      .from("project_shares")
      .select("object_path, project_name, expires_at, uploaded")
      .eq("share_hash", await sha256(body.id))
      .maybeSingle();
    if (error || !item || !item.uploaded || new Date(item.expires_at).getTime() <= Date.now()) {
      return json(404, { error: "This project link has expired or was revoked." });
    }
    const { data: signed, error: signError } = await admin.storage
      .from(BUCKET)
      .createSignedUrl(item.object_path, SIGNED_DOWNLOAD_SECONDS, { download: `${safeName(item.project_name)}.zip` });
    if (signError || !signed?.signedUrl) return json(503, { error: "Could not open the shared project." });
    return json(200, {
      name: item.project_name,
      expiresAt: item.expires_at,
      downloadUrl: signed.signedUrl,
    });
  }

  if (action === "revoke") {
    if (!shareId(body.id) || !shareId(body.ownerKey)) return json(404, { error: "Project link not found." });
    const hash = await sha256(body.id);
    const { data: item, error } = await admin
      .from("project_shares")
      .select("object_path")
      .eq("share_hash", hash)
      .eq("owner_hash", await sha256(body.ownerKey))
      .maybeSingle();
    if (error || !item) return json(404, { error: "Project link not found." });
    await admin.storage.from(BUCKET).remove([item.object_path]);
    const { error: deleteError } = await admin.from("project_shares").delete().eq("share_hash", hash);
    if (deleteError) return json(503, { error: "Could not revoke this project link." });
    return json(200, { ok: true });
  }

  if (action === "cleanup") {
    const expected = Deno.env.get("BANAFORGE_SHARE_CLEANUP_SECRET");
    if (!expected || request.headers.get("x-banaforge-cleanup") !== expected) {
      return json(404, { error: "Not found." });
    }
    await cleanup(500);
    await admin.from("project_share_rate_limits").delete().lt("window_started_at", new Date(Date.now() - 24 * 60 * 60 * 1000).toISOString());
    return json(200, { ok: true });
  }

  return json(400, { error: "Unknown project-share action." });
});
