# SAITEC Brand Animation Design

## Goal

Add a restrained SAITEC brand animation that makes the text logo feel alive in both the startup surface and the persistent header without reintroducing loud decorative effects.

## Direction

The animation should be low-frequency and layout-safe:

- startup text logo keeps the same geometry and cell occupancy
- per-block animation changes only foreground color and a small subset of block glyph density
- persistent header keeps the `🍇 SAITEC-TUI` wording and only applies a subtle color pulse
- all motion respects the existing decorative-animation policy, so disabled animations fall back to a static render

## Rendering Rules

### Startup text logo

- treat the SAITEC logo as a grid of visible block cells and spacer cells
- animate visible block cells using a slow phase offset derived from elapsed time and row/column position
- use a narrow glyph set such as `█`, `▓`, and `▒`
- keep the outer silhouette stable by preferring `█` on edge cells and limiting density changes to interior cells
- preserve row count, width, and alignment

### Persistent header brand line

- keep the existing text `🍇 SAITEC-TUI`
- apply a gentle pulse across the grape emoji and product name spans
- avoid character substitution in the persistent header

## Safety Constraints

- do not add a separate animation thread or timer
- reuse the existing redraw cadence and `animation_elapsed()`
- do not change footer layout or MCP/status rendering behavior
- do not change visible wording

## Verification

- add tests proving startup logo frames differ over time when animations are enabled
- add tests proving startup logo layout dimensions remain stable across animation frames
- add tests proving the persistent SAITEC brand line keeps the same text while the styling changes across frames
- add tests proving animation helpers return static output when decorative animations are disabled
