## ADDED Requirements

### Requirement: Denied 决策显示
当 agent loop 返回 `Denied` 决策时，TUI SHALL 在输出中插入一行拒绝反馈，包含拒绝原因。该反馈不通过审批 overlay 显示，而是直接作为工具输出的一部分呈现。

#### Scenario: Denied 反馈行
- **WHEN** agent loop 因 deny 规则拒绝了 `shell("rm -rf /tmp")` 且 reason 为 "禁止递归强制删除"
- **THEN** TUI SHALL 在工具输出区域显示红色标记的 `✗ Denied: 禁止递归强制删除`

#### Scenario: Denied 无 reason
- **WHEN** agent loop 因 deny 规则拒绝了工具调用且 reason 为空
- **THEN** TUI SHALL 显示 `✗ Denied by rule`

### Requirement: Ask 规则审批 overlay 显示 reason
当 ask 规则触发审批时，审批 overlay SHALL 在工具调用信息下方显示 ask 规则的 reason（如果有）。

#### Scenario: Ask overlay 显示原因
- **WHEN** ask 规则触发审批且 reason 为 "发布操作需要确认"
- **THEN** overlay SHALL 在 `⚡ shell("npm publish") — approve?` 下方显示 reason 文本
