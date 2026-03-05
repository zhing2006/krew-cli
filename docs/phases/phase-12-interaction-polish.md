# Phase 12: 交互打磨

> 目标：提升 TUI 交互体验，补全所有产品细节。

## 实现内容

- **@ 补全**：输入 `@` 后弹出 Agent 名称列表 ✅
- **/ 补全**：输入 `/` 后弹出命令列表 ✅
- **思考过程显示**：`💭` 显示 ThinkingDelta 内容 ✅
- **ESC 中断**：中断当前 Agent 输出，保留已接收内容 ✅
- **流式中断提示**：Agent 回复不完整时提示用户 ✅
- **Web Search**：Provider 原生搜索注入（OpenAI Responses / Anthropic / Gemini） ✅
- **fetch_url 工具**：抓取网页内容转换为 Markdown，域名白名单审批 ✅

## 验收标准

- `@` 和 `/` 触发补全菜单
- ESC 可中断流式输出
- `enable_web_search = true` 时搜索工具注入到 API 请求
- `fetch_url` 工具可抓取网页并返回 Markdown

## 参考

| 文档 | 位置 | 内容 |
| ---- | ---- | ---- |
| PDD | L439-446 | §5.2 输入交互（补全、历史、中断） |
| PDD | L476-485 | §5.3 思考过程显示 |
| TDD | L375-386 | §3.3.5 Web Search 引用显示 |
| TDD | L359-373 | §3.3.4 流式中断处理 |
