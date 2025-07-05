const puppeteer = require('puppeteer');
const path = require('path');

async function simpleTest() {
  console.log('🧪 Simple wallet test...');
  
  const browser = await puppeteer.launch({
    headless: false,
    args: [
      `--load-extension=${__dirname}`,
      `--disable-extensions-except=${__dirname}`,
      "--no-sandbox",
      "--disable-setuid-sandbox"
    ]
  });

  try {
    const page = await browser.newPage();
    
    // Listen for console logs and errors
    page.on('console', msg => {
      console.log('🖥️ Console:', msg.text());
    });
    
    page.on('pageerror', error => {
      console.log('❌ Page Error:', error.message);
    });
    
    // Navigate to fullscreen dashboard
    const dashboardPath = path.join(__dirname, "fullscreen.html");
    await page.goto(`file://${dashboardPath}`);
    
    // Wait for page to load
    await page.waitForSelector('body', { timeout: 10000 });
    await new Promise(resolve => setTimeout(resolve, 2000));
    
    // Check current state
    const pageInfo = await page.evaluate(() => {
      return {
        title: document.title,
        balance: document.getElementById('fullscreen-balance')?.textContent,
        address: document.getElementById('fullscreen-address')?.innerHTML,
        hasCreateBtn: !!document.getElementById('create-wallet-btn'),
        hasFaucetBtn: !!document.getElementById('faucet-btn'),
        testAddressCount: document.querySelectorAll('.test-address').length
      };
    });
    
    console.log('📊 Page Info:', pageInfo);
    
    // Take a screenshot
    await page.screenshot({ path: 'simple-test.png', fullPage: true });
    console.log('📸 Screenshot saved: simple-test.png');
    
  } catch (error) {
    console.error('❌ Test failed:', error);
  } finally {
    await browser.close();
  }
}

simpleTest();
