# Watchdog Implementation Guide for Rust Browser-Use

**For the next AI continuing this work**

哥，这份指南列出了所有Python watchdog，帮你继续实现剩余的。

---

## Current Status (2025-01-10)

### ✅ Implemented (Phase 2 Complete)

1. **CrashWatchdog** (`crash.rs`, 280 lines)
   - Monitors browser crashes via `Inspector.targetCrashed`
   - Tracks network request timeouts
   - Auto-detects hung requests

2. **DownloadsWatchdog** (`downloads.rs`, 293 lines)
   - Handles file downloads via `Browser.downloadWillBegin/Progress`
   - Manages download directory
   - Tracks download state (in progress, completed, canceled)

3. **SecurityWatchdog** (`security.rs`, 347 lines)
   - Enforces domain access policies (whitelist/blacklist)
   - Supports wildcard patterns (`*.example.com`)
   - Blocks IP addresses optionally

### ⏳ Remaining (11 Watchdogs)

Need to implement 11 more watchdogs from Python version.

---

## All Python Watchdogs (Complete List)

### Core Watchdogs (High Priority)

#### 4. **DOMWatchdog** (`dom_watchdog.py`, 781 lines)
**Purpose**: Bridge between BrowserSession and DomService

**Key Responsibilities**:
- Build DOM tree from CDP snapshots
- Cache DOM state per target
- Provide element access methods
- Handle accessibility tree integration

**CDP Events**:
- Uses `DOMSnapshot.captureSnapshot` (not subscribed, called on demand)
- Uses `Accessibility.getFullAXTree`

**Complexity**: **HIGH** (DOM parsing, caching, element queries)

**Python Key Methods**:
```python
async def get_current_dom_tree(self) -> DOMTree
async def get_element_by_index(self, index: int) -> EnhancedDOMTreeNode
async def click_element_by_index(self, index: int)
async def type_text_by_index(self, index: int, text: str)
```

**Rust Implementation Notes**:
- Already have `dom` crate (`crates/dom/`)
- Need to integrate as watchdog
- Cache DOM snapshots per target
- Event-driven updates on navigation

**Estimated Lines**: 600-800

---

#### 5. **DefaultActionWatchdog** (`default_action_watchdog.py`, 2681 lines)
**Purpose**: Executes browser actions (click, type, scroll, etc.)

**Key Responsibilities**:
- Click elements via CDP
- Type text with realistic delays
- Scroll to elements
- Handle special cases (print dialogs, file uploads)
- Implement "done" action (signal task completion)

**CDP Commands Used**:
- `DOM.getBoxModel` - Get element position
- `Input.dispatchMouseEvent` - Simulate clicks
- `Input.insertText` - Type text
- `Runtime.evaluate` - Execute JavaScript for scrolling

**Complexity**: **VERY HIGH** (largest watchdog, lots of edge cases)

**Python Key Methods**:
```python
async def click_element(self, index: int)
async def type_text(self, index: int, text: str)
async def scroll_to_element(self, index: int)
async def press_key_combination(self, keys: list[str])
async def go_back()
async fn extract_page_content()
```

**Rust Implementation Strategy**:
1. Start with basic actions (click, type)
2. Add scroll support
3. Handle edge cases incrementally
4. Split into helper modules if needed

**Estimated Lines**: 1500-2000

---

#### 6. **PopupsWatchdog** (`popups_watchdog.py`, 120 lines)
**Purpose**: Auto-handle JavaScript dialogs (alert/confirm/prompt)

**Key Responsibilities**:
- Subscribe to `Page.javascriptDialogOpening` per target
- Auto-accept all dialogs immediately
- Log dialog content for debugging

**CDP Events**:
- `Page.javascriptDialogOpening`

**CDP Commands**:
- `Page.handleJavaScriptDialog` (accept: true)

**Complexity**: **LOW**

**Python Key Code**:
```python
async def attach_to_target(self, target_id: TargetID):
    session = await self.browser_session.get_or_create_cdp_session(target_id)
    
    def on_dialog(event):
        task = asyncio.create_task(self._handle_dialog_cdp(target_id, event))
        self._dialog_tasks.add(task)
    
    session.cdp_client.register.Page.javascriptDialogOpening(on_dialog)
```

**Rust Implementation**:
```rust
async fn on_attach(&self, cdp_client: Arc<CDPClient>) -> Result<...> {
    cdp_client.subscribe("Page.javascriptDialogOpening", Arc::new(move |event| {
        tokio::spawn(async move {
            // Auto-accept dialog
            cdp_client.send_request(
                "Page.handleJavaScriptDialog",
                Some(json!({"accept": true})),
                None
            ).await?;
        });
    }));
}
```

**Estimated Lines**: 100-150

---

### Storage & State Watchdogs (Medium Priority)

#### 7. **StorageStateWatchdog** (`storage_state_watchdog.py`, 301 lines)
**Purpose**: Persist/restore browser state (cookies, localStorage)

**Key Responsibilities**:
- Save cookies to JSON file on `SaveStorageStateEvent`
- Restore cookies from JSON on `LoadStorageStateEvent`
- Handle localStorage persistence
- Auto-save on browser stop

**CDP Commands**:
- `Storage.getCookies` - Export cookies
- `Network.setCookie` - Restore cookies
- `Runtime.evaluate` - Access localStorage

**Complexity**: **MEDIUM**

**Python Key Code**:
```python
async def save_storage_state(self, path: Path):
    cookies = await cdp_client.send.Storage.getCookies()
    # Also get localStorage via JavaScript evaluation
    local_storage = await self._get_local_storage()
    
    state = {
        'cookies': cookies,
        'localStorage': local_storage,
        'timestamp': time.time()
    }
    path.write_text(json.dumps(state))
```

**Rust Implementation**:
- Use `serde_json` for serialization
- Create `StorageState` struct with cookies + localStorage
- Listen to custom events (need to define `SaveStorageStateEvent`)

**Estimated Lines**: 250-300

---

#### 8. **ScreenshotWatchdog** (`screenshot_watchdog.py`, 35 lines)
**Purpose**: Capture screenshots on demand

**Key Responsibilities**:
- Listen to `ScreenshotEvent`
- Call `Page.captureScreenshot` via CDP
- Return base64-encoded image
- Cache screenshots temporarily

**CDP Commands**:
- `Page.captureScreenshot` (format: png/jpeg, quality)

**Complexity**: **LOW** (simple event handler)

**Python Key Code**:
```python
async def on_ScreenshotEvent(self, event: ScreenshotEvent) -> str:
    session = self.browser_session.agent_focus
    result = await session.cdp_client.send.Page.captureScreenshot(
        params={'format': 'png'},
        session_id=session.session_id
    )
    return result['data']  # Base64 string
```

**Rust Implementation**:
- Subscribe to custom `ScreenshotEvent`
- Store result in shared state or return via channel
- Optionally decode base64 and save to file

**Estimated Lines**: 80-120

---

### Browser Lifecycle Watchdogs (Medium Priority)

#### 9. **LocalBrowserWatchdog** (`local_browser_watchdog.py`, 424 lines)
**Purpose**: Manage local Chrome/Chromium subprocess

**Key Responsibilities**:
- Launch Chrome with correct args on `BrowserLaunchEvent`
- Kill Chrome subprocess on `BrowserKillEvent`
- Monitor process health
- Handle graceful shutdown

**Complexity**: **MEDIUM** (process management, platform-specific)

**Python Key Code**:
```python
async def on_BrowserLaunchEvent(self, event: BrowserLaunchEvent):
    chrome_path = self._find_chrome_executable()
    args = self.browser_session.browser_profile.get_chrome_args()
    
    process = await asyncio.create_subprocess_exec(
        chrome_path, *args,
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE
    )
    self._browser_process = process
```

**Rust Implementation**:
- Use `tokio::process::Command`
- Store `Child` handle in Arc<RwLock>
- Platform detection: `cfg!(target_os = "macos")`
- Chrome path detection logic

**Estimated Lines**: 300-400

---

#### 10. **PermissionsWatchdog** (`permissions_watchdog.py`, 19 lines)
**Purpose**: Grant browser permissions on startup

**Key Responsibilities**:
- Grant geolocation, notifications, camera, microphone permissions
- Called once on `BrowserConnectedEvent`

**CDP Commands**:
- `Browser.grantPermissions` (origins, permissions list)

**Complexity**: **TRIVIAL**

**Python Key Code**:
```python
async def on_BrowserConnectedEvent(self, event: BrowserConnectedEvent):
    await self.browser_session.cdp_client.send.Browser.grantPermissions(
        params={
            'permissions': ['geolocation', 'notifications', 'camera', 'microphone'],
            'origin': '*'
        }
    )
```

**Rust Implementation**:
```rust
async fn on_attach(&self, cdp_client: Arc<CDPClient>) -> Result<...> {
    cdp_client.send_request(
        "Browser.grantPermissions",
        Some(json!({
            "permissions": ["geolocation", "notifications", "camera", "microphone"]
        })),
        None
    ).await?;
    Ok(())
}
```

**Estimated Lines**: 30-50

---

### Special Purpose Watchdogs (Lower Priority)

#### 11. **AboutBlankWatchdog** (`aboutblank_watchdog.py`, 219 lines)
**Purpose**: Maintain at least one tab, show DVD screensaver on about:blank

**Key Responsibilities**:
- Ensure at least one tab exists (create about:blank if needed)
- Inject DVD screensaver animation into about:blank pages
- Handle tab close events to prevent closing all tabs

**Complexity**: **LOW-MEDIUM** (UI logic, not critical)

**Python Key Code**:
```python
async def on_TabClosedEvent(self, event: TabClosedEvent):
    page_targets = await self.browser_session._cdp_get_all_pages()
    if len(page_targets) <= 1:
        # Create new about:blank tab to prevent browser from closing
        self.event_bus.dispatch(NavigateToUrlEvent(url='about:blank', new_tab=True))
```

**Rust Implementation**:
- Track tab count
- Create new tab via CDP when last tab closes
- Inject JS animation via `Runtime.evaluate`

**Estimated Lines**: 150-200

---

#### 12. **RecordingWatchdog** (`recording_watchdog.py`, 99 lines)
**Purpose**: Record browser session as video via CDP screencasting

**Key Responsibilities**:
- Start screencast on `BrowserConnectedEvent`
- Capture frames via `Page.screencastFrame`
- Encode frames to video (FFmpeg)
- Stop recording on `BrowserStopEvent`

**CDP Events**:
- `Page.screencastFrame` (emits base64 frames)

**CDP Commands**:
- `Page.startScreencast` (format, quality, maxWidth, maxHeight)
- `Page.stopScreencast`

**Complexity**: **MEDIUM** (video encoding, FFmpeg integration)

**Python Key Code**:
```python
async def on_BrowserConnectedEvent(self, event):
    self._recorder = VideoRecorderService(output_path='recording.mp4')
    
    def on_frame(frame_event):
        # frame_event['data'] is base64 image
        self._recorder.add_frame(frame_event['data'])
    
    cdp_client.register.Page.screencastFrame(on_frame)
    await cdp_client.send.Page.startScreencast(params={
        'format': 'png',
        'quality': 80
    })
```

**Rust Implementation**:
- Use `ffmpeg-next` crate or spawn `ffmpeg` process
- Buffer frames in memory
- Write to file on stop

**Estimated Lines**: 200-300

---

## Implementation Priority

### Phase 3 (Next Steps)

**HIGH PRIORITY** (Core functionality):
1. **PopupsWatchdog** - Easy, high impact (auto-dismiss dialogs)
2. **PermissionsWatchdog** - Trivial, one-time setup
3. **ScreenshotWatchdog** - Simple, useful for debugging

**Estimated effort**: 2-3 hours

### Phase 4 (Essential Features)

**MEDIUM PRIORITY**:
4. **StorageStateWatchdog** - Session persistence
5. **LocalBrowserWatchdog** - Launch Chrome locally
6. **AboutBlankWatchdog** - UX improvement

**Estimated effort**: 1-2 days

### Phase 5 (Advanced Features)

**LOWER PRIORITY**:
7. **DOMWatchdog** - Complex, but `dom` crate already exists
8. **DefaultActionWatchdog** - Largest, split into multiple files
9. **RecordingWatchdog** - Nice to have, not essential

**Estimated effort**: 3-5 days

---

## Implementation Pattern (Proven)

All watchdogs follow this pattern:

```rust
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::cdp::CDPClient;
use crate::events::BrowserEvent;
use crate::watchdog::Watchdog;

pub struct MyWatchdog {
    // Shared state with Arc<RwLock<T>>
    state: Arc<RwLock<MyState>>,
}

#[async_trait]
impl Watchdog for MyWatchdog {
    fn name(&self) -> &str {
        "MyWatchdog"
    }

    async fn on_event(&self, event: &BrowserEvent) {
        match event {
            BrowserEvent::Started => { /* ... */ }
            _ => {}
        }
    }

    async fn on_attach(&self, cdp_client: Arc<CDPClient>) -> Result<...> {
        let state = self.state.clone();
        
        // Subscribe to CDP events
        cdp_client.subscribe("CDP.Event", Arc::new(move |event| {
            let state = state.clone();
            tokio::spawn(async move {
                // Process event asynchronously
                state.write().await.handle(event);
            });
        }));
        
        Ok(())
    }

    async fn on_detach(&self) -> Result<...> {
        self.state.write().await.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_my_watchdog() {
        let watchdog = MyWatchdog::new();
        assert_eq!(watchdog.name(), "MyWatchdog");
    }
}
```

---

## Key Principles (Linus's Standards)

### 1. Good Taste
- **Eliminate special cases**: Use unified patterns
- **Simple data structures**: Arc + RwLock, no complex lifetimes
- **No magic**: Clear, obvious code

### 2. Simplicity
- **Start minimal**: Implement core functionality first
- **Add complexity incrementally**: Test each feature
- **Refactor when needed**: Don't over-engineer upfront

### 3. Never Break Userspace
- **Additive changes only**: New watchdogs don't affect existing ones
- **Backward compatible**: Default setup must work
- **Clear deprecation**: If removing features, warn users

### 4. Real Problems
- **Test against Python version**: Match behavior
- **Validate with real browser**: Don't mock everything
- **Measure performance**: Profile before optimizing

---

## Testing Strategy

### Unit Tests (Required)
```rust
#[tokio::test]
async fn test_watchdog_creation() {
    let watchdog = MyWatchdog::new();
    assert_eq!(watchdog.name(), "MyWatchdog");
}

#[tokio::test]
async fn test_watchdog_state() {
    let watchdog = MyWatchdog::new();
    watchdog.on_event(&BrowserEvent::Started).await;
    // Assert state changed correctly
}
```

### Integration Tests (Recommended)
```rust
// tests/integration/watchdog_test.rs
#[tokio::test]
#[ignore] // Requires running Chrome
async fn test_popup_watchdog_with_real_browser() {
    let session = BrowserSession::new(config);
    session.start().await.unwrap();
    
    // Navigate to page with alert
    // Verify alert is auto-dismissed
}
```

### Manual Testing (Essential)
- Launch real Chrome: `google-chrome --remote-debugging-port=9222`
- Connect Rust code to it
- Trigger watchdog events manually
- Verify behavior matches Python version

---

## Common Pitfalls

### ❌ Don't Do This

1. **Circular references**:
```rust
// ❌ BAD
struct Watchdog {
    session: Arc<BrowserSession>, // Session holds Watchdog, creates cycle
}
```

2. **Blocking in CDP callbacks**:
```rust
// ❌ BAD
cdp_client.subscribe("Event", Arc::new(move |event| {
    // Synchronous callback blocks event loop
    std::thread::sleep(Duration::from_secs(1));
}));
```

3. **Unwrap in production code**:
```rust
// ❌ BAD
let value = event.params["key"].as_str().unwrap(); // Panics if missing
```

### ✅ Do This

1. **Weak references or minimal coupling**:
```rust
// ✅ GOOD
async fn on_attach(&self, cdp_client: Arc<CDPClient>) { /* Only need CDPClient */ }
```

2. **Spawn async tasks**:
```rust
// ✅ GOOD
cdp_client.subscribe("Event", Arc::new(move |event| {
    tokio::spawn(async move {
        // Non-blocking async processing
    });
}));
```

3. **Safe extraction**:
```rust
// ✅ GOOD
let value = event.params
    .as_ref()
    .and_then(|p| p["key"].as_str())
    .unwrap_or("default");
```

---

## Useful Resources

### CDP Protocol
- **Docs**: https://chromedevtools.github.io/devtools-protocol/
- **Event list**: https://chromedevtools.github.io/devtools-protocol/tot/
- **Playground**: chrome://inspect → Protocol Monitor

### Rust Crates
- `tokio` - Async runtime
- `serde_json` - JSON parsing
- `dashmap` - Concurrent HashMap
- `async-trait` - Async trait methods
- `thiserror` - Error types

### Testing
- `tokio::test` - Async tests
- `tokio-test` - Testing utilities
- `tracing-subscriber` - Logging in tests

---

## File Structure

```
crates/browser/src/watchdogs/
├── mod.rs              # Exports
├── crash.rs            # ✅ Done
├── downloads.rs        # ✅ Done
├── security.rs         # ✅ Done
├── popups.rs           # ⏳ TODO (Priority 1)
├── permissions.rs      # ⏳ TODO (Priority 2)
├── screenshot.rs       # ⏳ TODO (Priority 3)
├── storage_state.rs    # ⏳ TODO (Priority 4)
├── local_browser.rs    # ⏳ TODO (Priority 5)
├── aboutblank.rs       # ⏳ TODO (Priority 6)
├── dom.rs              # ⏳ TODO (Priority 7)
├── default_action.rs   # ⏳ TODO (Priority 8)
└── recording.rs        # ⏳ TODO (Priority 9)
```

---

## Quick Start for Next AI

```bash
# 1. Read this guide
cat crates/browser/WATCHDOG_IMPLEMENTATION_GUIDE.md

# 2. Read existing watchdog implementations
cat crates/browser/src/watchdogs/crash.rs      # Network monitoring example
cat crates/browser/src/watchdogs/downloads.rs  # Download tracking example
cat crates/browser/src/watchdogs/security.rs   # Policy enforcement example

# 3. Pick next watchdog (recommend PopupsWatchdog - easiest)
cat browser_use/browser/watchdogs/popups_watchdog.py  # Python reference

# 4. Create new file
touch crates/browser/src/watchdogs/popups.rs

# 5. Implement following the pattern above

# 6. Add to mod.rs
echo "pub mod popups;" >> crates/browser/src/watchdogs/mod.rs

# 7. Test
cargo test --lib -p browser

# 8. Update this guide's status section
```

---

## Success Criteria

Each watchdog is considered complete when:

- [x] Compiles without errors
- [x] All unit tests pass
- [x] Behavior matches Python version (validated manually)
- [x] Documented with rustdoc comments
- [x] Integrated into BrowserSession (optional, can be done later)
- [x] No unsafe blocks
- [x] No unwrap() in production paths

---

**Good luck! 记住Linus的话：**

> "Talk is cheap. Show me the code."

The pattern is proven. The architecture is solid. Just follow it.

🚀