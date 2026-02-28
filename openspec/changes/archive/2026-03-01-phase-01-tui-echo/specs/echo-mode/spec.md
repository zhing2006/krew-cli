## ADDED Requirements

### Requirement: Echo 回显
在 Echo 模式下，用户通过输入框发送的文本 SHALL 原样回显到输出区域。用户输入 SHALL 以 `you> ` 前缀显示，echo 回复 SHALL 以 `echo> ` 前缀显示。

#### Scenario: 输入文本回显
- **WHEN** 用户输入 "hello world" 并按 Enter
- **THEN** viewport 上方 SHALL 依次插入 `you> hello world` 和 `echo> hello world`

#### Scenario: 多行文本回显
- **WHEN** 用户输入多行文本（通过 Shift+Enter 或 Ctrl+J 换行）并按 Enter
- **THEN** viewport 上方 SHALL 显示用户的多行输入和对应的多行 echo 回复，后续行 SHALL 与第一行内容对齐缩进

#### Scenario: 空输入不回显
- **WHEN** 用户在输入框为空或仅含空白字符时按 Enter
- **THEN** 系统 SHALL NOT 产生任何输出，输入框保持空白状态
