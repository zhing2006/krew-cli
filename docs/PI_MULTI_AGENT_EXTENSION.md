# Multi-Agent Extension — Design

A design proposal for a `pi-multi-agent` extension that brings the same `@name` / `#name` multi-agent conversation model used by [`krew-cli`](https://github.com/zhing2006/krew-cli) into the pi coding agent, implemented entirely on top of the existing extension API.

---

## 1. Goals

- Multiple LLM agents (e.g. `gpt`, `opus`, `gemini`, `deepseek`) participate in a single pi session.
- Users address agents with `@name` (public) or `#name` (whisper, private to the named group).
- Agents can address each other with `@name` in their replies, triggering AI-to-AI dispatch with a configurable round limit.
- Each agent inherits pi's full coding tool stack (bash, edit, grep, skills, tools registered by other extensions), with optional per-agent tool whitelists.
- No fork of pi-mono. Everything below is implementable as a single extension that loads via `pi -e ./pi-multi-agent.ts` or auto-discovery in `~/.pi/agent/extensions/`.

The architectural reference for behavior is `krew-cli`'s `multi-agent-dispatch`, `input-routing`, and `agent-to-agent-routing` specs. This document maps each requirement onto pi-mono's extension surface.

---

## 2. Architectural Mismatch with krew-cli

`krew-cli`'s `ChatMessage` carries two extra fields that the OpenAI/Anthropic message protocols do not:

```rust
struct ChatMessage {
  role: User | Assistant | Tool | System,
  content: String,
  name: Option<String>,            // which agent said this
  whisper_targets: Option<Vec<String>>, // who is allowed to see this
  // ...
}
```

pi-mono uses pi-ai's standard `AgentMessage`, which has no `name` or `whisper_targets`. Provider-level convergence is impossible without forking pi-ai.

The trick `krew-cli` uses at the provider boundary is universal across providers: it does **not** rely on protocol-level `name`. Instead, in `crates/krew-llm/src/{anthropic,openai_chat,google}.rs`, when a message belongs to "another agent" relative to the requesting one, it is rewritten as plain text with an `[agent_name]` content prefix and posted under the appropriate role per `other_agent_role`. The `name` field is purely an internal bookkeeping concern, never sent over the wire.

This means a pi-mono extension can do the same thing, in TypeScript, by intercepting the `context` event and rewriting the `messages[]` payload before each provider request.

---

## 3. Implementation Path: Custom Entry Bypass

Three implementation paths were considered:

| Path | Sketch | Verdict |
|---|---|---|
| A. `context` rewrite only | Extension owns shadow history. Per-call `context` event projects the shadow into pi's `AgentMessage[]`. pi's session file ends up storing the projection. | Session replay shows whichever projection was last written — confusing on review. |
| B. **Custom entry bypass** | Extension stores raw `ChatMessage`-equivalents via `pi.appendEntry("multi-agent.message", msg)`. pi's native message history holds only decorative custom messages. `context` event materializes the projection per call. Renderer reconstructs the multi-agent transcript on session reload. | **Chosen.** Clean separation; session file faithfully represents what each agent actually said. |
| C. Virtual provider | Register a synthetic `multi-agent` provider via `pi.registerProvider({ streamSimple })` that internally dispatches to real providers. | Loses pi's `setModel` UI, requires re-implementing ESC and pending queues inside `streamSimple`. |

The rest of this document describes path B.

**The load-bearing primitive that makes path B viable** is pi's `transformContext` callback (wired in `core/sdk.ts:351`). Before each provider request, pi calls `runner.emitContext(messages)`, which lets every extension replace the `messages[]` array. The replacement is what pi-ai sends to the provider — pi's own internal `state.messages` is never read directly by the provider call. This means a multi-agent extension can let pi's internal array contain whatever pi naturally accumulates (decorative custom messages, turn-trigger markers, even mismatched user/assistant entries) without affecting LLM input. The shadow history projection is the single source of truth for what each agent sees.

---

## 4. Shadow History

The extension owns a `ShadowHistory` keyed off the current pi session id. It is the source of truth for multi-agent semantics.

```typescript
type ShadowRole = "system" | "user" | "assistant" | "tool";

interface ShadowMessage {
  id: string;                          // ULID, also used as pi entryId reference
  sessionId: string;                   // pi session id
  role: ShadowRole;
  content: string;
  name?: string;                       // agent name for assistant; tool name for tool
  toolCalls?: ToolCallInfo[];          // assistant-with-tool-calls
  toolCallId?: string;                 // tool result -> originating call id
  addressee?: Addressee;               // user message routing target
  whisperTargets?: string[];           // None = public; Some = restricted to named agents
  createdAt: string;                   // ISO-8601
  usage?: TokenUsage;
}

type Addressee =
  | { kind: "all" }
  | { kind: "single"; name: string }
  | { kind: "multiple"; names: string[] }
  | { kind: "lastRespondent" };

interface ShadowHistory {
  messages: ShadowMessage[];
  lastRespondent?: string;
  pendingAgents: string[];             // VecDeque equivalent; agents queued to speak
  currentWhisperTargets?: string[];    // active whisper scope during dispatch
  a2aInsertCursor: number;             // immediate-routing insertion point
  a2aRoundsThisTurn: number;           // resets to 0 on each user message
}
```

### Persistence

Each `ShadowMessage` is appended via `pi.appendEntry("multi-agent.message", msg)` immediately after it is added to the in-memory shadow history. On session reload, the extension reads back all `multi-agent.message` entries from the session and reconstructs `ShadowHistory` in memory; `lastRespondent` and `pendingAgents` are recomputed from the message stream.

pi's native `AgentMessage[]` history continues to hold:
- Decorative `multi-agent.user-routed` custom messages (one per user submission, carrying the colored routing dots)
- Decorative `multi-agent.agent-response` custom messages wrapping each agent's reply with its display name and color

The decorative messages exist only for human review of the session file — they are not what the LLM sees on the next turn. The `context` event provides the actual LLM-facing history (see Section 6).

---

## 5. Configuration

Per-extension config lives at `~/.pi/agent/multi-agent.json`:

```jsonc
{
  "agents": [
    {
      "name": "gpt",
      "displayName": "GPT-5",
      "color": "#10a37f",
      "model": "openai/gpt-5",
      "thinkingLevel": "medium",
      "systemPrompt": "Optional override appended to the identity prompt.",
      "tools": ["bash", "read", "edit", "grep", "find", "ls", "write"],
      "excludeTools": []
    },
    {
      "name": "opus",
      "displayName": "Claude Opus 4.7",
      "color": "#d97706",
      "model": "anthropic/claude-opus-4-7",
      "thinkingLevel": "high",
      "tools": ["bash", "read", "edit", "grep", "find", "ls", "write"]
    },
    {
      "name": "gemini",
      "displayName": "Gemini 3 Pro",
      "color": "#4285f4",
      "model": "google/gemini-3-pro",
      "tools": ["read", "grep", "find", "ls"]
    }
  ],
  "settings": {
    "replyOrder": ["gpt", "opus", "gemini"],
    "agentToAgentMaxRounds": 10,
    "agentToAgentRouting": "immediate",
    "otherAgentRole": "user",
    "maxPendingMessages": 8,
    "whisperEnabled": true
  }
}
```

The model strings (`provider/id`) are resolved through `pi.modelRegistry.find(provider, id)` during `session_start`.

---

## 6. Event Wiring

Every multi-agent behavior maps to one or more existing extension events. The full table:

| Behavior | pi event | Action |
|---|---|---|
| Parse `@name` / `#name` from user input | `on("input")` | Return `{ action: "handled" }` after own dispatch; or `{ action: "transform" }` if the addressee is `LastRespondent` and the active model is already correct |
| Switch which model speaks next | (inside input handler / agent_end) | `pi.setModel(modelForAgent(name))` then `pi.sendUserMessage(...)` |
| Provide LLM-visible message history | `on("context")` | Return `{ messages: project(shadowHistory, currentAgent) }` |
| Inject identity / peer / whisper system prompt | `on("before_agent_start")` | Return `{ systemPrompt: build(currentAgent, peers, whisperTargets) }` |
| Capture agent reply, scan for `@other` | `on("agent_end")` | Append assistant `ShadowMessage` with `name: currentAgent`; scan `final_text` via `parseAgentMentions`; queue per routing strategy |
| Per-agent tool whitelist | `on("tool_call")` | Look up current agent's whitelist; if violated, return `{ block: true, reason: "Agent <name> is not allowed to use <tool>" }` |
| ESC cancellation propagation | `on("agent_end")` (cancelled status) | Synthesize a `[Cancelled]` assistant ShadowMessage with `whisperTargets` inherited from current scope; clear pending queue and whisper scope; trigger `drainPendingMessage()` |
| Pending-message queue while agent is responding | `on("input")` while `!ctx.isIdle()` | Validate that input contains `@`/`#`; push into local `pendingMessages[]`; show widget; return `{ action: "handled" }` |
| Drain pending after dispatch finishes | `on("agent_end")` when shadow `pendingAgents` empty | Pop next `pendingMessage`, run `submitRawInput()` |
| Compaction (chosen B: extension-driven) | `on("session_before_compact")` | Return `{ cancel: true }` to suppress pi's compaction; run extension's own compactor on shadow history; emit a `multi-agent.compaction-summary` custom message; replace shadow history with the compacted version. All agents see the same merged history on the next turn |
| Tree navigation / fork | `on("session_before_tree")`, `on("session_before_fork")` | If dispatch is in progress (active stream or non-empty `pendingAgents`), return `{ cancel: true }` with a notification. Otherwise, allow pi to navigate, then in the after event truncate `shadowHistory.messages` to the matching ULID, recompute `lastRespondent`, clear whisper / pending / a2a state |
| Block `setModel` UI in multi-agent mode | (no direct hook — wrap setModel handler) | The extension does not currently expose a hook on `pi.setModel`. Instead, the extension exposes `/agents` and uses `pi.registerShortcut` to rebind `Ctrl+P` to a no-op + notification ("Use @name to switch agent") when multi-agent is active. See Conflict 3 in Section 12 for the full discussion |

### Projection algorithm (`context` handler)

The projection is the literal port of `krew-core/src/agent/prepare.rs::prepare_messages_for_agent`:

```typescript
function project(shadow: ShadowMessage[], selfName: string): AgentMessage[] {
  const filtered = applyWhisperFilter(shadow, selfName);
  const folded   = foldOtherAgentToolChains(filtered, selfName);
  return folded.map(toAgentMessage);
}
```

`applyWhisperFilter` replaces messages whose `whisperTargets` does not include `selfName` with placeholders (`[Whisper to gemini, opus]` / `[Whisper]`); tool chains belonging to a hidden whispered assistant message are collapsed into a single placeholder.

`foldOtherAgentToolChains` keeps the current agent's own tool calls in native format, and converts other agents' assistant-with-tool-calls + subsequent `Tool` messages into a single text assistant message of the form:

```
Let me check.
[Used tool: read("path"="src/main.rs")]
[Result from read: fn main() {}]
```

`toAgentMessage` is the only place where the `[agent_name]` content prefix is applied to non-self assistant messages, matching `krew-llm`'s `convert_messages` logic.

### Input parsing

```typescript
type ParseResult =
  | { ok: true; addressee: Addressee; isWhisper: boolean; body: string }
  | { ok: false; error: string };

function parseInput(input: string, knownAgents: string[]): ParseResult;
```

Rules (verbatim from `krew-core/src/router.rs::parse_input`):

- Tokens are whitespace-delimited; only `@name` / `#name` matching a known agent or `all` are recognized as routing.
- Unknown `@token` and bare `@` / `#` are left as plain text.
- `#all` is rejected.
- Mixing `@` and `#` in the same input is rejected.
- The body is the **full original text**, not stripped — so the LLM sees what the user typed.

### Agent-mention scanning (A2A)

```typescript
function parseAgentMentions(text: string, knownAgents: string[], selfName: string): string[];
```

Verbatim from `krew-core/src/router.rs::parse_agent_mentions`:

- Iterate every `@` position (not just whitespace-delimited tokens — handles CJK punctuation like `太好了！@gemini`).
- Longest-prefix match against known agents; the next character must be non-ASCII-alphanumeric or end-of-text.
- Skip `@self` and `@all`.
- Skip when the character before `@` is ASCII alphanumeric (avoids matching `user@opus.com`).

### Routing strategies

```typescript
function applyImmediateRoutingAt(
  pending: string[],
  target: string,
  cursor: { value: number },
): void;

function applyQueuedRouting(pending: string[], target: string): void;
```

Verbatim from `krew-core/src/router.rs`. The `cursor` variant is used by `immediate` mode to prevent later A2A targets from jumping ahead of earlier ones (krew-cli's "starvation prevention").

---

## 7. System Prompt Injection

`on("before_agent_start")` returns a `systemPrompt` built from three layers, mirroring `krew-core/src/agent/mod.rs::build_identity_prompt`:

**Layer 1 — Identity (always injected when multi-agent is active):**

```
You are <displayName>, powered by the <model> model.
Your agent name in this conversation is "<name>".
You are participating in a multi-agent conversation hosted by pi-multi-agent.
Other agents in this conversation are DIFFERENT AI models, not you.
Their messages are prefixed with [agent_name] in the content. User messages are prefixed with [user].
Respond as yourself — do not role-play or impersonate other agents.
Current date: <YYYY-MM-DD HH:00 (Weekday)>
```

The hour-aligned timestamp is intentional — it stays stable within an hour to remain compatible with provider prompt caches (Anthropic 5min default / 1h beta, OpenAI ~5-10min, Gemini implicit). Day-only precision was over-conservative; second precision would invalidate cache on every call.

**Layer 2 — Peer collaboration (only when `agentToAgentMaxRounds > 0` and at least one peer exists):**

```
You can ask another agent to respond by writing @name (with spaces before and after, e.g. " @opus ").
Only use @name when you need that agent to reply — do NOT use @ when merely mentioning an agent by name.
Other agents: [opus] Claude Opus 4.7, [gemini] Gemini 3 Pro.
```

**Layer 3 — Whisper context (only when `currentWhisperTargets` is set):**

Four sub-blocks, all from `build_identity_prompt`:

1. Privacy: "You are in a private whisper conversation with the user and @opus, @gemini. Agents outside this group cannot see the conversation content."
2. Scope: "Everything in this conversation round — your response, tool calls, and tool results — is part of this whisper and only visible to whisper group members."
3. Confidentiality: "IMPORTANT: In subsequent non-whisper (normal) messages, you must NEVER reveal, reference, quote, or summarize any content from whisper conversations..."
4. A2A scope override (only if A2A enabled and there is more than one whisper member): "In this whisper group, you may only @mention group members: @opus, @gemini. Mentions of agents outside the group will be ignored."

The constructed prompt is **prepended** to whatever pi would otherwise build — extensions are chained, so the agent's own per-agent `systemPrompt` from config is appended after these three layers.

---

## 8. Routing State Machine

Each user submission and each agent reply progresses through this state machine. All state lives in `ShadowHistory`.

### User submission

```
User input → parseInput
  → addressee = All / Single / Multiple / LastRespondent
  → isWhisper = bool
  → body (untouched)

Validate:
  - LastRespondent requires shadowHistory.lastRespondent — else error
  - Resolve targets via reply_order ∩ available agents

State updates:
  - a2aRoundsThisTurn = 0
  - a2aInsertCursor   = 0
  - currentWhisperTargets = isWhisper ? targets : undefined

Append to shadow:
  ShadowMessage {
    role: "user",
    content: body,
    addressee,
    whisperTargets: isWhisper ? targets : undefined,
  }

pendingAgents = resolveDispatchQueue(addressee, replyOrder, available, lastRespondent)
startNextAgent()
```

### `startNextAgent()`

```
while pendingAgents.length > 0:
  name = pendingAgents.shift()
  a2aInsertCursor = max(a2aInsertCursor - 1, 0)   // saturating_sub
  if name not in available: continue
  pi.setModel(agentConfig[name].model)
  pi.sendUserMessage(<empty user turn — see note>, { deliverAs: "followUp" })
  return true
return false
```

**Note on triggering a turn for the next agent.** pi expects a user message to drive a new turn. The extension uses `pi.sendMessage({ customType: "multi-agent.continuation", display: false, content: "" }, { triggerTurn: true })`. Two things happen:

1. `display: false` keeps the marker out of the TUI — the user never sees a fake user turn appear.
2. The marker is appended to pi's internal `messages[]` and would, by default, be converted to a `role: "user"` message by `core/messages.ts::convertToLlm` (`display: false` does not gate LLM visibility — only TUI rendering). However, the `context` event handler that runs immediately before the provider request **completely replaces** the messages array (see `core/sdk.ts:351-355` → `runner.emitContext(messages)`). The extension's projection is what the LLM actually sees; the continuation marker is simply omitted from the projection.

This is the design's load-bearing property: **pi's internal `messages[]` and the LLM-facing payload are decoupled by `transformContext`**. The internal array drives turn triggering and TUI rendering; the projection drives LLM input. Multi-agent semantics live entirely in the projection — pi's array can hold whatever pi wants to put there.

### Agent finishes (`agent_end`)

```
if event was cancelled (ESC):
  Append shadow ChatMessage {
    role: "assistant",
    name: currentAgent,
    content: accumulatedTextSoFar + "\n[Cancelled by user]",
    whisperTargets: currentWhisperTargets,
  }
  pendingAgents = []
  if currentWhisperTargets: currentWhisperTargets = undefined
  drainPendingMessage()
  return

if event was error:
  Append shadow ChatMessage {
    role: "assistant",
    name: currentAgent,
    content: partialText + "\n[Error: <message>]",
    whisperTargets: currentWhisperTargets,
  }
  if pendingAgents.length > 0:
    startNextAgent()
  else:
    if currentWhisperTargets: currentWhisperTargets = undefined
    drainPendingMessage()
  return

// Normal completion
Append shadow ChatMessage {
  role: "assistant",
  name: currentAgent,
  content: finalText,
  toolCalls: ...,
  whisperTargets: currentWhisperTargets,
  usage: event.usage,
}
lastRespondent = currentAgent

// A2A scan
mentions = parseAgentMentions(finalText, agentNames, currentAgent)
if currentWhisperTargets:
  mentions = mentions.filter(m => currentWhisperTargets.includes(m))

for target of mentions:
  if a2aRoundsThisTurn >= agentToAgentMaxRounds:
    notify "AI-to-AI rounds exhausted"
    break
  if routing == "immediate":
    applyImmediateRoutingAt(pendingAgents, target, a2aInsertCursor)
  else:
    applyQueuedRouting(pendingAgents, target)
  a2aRoundsThisTurn++

if pendingAgents.length > 0:
  startNextAgent()
else:
  if currentWhisperTargets: currentWhisperTargets = undefined
  drainPendingMessage()
```

### `drainPendingMessage()`

```
if extension's pendingMessages queue is empty: return
next = pendingMessages.shift()
submitRawInput(next.rawInput)   // re-enters the user submission flow
```

### Pending message intake (input while busy)

```
on("input", ...) when !ctx.isIdle():
  if pendingMessages.length >= maxPendingMessages: return { action: "continue" }   // let pi do its thing (newline)
  parsed = parseInput(text, agentNames)
  if parsed addressee == LastRespondent:
    notify "Pending message requires @name or #name"
    return { action: "continue" }   // keep textarea content
  pendingMessages.push({ rawInput: text })
  ctx.ui.setEditorText("")
  return { action: "handled" }
```

The widget set via `ctx.ui.setWidget("multi-agent.pending", lines)` shows the queued messages above the editor, color-tagged by target.

---

## 9. Custom Message Types

The extension registers three custom types via `pi.registerMessageRenderer`:

| customType | Purpose | Renderer |
|---|---|---|
| `multi-agent.user-routed` | Wraps each user submission with `> ●●● <body>` (colored dot per target, `🔒` prefix when whisper) | One-line composite text with `Span`s in target colors |
| `multi-agent.agent-response` | Wraps each agent reply with `<color>● <displayName></color>` header + the markdown body | Full markdown render through pi's existing `AssistantMessageComponent`, with a colored prefix line |
| `multi-agent.compaction-summary` | The compaction summary produced by extension-driven compaction (Section 12, Conflict 1) | Single bordered block with the summary text |

These exist for session-file fidelity. On session reload, walking the entries in order reproduces a faithful transcript.

The extension also stores the raw `ShadowMessage` payloads as `multi-agent.message` custom entries (with `display: false`) — these are not rendered, they are the source of truth for the next turn's `context` projection.

---

## 10. Slash Commands

| Command | Behavior |
|---|---|
| `/agents` | List configured agents with status (active / unavailable / current LastRespondent) |
| `/whisper @a @b ...` | Convenience for users who want to start a whisper but don't want to retype targets — opens an editor prefilled with `#a #b ` |
| `/respond <name>` | Manually set `lastRespondent` (rare; useful when reloading a session and the next message has no `@`) |
| `/clear-pending` | Drop all queued pending messages and reset whisper / a2a state |

Registered via `pi.registerCommand(name, { handler, description, getArgumentCompletions })`. The autocomplete provider is also extended (`ctx.ui.addAutocompleteProvider`) so `@`, `#`, and `/agents` complete from the agent registry.

---

## 11. UI Composition

- **User-message routing dots** — rendered by `multi-agent.user-routed` renderer.
- **Agent reply header** — rendered by `multi-agent.agent-response` renderer.
- **Pending message widget** — `ctx.ui.setWidget("multi-agent.pending", linesOrFactory, { placement: "aboveEditor" })`. Shows queued messages with colored dots; updated on push/pop.
- **Whisper indicator** — `ctx.ui.setStatus("multi-agent.whisper", "🔒 → @opus, @gemini")` while `currentWhisperTargets` is set.
- **Last respondent indicator** — `ctx.ui.setStatus("multi-agent.last", "→ opus")` between turns (visible in the footer).
- **Footer** — left as pi default; the two `setStatus` keys above appear in pi's `FooterDataProvider` automatically.

---

## 12. Conflict Resolutions with pi-mono Native Behavior

### Conflict 1 — Compaction. Resolution: B (extension-driven, unified).

When pi triggers compaction, `on("session_before_compact")` returns `{ cancel: true }` to suppress pi's own pass. The extension then:

1. Selects a cut point in `shadowHistory.messages` (default: same heuristic as pi — keep the last N tokens worth of recent turns intact, summarize everything before).
2. Spawns a one-off summary agent (whichever model the user designates as `settings.summarizer`, defaulting to the cheapest available) using a fresh `streamSimple` call against `pi.modelRegistry`.
3. Produces a single summary `ShadowMessage` of role `system` with no `name` and no `whisperTargets` — it is visible to all agents on the next turn.
4. Replaces the cut prefix with the summary; persists the new shadow head via `appendEntry`.
5. Emits a `multi-agent.compaction-summary` custom message for human-visible session display.

This guarantees that all agents see the same compacted history on the next turn — the user-visible promise from this design conversation: "after a round ends, all agents see the same thing on the next round."

Whisper content in the cut prefix is summarized **separately per whisper group** to preserve confidentiality. Concretely: messages in the cut window are partitioned by `whisperTargets` (with `undefined` being "public"); each partition is summarized independently; the public summary becomes a system message, and each per-group summary becomes a system message with that group's `whisperTargets` set.

### Conflict 2 — Tree navigation / fork. Resolution: support, with state rebuild.

`navigateTree` and `fork` are pi's equivalent of `krew-cli`'s `/rewind`, but more powerful (true tree vs. linear truncate-and-fork). They are **supported** in multi-agent mode.

`on("session_before_tree")` and `on("session_before_fork")`:
- If a stream is active or `pendingAgents` is non-empty, return `{ cancel: true }` with a notification: "Cancel current dispatch (ESC) before navigating."
- Otherwise allow.

`on("session_tree")` (after navigation) and equivalent post-fork hook:
- Truncate `shadowHistory.messages` to the position whose `id` matches the new leaf's underlying entryId reference.
- Recompute `lastRespondent` from the truncated tail (most recent assistant with `name`).
- Clear `pendingAgents`, `currentWhisperTargets`, `a2aInsertCursor`, `a2aRoundsThisTurn`, and `pendingMessages`.
- Refresh widgets and status indicators.

This is the literal port of `krew-cli`'s `apply_rewind` (`crates/krew-cli/src/app/commands.rs:616`).

### Conflict 3 — `setModel` UI (Ctrl+P). Resolution: A (suppress in multi-agent mode).

Multi-agent semantics make `setModel` a foot-gun: the user might press Ctrl+P expecting "switch which AI I'm talking to," but the actual semantics are "change which agent is the LastRespondent target" — and even that doesn't survive the next `@name` user message.

The extension does the minimum: register a Ctrl+P shortcut that overrides pi's default with a no-op + notification:

```typescript
pi.registerShortcut("ctrl+p", {
  description: "Disabled in multi-agent mode — use @name to address an agent",
  handler: (ctx) => ctx.ui.notify("Use @name to switch agent (e.g. @opus)", "info"),
});
```

Internally, the extension still calls `pi.setModel` itself during `startNextAgent()`. The shortcut suppression only prevents the user from doing so directly.

### Conflict 4 — Per-agent tool whitelists. Resolution: B (`tool_call` interception).

Each agent's config has an optional `tools: string[]` whitelist (empty / missing means "all available"). On `on("tool_call")`:

```typescript
const agent = currentAgent();
if (agent.tools && !agent.tools.includes(event.toolName)) {
  return {
    block: true,
    reason: `Agent "${agent.name}" is not allowed to use the "${event.toolName}" tool. Available tools: ${agent.tools.join(", ")}.`,
  };
}
```

The reason string is fed back to the LLM as the tool result, so the agent can adapt (e.g. ask another agent with `@`).

This avoids the alternative of `setActiveTools()` per-turn switching, which would mutate pi's session-wide tool state and corrupt the system prompt's "Available tools" section.

---

## 13. Session File Layout

A multi-agent session file (a pi `.jsonl` session) interleaves three classes of entries:

```
SessionInfoEntry
SessionMessageEntry (system prompt — pi default)
CustomMessageEntry { customType: "multi-agent.message", display: false, ...ShadowMessage }   // user
CustomMessageEntry { customType: "multi-agent.user-routed", display: true, ... }
CustomMessageEntry { customType: "multi-agent.message", display: false, ...ShadowMessage }   // assistant: gpt
CustomMessageEntry { customType: "multi-agent.agent-response", display: true, ... }
CustomMessageEntry { customType: "multi-agent.message", display: false, ...ShadowMessage }   // assistant: opus
CustomMessageEntry { customType: "multi-agent.agent-response", display: true, ... }
...
```

The `display: true` ones drive the TUI replay; the `display: false` ones drive the next turn's `context` projection. pi's own `SessionMessageEntry` (built-in user/assistant) is **not used** in multi-agent mode — the extension intercepts `on("input")` before pi appends a built-in user message.

A side benefit: a session file authored by multi-agent is fully readable by an unmodified pi (you'll see the decorative messages but lose the dispatch behavior). Sessions are forward-compatible.

---

## 14. Implementation Roadmap

### MVP (stage 1) — `@` only, no whisper, no compaction handling

- [ ] Config loader (`~/.pi/agent/multi-agent.json`)
- [ ] `parseInput` (without `#` support — simpler)
- [ ] `parseAgentMentions`
- [ ] Shadow history (in-memory only, no persistence)
- [ ] `on("input")` handler — `@name`, `@all`, `@a @b`, LastRespondent
- [ ] `on("context")` handler — `applyWhisperFilter` + `foldOtherAgentToolChains` + `[name]` prefix
- [ ] `on("before_agent_start")` — Layers 1 + 2 of identity prompt
- [ ] `on("agent_end")` — append to shadow, A2A scan, dispatch next
- [ ] `on("tool_call")` — per-agent whitelist enforcement
- [ ] `pi.registerCommand("/agents", ...)`
- [ ] User-routed message renderer (colored dots, no lock)
- [ ] Agent-response renderer
- [ ] Ctrl+P override notification
- [ ] Cancel `before_compact`, `before_tree`, `before_fork` unconditionally with a notification (defer real handling)

Deliverable: usable multi-agent within a single session. Session restart loses state.

### Stage 2 — persistence + whisper

- [ ] `pi.appendEntry("multi-agent.message", ...)` for every shadow message
- [ ] On `session_start`, replay all `multi-agent.message` entries to rebuild shadow
- [ ] Add `#` parsing to `parseInput`; reject `#all` and mixed `@`/`#`
- [ ] `currentWhisperTargets` propagation through dispatch / A2A / cancel / error
- [ ] Layers 3 + 4 of identity prompt (whisper privacy / scope / confidentiality / A2A override)
- [ ] `applyWhisperFilter` placeholders
- [ ] Whisper status indicator (`ctx.ui.setStatus`)
- [ ] User-routed renderer — add `🔒` prefix when whisper

### Stage 3 — pending queue + tree/fork

- [ ] `pendingMessages` queue (max 8 by default)
- [ ] Pending widget above editor
- [ ] Up-arrow undo of last pending (interaction with pi's input history requires `ctx.ui.setEditorComponent` — investigate scope)
- [ ] `before_tree` / `before_fork`: cancel only when dispatch active
- [ ] `session_tree` post-handler: truncate shadow + state rebuild
- [ ] `/clear-pending` command

### Stage 4 — compaction

- [ ] `before_compact` cancellation
- [ ] Extension-driven compaction (cut point, summarizer, per-whisper-group partition)
- [ ] `multi-agent.compaction-summary` renderer
- [ ] Token budget tracking per agent (`agent_token_usage` equivalent)

---

## 15. Known Limitations

- **No protocol-level `name`.** Other agents always appear with `[name] content` text prefixes. Some models (especially smaller ones) will occasionally misattribute or impersonate, even with the identity prompt. The fix is in the system prompt, not the data layer.
- **Token usage attribution is per-agent, but pi's footer shows session totals.** A custom footer (`ctx.ui.setFooter`) can render per-agent breakdown, deferred to Stage 4.
- **No streaming visibility into other agents' tool calls.** When agent A is running tools, agent B's view of A's work only materializes when A finishes — the `agent_end` handler is what writes to shadow. This matches `krew-cli`'s behavior.
- **Skill activation is global, not per-agent.** pi's skill system loads skills into the system prompt; the extension does not currently scope them per agent. Per-agent skills would require building each agent's system prompt manually (replacing pi's default), which is in scope for `before_agent_start` but adds complexity.
- **Sub-agents (pi's own delegation feature).** If an agent in the multi-agent set itself spawns a sub-agent, the sub-agent runs under pi's default semantics, not multi-agent. This is the intended boundary — sub-agent delegation is orthogonal to peer-to-peer dispatch.
- **Session file size.** Each shadow message is stored twice (raw + decorative), roughly doubling session-file size. Acceptable for typical session lengths.

---

## 16. References

- `krew-cli` specs (architectural contract this design ports):
  - `openspec/specs/input-routing/spec.md`
  - `openspec/specs/multi-agent-dispatch/spec.md`
  - `openspec/specs/agent-to-agent-routing/spec.md`
- `krew-cli` core implementations (algorithms ported verbatim):
  - `crates/krew-core/src/router.rs` — `parseInput`, `parseAgentMentions`, routing strategies
  - `crates/krew-core/src/agent/prepare.rs` — `applyWhisperFilter`, tool-chain folding
  - `crates/krew-core/src/agent/mod.rs::build_identity_prompt` — three-layer system prompt
  - `crates/krew-cli/src/app/message.rs` — user submission flow, `startNextAgent`
  - `crates/krew-cli/src/app/commands.rs::apply_rewind` — state rebuild on tree navigation
- pi-mono extension surface:
  - `packages/coding-agent/src/core/extensions/types.ts` — events, ExtensionAPI, ExtensionContext
  - `packages/coding-agent/docs/extensions.md` — extension authoring guide
  - `packages/coding-agent/examples/extensions/` — reference extensions (`custom-compaction.ts`, `custom-provider-anthropic/`, `commands.ts` are most relevant priors)
