// Focus ring only while focus is moved with the keyboard (docs/design.md 8.1). WebView2 turns
// :focus-visible on for a clicked button as soon as any key is pressed, and the window gets
// F6/F7/F8 and the control keys too, so the ring follows data-keyboard-nav on <html> instead.
const ARROWS = new Set(['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight']);
const ARROW_GROUPS = '[role="radiogroup"], [role="menu"], [role="slider"]';

export function trackKeyboardNav(): void {
  const root = document.documentElement;
  addEventListener(
    'keydown',
    (e) => {
      if (e.altKey || e.ctrlKey || e.metaKey) return;
      const inGroup = ARROWS.has(e.key) && e.target instanceof Element && e.target.closest(ARROW_GROUPS);
      if (e.key === 'Tab' || inGroup) root.dataset.keyboardNav = '';
    },
    true,
  );
  addEventListener('pointerdown', () => delete root.dataset.keyboardNav, true);
}
