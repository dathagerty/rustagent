# TUI Design Document

**Date:** 2026-01-24
**Status:** Draft

## Overview

Interactive terminal user interface for rustagent with three main views: Dashboard, Planning, and Execution. Built with Ratatui.

## Layout Structure

```
┌─────────────────────────────────────────────────────────┐
│  [1] Dashboard  │  [2] Planning  │  [3] Execution       │  ← Tab bar
├─────────────────────────────────────────────────────────┤
│                                                         │
│                    Active View                          │
│                                                         │
├─────────────────────────────────────────────────────────┤
│  Status bar: context • progress • shortcuts             │
└─────────────────────────────────────────────────────────┘
```

## Navigation

- `1`, `2`, `3` keys switch tabs directly
- `Tab` / `Shift+Tab` cycle through tabs
- `?` opens help overlay with all keybindings
- `[` / `]` toggles slide-in side panel (context-aware)
- `Esc` closes any open panel

## Side Panel System

Slide-in panel from right, persists until dismissed. Content is context-aware:

- **In Execution** → shows task list or spec details
- **In Planning** → shows generated spec preview
- **In Dashboard** → shows spec details for selected item

---

## Dashboard View

Toggle between Kanban (`K`) and Activity Feed (`A`) views.

```
┌─────────────────────────────────────────────────────────┐
│  View: [K]anban │ [A]ctivity          Filter: [a]ll ▼   │
├─────────────────────────────────────────────────────────┤
│                                                         │
│   (Kanban or Activity view content)                     │
│                                                         │
├─────────────────────────────────────────────────────────┤
│  ↑↓ navigate • Enter run • e edit • d delete • n new   │
└─────────────────────────────────────────────────────────┘
```

### Kanban View

```
│ Draft        │ Ready        │ Running      │ Completed   │
├──────────────┼──────────────┼──────────────┼─────────────┤
│ ┌──────────┐ │ ┌──────────┐ │ ┌──────────┐ │             │
│ │ auth-sys │ │ │ logging  │ │ │ api-refac│ │             │
│ │ 0/5 tasks│ │ │ 4 tasks  │ │ │ 2/6 ████░│ │             │
│ └──────────┘ │ └──────────┘ │ └──────────┘ │             │
```

- Arrow keys move between cards
- `h`/`l` or `←`/`→` move between columns
- `j`/`k` or `↑`/`↓` move within column

### Activity Feed View

```
│ Time     │ Event             │ Spec         │ Details       │
├──────────┼───────────────────┼──────────────┼───────────────┤
│ 2m ago   │ ✓ Task completed  │ api-refactor │ Add endpoints │
│ 5m ago   │ ▶ Run started     │ api-refactor │               │
│ 1h ago   │ ✗ Blocked         │ auth-system  │ Missing keys  │
│ 2h ago   │ ✓ Spec created    │ logging      │               │
```

- Column headers always visible (frozen at top)
- Columns sortable with `s` then select column
- `Enter` on an item jumps to that spec/task
- `f` opens filter menu (by status, date range, spec)

---

## Planning View

Chat-based interface for spec creation with vim-style input.

```
┌─────────────────────────────────────────────────────────┐
│ ┌─────────────────────────────────────────────────────┐ │
│ │ Assistant                                           │ │
│ │ What would you like to build?                       │ │
│ │                                                     │ │
│ │ You                                                 │ │
│ │ I need a user authentication system with JWT...    │ │
│ │                                                     │ │
│ │ Assistant                                     ◐     │ │
│ │ Breaking that down into tasks:                     │ │
│ │  ☐ 1. Create User model                            │ │
│ │  ☐ 2. Implement JWT generation                     │ │
│ │  ☐ 3. Add login/logout endpoints                   │ │
│ └─────────────────────────────────────────────────────┘ │
├─────────────────────────────────────────────────────────┤
│ >                                                       │
├─────────────────────────────────────────────────────────┤
│ i insert • ↑↓ scroll • Ctrl+S save • ] spec panel • ?  │
└─────────────────────────────────────────────────────────┘
```

### Features

- **Vim-style input**: `i` to enter insert mode, `Esc` to exit and scroll
- **Clear message attribution**: Labels ("Assistant", "You") above messages
- **Thinking indicator**: Spinner (◐) while waiting for LLM response
- **Inline task checkboxes**: Tasks rendered as actionable items in chat
- **Cancel generation**: `Ctrl+X` cancels in-progress generation

### Keybindings

| Key | Action |
|-----|--------|
| `i` | Enter insert mode |
| `Esc` | Exit insert mode, scroll messages |
| `↑`/`↓` | Scroll chat history |
| `Ctrl+S` | Save spec to disk |
| `Ctrl+P` | Open spec preview panel |
| `Ctrl+R` | Save and run (switch to Execution) |
| `Ctrl+X` | Cancel in-progress generation |
| `]` | Toggle spec JSON panel |

### Side Panel (Spec Preview)

```
│ Chat conversation    ││ spec.json              │
│                      ││ ────────────────────── │
│                      ││ {                      │
│                      ││   "name": "user-auth", │
│                      ││   "tasks": [...]       │
│                      ││ }                      │
```

- Live-updates as conversation progresses
- `e` in panel opens inline editor to tweak tasks manually

---

## Execution View

Split layout with task focus on left, streaming output on right.

```
┌─────────────────────────────────────────────────────────┐
├────────────────────────┬────────────────────────────────┤
│ Current Task       2/6 │ Output                         │
├────────────────────────┤────────────────────────────────┤
│ ▶ Add login endpoints  │ Assistant                      │
│                        │ I'll create the login route... │
│ Acceptance Criteria:   │                                │
│  ☐ POST /login exists  │ ┌─ run_command ──────────────┐ │
│  ☐ Returns JWT token   │ │ $ cargo check              │ │
│  ☐ Validates password  │ │ Compiling auth v0.1.0      │ │
│                        │ │ Finished dev [unopt]       │ │
├────────────────────────┤ └────────────────────────────┘ │
│ Tasks                  │                                │
│  ✓ Create User model   │ ┌─ write_file ───────────────┐ │
│  ✓ JWT generation      │ │ src/routes/login.rs        │ │
│  ▶ Login endpoints     │ │ +42 lines                  │ │
│  ○ Password reset      │ └────────────────────────────┘ │
│  ○ Logout endpoint     │                                │
│  ○ Tests               │ Assistant                  ◐   │
├────────────────────────┴────────────────────────────────┤
│ Running • 2/6 tasks • Ctrl+X stop • ] tasks • [ spec    │
└─────────────────────────────────────────────────────────┘
```

### Left Pane (Task Focus)

- Current task prominently displayed with acceptance criteria
- Task list with status indicators:
  - `✓` complete
  - `▶` in progress
  - `○` pending
  - `✗` blocked
- Progress fraction in header (2/6)

### Right Pane (Streaming Output)

- LLM messages stream in real-time
- Tool calls displayed in bordered boxes with tool name header
- Collapsible tool output (`Enter` on a tool box to expand/collapse)
- Spinner on active response

### Keybindings

| Key | Action |
|-----|--------|
| `Ctrl+X` | Stop/pause execution |
| `↑`/`↓` or `j`/`k` | Scroll output |
| `[` | Slide-in spec panel |
| `]` | Slide-in full task list |
| `r` | Resume if paused |
| `Enter` | Expand/collapse tool output |
| `?` | Help |

---

## Technical Architecture

### Module Structure

```
src/tui/
├── mod.rs              # App state, event loop, main TUI entry
├── tabs.rs             # Tab bar widget
├── panel.rs            # Slide-in panel system
├── status_bar.rs       # Status bar widget
├── dashboard/
│   ├── mod.rs          # Dashboard view container
│   ├── kanban.rs       # Kanban board widget
│   └── activity.rs     # Activity feed widget
├── planning/
│   ├── mod.rs          # Planning view container
│   ├── chat.rs         # Chat log widget
│   └── input.rs        # Vim-style input box
├── execution/
│   ├── mod.rs          # Execution view container
│   ├── task_pane.rs    # Left pane with task info
│   └── output_pane.rs  # Right pane with streaming output
└── widgets/
    ├── mod.rs          # Shared widget exports
    ├── spinner.rs      # Animated spinner
    └── tool_box.rs     # Tool call display box
```

### Component Hierarchy

```
App
├── TabBar                    # Navigation
├── StatusBar                 # Context hints, shortcuts
├── SidePanel                 # Slide-in overlay
└── Views
    ├── DashboardView
    │   ├── KanbanBoard       # Card grid by status
    │   └── ActivityFeed      # Table with headers
    ├── PlanningView
    │   ├── ChatLog           # Scrollable message list
    │   └── InputBox          # Vim-style text input
    └── ExecutionView
        ├── TaskPane          # Left split
        │   ├── CurrentTask
        │   └── TaskList
        └── OutputPane        # Right split
            ├── MessageStream
            └── ToolCallBox
```

### Dependencies

| Crate | Purpose |
|-------|---------|
| `ratatui` | TUI framework |
| `crossterm` | Terminal backend |
| `tokio` | Async runtime (already in use) |
| `tui-textarea` | Vim-style text input widget |

### Integration Points

- **PlanningAgent** streams messages to `PlanningView` via `tokio::sync::mpsc` channel
- **RalphLoop** streams messages/tool calls to `ExecutionView` via channel
- **Spec files** read/written through existing `spec.rs` module
- **Config** loaded through existing `config.rs`

### Event Flow

```
Terminal Events (crossterm)
        │
        ▼
    App::handle_event()
        │
        ├── Tab navigation → switch active view
        ├── Global keys → help, panels
        └── View-specific → delegate to active view

Async Streams (tokio channels)
        │
        ▼
    App::handle_message()
        │
        ├── LLM response → update chat/output
        ├── Tool call → add tool box
        └── Task update → refresh task list
```

---

## Out of Scope

- Session management (save/restore conversations)
- Multi-session switching

---

## Next Steps

1. Add `ratatui`, `crossterm`, `tui-textarea` to `Cargo.toml`
2. Create `src/tui/mod.rs` with basic app skeleton
3. Implement tab bar and view switching
4. Build out views incrementally: Dashboard → Planning → Execution
5. Add channel-based streaming from existing agents
