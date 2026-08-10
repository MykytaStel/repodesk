import { EditorState, StateEffect, StateField } from "@codemirror/state";
import { Decoration, EditorView } from "@codemirror/view";

export function offsetForLspPosition(
  state: EditorState,
  line: number,
  character: number,
): number {
  const safeLine = Math.max(1, Math.min(line + 1, state.doc.lines));
  const target = state.doc.line(safeLine);
  return Math.min(target.to, target.from + Math.max(0, character));
}

export function wordRangeAt(state: EditorState, offset: number): { from: number; to: number } {
  const line = state.doc.lineAt(offset);
  const text = line.text;
  let from = Math.max(0, offset - line.from);
  let to = from;
  while (from > 0 && /[\w$]/.test(text[from - 1])) from -= 1;
  while (to < text.length && /[\w$]/.test(text[to])) to += 1;
  return { from: line.from + from, to: line.from + to };
}

export const showNavigationTarget = StateEffect.define<{ from: number; to: number }>();
export const clearNavigationTarget = StateEffect.define<void>();

export const navigationTargetField = StateField.define({
  create: () => Decoration.none,
  update(decorations, transaction) {
    let next = decorations.map(transaction.changes);
    if (transaction.docChanged || transaction.selection) next = Decoration.none;
    for (const effect of transaction.effects) {
      if (effect.is(clearNavigationTarget)) next = Decoration.none;
      if (effect.is(showNavigationTarget)) {
        const { from, to } = effect.value;
        const line = transaction.state.doc.lineAt(from);
        const ranges = [Decoration.line({ class: "cm-navigation-target-line" }).range(line.from)];
        if (to > from) ranges.push(Decoration.mark({ class: "cm-navigation-target" }).range(from, to));
        next = Decoration.set(ranges, true);
      }
    }
    return next;
  },
  provide: (field) => EditorView.decorations.from(field),
});

export const showDefinitionLink = StateEffect.define<{ from: number; to: number }>();
export const clearDefinitionLink = StateEffect.define<void>();

export const definitionLinkField = StateField.define({
  create: () => Decoration.none,
  update(decorations, transaction) {
    let next = transaction.docChanged ? Decoration.none : decorations.map(transaction.changes);
    for (const effect of transaction.effects) {
      if (effect.is(clearDefinitionLink)) next = Decoration.none;
      if (effect.is(showDefinitionLink)) {
        const { from, to } = effect.value;
        next = to > from
          ? Decoration.set([Decoration.mark({ class: "cm-definition-link" }).range(from, to)])
          : Decoration.none;
      }
    }
    return next;
  },
  provide: (field) => EditorView.decorations.from(field),
});
