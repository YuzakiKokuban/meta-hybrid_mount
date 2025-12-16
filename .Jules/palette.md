## 2025-05-23 - Accessibility of Icon-Only Buttons
**Learning:** Icon-only buttons (like `md-icon-button`) often rely on `title` for context, which is insufficient for screen readers.
**Action:** Always add `aria-label` to icon-only buttons, even if a `title` is present, to ensure full accessibility.
