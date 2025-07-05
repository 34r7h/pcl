const puppeteer = require('puppeteer');
const path = require('path');

async function testConnectivity() {
  console.log('🔌 Testing XMBL Extension → Node Connectivity...');
  
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
    
    // Capture console logs
    const logs = [];
    page.on('console', msg => {
      const text = msg.text();
      logs.push(text);
      if (text.includes('XMBL')) {
        console.log('📱', text);
      }
    });
    
    // Navigate to popup
    const popupPath = path.join(__dirname, "popup.html");
    await page.goto(`file://${popupPath}`);
    await page.waitForSelector('body', { timeout: 5000 });
    
    // Wait for extension to initialize and check connectivity
    console.log('⏳ Waiting for extension initialization...');
    await new Promise(resolve => setTimeout(resolve, 5000));
    
    // Check if network status element exists and shows connection
    const networkStatus = await page.evaluate(() => {
      const statusEl = document.getElementById('networkStatus');
      return statusEl ? statusEl.textContent : 'No status element found';
    });
    
    console.log('🌐 Network Status:', networkStatus);
    
    // Test direct API call from extension context
    const apiTest = await page.evaluate(async () => {
      try {
        const response = await fetch('http://localhost:8080/health');
        const data = await response.json();
        return { success: true, data };
      } catch (error) {
        return { success: false, error: error.message };
      }
    });
    
    console.log('🔗 Direct API Test:', apiTest);
    
    // Take final screenshot
    await page.screenshot({ path: 'connectivity-test.png', fullPage: true });
    console.log('✅ Screenshot saved: connectivity-test.png');
    
    // Summary
    console.log('\n📊 Test Summary:');
    console.log('Extension Logs:', logs.filter(log => log.includes('XMBL')).length, 'XMBL-related messages');
    console.log('Network Status:', networkStatus);
    console.log('API Connectivity:', apiTest.success ? '✅ Working' : '❌ Failed');
    
  } catch (error) {
    console.error('❌ Connectivity test failed:', error);
  } finally {
    await browser.close();
  }
}

testConnectivity();
