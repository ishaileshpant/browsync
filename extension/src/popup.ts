// Popup script for browsync extension

async function updateStats(): Promise<void> {
  const data = await chrome.storage.local.get([
    "bookmarkCount",
    "lastFullSync",
    "pendingSync",
  ]);

  const countEl = document.getElementById("bookmarkCount");
  const syncEl = document.getElementById("lastSync");
  const pendingEl = document.getElementById("pendingCount");
  const statusEl = document.getElementById("status");

  if (countEl) {
    countEl.textContent = String(data.bookmarkCount ?? 0);
  }

  if (syncEl && data.lastFullSync) {
    const d = new Date(data.lastFullSync as string);
    syncEl.textContent = d.toLocaleString();
  }

  if (pendingEl) {
    const pending = (data.pendingSync as unknown[]) ?? [];
    pendingEl.textContent = String(pending.length);
  }

  if (statusEl) {
    // Check if native host is reachable by attempting connection
    try {
      const port = chrome.runtime.connectNative("com.browsync.daemon");
      port.disconnect();
      statusEl.textContent = "Daemon connected";
      statusEl.className = "status connected";
    } catch {
      statusEl.textContent = "Daemon not running";
      statusEl.className = "status disconnected";
    }
  }
}

document.getElementById("syncBtn")?.addEventListener("click", async () => {
  const btn = document.getElementById("syncBtn") as HTMLButtonElement;
  btn.textContent = "Syncing...";
  btn.disabled = true;

  // Trigger a full bookmark tree sync
  const tree = await chrome.bookmarks.getTree();
  let count = 0;

  function walk(nodes: chrome.bookmarks.BookmarkTreeNode[]): void {
    for (const node of nodes) {
      if (node.url) count++;
      if (node.children) walk(node.children);
    }
  }
  walk(tree);

  await chrome.storage.local.set({
    lastFullSync: new Date().toISOString(),
    bookmarkCount: count,
  });

  btn.textContent = `Synced ${count} bookmarks!`;
  setTimeout(() => {
    btn.textContent = "Sync Now";
    btn.disabled = false;
    updateStats();
  }, 2000);
});

updateStats();
