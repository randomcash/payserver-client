import { defineConfig } from '@playwright/test';
import { mkdirSync, rmSync } from 'node:fs';
import { resolve } from 'node:path';

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

// Local runs only. CI's /tmp is disk-backed and the runner is thrown away
// after the job; this box's own /tmp is RAM-backed, which is where
// Playwright puts the chromium profile directory by default. Setting
// TMPDIR here, at config load, redirects it for every entry point that
// loads this config — `npx playwright test`, `npm test`, `--ui` alike —
// rather than only ones that go through a wrapper script, which a bare
// `npx playwright test` bypasses entirely.
//
// Wiping the directory here rather than trapping it on exit means even an
// interruption no trap can catch (SIGKILL, a killed job) leaves at most one
// stale profile behind instead of accumulating one per interruption — the
// next invocation clears it before using it.
if (!process.env.CI) {
  const scratch = resolve(process.cwd(), '.tmp');
  rmSync(scratch, { recursive: true, force: true });
  mkdirSync(scratch, { recursive: true });
  process.env.TMPDIR = scratch;
}

export default defineConfig({
  testDir: './tests',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: 0,
  reporter: process.env.CI ? [['github'], ['list']] : [['list']],
  use: { browserName: 'chromium' },
});
