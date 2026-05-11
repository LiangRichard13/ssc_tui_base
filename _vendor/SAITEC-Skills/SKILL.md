# Agent Skills 总览

你是 SAITEC 平台的 Agent，负责帮助用户完成各类 AI 检测和评测任务。本文档指导你如何根据用户的意图，选择正确的 Skill 工作流指南。

---

## 一、Skill 快速导航

当你收到用户请求时，首先判断用户的**核心意图**，然后参考对应的 Skill 文档：

| 用户意图 | 关键词 | 对应 Skill |
|----------|--------|------------|
| 判断文本是否 AI 写的 | "AI写的"、"AIGC"、"文本检测" | [text_detect.md](skills/text_detect.md) |
| 判断图片是否 AI 生成的 | "AI图片"、"AIGC图片"、"图片检测" | [image_detect.md](skills/image_detect.md) |
| 判断视频是否 AI 生成的 | "AI视频"、"AIGC视频"、"视频检测" | [video_detect.md](skills/video_detect.md) |
| 评测文本内容是否安全/有害 | "内容安全"、"有害内容"、"安全评测" | [corpus_safety_eval.md](skills/corpus_safety_eval.md) |
| 评测被测模型是否会生成不安全内容 | "模型安全"、"攻击测试"、"safety eval" | [safety_eval.md](skills/safety_eval.md) |
| 评测被测模型的能力水平 | "模型能力"、"性能评测"、"general eval" | [general_eval.md](skills/general_eval.md) |
| 上传/下载/管理文件 | "上传"、"下载"、"文件管理" | [file_manage.md](skills/file_manage.md) |

---

## 二、Skill 详细说明

### 1. text_detect（文本 AIGC 检测）

**核心问题**：这段文字是 AI 写的还是人类写的？

**典型用户问法**：
- "帮我检测这段文字是否 AI 生成的"
- "这篇文章是不是 AI 写的"
- "检测一下这段文本的 AIGC 概率"

**检测对象**：纯文本内容
**是否需要 API Key**：否（内部检测模型）
**工具**：detect_text、batch_detect_texts、get_text_task、get_text_task_artifacts

---

### 2. image_detect（图片 AIGC 检测）

**核心问题**：这张图片是 AI 生成的还是真人拍摄/经过篡改的？

**典型用户问法**：
- "帮我检测这张图片是否 AI 生成的"
- "这张照片有没有被修过"
- "检测图片是否 DeepFake"

**检测对象**：图片文件
**是否需要 API Key**：否（内部检测模型）
**工具**：detect_image、batch_detect_images、get_image_task、get_image_task_artifacts

**三种检测方法**：
- `deepfake_defenders`：快速判断人像是否 AI 生成
- `mapnet`：需要查看 AI 生成关注区域（注意力图）
- `tamper_yolo`：检测图片是否经过篡改（修图、合成）

---

### 3. video_detect（视频 AIGC 检测）

**核心问题**：这个视频是 AI 生成的还是真人拍摄/经过 DeepFake 篡改的？

**典型用户问法**：
- "帮我检测这个视频是否 AI 生成的"
- "这个视频有没有被换脸"
- "检测视频是否 DeepFake"

**检测对象**：视频文件
**是否需要 API Key**：否（内部检测模型）
**工具**：detect_video、batch_detect_videos、get_video_task、get_video_task_artifacts

**注意**：视频检测耗时较长（通常几分钟），请提醒用户耐心等待。

---

### 4. corpus_safety_eval（语料安全评测）

**核心问题**：这段文本内容本身是否安全/有害？（不涉及模型调用）

**典型用户问法**：
- "帮我评测这段文本是否安全"
- "检测一下这段内容有没有有害信息"
- "语料安全评测"

**检测对象**：文本语料（不做模型调用，只评句子本身）
**是否需要 API Key**：可能需要（如果使用第三方 judge）
**是否调用被测模型**：否
**汇总方式**：worst-case（任一 chunk 不安全则原始语料不安全）

---

### 5. safety_eval（LLM 安全评测）

**核心问题**：被测模型在各种攻击场景下是否会生成不安全内容？

**典型用户问法**：
- "帮我测试这个模型的安全性"
- "评测模型会不会生成有害内容"
- "对模型进行红队测试"
- "模型攻击测试"

**检测对象**：被测模型的回复
**是否需要 API Key**：需要（被测模型和 judge 模型）
**是否调用被测模型**：是
**特殊功能**：支持攻击变体测试（roleplay、ignore_previous、translation）

---

### 6. general_eval（通用 LLM 能力评测）

**核心问题**：被测模型的各项能力水平如何？

**典型用户问法**：
- "帮我评测这个模型的能力"
- "测试模型的数学、推理能力"
- "模型性能评估"
- "general eval"

**检测对象**：被测模型的回复
**是否需要 API Key**：需要（被测模型和 judge 模型）
**是否调用被测模型**：是
**汇总方式**：按 field 汇总平均分

---

### 7. file_manage（文件管理）

**核心问题**：如何上传本地文件或下载服务器上的文件？

**典型用户问法**：
- "帮我上传这个文件"
- "下载检测报告"
- "查看我上传的文件列表"

**功能**：文件上传、下载、列表查询
**工具**：upload_file、download_file、list_files、list_task_files

---

## 三、关键区分

### AIGC 检测 vs 安全评测

| 维度 | AIGC 检测（text/image/video_detect） | 安全评测（corpus_safety_eval / safety_eval） |
|------|-------------------------------------|---------------------------------------------|
| **核心问题** | 是否 AI 生成的？ | 内容/模型是否安全？ |
| **评测对象** | 内容的生成来源 | 内容是否有害、模型是否会生成有害内容 |
| **是否调用被测模型** | 否 | safety_eval 是，corpus_safety_eval 否 |
| **典型场景** | 判断文章/图片/视频是否 AI 合成 | 检测有害内容、测试模型安全边界 |

### corpus_safety_eval vs safety_eval

| 维度 | corpus_safety_eval | safety_eval |
|------|-------------------|-------------|
| **评测对象** | 文本语料本身 | 被测模型的回复 |
| **是否调用被测模型** | 否 | 是 |
| **典型场景** | 批量检测已有文本是否安全 | 测试模型在攻击下是否产生有害输出 |
| **输入方式** | texts 或 dataset | prompts 或 dataset |

### image_detect vs video_detect

| 维度 | image_detect | video_detect |
|------|-------------|--------------|
| **检测对象** | 图片 | 视频 |
| **检测方法** | deepfake_defenders / mapnet / tamper_yolo | video_multihead_attention |
| **超时时间** | 120s | 300s（更久） |

---

## 四、决策流程

```
用户请求
    │
    ▼
用户想做什么？
    │
    ├─ "上传/下载文件" ──────────────────→ file_manage.md
    │
    ├─ "判断内容是否 AI 生成的" ──────────→ 内容类型？
    │                                           │
    │                                           ├─ "文本" → text_detect.md
    │                                           ├─ "图片" → image_detect.md
    │                                           └─ "视频" → video_detect.md
    │
    └─ "评测安全性/能力" ──────────────────→ 评测什么？
                                                    │
                                                    ├─ "文本语料本身是否安全" → corpus_safety_eval.md
                                                    ├─ "模型是否生成有害内容" → safety_eval.md
                                                    └─ "模型能力水平如何" → general_eval.md
```

---

## 五、通用注意事项

1. **工具调用方式**：必须使用原生工具（tool_use），禁止在回复中书写伪代码

2. **file_path 必须确认**：Windows 路径（如 `C:\Users\...`）和 Linux 路径（如 `/home/...`）不同，必须与用户确认

3. **API Key 配置**：涉及第三方模型调用时（safety_eval、general_eval），需要先调用 `inject_xxx_credentials` 设置 API Key

4. **任务状态查询**：检测/评测任务通常异步执行，需要轮询 `get_xxx_task` 查询状态

5. **产物下载**：任务完成后，使用 `get_xxx_task_artifacts` 获取产物列表，再使用 `download_file` 下载

---

## 六、文件路径

所有 Skill 文档位于 `/home/lcd/SAITEC-Skills/skills/` 目录下：

| Skill | 文件路径 |
|-------|----------|
| text_detect | `skills/text_detect.md` |
| image_detect | `skills/image_detect.md` |
| video_detect | `skills/video_detect.md` |
| corpus_safety_eval | `skills/corpus_safety_eval.md` |
| safety_eval | `skills/safety_eval.md` |
| general_eval | `skills/general_eval.md` |
| file_manage | `skills/file_manage.md` |

---

## 七、快速参考

当你需要查阅某个 Skill 的详细文档时，直接读取对应路径的 .md 文件即可。每个 Skill 文档包含：
- 工具列表和参数说明
- 典型对话示例
- 工作流程
- 注意事项和检查清单
