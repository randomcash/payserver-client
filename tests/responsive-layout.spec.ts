import { test, expect } from '@playwright/test';
import { readFileSync } from 'fs';
import path from 'path';

/**
 * Layout regressions in the real stylesheet, with no server and no auth.
 *
 * `styles.css` is over 5000 lines and layers two design systems, so it contains
 * several pairs of rules with IDENTICAL specificity where the later one wins by
 * source order alone. That has produced three separate user-visible bugs, and
 * none of them could fail a normal test: the app compiles, renders, and is
 * simply wrong.
 *
 * These tests load the actual stylesheet against the actual class combinations
 * and assert computed style. No auth means no rate limit, and no server means
 * they run in milliseconds - which matters, because the alternative is noticing
 * from a screenshot.
 */
// Resolved from this file, not the process cwd. Playwright can be run from the
// repo root with `--config e2e/playwright.config.ts`, and a relative path there
// makes the whole spec die during collection with ENOENT - reported as "no
// tests found" rather than as a broken path.
//
// The @import of Google Fonts is stripped because these tests exist to be fast
// and network-free: leaving it in makes every setContent fetch fonts.googleapis
// .com (542ms per call measured, versus 28ms without) and hang to the test
// timeout on a runner with no egress. The font changes no measurement here -
// input and select are 36px with or without Inter.
//
// Matched to end of LINE, not to the first semicolon. The font URL contains
// semicolons of its own (`wght@400;500;600;700`), so `@import[^;]+;` cuts
// inside the url() and leaves `500;600;700&display=swap');` behind, which
// swallows the :root block that defines --space-3 - every `gap` in the file
// then computes to `normal` and card spacing silently becomes 0. Measured: the
// naive strip turned a 12px gap into 0.
const CSS = readFileSync(path.join(__dirname, '../styles.css'), 'utf8')
  .replace(/^\s*@import\b.*$/gm, '');

async function displays(page: import('@playwright/test').Page, html: string, sels: string[]) {
  await page.setContent(`<style>${CSS}</style>${html}`);
  const out: Record<string, string> = {};
  for (const s of sels) out[s] = await page.locator(s).evaluate((el) => getComputedStyle(el).display);
  return out;
}

/**
 * `.mobile-only { display: none }` sits at line ~2714; `.invoices-cards` and
 * `.payments-cards` set `display: flex` at ~2729 and ~3189. Same specificity,
 * so the component won and the card list rendered on desktop UNDER the table -
 * every table showed its rows twice.
 */
test.describe('responsive visibility', () => {
  const MARKUP = `
    <div class="payments-table-container desktop-only"><table class="payments-table"><tr><td>row</td></tr></table></div>
    <div class="payments-cards mobile-only"><div class="payment-card">card</div></div>
    <div class="invoices-cards mobile-only"><div>card</div></div>`;

  test('desktop shows the table and hides both card lists', async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    const d = await displays(page, MARKUP, [
      '.payments-table-container', '.payments-cards', '.invoices-cards',
    ]);
    expect(d['.payments-table-container']).not.toBe('none');
    expect(d['.payments-cards'], 'payment cards must not double the table').toBe('none');
    expect(d['.invoices-cards'], 'invoice cards must not double the table').toBe('none');
  });

  test('mobile shows the card lists and hides the table', async ({ page }) => {
    await page.setViewportSize({ width: 500, height: 900 });
    const d = await displays(page, MARKUP, [
      '.payments-table-container', '.payments-cards', '.invoices-cards',
    ]);
    expect(d['.payments-table-container']).toBe('none');
    // `flex`, not merely "not none". Forcing the utility to `display: block`
    // showed the list while silently killing its `gap`, and an assertion of
    // `!== 'none'` passed the whole time.
    expect(d['.payments-cards'], 'card list must stay a flex column').toBe('flex');
    expect(d['.invoices-cards'], 'card list must stay a flex column').toBe('flex');
  });

  /**
   * The above checks the declared display; this checks the spacing it is for.
   * A card list can be visible, be flex, and still render its cards flush if a
   * later rule wins - which is the failure mode this whole file exists for.
   */
  test('mobile cards are actually spaced apart', async ({ page }) => {
    await page.setViewportSize({ width: 500, height: 900 });
    await page.setContent(`<style>${CSS}</style>
      <div class="payments-cards mobile-only">
        <div class="payment-card">a</div><div class="payment-card">b</div>
      </div>`);
    const gap = await page.locator('.payments-cards').evaluate((el) => {
      const cards = el.querySelectorAll('.payment-card');
      const a = cards[0].getBoundingClientRect();
      const b = cards[1].getBoundingClientRect();
      return Math.round(b.top - a.bottom);
    });
    expect(gap, 'cards must not sit flush against each other').toBeGreaterThan(0);
  });
});

/**
 * `.form-group + .form-group { margin-top }` is a stacked-form rule that also
 * applied inside containers with their own `gap`, pushing every second field
 * 16px below the first. Reported twice: Amount vs Currency in the create-invoice
 * modal, and Account Created vs Last Login in Settings.
 */
test.describe('form field alignment', () => {
  const cases: [string, string][] = [
    ['.form-row', `<div class="form-row">
       <div class="form-group form-group-grow"><label class="form-label">A</label><input class="form-input"></div>
       <div class="form-group"><label class="form-label">B</label><select class="form-input"><option>x</option></select></div>
     </div>`],
    ['.settings-grid', `<div class="settings-grid">
       <div class="form-group"><label class="form-label">A</label><div class="form-static">v</div></div>
       <div class="form-group"><label class="form-label">B</label><div class="form-static">v</div></div>
     </div>`],
  ];

  for (const [container, markup] of cases) {
    test(`${container} aligns its fields on one line`, async ({ page }) => {
      await page.setViewportSize({ width: 1440, height: 900 });
      await page.setContent(`<style>${CSS}</style><div style="width:900px;padding:24px">${markup}</div>`);
      const groups = page.locator(`${container} > .form-group`);
      const a = await groups.nth(0).boundingBox();
      const b = await groups.nth(1).boundingBox();
      expect(Math.round(b!.y - a!.y), `${container} second field must not sit lower`).toBe(0);
    });
  }

  test('an input and a select are the same height side by side', async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.setContent(
      `<style>${CSS}</style><div style="width:900px"><input class="form-input"><select class="form-input"><option>x</option></select></div>`);
    const i = await page.locator('input.form-input').boundingBox();
    const s = await page.locator('select.form-input').boundingBox();
    expect(Math.round(s!.height - i!.height), 'select must not sag beside an input').toBe(0);
  });
});

/**
 * `.payment-row` is used for two different things: the dashboard renders each
 * recent payment as `<a class="payment-row">` and wants the flex list layout,
 * while the payments and invoice-detail tables use `<tr class="payment-row">`.
 * `display: flex` on a table row removes it from table layout, so its cells stop
 * sharing the widths the <thead> computed and every header drifts off its
 * column - measured at 600-700px of drift before this was fixed.
 */
test.describe('table column alignment', () => {
  const TABLE = `
    <div class="payments-table-container desktop-only">
      <table class="payments-table">
        <thead><tr>
          <th>Transaction</th><th>Store</th><th>Amount</th><th>Network</th>
          <th>Invoice</th><th>Status</th><th>Date</th><th></th>
        </tr></thead>
        <tbody>
          <tr class="payment-row">
            <td><div class="payment-tx-cell"><code class="tx-hash">0x1a2b…3c4d</code></div></td>
            <td><span class="payment-store">Acme Store</span></td>
            <td><span class="payment-amount">0.5 ETH</span></td>
            <td><span class="payment-network">Sepolia</span></td>
            <td><a class="payment-invoice-link">inv_123</a></td>
            <td><span class="status-badge status-confirmed">Confirmed</span></td>
            <td><span class="payment-date">Sep 7, 2026</span></td>
            <td><a class="btn btn-ghost btn-sm btn-icon">⋯</a></td>
          </tr>
        </tbody>
      </table>
    </div>`;

  test('every header sits exactly over its column', async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.setContent(`<style>${CSS}</style><div style="padding:24px">${TABLE}</div>`);

    const cols = await page.evaluate(() => {
      const ths = [...document.querySelectorAll('.payments-table thead th')];
      const tds = [...document.querySelectorAll('.payments-table tbody td')];
      return ths.map((th, i) => ({
        col: (th.textContent || 'actions').trim() || 'actions',
        dx: Math.round(tds[i].getBoundingClientRect().left - th.getBoundingClientRect().left),
      }));
    });

    expect(cols.length, 'header and body must have the same number of cells').toBe(8);
    for (const { col, dx } of cols) {
      expect(dx, `"${col}" header must sit over its column`).toBe(0);
    }
  });

  test('a table row stays a table row', async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.setContent(`<style>${CSS}</style>${TABLE}`);
    const display = await page.locator('.payments-table tbody tr').evaluate((el) => getComputedStyle(el).display);
    expect(display, 'flex here silently destroys column alignment').toBe('table-row');
  });
});

/**
 * `.ps-recovery-confirm` carried a copy of the `.ps-checkbox-label` rule -
 * `display: flex` with the warning background and padding. Applied to the step
 * container instead of the checkbox, it laid the four children out as four
 * columns: the title, the description, the checkbox label and the buttons, each
 * a narrow strip of text. It also nested the warning background
 * inside itself, invisibly.
 *
 * Reported twice from real use - passkey registration and wallet registration,
 * which share this component from ui-kit.
 */
test.describe('recovery confirm step', () => {
  const MARKUP = `
    <div class="ps-recovery-confirm">
      <h3 class="ps-recovery-title">Confirm Your Recovery Phrase</h3>
      <p class="ps-recovery-description">Please confirm that you have saved your recovery phrase securely.</p>
      <label class="ps-checkbox-label"><input type="checkbox" class="ps-checkbox"/><span>I have written down my recovery phrase.</span></label>
      <div class="ps-recovery-actions"><button class="ps-button ps-button-primary">Complete Setup</button></div>
    </div>`;

  test('the step stacks its children instead of laying them out as columns', async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 900 });
    await page.setContent(`<style>${CSS}</style>${MARKUP}`);

    const boxes = await page.locator('.ps-recovery-confirm > *').evaluateAll((els) =>
      els.map((el) => el.getBoundingClientRect()).map((r) => ({ x: Math.round(r.x), y: Math.round(r.y) })),
    );
    expect(boxes.length).toBe(4);

    // Stacked: every child starts at the same x and strictly below the previous.
    // Side-by-side columns is the bug, and it shows up as increasing x.
    for (let i = 1; i < boxes.length; i++) {
      expect(boxes[i].x, `child ${i} shifted right — laid out as a column`).toBe(boxes[0].x);
      expect(boxes[i].y, `child ${i} did not stack below child ${i - 1}`).toBeGreaterThan(boxes[i - 1].y);
    }
  });

  test('the warning background belongs to the checkbox, not the whole step', async ({ page }) => {
    await page.setContent(`<style>${CSS}</style>${MARKUP}`);
    const step = await page.locator('.ps-recovery-confirm').evaluate((el) => getComputedStyle(el).backgroundColor);
    const label = await page.locator('.ps-checkbox-label').evaluate((el) => getComputedStyle(el).backgroundColor);

    expect(label, 'the checkbox should sit in the warning box').not.toBe('rgba(0, 0, 0, 0)');
    expect(step, 'the step container should not repeat it').toBe('rgba(0, 0, 0, 0)');
  });
});
