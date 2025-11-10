# Browser-Use Rust Implementation Guide

**哥，这是给后续 AI 继续完成工作的指导文档。**

## 设计哲学 (Linus-Approved)

### 1. 数据结构优先 (Data Structures First)

```
"Bad programmers worry about code. Good programmers worry about data structures."
```

**核心数据流**：

```
WebSocket (单一连接)
    ↓
CDPClient (请求路由)
    ↓
CDPSession (per-target 上下文)
    ↓
BrowserSession (状态管理)
```

**关键决策**：

- 单一 WebSocket 连接：消除 Python 代码中的连接管理复杂性
- 无锁热路径：用 `DashMap`而不是 `Arc<Mutex<HashMap>>`
- 零拷贝：`Arc<CDPClient>`共享，不是克隆

### 2. 消除特殊情况 (Eliminate Special Cases)

Python 代码中的典型烂摊子：

```python
if event.new_tab:
    if current_page_is_new_tab():
        event.new_tab = False  # WTF?!
```

Rust 版本：

```rust
// 根本不需要这个逻辑 - 让调用者明确意图
pub async fn navigate(&self, url: String, mode: NavigationMode)
```

**消除了**：

- 10+ if/else 分支判断是否是新标签页
- 3 层嵌套的异常处理
- "maybe_cache" 之类的命名

### 3. 向后兼容 (Never Break Userspace)

**FFI 桥接策略**：

```rust
// Phase 1: Rust核心，Python可以通过FFI调用
#[no_mangle]
pub extern "C" fn browser_session_new(config: *const SessionConfig) -> *mut BrowserSession

// Phase 2: 逐步迁移Python代码到Rust
// Phase 3: Python只保留薄的包装层
```

**不破坏的东西**：

- Python API 表面（`session.navigate()` 仍然工作）
- 配置文件格式
- 事件名称和结构

## 已完成 (Phase 1 - 核心 CDP 层)

### ✅ CDP 通信层 (`cdp/client.rs`)

**核心洞察**：

- Python 用 `asyncio.Queue`做请求响应匹配 → Rust 用 `DashMap<RequestId, oneshot::Sender>`
- Python 每个事件类型一个 handler 列表 → Rust 用 `Arc<dyn Fn>`避免动态分发

**性能赢点**：

```
Python: 每条消息都要GIL + 字典查找 + 动态分发
Rust:   无锁DashMap + 零成本闭包 + 编译期类型检查
```

### ✅ CDP 会话层 (`cdp/session.rs`)

**简化的地方**：

- 移除了 Python 的"maybe attach"逻辑 → 要么 attach 要么失败
- 并行 enable domains：`join_all` 而不是顺序 await
- 没有"session pool cache" → 调用者自己管理生命周期

### ✅ 事件系统 (`events.rs`)

**为什么用 enum 不是 trait**：

```rust
// ❌ Python风格 - 动态分发
trait BrowserEvent {}
Box<dyn BrowserEvent>

// ✅ Rust风格 - 零成本
enum BrowserEvent {
    Started,
    Stopped { reason: String },
    // ...
}
```

**好处**：

- 编译期检查所有事件类型
- 模式匹配 > if/else chains
- `size_of::<BrowserEvent>()` = 最大 variant 大小（栈分配）

## 未完成 (Phase 2 & 3)

### ⏳ Phase 2: 状态管理和 Watchdogs

**当前问题**：Python 代码有 11 个 watchdog，每个都有自己的状态：

```python
_crash_watchdog: Any | None
_downloads_watchdog: Any | None
# ... 9 more ...
```

**Rust 方案**：

```rust
// 统一的Watchdog trait
#[async_trait]
trait Watchdog {
    async fn on_event(&self, event: &BrowserEvent);
}

// 组合，不是继承
struct BrowserSession {
    watchdogs: Vec<Box<dyn Watchdog>>,
}
```

**要做的**：

1. 定义 `Watchdog` trait
2. 实现关键 watchdog：
   - CrashWatchdog (检测页面崩溃)
   - DownloadsWatchdog (文件下载)
   - SecurityWatchdog (绕过证书警告)
3. 测试与 Python 行为一致性

### ⏳ Phase 3: DOM 和截图

**复杂点**：

- DOM 序列化涉及大量 JSON
- 截图需要二进制数据处理
- iframe 递归处理

**性能关键**：

```rust
// 使用 serde_json::from_str 的零拷贝特性
#[derive(Deserialize)]
struct DOMNode<'a> {
    #[serde(borrow)]
    tag_name: &'a str,  // 不分配新String
}
```

## 下一步优先级

1. **⚡ 立即做**：实现 `CrashWatchdog` - 这是用户最常遇到的问题
2. **🔜 尽快做**：完成 `session.rs`中的错误恢复逻辑
3. **📅 可以晚点**：DOM 处理（Python 版本够用）
4. **🤔 待定**：是否需要 Rust 版本的录屏功能

## 避免的陷阱

### ❌ 不要做的事

1. **不要过度工程化**

   ```rust
   // ❌ 别搞一堆trait hierarchy
   trait Session {}
   trait AttachedSession: Session {}
   trait FocusedSession: AttachedSession {}

   // ✅ 简单的struct就够了
   struct CDPSession { ... }
   ```

2. **不要异步所有东西**

   ```rust
   // ❌ 这个不需要async
   async fn get_session_id(&self) -> String

   // ✅ 简单getter
   fn session_id(&self) -> &str
   ```

3. **不要忽略错误**

   ```rust
   // ❌ Python风格
   let _ = self.do_something();

   // ✅ 显式处理
   if let Err(e) = self.do_something() {
       tracing::warn!("Failed: {}", e);
   }
   ```

## 性能目标

基于 Python 版本的 profiling：

| 操作          | Python 耗时 | Rust 目标 | 瓶颈           |
| ------------- | ----------- | --------- | -------------- |
| CDP 请求/响应 | ~5ms        | <1ms      | 网络延迟占主要 |
| DOM 解析      | ~50ms       | <10ms     | JSON 解析      |
| 创建新标签    | ~100ms      | <50ms     | Chrome 启动    |
| 截图          | ~200ms      | ~200ms    | Chrome 渲染    |

**现实检查**：网络和 Chrome 本身的延迟才是主要瓶颈。Rust 的赢点在于：

- 更低的 CPU 占用
- 更小的内存占用
- 更好的并发性能（多个 browser session）

## 代码风格

遵循 Linus 的"好品味"原则：

```rust
// ✅ 好品味 - 没有特殊情况
async fn attach_to_target(&self, target_id: TargetId) -> Result<CDPSession> {
    let session = CDPSession::attach(self.client.clone(), target_id, None).await?;
    self.sessions.insert(target_id, session.clone());
    Ok(session)
}

// ❌ 烂代码 - 充满特殊情况
async fn maybe_attach_to_target(&self, target_id: Option<TargetId>) -> Result<Option<CDPSession>> {
    if let Some(target_id) = target_id {
        if !self.sessions.contains_key(&target_id) {
            if self.should_attach() {
                // ...
            }
        }
    }
    Ok(None)
}
```

## 结语

**哥，记住**：

1. 数据结构优先 - 正确的类型让代码自己写自己
2. 消除特殊情况 - 如果有 3 层 if 嵌套，重新设计
3. 实用主义 - 先让它工作，再让它快，最后让它优雅
4. 向后兼容 - 不破坏 Python 用户的代码

这不是"重写"，这是"重新思考"。Python 版本教会了我们什么是不该做的，现在我们用 Rust 做对的方式。

**下一个 AI 继续时的检查清单**：

- [ ] 读完这个文档
- [ ] 运行 `cargo build`确保编译通过
- [ ] 看看 Python 的 `watchdog_base.py` - 那是下一步要攻克的
- [ ] 记住：简单 > 聪明

---

_"Talk is cheap. Show me the code."_ - Linus Torvalds
