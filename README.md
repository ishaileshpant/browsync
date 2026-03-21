# browsync

Cross-browser bookmark, history, and session consolidation. Move seamlessly between Chrome, Firefox, Safari, Edge, Brave, and Arc with a single unified CLI + TUI.

## Quick Install

**macOS** (one-liner):
```bash
curl -fsSL https://ishaileshpant.github.io/browsync/install.sh | bash
```

**Windows** (PowerShell):
```powershell
irm https://ishaileshpant.github.io/browsync/install.ps1 | iex
```

**From source**:
```bash
cargo install --path crates/browsync-cli
```

## What It Does

```
┌──────────────────────────────────────────────────────────┐
│  Chrome  Firefox  Safari  Edge  Brave  Arc               │
│    │        │       │      │      │      │                │
│    └────────┴───────┴──────┴──────┴──────┘                │
│                      │                                    │
│              ┌───────▼───────┐                            │
│              │   browsync    │  Unified SQLite DB          │
│              │  ~/.browsync  │  FTS5 full-text search     │
│              └───────┬───────┘                            │
│                      │                                    │
│    ┌─────────┬───────┼───────┬──────────┐                │
│    ▼         ▼       ▼       ▼          ▼                │
│   CLI      TUI    Export   Daemon    Extension            │
└──────────────────────────────────────────────────────────┘
```

- **Import** bookmarks and history from any installed browser
- **Search** across all browsers with full-text search
- **Export** to HTML (importable by all browsers), JSON, or CSV
- **TUI** — browse, search, and open URLs from the terminal
- **Daemon** — background sync watches for browser changes
- **Auth** — track saved logins, generate migration reports
- **Extension** — real-time bookmark/tab sync (Chrome/Firefox)

## Usage

### Detect installed browsers
```bash
$ browsync detect

Detected browsers:

  [+] Chrome   active (bookmarks, history, logins)
  [ ] Firefox  not found
  [+] Safari   active (bookmarks, history)
  [ ] Edge     not found
  [ ] Brave    not found
  [ ] Arc      not found

2 browser(s) with importable data
```

### Import browser data
```bash
# Import from all detected browsers
$ browsync import

# Import from a specific browser
$ browsync import --browser chrome

# Import only bookmarks
$ browsync import --browser chrome --bookmarks-only
```

### Search across everything
```bash
$ browsync search "kubernetes"

Bookmarks (5 results):
  1. How we run Kubernetes in Kubernetes [C]
     https://kubernetes.io/blog/...
  2. RBAC Support in Kubernetes [C]
     https://kubernetes.io/blog/...

History (5 results):
  1. Kubernetes Documentation [C] (12 visits, 3d ago)
     https://kubernetes.io/docs/
```

### Export bookmarks
```bash
# Export as HTML (importable by any browser)
$ browsync export --format html --output bookmarks.html

# Export as JSON
$ browsync export --format json --output bookmarks.json

# Export as CSV
$ browsync export --format csv --output bookmarks.csv

# Export only Chrome bookmarks
$ browsync export --browser chrome --format html -o chrome.html
```

### Open URL in any browser
```bash
$ browsync open https://github.com --browser arc
$ browsync open https://docs.rs --browser firefox
```

### Interactive TUI
```bash
$ browsync tui
```
```
┌─ browsync ──────────────────────────────────────────────┐
│ [Bookmarks] [History] [Search] [Status]                 │
├─────────────────────────────────────────────────────────-│
│ [C] GitHub                    github.com                │
│ [C] Docs.rs                   docs.rs                   │
│ [F] Crates.io                 crates.io                 │
│ [S] Apple Developer           developer.apple.com       │
├─────────────────────────────────────────────────────────-│
│ [/]search [1-4]tabs [j/k]navigate [o]pen [q]uit        │
└─────────────────────────────────────────────────────────┘
```

**TUI keybindings:**
| Key | Action |
|-----|--------|
| `1-4` | Switch tabs |
| `Tab` | Next tab |
| `j/k` | Navigate up/down |
| `/` | Toggle search |
| `Enter` / `o` | Open URL in default browser |
| `q` / `Ctrl+C` | Quit |

### Background daemon
```bash
# Start watching for browser changes
$ browsyncd start

# Check daemon status
$ browsyncd status

# Stop daemon
$ browsyncd stop
```

### Auth management
```bash
# List all saved login domains
$ browsync auth list

# Generate migration report (Chrome → Arc)
$ browsync auth migrate --from chrome --to arc

Sites that NEED RE-LOGIN in Arc:
  ! gitlab.com (user@example.com)
  ! stackoverflow.com (user@example.com)

Sites already saved in Arc:
  + github.com
```

### Show sync status
```bash
$ browsync status

Database: 3061 bookmarks, 5614 history entries
Location: /Users/you/.browsync/browsync.db

Recent syncs:
  chrome   history       5614 items  2026-03-21T14:23:17Z
  chrome   bookmarks     3061 items  2026-03-21T14:23:17Z
```

## Browser Extension

The browsync extension provides real-time bookmark and tab sync via Native Messaging.

### Install (Chrome)
1. Build: `cd extension && npm install && npm run build`
2. Open `chrome://extensions` → Enable Developer Mode
3. Click "Load unpacked" → select the `extension/` directory
4. Install native messaging host:
   ```bash
   cp extension/native-messaging-host.json \
     ~/Library/Application\ Support/Google/Chrome/NativeMessagingHosts/com.browsync.daemon.json
   ```

### Install (Firefox)
1. Open `about:debugging` → "This Firefox"
2. Click "Load Temporary Add-on" → select `extension/manifest.json`

## Architecture

```
browsync/
├── crates/
│   ├── browsync-core/     Core library: models, parsers, DB, sync engine
│   ├── browsync-cli/      CLI binary + TUI (ratatui)
│   └── browsync-daemon/   Background sync service
└── extension/             Browser extension (TypeScript, Manifest V3)
```

| Component | Technology |
|-----------|-----------|
| Language | Rust (Edition 2024) |
| CLI | clap (derive macros) |
| TUI | ratatui + crossterm |
| Database | SQLite via rusqlite (FTS5) |
| File Watch | notify |
| Extension | TypeScript, WebExtension Manifest V3 |

### Data flow
1. **Parsers** read browser profile data (JSON/SQLite/plist)
2. **Sync engine** deduplicates by URL, merges metadata
3. **SQLite DB** stores unified data at `~/.browsync/browsync.db`
4. **FTS5** enables full-text search across all fields
5. **Exporters** write back to any browser format

### Security
- Passwords are **never** stored or exported
- Auth entries track domain + username only
- Login Data encryption keys stay in macOS Keychain
- Optional 1Password/Bitwarden CLI integration

## Building from Source

```bash
git clone https://github.com/ishaileshpant/browsync.git
cd browsync
cargo build --release

# Install binaries
cargo install --path crates/browsync-cli
cargo install --path crates/browsync-daemon
```

## Tests

```bash
cargo test --workspace
```

**46 tests**, 100% pass rate. See [TEST_REPORT.md](TEST_REPORT.md) for details.

## License

MIT
