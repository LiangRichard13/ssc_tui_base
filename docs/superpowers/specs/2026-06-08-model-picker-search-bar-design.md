# Model Picker Search Bar Design

## Goal

Make `/model` keyboard filtering visible and self-explanatory. When the model picker is open, users should see what search text they have typed and how to return or close the picker with `Esc`.

## Current Context

The model picker is rendered by `src/tui/ui_inline_interactive.rs` from `InlineInteractiveState`. Filtering already works through `picker.filter` and `picker.filtered`, and typed characters are handled by `App::handle_inline_interactive_key`.

The current UI appends the filter text to the table header, which is easy to miss and does not look like an input field. The active key hints also do not make `Esc` discoverable enough for users who typed into the picker.

## Design

Only `PickerKind::Model` should get the new UI. Account, login, usage, and agent target pickers must keep their current layout and behavior.

For model pickers, render a dedicated search/help row above the existing column header:

- Empty filter: `Search: type to filter models`
- Non-empty filter: `Search: <typed filter>`
- Help text: `Esc: back/close  Enter: select  Up/Down: move`

The existing column header should remain below this row, and the model list should start below the header. Filtering behavior, selection behavior, route display, and model switching behavior should not change.

If the picker is narrow, the row should truncate gracefully rather than wrapping or shifting the table. The search row must not be rendered for non-model pickers.

## Tests

Add focused renderer tests around `draw_inline_interactive`:

- model picker with a typed filter renders `Search:` and the filter text
- model picker renders an `Esc` close/back hint
- non-model picker does not render the model search row

Run targeted TUI tests first, then a full build before finishing.

## Non-Goals

- Do not change the fuzzy matching or filter input behavior.
- Do not redesign the model picker table columns.
- Do not alter account picker, login picker, usage picker, or agent target picker UI.
