# Editor gutter interaction

## Goal

Restore a full-width active-line highlight in the code editor gutter and let a user navigate by clicking a line number.

## Visual behavior

- The active-line highlight spans the complete gutter width, from its left edge to the vertical divider.
- The highlight stays behind the number and remains clipped to the gutter.
- There is no glow or shadow outside the gutter and no overlap with the right-side editor scrollbar.
- Hovering a line-number row provides a subtle affordance without competing with the active state.

## Interaction

- Every rendered line-number row is clickable across the full gutter width.
- Clicking a row places the text caret at column 1 of that line.
- The textarea receives focus after the click.
- The existing editor scroll position is preserved for a visible clicked row.
- Gutter rows do not enter the normal Tab sequence; keyboard editing remains centered on the textarea.

## Implementation boundary

- Replace the gutter's single text node with one button per line while keeping the gutter as a non-scrollable visual layer translated by the textarea scroll position.
- Keep the current right-side custom scrollbar and single textarea scroll source unchanged.
- Use the existing line-offset and cursor-state helpers rather than introducing editor state elsewhere.

## Verification

- An editor UI test confirms the active highlight spans the gutter through the divider.
- An editor UI test clicks a line number and confirms the caret, reported line and focus.
- Existing tests continue to confirm that all line numbers render, the gutter has no scrollbar, and the only visible scrollbar is at the editor's right edge.
