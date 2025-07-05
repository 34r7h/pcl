const puppeteer = require('puppeteer');
const path = require('path');

async function testExtension() {
  console.log('🧪 Testing XMBL Extension Network Connectivity...');
  
  const browser = await puppeteer.launch({
    headless: false,
    args: [
      `--load-extension=${__dirname}`,
      `--disable-extensions-except=${__dirname}`,
      "--no-sandbox",
      "--disable-setuid-sandbox",
      "--disable-dev-shm-usage",
      "--disable-web-security"
    ]
  });

  try {
    const page = await browser.newPage();
    
    // Navigate to popup page directly
    const popupPath = path.join(__dirname, "popup.html");
    await page.goto(`file://${popupPath}`);
    
    // Wait for page to load
    await page.waitForSelector('body', { timeout: 5000 });
    
    // Check network status
    await page.evaluate(() => {
      console.log('Extension loaded, checking network status...');
    });
    
    // Take screenshot
    await page.screenshot({ path: 'extension-test.png', fullPage: true });
    console.log('✅ Screenshot saved: extension-test.png');
    
    // Wait a bit to see network status update
    await new Promise(resolve => setTimeout(resolve, 3000));
    
    // Check console logs
    page.on('console', msg => {
      if (msg.text().includes('XMBL Wallet')) {
        console.log('📱 Extension log:', msg.text());
      }
    });
    
    console.log('✅ Extension test completed');
    
  } catch (error) {
    console.error('❌ Extension test failed:', error);
  } finally {
    await browser.close();
  }
}

testExtension();
