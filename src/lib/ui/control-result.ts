// Switches and segmented controls show the new value at once. When the change handler returns a
// promise that resolves to `false` (a failed command, see `succeeded` in state.svelte.ts), the core
// state did not change, so the control goes back to the value it showed before.
export function revertOnFailure(result: unknown, revert: () => void) {
  if (!(result instanceof Promise)) return;
  result.then(
    (ok) => {
      if (ok === false) revert();
    },
    () => revert(),
  );
}
