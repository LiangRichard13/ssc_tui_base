# SAITEC Packager Skill Design

## Goal

为 `G:\Workspace\Project2026\JCode\jcode` 生成一套可复用的打包 skill，用来稳定触发 SAITEC-TUI 的本地打包流程，并支持参数化指定输出位置。默认输出目录为：

`dist/saitec-tui-<yyyyMMdd-HHmmss>`

这个 skill 的目标不是重新实现一套新的打包体系，而是把现有仓库里的 `scripts/package_saitec.ps1` 封装成一个更适合日常调用、可参数化、可校验、可复用的技能。

## Recommendation

推荐采用“薄封装 skill”方案：

- skill 负责描述何时使用、需要哪些参数、如何做前置检查、如何解释产物位置
- skill 自带一个很薄的包装脚本，统一时间戳目录与参数解析
- 实际打包动作仍复用仓库已有的 `scripts/package_saitec.ps1`

不推荐把现有打包逻辑完整复制到 skill 中，因为那会造成两套 PowerShell 打包流程并行维护，后续仓库脚本升级时容易漂移。

## Scope

本次 skill 设计覆盖：

- 定义 skill 的触发场景
- 统一 SAITEC-TUI 打包参数入口
- 提供默认输出目录规则
- 复用现有 `scripts/package_saitec.ps1`
- 在打包前做最小必要校验
- 在打包后明确返回实际产物路径

本次不覆盖：

- 重写 `scripts/package_saitec.ps1` 的内部打包逻辑
- 新增跨平台打包能力
- 修改 Rust 构建链
- 修改 installer 行为

## User Experience

### Invocation intent

当用户提出以下需求时应触发这个 skill：

- “打包 SAITEC-TUI”
- “生成 SAITEC 发布目录”
- “把当前版本导出到 dist”
- “指定目录打包 SAITEC-TUI”
- “输出到某个自定义目录”

### Default behavior

如果用户没有指定输出目录，skill 应默认把产物放到：

`G:\Workspace\Project2026\JCode\jcode\dist\saitec-tui-<yyyyMMdd-HHmmss>`

其中时间戳格式固定为：

`yyyyMMdd-HHmmss`

例如：

`dist/saitec-tui-20260513-153045`

### Parameterized behavior

如果用户指定输出目录，则优先使用用户提供的目录。

如果用户只指定父目录而没有指定最终目录名，则 skill 仍可在该父目录下生成：

`saitec-tui-<yyyyMMdd-HHmmss>`

这样既保持默认行为一致，也避免覆盖已有产物。

## Skill Shape

skill 名称：

`saitec-tui-packager`

建议目录结构：

```text
saitec-tui-packager/
├── SKILL.md
├── agents/
│   └── openai.yaml
├── scripts/
│   └── package_saitec_tui.ps1
└── references/
    └── usage.md
```

说明：

- `SKILL.md` 负责定义触发条件、参数语义、执行顺序
- `scripts/package_saitec_tui.ps1` 是对仓库 `scripts/package_saitec.ps1` 的薄包装
- `references/usage.md` 放更具体的参数示例，避免把 SKILL.md 写得太长

## Parameters

推荐支持以下参数：

- `output_dir`
- `timestamp`
- `profile`
- `target_triple`
- `include_debug_symbols`
- `skip_build`
- `open_output`

参数语义如下。

### output_dir

用户显式指定最终输出目录。

示例：

`output_dir=G:\Builds\saitec-demo`

### timestamp

允许用户覆盖默认时间戳，主要用于可复现目录名或 CI 场景。

如果未提供，则自动生成当前本地时间戳。

### profile

默认值：

`release`

直接映射到现有 `scripts/package_saitec.ps1 -Profile ...`

### target_triple

默认空字符串，保持与现有打包脚本兼容。

### include_debug_symbols

默认关闭。开启时透传给现有打包脚本。

### skip_build

默认关闭。

打开后，skill 只做产物存在性检查与打包，不主动先执行构建步骤。适合用户已经手动构建完成的情况。

### open_output

默认关闭。

打开后，在打包成功后可用系统文件管理器打开输出目录，便于用户快速检查产物。

## Execution Flow

### Step 1: Resolve repo root

skill 先确认当前工作目录位于 `jcode` 仓库内，至少应能找到：

- `Cargo.toml`
- `scripts/package_saitec.ps1`

如果找不到，则直接报错，不进入后续步骤。

### Step 2: Resolve output path

输出路径规则如下：

- 若提供 `output_dir`，直接使用该目录
- 否则使用 `dist/saitec-tui-<timestamp>`
- 若只提供上级目录这一变体，则在上级目录下拼接 `saitec-tui-<timestamp>`

### Step 3: Optional build

若 `skip_build=false`，先执行 release 构建检查。推荐行为：

- 检查 `target/<profile>/jcode.exe` 是否存在
- 若不存在，则执行 `cargo build --release -p jcode --bin jcode`

这里不扩展新的 build 策略，只维持最小必要构建。

### Step 4: Invoke existing packager

调用仓库现有：

`G:\Workspace\Project2026\JCode\jcode\scripts\package_saitec.ps1`

该脚本当前默认输出到：

`dist/saitec-tui`

因此包装层需要在其执行完成后，把该目录内容复制或移动到最终目标目录。

推荐采用“先生成标准目录，再复制到时间戳目录”的策略，而不是修改仓库脚本默认行为。这样对现有仓库侵入更小。

### Step 5: Report outputs

skill 在成功后必须返回：

- 最终输出目录
- `saitec-tui.exe` 的绝对路径
- `install.ps1` 的绝对路径
- 是否包含 `SAITEC_logo.png`
- 是否包含 `saitec-tui.pdb`

## Wrapper Script Design

`scripts/package_saitec_tui.ps1` 建议只做四件事：

1. 解析参数
2. 解析默认时间戳输出目录
3. 调用仓库现有 `scripts/package_saitec.ps1`
4. 将 `dist/saitec-tui` 同步到最终输出目录

包装脚本不应该重复实现 installer 生成逻辑，也不应该在 skill 内维护第二套品牌复制规则。

## Validation

至少需要以下校验：

- `scripts/package_saitec.ps1` 存在
- 若 `skip_build=true`，则 `target/<profile>/jcode.exe` 存在
- 仓库默认产物目录 `dist/saitec-tui` 在打包后存在
- 最终输出目录存在
- 最终输出目录下存在 `saitec-tui.exe`
- 最终输出目录下存在 `install.ps1`

## Risks And Tradeoffs

### Risk 1: Two output directories

由于现有仓库脚本固定输出 `dist/saitec-tui`，而 skill 默认需要输出时间戳目录，因此短期内会存在：

- `dist/saitec-tui`
- `dist/saitec-tui-<timestamp>`

这是当前推荐方案接受的代价，优点是无需侵入修改仓库原脚本。

### Risk 2: Copy vs move

如果采用 move，用户可能失去仓库默认目录。

如果采用 copy，会多占一份磁盘空间。

推荐默认 copy，更安全，也更符合“skill 是封装层”的定位。

### Risk 3: Build assumptions

若用户使用非 `release` 目录或自定义 `target_triple`，包装层必须显式透传这些参数，否则容易误判产物不存在。

## Success Criteria

当该 skill 实现完成后，以下场景应工作正常：

1. 用户在 `jcode` 仓库中直接说“打包 SAITEC-TUI”，skill 输出到 `dist/saitec-tui-<timestamp>`
2. 用户指定 `output_dir`，skill 输出到指定目录
3. 用户指定 `skip_build=true` 且已有构建产物，skill 不重复构建
4. 用户开启 `include_debug_symbols`，最终目录中包含 `.pdb`
5. skill 返回清晰的最终产物路径，便于后续分发

## Implementation Plan Boundary

下一阶段实现只应包含：

- 创建 `saitec-tui-packager` skill 目录
- 编写 `SKILL.md`
- 生成 `agents/openai.yaml`
- 编写 `scripts/package_saitec_tui.ps1`
- 编写 `references/usage.md`
- 运行 skill 基本校验

不应在这个阶段顺手重构仓库原始打包脚本，除非后续验证发现薄封装无法满足目标。
