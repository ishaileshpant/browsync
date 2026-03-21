/**
 * browsync extension - background service worker
 *
 * Listens for bookmark and tab changes, sends them to the
 * native browsync daemon via Native Messaging.
 */

const NATIVE_HOST = "com.browsync.daemon";

// ── Types ──────────────────────────────────────────────

interface BrowsyncMessage {
  type: "bookmark_created" | "bookmark_removed" | "bookmark_moved" | "bookmark_changed"
      | "tab_created" | "tab_updated" | "tab_removed" | "tab_activated"
      | "history_visited";
  data: Record<string, unknown>;
  timestamp: string;
}

// ── Native Messaging ───────────────────────────────────

let nativePort: chrome.runtime.Port | null = null;

function connectNative(): chrome.runtime.Port | null {
  try {
    const port = chrome.runtime.connectNative(NATIVE_HOST);
    port.onDisconnect.addListener(() => {
      console.log("browsync: native host disconnected");
      nativePort = null;
    });
    port.onMessage.addListener((msg: unknown) => {
      console.log("browsync: native response:", msg);
    });
    console.log("browsync: connected to native host");
    return port;
  } catch (e) {
    console.error("browsync: failed to connect to native host:", e);
    return null;
  }
}

function sendToNative(message: BrowsyncMessage): void {
  if (!nativePort) {
    nativePort = connectNative();
  }
  if (nativePort) {
    try {
      nativePort.postMessage(message);
    } catch {
      nativePort = null;
    }
  }
  // Always store locally as fallback
  storeLocally(message);
}

async function storeLocally(message: BrowsyncMessage): Promise<void> {
  const { pendingSync = [] } = await chrome.storage.local.get("pendingSync");
  (pendingSync as BrowsyncMessage[]).push(message);
  // Keep only last 1000 events
  if ((pendingSync as BrowsyncMessage[]).length > 1000) {
    (pendingSync as BrowsyncMessage[]).splice(0, (pendingSync as BrowsyncMessage[]).length - 1000);
  }
  await chrome.storage.local.set({ pendingSync });
}

// ── Bookmark Listeners ────────────────────────────────

chrome.bookmarks.onCreated.addListener((id, bookmark) => {
  sendToNative({
    type: "bookmark_created",
    data: { id, url: bookmark.url, title: bookmark.title, parentId: bookmark.parentId },
    timestamp: new Date().toISOString(),
  });
});

chrome.bookmarks.onRemoved.addListener((id, removeInfo) => {
  sendToNative({
    type: "bookmark_removed",
    data: { id, parentId: removeInfo.parentId, index: removeInfo.index },
    timestamp: new Date().toISOString(),
  });
});

chrome.bookmarks.onMoved.addListener((id, moveInfo) => {
  sendToNative({
    type: "bookmark_moved",
    data: {
      id,
      oldParentId: moveInfo.oldParentId,
      newParentId: moveInfo.parentId,
      oldIndex: moveInfo.oldIndex,
      newIndex: moveInfo.index,
    },
    timestamp: new Date().toISOString(),
  });
});

chrome.bookmarks.onChanged.addListener((id, changeInfo) => {
  sendToNative({
    type: "bookmark_changed",
    data: { id, title: changeInfo.title, url: changeInfo.url },
    timestamp: new Date().toISOString(),
  });
});

// ── Tab Listeners ──────────────────────────────────────

chrome.tabs.onCreated.addListener((tab) => {
  sendToNative({
    type: "tab_created",
    data: { id: tab.id, url: tab.url, title: tab.title, windowId: tab.windowId },
    timestamp: new Date().toISOString(),
  });
});

chrome.tabs.onUpdated.addListener((tabId, changeInfo, tab) => {
  if (changeInfo.url || changeInfo.title) {
    sendToNative({
      type: "tab_updated",
      data: {
        id: tabId,
        url: changeInfo.url ?? tab.url,
        title: changeInfo.title ?? tab.title,
        windowId: tab.windowId,
      },
      timestamp: new Date().toISOString(),
    });
  }
});

chrome.tabs.onRemoved.addListener((tabId, removeInfo) => {
  sendToNative({
    type: "tab_removed",
    data: { id: tabId, windowId: removeInfo.windowId },
    timestamp: new Date().toISOString(),
  });
});

chrome.tabs.onActivated.addListener((activeInfo) => {
  sendToNative({
    type: "tab_activated",
    data: { tabId: activeInfo.tabId, windowId: activeInfo.windowId },
    timestamp: new Date().toISOString(),
  });
});

// ── History Listener ───────────────────────────────────

chrome.history.onVisited.addListener((result) => {
  sendToNative({
    type: "history_visited",
    data: { url: result.url, title: result.title, visitCount: result.visitCount },
    timestamp: new Date().toISOString(),
  });
});

// ── Initial sync on install ────────────────────────────

chrome.runtime.onInstalled.addListener(async () => {
  console.log("browsync: extension installed, performing initial bookmark sync");

  const tree = await chrome.bookmarks.getTree();
  const bookmarks: Array<{ url: string; title: string; path: string[] }> = [];

  function walk(nodes: chrome.bookmarks.BookmarkTreeNode[], path: string[]): void {
    for (const node of nodes) {
      if (node.url) {
        bookmarks.push({ url: node.url, title: node.title, path: [...path] });
      }
      if (node.children) {
        walk(node.children, [...path, node.title]);
      }
    }
  }

  walk(tree, []);

  await chrome.storage.local.set({
    lastFullSync: new Date().toISOString(),
    bookmarkCount: bookmarks.length,
  });

  console.log(`browsync: synced ${bookmarks.length} bookmarks`);
});

console.log("browsync: background service worker loaded");
