# Browser + DOM 集成指南

## 概述

哥，`crates/dom/` 已经有一个优秀的 DOM 实现了！现在只需要把它和 `crates/browser/` 集成起来。

## DOM Crate 分析

### 核心优势 (已实现)

1. **Arena 分配** - Linus 会赞许的设计：

```rust
// 不是这样（Python风格，慢）：
struct Node {
    children: Vec<Rc<Node>>  // 16字节/指针 + 递归 + GC压力
}

// 而是这样（好品味）：
struct DomArena {
    nodes: Vec<DomNode>,  // 顺序内存，缓存友好
}
type NodeId = u32;  // 4字节索引，不是8字节指针
```

2. **零拷贝设计** - 借用优于拥有
3. **类型安全** - 编译期检查节点类型

### 已有功能

```rust
// crates/dom/src/arena.rs
pub struct DomArena {
    nodes: Vec<DomNode>,              // 所有节点
    backend_id_map: AHashMap<u32, NodeId>,  // CDP ID映射
    root_id: Option<NodeId>,
}

// crates/dom/src/types.rs
pub struct DomNode {
    pub node_id: NodeId,
    pub backend_node_id: u32,        // CDP 提供的ID
    pub node_type: NodeType,
    pub node_name: String,
    pub parent_id: Option<NodeId>,
    pub children: Vec<NodeId>,
    // ... 更多字段
}

// crates/dom/src/service.rs
pub struct DomService {
    config: DomServiceConfig,
    arena: DomArena,
}
```

## 集成步骤

### Step 1: 添加依赖

**文件**: `crates/browser/Cargo.toml`

```toml
[dependencies]
# ... 现有依赖 ...
dom = { path = "../dom" }
```

### Step 2: 创建集成模块

**文件**: `crates/browser/src/dom_integration.rs`

```rust
//! DOM Integration - Bridge between CDP and DOM crate
//!
//! This module:
//! 1. Fetches DOM from CDP (Page.captureSnapshot, DOM.getDocument)
//! 2. Converts CDP JSON to DomArena
//! 3. Provides selector maps for agent actions

use crate::cdp::CDPSession;
use dom::{DomService, DomArena, DomNode, NodeId};
use serde_json::Value;
use std::collections::HashMap;

pub struct DomIntegration {
    service: DomService,
}

impl DomIntegration {
    pub fn new() -> Self {
        Self {
            service: DomService::new(),
        }
    }

    /// Capture current page DOM via CDP
    pub async fn capture_from_session(
        &mut self,
        session: &CDPSession,
    ) -> Result<&DomArena, Box<dyn std::error::Error>> {
        // 1. Get document node
        let doc_result = session.send("DOM.getDocument", None).await?;

        // 2. Get flattened document (faster than recursive)
        let flatten_result = session.send(
            "DOM.getFlattenedDocument",
            Some(serde_json::json!({
                "depth": -1,  // Get entire tree
                "pierce": true,  // Include shadow DOM
            })),
        ).await?;

        // 3. Parse into DomArena
        self.parse_cdp_nodes(&flatten_result)?;

        Ok(self.service.arena())
    }

    /// Parse CDP node array into DomArena
    fn parse_cdp_nodes(&mut self, result: &Value) -> Result<(), Box<dyn std::error::Error>> {
        let nodes = result["nodes"]
            .as_array()
            .ok_or("Invalid CDP response")?;

        // TODO: 遍历nodes，调用 self.service.arena_mut().add_node()
        // 这里需要根据 dom crate 的具体API调整

        Ok(())
    }

    /// Generate selector map for agent (index -> node)
    /// This is what the agent uses to click elements
    pub fn get_selector_map(&self) -> HashMap<u32, &DomNode> {
        let arena = self.service.arena();
        let mut map = HashMap::new();

        // TODO: 遍历arena中的可交互节点，生成索引
        // 匹配 Python 的 get_selector_map 逻辑

        map
    }
}
```

### Step 3: 在 BrowserSession 中使用

**文件**: `crates/browser/src/session.rs`

添加字段：

```rust
use crate::dom_integration::DomIntegration;

pub struct BrowserSession {
    // ... 现有字段 ...

    /// DOM integration
    dom_integration: Arc<RwLock<DomIntegration>>,
}
```

添加方法：

```rust
impl BrowserSession {
    /// Get current page DOM
    pub async fn get_dom(&self) -> Result<DomArena, Box<dyn std::error::Error>> {
        let session = self.current_session().await
            .ok_or("No active session")?;

        let mut dom = self.dom_integration.write().await;
        dom.capture_from_session(&session).await?;

        // TODO: 返回arena的克隆或引用
        Ok(DomArena::new())  // 占位
    }

    /// Get element by index (for agent actions)
    pub async fn get_element_by_index(&self, index: u32) -> Option<DomNode> {
        let dom = self.dom_integration.read().await;
        let selector_map = dom.get_selector_map();
        selector_map.get(&index).map(|n| (*n).clone())
    }
}
```

### Step 4: 更新 lib.rs

**文件**: `crates/browser/src/lib.rs`

```rust
pub mod dom_integration;

// 重新导出 dom crate 的类型，方便使用
pub use dom::{DomArena, DomNode, NodeId, NodeType};
```

## 测试集成

**文件**: `crates/browser/examples/dom_capture.rs`

```rust
use browser::session::{BrowserSession, SessionConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = SessionConfig::default();
    let session = BrowserSession::new(config);

    session.start().await?;
    session.new_tab(Some("https://example.com".to_string())).await?;

    // Wait for page load
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Capture DOM
    let dom = session.get_dom().await?;
    println!("Captured {} nodes", dom.node_count());

    // Get selector map
    if let Some(element) = session.get_element_by_index(1).await {
        println!("Element 1: {:?}", element);
    }

    session.stop().await?;
    Ok(())
}
```

## 性能优化提示

### 1. 避免不必要的克隆

```rust
// ❌ 克隆整个arena
pub async fn get_dom(&self) -> DomArena {
    self.arena.clone()  // 慢！
}

// ✅ 返回引用
pub async fn get_dom(&self) -> &DomArena {
    &self.arena
}
```

### 2. 使用 Arc 共享 DomArena

```rust
pub struct BrowserSession {
    current_dom: Arc<RwLock<Option<DomArena>>>,
}

// 多个地方可以持有引用，不需要复制
```

### 3. 缓存 selector map

```rust
pub struct DomIntegration {
    cached_selector_map: Option<HashMap<u32, NodeId>>,
}

impl DomIntegration {
    pub fn get_selector_map(&mut self) -> &HashMap<u32, NodeId> {
        if self.cached_selector_map.is_none() {
            self.cached_selector_map = Some(self.build_selector_map());
        }
        self.cached_selector_map.as_ref().unwrap()
    }
}
```

## 下一步

1. ✅ 阅读 `crates/dom/src/service.rs` 了解 API
2. ✅ 实现 `parse_cdp_nodes()` 将 CDP JSON 转为 DomNode
3. ✅ 实现 `get_selector_map()` 匹配 Python 行为
4. ✅ 添加截图功能（`Page.captureScreenshot`）
5. ✅ 测试性能（应该比 Python 快 5-10x）

## 参考

- Python DOM: `browser_use/dom/service.py`
- Python selector map: `browser_use/browser/session.py::get_selector_map()`
- CDP DOM API: https://chromedevtools.github.io/devtools-protocol/tot/DOM/

---

**记住**: DOM crate 已经做了最难的部分（arena 分配），你只需要写胶水代码！
