const puppeteer = require('puppeteer');
const fs = require('fs');
const path = require('path');

// Real Mempool Data Display Test
async function testRealMempoolDisplay() {
  console.log('🔍 Testing Real Mempool Data Display');
  console.log('=' .repeat(50));

  // Submit a transaction first to populate mempools
  console.log('\n📤 Submitting transaction to populate mempools...');
  try {
    const { default: fetch } = await import('node-fetch');
    
    const txResponse = await fetch('http://localhost:8080/transaction', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        from: 'faucet_address_123456789',
        to: 'test_user_mempool_display',
        amount: 75.0,
        user: 'mempool_test',
        stake: 1.0,
        fee: 0.1
      })
    });
    
    console.log('Transaction submitted for mempool testing');
  } catch (error) {
    console.log('Note: Transaction submission may have failed, proceeding with test');
  }

  // Test extension display
  console.log('\n🖥️  Testing Extension Mempool Display');
  await testExtensionMempoolDisplay();
  
  console.log('\n✅ Real Mempool Test Complete');
}

async function testExtensionMempoolDisplay() {
  console.log('Testing extension mempool display...');
  
  const browser = await puppeteer.launch({ 
    headless: false,
    args: ['--no-sandbox', '--disable-setuid-sandbox', '--disable-web-security']
  });
  
  try {
    const page = await browser.newPage();
    
    // Enable console logging
    page.on('console', msg => {
      if (msg.text().includes('XMBL') || msg.text().includes('mempool') || msg.text().includes('error')) {
        console.log(`   Console: ${msg.text()}`);
      }
    });
    
    await page.goto('file://' + path.resolve(__dirname, 'fullscreen.html'));
    
    // Wait for page to load
    await page.waitForTimeout(3000);
    
    // Test mempool tab
    console.log('\n  Mempool Tab Test:');
    const mempoolBtn = await page.$('button[data-view="mempool"]');
    if (mempoolBtn) {
      await mempoolBtn.click();
      await page.waitForTimeout(5000); // Wait for mempool data to load
      
      // Check mempool stats
      const rawTxCount = await page.$eval('#raw-tx-count', el => el.textContent).catch(() => 'N/A');
      const processingTxCount = await page.$eval('#processing-tx-count', el => el.textContent).catch(() => 'N/A');
      const validationTaskCount = await page.$eval('#validation-task-count', el => el.textContent).catch(() => 'N/A');
      const lockedUtxoCount = await page.$eval('#locked-utxo-count', el => el.textContent).catch(() => 'N/A');
      
      console.log(`    Raw Transactions: ${rawTxCount}`);
      console.log(`    Processing Transactions: ${processingTxCount}`);
      console.log(`    Validation Tasks: ${validationTaskCount}`);
      console.log(`    Locked UTXOs: ${lockedUtxoCount}`);
      
      // Check if mempool sections are displayed
      const mempoolSections = await page.$$('.mempool-section');
      console.log(`    Mempool Sections Displayed: ${mempoolSections.length}`);
      
      if (mempoolSections.length > 0) {
        // Get titles of displayed sections
        const sectionTitles = await page.$$eval('.mempool-title', titles => 
          titles.map(title => title.textContent)
        );
        console.log(`    Section Titles: ${sectionTitles.join(', ')}`);
      }
    }
    
    // Test dashboard transactions
    console.log('\n  Dashboard Transactions Test:');
    const dashboardBtn = await page.$('button[data-view="dashboard"]');
    if (dashboardBtn) {
      await dashboardBtn.click();
      await page.waitForTimeout(3000);
      
      const transactionItems = await page.$$('.transaction-item');
      console.log(`    Transaction Items: ${transactionItems.length}`);
      
      if (transactionItems.length > 0) {
        // Get transaction details
        const txHashes = await page.$$eval('.tx-hash', hashes => 
          hashes.map(hash => hash.textContent)
        );
        console.log(`    Transaction Hashes: ${txHashes.join(', ')}`);
        
        // Check consensus steps
        const consensusSteps = await page.$$eval('.step-list li', steps => 
          steps.map(step => step.textContent)
        );
        console.log(`    Consensus Steps Found: ${consensusSteps.length}`);
        if (consensusSteps.length > 0) {
          console.log(`    First Few Steps: ${consensusSteps.slice(0, 3).join(', ')}`);
        }
      }
    }
    
    // Take screenshot
    const screenshotPath = path.join(__dirname, 'real-mempool-display.png');
    await page.screenshot({ path: screenshotPath, fullPage: true });
    console.log(`    📸 Screenshot saved: ${screenshotPath}`);
    
    console.log('\n✅ Extension mempool display test completed');
    
  } catch (error) {
    console.log('❌ Extension test failed:', error.message);
  } finally {
    await browser.close();
  }
}

// Run the test
testRealMempoolDisplay().catch(console.error); 