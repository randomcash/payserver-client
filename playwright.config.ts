import { defineConfig } from '@playwright/test';

/**
 * Layout regression tests only.
 *
 * These came across with the frontend when it left ethpayserver, and they
 * belong here for the same reason the stylesheet does: a CSS specificity
 * regression should fail the repository that owns the CSS, not a payserver's
 * pipeline. They need no server, no database and no auth — each test injects
 * `styles.css` into a blank page and asserts computed style — so this config
 * deliberately has no `webServer` and no fixtures.
 */
export default defineConfig({
  testDir: './tests',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: 0,
  reporter: process.env.CI ? [['github'], ['list']] : [['list']],
  use: { browserName: 'chromium' },
});
