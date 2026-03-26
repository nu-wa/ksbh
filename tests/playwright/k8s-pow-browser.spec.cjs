const { test, expect, chromium } = require('@playwright/test');
const crypto = require('node:crypto');

function browserBaseUrl() {
  return process.env.KSBH_E2E_BROWSER_BASE_URL || 'http://app.test.local:18080';
}

function browserHostRule() {
  return process.env.KSBH_E2E_BROWSER_HOST_RULE || '';
}

function findPowNonce(challenge, difficulty) {
  let nonce = 1;

  while (true) {
    const hash = crypto
      .createHash('sha256')
      .update(`${challenge}${nonce}`)
      .digest('hex');

    if (hash.startsWith('0'.repeat(difficulty))) {
      return nonce;
    }

    nonce += 1;
  }
}

test('k8s_pow_module_is_solved_by_browser_javascript', async () => {
  test.setTimeout(60000);
  const hostRule = browserHostRule();
  const launchArgs = [
    '--disable-dev-shm-usage',
    '--disable-gpu',
    '--disable-setuid-sandbox',
    '--no-sandbox',
    '--no-first-run',
    '--no-default-browser-check',
    '--disable-background-networking',
  ];

  if (hostRule.length > 0) {
    launchArgs.push(`--host-resolver-rules=${hostRule}`);
  }

  const context = await chromium.launchPersistentContext('/tmp/ksbh-playwright-profile', {
    headless: true,
    args: launchArgs,
  });

  const page = context.pages()[0] || await context.newPage();
  let powPostDebug = null;

  page.on('request', request => {
    if (request.method() === 'POST' && request.url().includes('/pow')) {
      powPostDebug = {
        url: request.url(),
        method: request.method(),
        postData: request.postData(),
      };

      console.log('pow post request:', JSON.stringify(powPostDebug, null, 2));
    }
  });

  try {
    await page.goto(`${browserBaseUrl()}/get`, {
      waitUntil: 'domcontentloaded',
      timeout: 30000,
    });

    await expect(page.locator('#status')).toBeVisible({ timeout: 30000 });
    await expect(page.locator('#powForm')).toHaveCount(1, { timeout: 30000 });
    const challenge = await page.locator('input[name="challenge"]').inputValue();
    const difficultyText = (await page.locator('#zero-count').textContent()) || '';
    const difficultyMatch = difficultyText.match(/\/\s*(\d+)\s+zeroes/i);
    const difficulty = difficultyMatch ? Number.parseInt(difficultyMatch[1], 10) : 1;
    const nonce = findPowNonce(challenge, difficulty);

    await page.locator('#nonce').evaluate((element, value) => {
      element.value = String(value);
    }, nonce);

    const formDebug = await page.locator('#powForm').evaluate(form => ({
      action: form.action,
      challenge: form.querySelector('input[name="challenge"]')?.value || null,
      nonce: form.querySelector('#nonce')?.value || null,
      redirectToQuery: new URL(form.action, window.location.href).searchParams.get('redirect_to'),
      entries: Array.from(new FormData(form).entries()),
    }));

    console.log('pow form debug:', JSON.stringify(formDebug, null, 2));

    await Promise.all([
      page.waitForURL('**/get', { timeout: 45000 }),
      page.locator('#powForm').evaluate(form => form.submit()),
    ]);

    await expect(page.locator('body')).toContainText('"url"', { timeout: 45000 });
    await expect(page.locator('body')).toContainText('/get', { timeout: 45000 });

    const secondResponse = await page.goto(`${browserBaseUrl()}/get`, {
      waitUntil: 'domcontentloaded',
      timeout: 30000,
    });

    expect(secondResponse).not.toBeNull();
    expect(secondResponse.status()).toBe(200);
    await expect(page.locator('#powForm')).toHaveCount(0, { timeout: 5000 });
    await expect(page.locator('body')).toContainText('"url"', { timeout: 5000 });
    await expect(page.locator('body')).toContainText('/get', { timeout: 5000 });

    const bodyText = await page.locator('body').innerText();
    expect(bodyText).toContain('"url"');
    expect(bodyText).toContain('/get');

    const cookies = await context.cookies();
    expect(cookies.length).toBeGreaterThan(0);
  } finally {
    await context.close();
  }
});
