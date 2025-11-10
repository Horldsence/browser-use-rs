# Documentation Guide - For AI Engineers

**Purpose**: 确保文档质量、一致性和可维护性

---

## Documentation Structure

当前文档结构（简化后）:

```
crates/browser/
├── README.md                           # 项目总览、快速开始
├── NEXT_AI_START_HERE.md              # 新AI快速入口（最重要）
├── WATCHDOG_IMPLEMENTATION_GUIDE.md   # Watchdog详细实现指南
├── IMPLEMENTATION_HISTORY.md          # 已完成工作的历史记录
└── DOCUMENTATION_GUIDE.md             # 本文档（文档规范）
```

### 文档职责

| 文档 | 目标读者 | 更新频率 | 内容 |
|------|---------|----------|------|
| **README.md** | 所有人 | 每个Phase | 项目状态、架构、快速开始 |
| **NEXT_AI_START_HERE.md** | 下一个AI | 每次交接 | 当前状态、下一步任务、工作流程 |
| **WATCHDOG_IMPLEMENTATION_GUIDE.md** | 实现者 | 实现新watchdog时 | 详细技术规格、CDP事件 |
| **IMPLEMENTATION_HISTORY.md** | 维护者 | Phase完成时 | 设计决策、性能数据、经验教训 |
| **DOCUMENTATION_GUIDE.md** | 文档维护者 | 添加新文档类型时 | 本文档规范 |

---

## Update Checklist

### 完成一个Watchdog后

- [ ] 更新 `README.md`:
  - [ ] 更新"项目状态"部分的完成数量
  - [ ] 更新"代码统计"表格
  
- [ ] 更新 `NEXT_AI_START_HERE.md`:
  - [ ] 标记完成的watchdog为 ✅
  - [ ] 更新"当前状态"部分
  - [ ] 更新"估算工时"表格

- [ ] 更新 `WATCHDOG_IMPLEMENTATION_GUIDE.md`:
  - [ ] 将watchdog移到"已实现"部分
  - [ ] 添加实际行数和实现笔记

### 完成一个Phase后

- [ ] 更新 `IMPLEMENTATION_HISTORY.md`:
  - [ ] 添加新的Phase章节
  - [ ] 记录关键设计决策
  - [ ] 更新性能指标
  - [ ] 总结经验教训

- [ ] 创建新的Git标签:
  ```bash
  git tag -a phase-N-complete -m "Phase N: [description]"
  ```

### 添加新功能模块后

- [ ] 在 `README.md` 添加架构说明
- [ ] 在 `NEXT_AI_START_HERE.md` 添加新的任务项
- [ ] 创建专门的实现指南（如有必要）

---

## Writing Standards

### 语言规范

**中英混合原则**:
- 技术术语：英文（CDP, watchdog, trait）
- 解释说明：中文（为了清晰）
- 代码注释：英文（Rust社区标准）
- 用户提示：中文（更亲切）

**例子**:
```markdown
✅ 好: "CrashWatchdog监听 `Inspector.targetCrashed` 事件"
❌ 差: "Crash监测器listens to崩溃事件"
```

### 代码示例规范

**必须包含**:
- 文件路径注释
- 关键行数范围（如适用）
- 上下文说明

**格式**:
````markdown
```rust
// src/watchdogs/crash.rs (lines 180-195)
async fn on_attach(&self, cdp_client: Arc<CDPClient>) -> Result<...> {
    cdp_client.subscribe("Inspector.targetCrashed", Arc::new(move |event| {
        // Handle crash
    }));
}
```
````

### 状态标记规范

使用统一的emoji标记：

- ✅ 已完成 (绿色勾)
- ⏳ 进行中 (沙漏)
- 🚧 暂停/受阻 (施工)
- ❌ 失败/废弃 (红叉)
- 📖 文档相关 (书)
- 🎯 优先级高 (靶心)
- 🚀 准备交付 (火箭)

**例子**:
```markdown
- ✅ CrashWatchdog - 已实现
- ⏳ PopupsWatchdog - 进行中
- 🎯 PermissionsWatchdog - 优先级1
```

---

## Template: NEXT_AI_START_HERE.md

每次交接时，必须更新以下章节：

```markdown
## 当前状态 (YYYY-MM-DD)

### ✅ 已完成的工作
[列出所有完成的模块/功能]

### ⏳ 正在进行的工作
[如果有未完成的，说明进度和剩余工作]

### 🎯 你的任务
[清晰列出下一步要做什么，按优先级排序]

## 工作流程（照着做）
[Step by step指南，假设读者是第一次接触项目]

## 必读文档（按顺序）
[列出相关文档，解释为什么要读]

## 估算工时
[实际的时间估算，基于已完成工作的经验]
```

---

## Template: Watchdog Implementation

实现新watchdog时，在 `WATCHDOG_IMPLEMENTATION_GUIDE.md` 添加：

```markdown
#### N. **WatchdogName** (`filename.py`, XXX lines)
**Purpose**: [一句话说明用途]

**Key Responsibilities**:
- [责任1]
- [责任2]

**CDP Events**:
- `Domain.eventName` - [说明]

**CDP Commands**:
- `Domain.commandName` - [说明]

**Complexity**: ⭐⭐⭐☆☆ ([简单/中等/复杂])

**Python Key Code**:
```python
[关键代码片段，不超过20行]
```

**Rust Implementation Notes**:
- [实现要点1]
- [实现要点2]
- [注意事项]

**Estimated Lines**: XXX-XXX
```

---

## Template: Phase Completion

Phase完成后，在 `IMPLEMENTATION_HISTORY.md` 添加：

```markdown
## Phase N: [Phase名称] (YYYY-MM-DD)

### Overview
[2-3段总结这个Phase做了什么]

### Key Metrics
| Metric | Value |
|--------|-------|
| Lines added | XXX |
| Tests added | XX |
| Time spent | XX days |

### Technical Decisions
[重要的技术决策，用ADR格式]

### Challenges & Solutions
[遇到的主要问题及解决方案]

### Lessons Learned
[经验教训，供未来参考]

### Performance Data
[性能测试结果，如有]
```

---

## Anti-Patterns (Don't Do This)

### ❌ 过时信息
```markdown
❌ "Phase 2.1完成，Phase 2.2进行中"
# 如果整个Phase 2都完成了，这就是过时信息
```

### ❌ 重复内容
```markdown
❌ 在3个文档里都写相同的"如何实现watchdog"
# 应该：一个权威文档，其他引用它
```

### ❌ 模糊指令
```markdown
❌ "实现剩余的watchdog"
# 应该：列出具体的watchdog名称、优先级、预估时间
```

### ❌ 缺少上下文
```markdown
❌ "修复了bug"
# 应该：什么bug？为什么出现？如何修复？如何避免？
```

### ❌ 假设读者知识
```markdown
❌ "用标准的CDP模式"
# 应该：明确说明是什么模式，提供代码示例
```

---

## Best Practices

### ✅ 时间戳所有重要更新
```markdown
## 项目状态 (2025-01-10更新)
```

### ✅ 链接相关文档
```markdown
详见 [WATCHDOG_IMPLEMENTATION_GUIDE.md](./WATCHDOG_IMPLEMENTATION_GUIDE.md)
```

### ✅ 提供快速开始命令
```markdown
```bash
# 启动测试
cargo test --lib -p browser
```
```

### ✅ 包含失败案例
```markdown
## 尝试过但不work的方案
- 方案A：[为什么失败]
- 方案B：[为什么不采用]
```

### ✅ 量化信息
```markdown
✅ "实现PopupsWatchdog需要2小时"
❌ "PopupsWatchdog很简单"
```

---

## Document Maintenance Schedule

### 每天（如果有代码改动）
- 更新代码注释
- 运行测试确保文档中的命令有效

### 每周（如果有进度）
- 更新 `NEXT_AI_START_HERE.md` 的进度
- 检查文档是否有过时信息

### 每个Phase完成后
- 更新所有相关文档
- 整合可整合的内容
- 删除过时文档
- 验证所有链接有效

### 每次交接前
- 完整阅读 `NEXT_AI_START_HERE.md`
- 确保新AI能够无障碍开始工作
- 测试所有命令和代码示例

---

## Documentation Quality Checklist

提交文档前，确认：

- [ ] **准确性**: 所有技术信息正确无误
- [ ] **完整性**: 包含必要的上下文和背景
- [ ] **清晰性**: 假设读者首次接触，解释充分
- [ ] **时效性**: 标注日期，反映当前状态
- [ ] **可操作**: 提供具体步骤和命令
- [ ] **简洁性**: 避免冗余，每个信息只说一次
- [ ] **链接性**: 正确引用其他文档
- [ ] **测试过**: 所有命令都实际运行过

---

## File Size Guidelines

控制文档大小，超过则考虑拆分：

| 文档类型 | 建议行数 | 最大行数 |
|---------|---------|---------|
| README | 200-400 | 600 |
| 快速开始 | 300-500 | 800 |
| 实现指南 | 500-800 | 1200 |
| 历史记录 | 不限 | 每Phase独立章节 |

---

## Example: Good Documentation Flow

新AI的阅读路径：

1. **README.md** (5分钟)
   - 理解项目是什么
   - 看到当前状态
   - 知道如何运行测试

2. **NEXT_AI_START_HERE.md** (15分钟)
   - 理解自己的任务
   - 看到工作流程
   - 获得工时估算

3. **WATCHDOG_IMPLEMENTATION_GUIDE.md** (30分钟)
   - 深入理解要实现的watchdog
   - 查看CDP事件/命令
   - 复制实现模板

4. **现有代码** (1小时)
   - 阅读已实现的watchdog
   - 理解代码模式
   - 开始编码

**总计**: ~2小时从零到开始编码

---

## Version Control for Docs

### Commit Message Format

文档更新的commit message：

```
docs: [type] brief description

[optional detailed explanation]

Files changed:
- README.md: updated status
- NEXT_AI_START_HERE.md: added PopupsWatchdog task
```

**Types**:
- `docs: update` - 更新现有内容
- `docs: add` - 添加新章节
- `docs: fix` - 修正错误信息
- `docs: refactor` - 重组文档结构
- `docs: delete` - 删除过时内容

### Branch Strategy

- 主分支文档必须始终准确反映代码状态
- 大规模文档重构在单独分支进行
- 文档PR与代码PR分开（便于review）

---

## Metrics for Success

好文档的标准：

1. **新AI启动时间** < 2小时
2. **文档准确率** > 95%（通过代码验证）
3. **重复问题** = 0（相同问题不问第二次）
4. **过时信息** = 0（定期审查）
5. **链接失效** = 0（自动化检查）

---

## Tools & Automation

### 推荐工具

```bash
# 检查Markdown格式
markdownlint *.md

# 检查链接有效性
markdown-link-check *.md

# 统计文档行数
wc -l *.md
```

### 自动化脚本示例

```bash
#!/bin/bash
# scripts/check_docs.sh

echo "Checking documentation..."

# 1. 验证所有代码示例可编译
rg '```rust' -A 20 *.md | grep -v test | rustfmt --check

# 2. 检查日期戳是否过时（超过30天）
# ... (省略具体实现)

# 3. 验证所有链接
markdown-link-check *.md

echo "Documentation check complete!"
```

---

## Emergency Contact (For Critical Issues)

如果文档严重过时或误导：

1. **立即停止依赖该文档**
2. **在文档顶部添加警告**:
   ```markdown
   > ⚠️ WARNING: This document is outdated as of YYYY-MM-DD
   > Please refer to [updated_doc.md] instead.
   ```
3. **创建issue跟踪更新任务**
4. **优先级最高修复**

---

## Final Words

**文档是代码的一部分**。

像对待生产代码一样对待文档：
- Code review文档PR
- 测试文档中的命令
- 重构过时的文档
- 维护文档的"测试覆盖率"

**Remember**: 下一个AI依赖你的文档。不要让他们浪费时间猜测。

---

**Linus说**: "Bad documentation is worse than no documentation."

让文档配得上这个项目的代码质量。

🚀