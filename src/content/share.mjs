const MAX_BYTES = 128 * 1024 * 1024;
const FUNCTION_NAME = "project-share";
const linksKey = "banaforge.project-shares.v1";
let adapter = null;
let openedFromLink = false;

const config = () => globalThis.BANAFORGE_SHARE_CONFIG || {};
const configured = () => Boolean(config().projectUrl && config().publicKey);
const endpoint = () => `${String(config().projectUrl).replace(/\/$/, "")}/functions/v1/${FUNCTION_NAME}`;

async function api(action, fields = {}) {
  if (!configured()) throw new Error("Project sharing is not configured on this BanaForge site.");
  const response = await fetch(endpoint(), {
    method: "POST",
    headers: {
      "content-type": "application/json",
      apikey: config().publicKey,
      authorization: `Bearer ${config().publicKey}`,
    },
    body: JSON.stringify({ action, ...fields }),
  });
  const body = await response.json().catch(() => ({}));
  if (!response.ok) throw new Error(body.error || `Share service returned ${response.status}.`);
  return body;
}

function savedLinks() {
  try {
    const links = JSON.parse(localStorage.getItem(linksKey) || "[]");
    return Array.isArray(links) ? links.filter((item) => item && item.id && item.ownerKey) : [];
  } catch {
    return [];
  }
}

function storeLinks(links) {
  localStorage.setItem(linksKey, JSON.stringify(links.slice(-20)));
}

function linkUrl(id) {
  const url = new URL(location.href);
  url.hash = `share=${encodeURIComponent(id)}`;
  return url.toString();
}

function createDialog() {
  document.getElementById("banaforge-share-dialog")?.remove();
  const dialog = document.createElement("dialog");
  dialog.id = "banaforge-share-dialog";
  dialog.setAttribute("aria-labelledby", "banaforge-share-title");
  dialog.innerHTML = `
    <form method="dialog" class="share-shell">
      <header><strong id="banaforge-share-title">Share project</strong><button class="share-close" value="close" aria-label="Close">×</button></header>
      <p class="share-status" role="status">Create a link to open a copy of this project in BanaForge Web.</p>
      <select class="share-existing" aria-label="Previously created project links" hidden></select>
      <div class="share-result" hidden>
        <label for="banaforge-share-url">Anyone with this link can open the project until its expiry.</label>
        <input id="banaforge-share-url" readonly />
        <div class="share-actions">
          <button class="share-copy" type="button">Copy link</button>
          <button class="share-revoke" type="button">Revoke link</button>
        </div>
      </div>
      <footer><button class="share-create" type="button">Create link…</button><button value="close">Done</button></footer>
    </form>`;
  document.body.append(dialog);
  return dialog;
}

function showDialog(dialog, status) {
  if (status) dialog.querySelector(".share-status").textContent = status;
  if (!dialog.open) dialog.showModal();
}

function populateSaved(dialog, selectedId = null) {
  const select = dialog.querySelector(".share-existing");
  const links = savedLinks();
  select.replaceChildren();
  for (const item of links) {
    const option = document.createElement("option");
    option.value = item.id;
    option.textContent = `${item.name} — expires ${new Date(item.expiresAt).toLocaleDateString()}`;
    select.append(option);
  }
  select.hidden = links.length < 2;
  if (!links.length) return;
  select.value = selectedId && links.some((item) => item.id === selectedId) ? selectedId : links.at(-1).id;
  select.onchange = () => {
    const item = savedLinks().find((saved) => saved.id === select.value);
    if (item) showResult(dialog, item);
  };
  return links.find((item) => item.id === select.value);
}

function setBusy(dialog, text) {
  dialog.querySelector(".share-status").textContent = text;
  dialog.querySelectorAll("button").forEach((button) => {
    if (button.value !== "close") button.disabled = true;
  });
}

function clearBusy(dialog) {
  dialog.querySelectorAll("button").forEach((button) => (button.disabled = false));
}

async function copyText(text) {
  if (navigator.clipboard?.writeText) return navigator.clipboard.writeText(text);
  const input = document.createElement("textarea");
  input.value = text;
  input.style.position = "fixed";
  input.style.opacity = "0";
  document.body.append(input);
  input.select();
  const copied = document.execCommand("copy");
  input.remove();
  if (!copied) throw new Error("Could not copy the link. Select it and copy it manually.");
}

function showResult(dialog, item) {
  const box = dialog.querySelector(".share-result");
  const input = dialog.querySelector("#banaforge-share-url");
  const url = linkUrl(item.id);
  box.hidden = false;
  input.value = url;
  dialog.querySelector(".share-status").textContent = `Link created for “${item.name}”. It expires ${new Date(item.expiresAt).toLocaleDateString()}.`;
  dialog.querySelector(".share-create").textContent = "Create another link…";
  dialog.querySelector(".share-copy").onclick = async () => {
    try {
      await copyText(url);
      dialog.querySelector(".share-status").textContent = "Link copied. Anyone with it can open this project until expiry.";
    } catch (error) {
      dialog.querySelector(".share-status").textContent = error.message;
      input.focus();
      input.select();
    }
  };
  dialog.querySelector(".share-revoke").onclick = async () => {
    if (!confirm("Revoke this link now? Anyone who has it will no longer be able to open the project.")) return;
    setBusy(dialog, "Revoking link…");
    try {
      await api("revoke", { id: item.id, ownerKey: item.ownerKey });
      const remaining = savedLinks().filter((saved) => saved.id !== item.id);
      storeLinks(remaining);
      const previous = populateSaved(dialog);
      if (previous) {
        showResult(dialog, previous);
      } else {
        box.hidden = true;
        dialog.querySelector(".share-create").textContent = "Create link…";
        dialog.querySelector(".share-status").textContent = "Link revoked.";
      }
    } catch (error) {
      dialog.querySelector(".share-status").textContent = error.message;
    } finally {
      clearBusy(dialog);
    }
  };
}

async function createShare(dialog) {
  if (!configured()) {
    dialog.querySelector(".share-status").textContent = "Sharing is not enabled on this deployment. Export a ZIP from PROJECT → Save New → ZIP, or contact the site administrator.";
    return;
  }
  const ok = confirm(
    "Create an unlisted link? Anyone who receives the link can open this project until it expires in 30 days. The ZIP includes user media used by the project; it does not include BanaForge's built-in or imported original-content files. You can revoke the link from this browser.",
  );
  if (!ok) return;

  setBusy(dialog, "Preparing project archive…");
  try {
    if (!adapter) throw new Error("BanaForge is still starting. Try again in a moment.");
    const archive = await adapter.shareArchive();
    const bytes = archive.bytes;
    if (!(bytes instanceof Uint8Array) || bytes.byteLength === 0) throw new Error("Could not prepare the project archive.");
    if (bytes.byteLength > MAX_BYTES) throw new Error("This project archive is larger than the 128 MiB sharing limit.");

    setBusy(dialog, "Creating a private upload…");
    const created = await api("init", { name: archive.name, size: bytes.byteLength });
    const upload = await fetch(created.uploadUrl, {
      method: "PUT",
      headers: { "content-type": "application/zip", "x-upsert": "false" },
      body: bytes,
    });
    if (!upload.ok) throw new Error(`Project upload failed (${upload.status}).`);
    await api("finalize", { id: created.id });

    const item = {
      id: created.id,
      ownerKey: created.ownerKey,
      name: archive.name,
      expiresAt: created.expiresAt,
    };
    let revocationSaved = true;
    try {
      storeLinks([...savedLinks(), item]);
    } catch {
      revocationSaved = false;
    }
    populateSaved(dialog, item.id);
    showResult(dialog, item);
    if (!revocationSaved) {
      dialog.querySelector(".share-status").textContent += " This browser could not save the revocation key; revoke before closing this tab or let the link expire.";
    }
    if (archive.omittedOriginalContent > 0) {
      dialog.querySelector(".share-status").textContent += ` ${archive.omittedOriginalContent} original-content file(s) were left out.`;
    }
  } catch (error) {
    dialog.querySelector(".share-status").textContent = error?.message || "Could not create the project link.";
  } finally {
    clearBusy(dialog);
  }
}

async function openIncomingLink(id) {
  if (openedFromLink) return;
  openedFromLink = true;
  const dialog = createDialog();
  dialog.querySelector(".share-create").hidden = true;
  dialog.querySelector(".share-result").hidden = true;
  showDialog(dialog, "Loading the shared project…");
  try {
    if (!configured()) throw new Error("This BanaForge site is not configured to open project-share links.");
    const shared = await api("open", { id });
    const response = await fetch(shared.downloadUrl, { cache: "no-store" });
    if (!response.ok) throw new Error(response.status === 404 ? "This project link has expired or was revoked." : `Project download failed (${response.status}).`);
    const declared = Number(response.headers.get("content-length") || 0);
    if (declared > MAX_BYTES) throw new Error("This project archive exceeds BanaForge Web's 128 MiB import limit.");
    const bytes = new Uint8Array(await response.arrayBuffer());
    if (!bytes.length || bytes.byteLength > MAX_BYTES) throw new Error("The shared project is empty or exceeds the 128 MiB import limit.");
    if (!confirm(`Open “${shared.name}” as a new project copy in BanaForge? This will replace the current song after its unsaved-changes prompt.`)) {
      dialog.close();
      return;
    }
    const opened = await adapter.openShared(bytes);
    dialog.close();
    if (!opened) {
      // ProjectFiles owns the unsaved-changes prompt or the invalid-file message.
      return;
    }
  } catch (error) {
    showDialog(dialog, error?.message || "Could not open this project link.");
    dialog.querySelector(".share-create").hidden = true;
  }
}

function attach(nextAdapter) {
  adapter = nextAdapter;
  if (openedFromLink) return;
  const params = new URLSearchParams(location.hash.replace(/^#/, ""));
  const id = params.get("share");
  if (id) void openIncomingLink(id);
}

function openDialog() {
  const dialog = createDialog();
  dialog.querySelector(".share-create").addEventListener("click", () => void createShare(dialog));
  const existing = populateSaved(dialog);
  if (existing) showResult(dialog, existing);
  showDialog(dialog);
  if (!configured()) dialog.querySelector(".share-status").textContent = "Sharing is not enabled on this deployment yet. The administrator must configure the BanaForge project-sharing service.";
}

globalThis.banaforgeShare = { attach, openDialog };
