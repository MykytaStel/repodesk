import type { Page } from "@playwright/test";
import type { CommandFixtures } from "./fixtures";

// Installs a fake Tauri IPC layer before any app script runs.
//
// Tauri v2's `invoke(cmd, args)` (from @tauri-apps/api/core) dispatches through
// `window.__TAURI_INTERNALS__.invoke(cmd, args, options)`. By defining that object
// in an init script, we intercept every command the frontend issues — no app
// changes, no Rust backend. Commands present in `fixtures` resolve to their value;
// anything else resolves to `null` (which optionalCommand and the shell hooks tolerate).
//
// A fixture may opt into a delayed response with
// `{ __mock_delay_ms: number, __mock_value: unknown }`. This keeps normal array/object
// fixtures literal while letting UI tests observe in-flight states such as installation.
//
// A fixture may also return a deterministic response sequence with
// `{ __mock_sequence: unknown[] }`. Each invocation consumes the next item and
// subsequent calls keep returning the final item. This is useful for testing
// evidence refreshes such as plan-lock invalidation without teaching the mock
// transport any product-specific migration behavior.
//
// `__repodeskMockCalls` records the command sequence so specs can assert that the
// frontend actually drove the daily loop through the IPC seam.
export async function installMockIpc(page: Page, fixtures: CommandFixtures): Promise<void> {
  await page.addInitScript((data: CommandFixtures) => {
    const calls: string[] = [];
    const invocations: Array<{ cmd: string; args: Record<string, unknown> | undefined }> = [];
    const callbacks = new Map<number, (payload: unknown) => unknown>();
    const listeners = new Map<string, number[]>();
    const sequencePositions = new Map<string, number>();
    let callbackSequence = 0;
    (window as unknown as { __repodeskMockCalls: string[] }).__repodeskMockCalls = calls;
    (window as unknown as { __repodeskMockInvocations: typeof invocations }).__repodeskMockInvocations = invocations;

    const unregisterCallback = (identifier: number) => {
      callbacks.delete(identifier);
      for (const [event, identifiers] of listeners) {
        const next = identifiers.filter((candidate) => candidate !== identifier);
        if (next.length > 0) listeners.set(event, next);
        else listeners.delete(event);
      }
    };

    (window as unknown as {
      __repodeskEmitMockTauriEvent: (event: string, payload: unknown) => void;
    }).__repodeskEmitMockTauriEvent = (event, payload) => {
      for (const identifier of listeners.get(event) ?? []) {
        callbacks.get(identifier)?.({ event, id: identifier, payload });
      }
    };

    (window as unknown as { __TAURI_INTERNALS__: unknown }).__TAURI_INTERNALS__ = {
      invoke(cmd: string, args?: Record<string, unknown>) {
        calls.push(cmd);
        invocations.push({ cmd, args });

        if (cmd === "plugin:event|listen") {
          const event = String(args?.event ?? "");
          const handler = Number(args?.handler);
          listeners.set(event, [...(listeners.get(event) ?? []), handler]);
          return Promise.resolve(handler);
        }
        if (cmd === "plugin:event|unlisten") {
          unregisterCallback(Number(args?.eventId));
          return Promise.resolve(null);
        }

        const action = args?.action;
        const actionKind = action && typeof action === "object" && "kind" in action
          ? String((action as { kind: unknown }).kind)
          : action === null ? "snapshot" : null;
        const actionKey = actionKind ? `${cmd}:${actionKind}` : null;
        const fixtureKey = actionKey && Object.prototype.hasOwnProperty.call(data, actionKey)
          ? actionKey
          : cmd;
        let value = Object.prototype.hasOwnProperty.call(data, fixtureKey) ? data[fixtureKey] : null;

        if (value && typeof value === "object" && "__mock_sequence" in value) {
          const sequence = (value as { __mock_sequence: unknown }).__mock_sequence;
          if (Array.isArray(sequence) && sequence.length > 0) {
            const position = sequencePositions.get(fixtureKey) ?? 0;
            value = sequence[Math.min(position, sequence.length - 1)];
            sequencePositions.set(fixtureKey, position + 1);
          }
        }

        if (value && typeof value === "object" && "__mock_error" in value) {
          return Promise.reject(new Error(String((value as { __mock_error: unknown }).__mock_error)));
        }

        if (
          value
          && typeof value === "object"
          && "__mock_delay_ms" in value
          && "__mock_value" in value
        ) {
          const delayed = value as { __mock_delay_ms: unknown; __mock_value: unknown };
          const delay = typeof delayed.__mock_delay_ms === "number" ? delayed.__mock_delay_ms : 0;
          return new Promise((resolve) => window.setTimeout(() => resolve(delayed.__mock_value), delay));
        }

        return Promise.resolve(value);
      },
      // Stubs for the event/callback machinery so `@tauri-apps/api/event` and
      // friends don't throw if a component subscribes on mount.
      transformCallback(callback: unknown) {
        callbackSequence += 1;
        if (typeof callback === "function") {
          callbacks.set(callbackSequence, callback as (payload: unknown) => unknown);
        }
        return callbackSequence;
      },
      unregisterCallback,
      convertFileSrc(path: string) {
        return path;
      },
      metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main" } },
    };
    (window as unknown as { __TAURI_EVENT_PLUGIN_INTERNALS__: unknown }).__TAURI_EVENT_PLUGIN_INTERNALS__ = {
      unregisterListener(_event: string, identifier: number) {
        unregisterCallback(identifier);
      },
    };
  }, fixtures);
}

/** Delivers one event only to listeners that are currently registered in the page. */
export async function emitMockTauriEvent(
  page: Page,
  event: string,
  payload: unknown,
): Promise<void> {
  await page.evaluate(
    ({ eventName, eventPayload }) => {
      (window as unknown as {
        __repodeskEmitMockTauriEvent: (name: string, value: unknown) => void;
      }).__repodeskEmitMockTauriEvent(eventName, eventPayload);
    },
    { eventName: event, eventPayload: payload },
  );
}

/** Reads the ordered list of commands the frontend invoked through the mock. */
export async function recordedCommands(page: Page): Promise<string[]> {
  return page.evaluate(() => (window as unknown as { __repodeskMockCalls?: string[] }).__repodeskMockCalls ?? []);
}

export async function recordedInvocations(
  page: Page,
): Promise<Array<{ cmd: string; args: Record<string, unknown> | undefined }>> {
  return page.evaluate(() => (
    window as unknown as {
      __repodeskMockInvocations?: Array<{ cmd: string; args: Record<string, unknown> | undefined }>;
    }
  ).__repodeskMockInvocations ?? []);
}
