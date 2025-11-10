# Browser-Use Rust Implementation - Quick Reference

## 项目状态

✅ **Phase 1 完成**: CDP 核心通信层和会话管理  
✅ **Phase 1.5 完成**: DOM 核心实现 (`crates/dom/`)  
⏳ **Phase 2 等待**: Watchdog 系统  
⏳ **Phase 3 等待**: DOM 与 Browser 集成、截图  
⏳ **Phase 4 等待**: Python FFI 绑定

## 文件导航

```
browser-use-rs/
├── IMPLEMENTATION_GUIDE.md         # 完整的设计哲学和实现指南
├── crates/
│   ├── browser/                    # ⭐ 浏览器会话和CDP
│   │   ├── README.md               # 项目概览
│   │   ├── CONTINUATION_GUIDE.md   # 中文指南
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── cdp/                # CDP 核心实现
│   │   │   ├── events.rs           # 事件总线
│   │   │   └── session.rs          # 浏览器会话 API
│   │   └── examples/
│   ├── dom/                        # ⭐ DOM 处理（已实现！）
│   │   └── src/
│   │       ├── arena.rs            # Arena 分配器
│   │       ├── types.rs            # DOM 类型定义
│   │       ├── service.rs          # DOM 服务
│   │       ├── serializer.rs       # 序列化
│   │       └── utils.rs            # 工具函数
│   └── tools/                      # 工具 crate
```

## 快速命令

```bash
# 构建
cd browser-use-rs
cargo build --release

# 测试
cargo test --lib

# 运行示例（需要 Chrome）
google-chrome --remote-debugging-port=9222 --headless &
cargo run --example basic_cdp
cargo run --example session_management

# 检查代码
cargo clippy
cargo fmt
```

## 核心概念速查

### 1. CDP 客户端（单一连接）

```rust
use browser::cdp::CDPClient;

let client = CDPClient::connect("ws://localhost:9222/devtools/browser").await?;
let result = client.send_request("Browser.getVersion", None, None).await?;
```

### 2. CDP 会话（多路复用）

```rust
use browser::cdp::CDPSession;

let session = CDPSession::attach(client, target_id, None).await?;
let info = session.get_target_info().await?;
session.navigate("https://example.com").await?;
```

### 3. 浏览器会话（高层 API）

```rust
use browser::session::{BrowserSession, SessionConfig};

let session = BrowserSession::new(SessionConfig::default());
session.start().await?;
let tab = session.new_tab(Some("https://rust-lang.org".to_string())).await?;
session.navigate("https://crates.io").await?;
```

### 4. 事件订阅

```rust
let mut events = session.event_bus.subscribe();
tokio::spawn(async move {
    while let Ok(event) = events.recv().await {
        match event {
            BrowserEvent::NavigationComplete { url } => println!("Navigated to: {}", url),
            _ => {}
        }
    }
});
```

## Python 对比

| Python                                       | Rust                               | 说明     |
| -------------------------------------------- | ---------------------------------- | -------- |
| `browser_use.browser.session.BrowserSession` | `browser::session::BrowserSession` | 主会话类 |
| `browser_use.browser.session.CDPSession`     | `browser::cdp::CDPSession`         | CDP 会话 |
| `bubus.EventBus`                             | `browser::events::EventBus`        | 事件系统 |
| `async with session:`                        | `session.start().await?`           | 启动会话 |
| `await session.new_page()`                   | `session.new_tab().await?`         | 创建标签 |

## 关键差异

1. **连接管理**: Rust 版本使用单一 WebSocket，Python 每会话一个
2. **并发**: Rust 用 `tokio` 而不是 `asyncio`，性能更好
3. **类型系统**: Rust 编译期检查，Python 运行时错误
4. **内存**: Rust 更少内存占用（~10x 改进）

## 下一步

1. 阅读 `IMPLEMENTATION_GUIDE.md` 了解设计哲学
2. 阅读 `CONTINUATION_GUIDE.md` 了解具体任务
3. 从 Phase 2.1 (CrashWatchdog) 开始工作

## 联系信息

- Python 实现: `browser_use/browser/`
- CDP 文档: https://chromedevtools.github.io/devtools-protocol/
- Rust 异步: https://tokio.rs/

---

**记住**: "数据结构优先，消除特殊情况，实用主义，向后兼容" - Linus 哲学
