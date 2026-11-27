// Set by the app's mock core (src/dev/overflow-check.ts), called from page.evaluate in the view test.
interface Window {
  wystaje(root?: Element, limit?: number): string[];
}
