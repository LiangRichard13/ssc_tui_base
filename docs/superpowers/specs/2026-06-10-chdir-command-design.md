# Chdir Command Design

## Goal

Add `/chdir` so users can change the current session working directory from inside the TUI. The change should update the bottom-left directory display and the directory used by TUI-controlled operations such as tools, input-line shell commands, git status, and spawned child sessions.

## Current Context

The TUI already carries the active working directory on `session.working_dir`. The footer reads this value through `TuiState::working_dir()`, and local command paths such as input-line `!` shell execution, `/git`, skill loading, and several child-session launch flows use the same session field.

Startup can set the process directory through CLI arguments, but that is not enough for an in-session directory switch. Changing `session.working_dir` is the narrowest state update that matches the existing architecture.

## Design

Add a public slash command:

- `/chdir <path>` changes the current session working directory.
- `/cd <path>` is a short alias for the same behavior.

The command resolves paths this way:

- Absolute paths are used as provided.
- Relative paths are resolved against the current `session.working_dir`.
- If the session has no working directory, relative paths are resolved against `std::env::current_dir()`.
- `~` is not expanded in this first pass.

On success, the command canonicalizes the directory to an absolute path, writes it to `self.session.working_dir`, saves the current session record, and shows a concise system message plus status notice. The existing footer should refresh naturally because it reads the session working directory.

On failure, the command must not partially update state:

- Missing path: show `Usage: /chdir <path>`.
- Nonexistent path: show a clear error.
- Existing non-directory path: show a clear error.
- Session save failure: keep the previous `working_dir` and report the save error.

## Tests

Add focused slash-command tests for:

- `/chdir <absolute-dir>` updates `session.working_dir` and saves the session.
- `/chdir <relative-dir>` resolves relative to the previous session working directory.
- Invalid paths leave `session.working_dir` unchanged.
- `/cd <path>` behaves as an alias.

Run targeted command tests while iterating, then `cargo check`, then the repo dev launch script before finishing.

## Non-Goals

- Do not change global process cwd as the primary mechanism.
- Do not add directory browsing UI.
- Do not add shell-style `~` or environment-variable expansion.
- Do not rewrite existing working-directory consumers.
- Do not change release versioning; this is not a release task.
