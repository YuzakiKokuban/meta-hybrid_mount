## 2025-12-15 - Svelte 5 and Web Components Accessibility
**Learning:** Svelte 5 may map `aria-label` to properties on custom elements rather than attributes. While `md-icon-button` correctly handles the accessible name via `ElementInternals` or internal delegation, the `aria-label` attribute might not appear in the DOM on the custom element itself.
**Action:** Use Playwright's accessible locators (e.g., `get_by_role('button', name='...')`) to verify accessibility, rather than relying on attribute existence in the DOM inspector.
