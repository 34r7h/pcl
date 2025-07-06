const puppeteer = require('puppeteer');
const path = require('path');

async function offlineTest() {
  console.log('🔴 OFFLINE DETECTION TEST');
  console.log('=== Verifying Network Status Accuracy ===\n');
  
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
    
    // Monitor console for network status checks
    page.on('console', msg => {
      const text = msg.text();
      if (text.includes('Network status') || text.includes('Node status') || text.includes('Simulator status')) {
        console.log('🌐', text);
      }
    });
    
    // Navigate to wallet
    const dashboardPath = path.join(__dirname, "fullscreen.html");
    await page.goto(`file://${dashboardPath}`);
    await page.waitForSelector('body', { timeout: 10000 });
    
    // Setup storage
    await page.evaluate(() => {
      window.chrome = window.chrome || {};
      window.chrome.storage = {
        local: {
          get: async () => ({}),
          set: async () => true
        }
      };
    });
    
    // Wait for network status check
    await new Promise(resolve => setTimeout(resolve, 5000));
    
    const networkStatus = await page.evaluate(() => {
      return {
        nodeStatus: document.getElementById('node-status')?.textContent,
        nodeColor: document.getElementById('node-status')?.style.color,
        simulatorStatus: document.getElementById('simulator-status')?.textContent,
        simulatorColor: document.getElementById('simulator-status')?.style.color,
        networkText: document.getElementById('networkText')?.textContent
      };
    });
    
    console.log('📊 Network Status when backend is OFF:');
    console.log('   Node Status:', networkStatus.nodeStatus, networkStatus.nodeColor);
    console.log('   Simulator Status:', networkStatus.simulatorStatus, networkStatus.simulatorColor);
    console.log('   Network Text:', networkStatus.networkText);
    
    const isCorrectlyOffline = networkStatus.nodeStatus === '○' && 
                              networkStatus.simulatorStatus === '○' &&
                              networkStatus.networkText === 'Offline';
    
    if (isCorrectlyOffline) {
      console.log('✅ PASS: Wallet correctly detects offline status');
    } else {
      console.log('❌ FAIL: Wallet should show white circles and "Offline" text');
    }
    
    await page.screenshot({ path: 'offline-test.png' });
    console.log('📸 Offline test screenshot saved');
    
  } catch (error) {
    console.error('❌ Test failed:', error);
  } finally {
    await browser.close();
  }
}

offlineTest();
