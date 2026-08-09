# Editor definition navigation

## Goal

Make definition navigation feel familiar to users of VS Code and other IDEs: holding the platform modifier reveals a navigable symbol, its language information appears near the symbol, and activating it makes the destination immediately recognizable.

## Interaction

- On macOS, holding Command while hovering a Rust symbol requests language information for that position.
- On Windows and Linux, Control provides the same behavior.
- A resolvable symbol is underlined and uses the pointer cursor while the modifier remains held.
- A compact hover card is anchored near the symbol and displays the signature, type, alias, and documentation returned by rust-analyzer.
- Command/Control-click opens the definition. Existing F12 navigation remains available.
- A normal hover or click does not turn source text into a link and does not interfere with selection, dragging, or editing.
- Stale asynchronous responses cannot decorate a different symbol after the pointer moves or the modifier is released.
- If the symbol has no definition, it is not presented as a link. Available hover information may still be shown without claiming that navigation is possible.
- Multiple definitions continue to use the existing definition-results panel.

## Destination reveal

- A single definition navigation carries the complete language-server target range: start line and column plus end line and column.
- After the destination document opens, the exact target range receives a clear accent highlight and its line receives a subtle background wash.
- The editor scrolls the range into the center of the viewport, puts the caret at the target start, and receives focus.
- The reveal is transient: it fades after approximately 1.5 seconds and clears immediately when the user edits, changes the selection, or starts another navigation.
- The reveal decoration is not a text selection, so it does not overwrite selection state or change copy behavior.
- With reduced motion enabled, the highlight remains static for the same short interval and disappears without animation.

## Visual design

- The source link uses the existing accent token for a one-pixel underline and does not alter syntax color.
- The hover card reuses RepoDesk surface, border, text, muted-text, monospace, and shadow tokens; no VS Code-specific colors are copied.
- The card stays inside the editor viewport and never covers the pointer target when an alternate placement is available.
- The destination range uses an accent-tinted fill and inset outline. The line wash is intentionally weaker than the active-line treatment.
- Decorations stay inside CodeMirror content and cannot overlap the gutter or the right-side scrollbar.

## Architecture

- Keep language requests in `useLiveRustLanguage`; add a position-based preview action that resolves hover and definition information together and supports cancellation by request identity.
- Keep pointer/modifier interpretation and CodeMirror decorations in the editor layer.
- Represent the source link and destination reveal as CodeMirror state effects and decorations instead of React overlays or native text selection.
- Extend the existing one-shot workspace location hand-off with optional end coordinates while remaining compatible with older start-only requests.
- Reuse the existing language-location fields returned by rust-analyzer; no backend protocol change is required.

## Accessibility and failure behavior

- F12 remains the keyboard-accessible definition action.
- The hover card is supplementary and does not take focus from the editor.
- Escape dismisses the preview and existing language panels.
- A missing, starting, or failed language server leaves ordinary editor behavior intact and shows no false link affordance.
- Request failures use the existing language error surface and never leave a stuck underline or busy state.

## Verification

- Unit-level frontend behavior confirms the extended workspace location is normalized, consumed once, and backward compatible.
- Editor UI coverage confirms modifier-hover produces a link affordance and preview without moving the caret.
- Editor UI coverage confirms releasing the modifier or moving away clears the affordance and stale responses do not restore it.
- Editor UI coverage confirms Command/Control-click forwards the exact clicked position and does not activate on a normal click.
- Editor UI coverage confirms a navigated range is centered, highlighted without becoming a selection, and clears after interaction or timeout.
- The desktop TypeScript/Vite build and focused Playwright editor suite remain green.
