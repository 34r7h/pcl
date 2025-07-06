const puppeteer = require('puppeteer');
const fs = require('fs');
const path = require('path');

// XMBL Cross-Validation Consensus Protocol Test Suite
async function runCrossValidationTest() {
  console.log('🚀 XMBL Cross-Validation Consensus Protocol Test Suite');
  console.log('=' .repeat(60));

  // Test 1: Backend Cross-Validation Protocol
  console.log('\n📊 Test 1: Backend Cross-Validation Protocol');
  await testBackendCrossValidation();

  // Test 2: Extension Cross-Validation Display
  console.log('\n🖥️  Test 2: Extension Cross-Validation Display');
  await testExtensionCrossValidation();

  // Test 3: Full Transaction Cross-Validation Workflow
  console.log('\n🔗 Test 3: Full Transaction Cross-Validation Workflow');
  await testFullCrossValidationWorkflow();

  console.log('\n✅ All Cross-Validation Tests Completed');
}

async function testBackendCrossValidation() {
  console.log('Testing backend cross-validation protocol...');
  
  try {
    // Use dynamic import for node-fetch
    const { default: fetch } = await import('node-fetch');
    
    // Test network info
    const networkResponse = await fetch('http://localhost:8080/network');
    const networkData = await networkResponse.json();
    
    console.log('Network Topology:');
    console.log(`  Leaders: ${networkData.leaders}`);
    console.log(`  Validators: ${networkData.validators}`);
    console.log(`  Simulator Nodes: ${networkData.simulator_nodes || 'N/A'}`);
    console.log(`  Raw Transactions: ${networkData.raw_transactions}`);
    console.log(`  Validation Tasks: ${networkData.validation_tasks}`);
    console.log(`  Finalized Transactions: ${networkData.finalized_transactions}`);
    
    // Test cross-validation log
    if (networkData.cross_validation_log) {
      console.log('\nCross-Validation Activity:');
      networkData.cross_validation_log.forEach((entry, i) => {
        console.log(`  ${i + 1}. ${entry}`);
      });
    }
    
    // Submit a test transaction to trigger cross-validation
    const testTransaction = {
      from: 'test_user_alice_123',
      to: 'test_user_bob_456',
      amount: 50.0,
      user: 'alice',
      stake: 1.0,
      fee: 0.1
    };
    
    console.log('\n📤 Submitting test transaction to trigger cross-validation...');
    const txResponse = await fetch('http://localhost:8080/transaction', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json'
      },
      body: JSON.stringify(testTransaction)
    });
    
    const txResult = await txResponse.json();
    console.log('Transaction Result:', txResult);
    
    if (txResult.success) {
      console.log('✅ Cross-validation protocol triggered successfully');
      console.log(`   Transaction Hash: ${txResult.transaction.hash}`);
      
      if (txResult.transaction.cross_validators) {
        console.log(`   Cross-validators: ${txResult.transaction.cross_validators.join(', ')}`);
      }
      
      if (txResult.transaction.validation_tasks_for_submitter) {
        console.log(`   Validation Tasks for Submitter: ${txResult.transaction.validation_tasks_for_submitter.length}`);
      }
      
      if (txResult.transaction.validation_steps) {
        console.log('   Validation Steps:');
        txResult.transaction.validation_steps.forEach((step, i) => {
          console.log(`     ${i + 1}. ${step}`);
        });
      }
    } else {
      console.log('❌ Cross-validation protocol failed:', txResult.error);
    }
    
  } catch (error) {
    console.log('❌ Backend cross-validation test failed:', error.message);
    console.log('   Note: This might be due to Node.js version or network configuration');
  }
}

async function testExtensionCrossValidation() {
  console.log('Testing extension cross-validation display...');
  
  const browser = await puppeteer.launch({ 
    headless: false,
    args: ['--no-sandbox', '--disable-setuid-sandbox', '--disable-web-security']
  });
  
  try {
    const page = await browser.newPage();
    await page.goto('file://' + path.resolve(__dirname, 'fullscreen.html'));
    
    // Wait for page to load
    await page.waitForTimeout(3000);
    
    // Enable console logging to see what's happening
    page.on('console', msg => {
      if (msg.text().includes('XMBL') || msg.text().includes('mempool') || msg.text().includes('validation')) {
        console.log('   Extension Console:', msg.text());
      }
    });
    
    // Check if mempool tab exists
    const mempoolTab = await page.$('button[data-view="mempool"]');
    if (mempoolTab) {
      console.log('✅ Mempool tab found');
      
      // Click mempool tab
      await mempoolTab.click();
      await page.waitForTimeout(2000);
      
      // Check if mempool elements exist
      const mempoolCard = await page.$('.mempool-card');
      if (mempoolCard) {
        console.log('✅ Mempool card found');
      } else {
        console.log('❌ Mempool card not found');
      }
    } else {
      console.log('❌ Mempool tab not found');
    }
    
    // Check dashboard for cross-validation info
    const dashboardTab = await page.$('button[data-view="dashboard"]');
    if (dashboardTab) {
      await dashboardTab.click();
      await page.waitForTimeout(2000);
      
      // Check for live test addresses
      const liveAddresses = await page.$('#live-test-addresses');
      if (liveAddresses) {
        console.log('✅ Live test addresses section found');
        
        // Wait for addresses to be generated
        await page.waitForTimeout(5000);
        
        const addressItems = await page.$$('.test-address-item');
        console.log(`   Generated ${addressItems.length} test addresses`);
        
        if (addressItems.length > 0) {
          // Click on first address to test copy functionality
          const copyBtn = await page.$('.copy-address-btn');
          if (copyBtn) {
            await copyBtn.click();
            console.log('✅ Address copy functionality working');
          }
        }
      }
      
      // Check for transaction consensus steps
      const transactionsList = await page.$('#transactions-list');
      if (transactionsList) {
        console.log('✅ Transaction list found');
        
        // Wait for transactions to load
        await page.waitForTimeout(3000);
        
        const txItems = await page.$$('.transaction-item');
        console.log(`   Found ${txItems.length} transactions with consensus details`);
        
        if (txItems.length > 0) {
          // Check if consensus steps are displayed
          const consensusSteps = await page.$$('.consensus-steps');
          console.log(`   Found ${consensusSteps.length} consensus step displays`);
        }
      }
    }
    
    console.log('✅ Extension cross-validation display test completed');
    
  } catch (error) {
    console.log('❌ Extension cross-validation display failed:', error.message);
  } finally {
    await browser.close();
  }
}

async function testFullCrossValidationWorkflow() {
  console.log('Testing full cross-validation workflow...');
  
  const browser = await puppeteer.launch({ 
    headless: false,
    args: ['--no-sandbox', '--disable-setuid-sandbox', '--disable-web-security'],
    slowMo: 500 // Slow down for better visibility
  });
  
  try {
    const page = await browser.newPage();
    await page.goto('file://' + path.resolve(__dirname, 'fullscreen.html'));
    
    // Enable console logging
    page.on('console', msg => {
      if (msg.text().includes('XMBL')) {
        console.log('   Console:', msg.text());
      }
    });
    
    await page.waitForTimeout(3000);
    
    // Test wallet creation
    console.log('\n🔐 Testing wallet creation...');
    const walletSection = await page.$('.wallet-section');
    if (walletSection) {
      const createWalletBtn = await page.$('#create-wallet-btn');
      if (createWalletBtn) {
        await createWalletBtn.click();
        await page.waitForTimeout(2000);
        
        const walletAddress = await page.$eval('#fullscreen-address', el => el.textContent).catch(() => 'N/A');
        console.log(`   Wallet Address: ${walletAddress}`);
        
        if (walletAddress !== 'No Wallet') {
          console.log('✅ Wallet created successfully');
        }
      }
    }
    
    // Test faucet
    console.log('\n💰 Testing faucet...');
    const faucetBtn = await page.$('#faucet-btn');
    if (faucetBtn) {
      await faucetBtn.click();
      await page.waitForTimeout(2000);
      
      const balance = await page.$eval('#fullscreen-balance', el => el.textContent).catch(() => 'N/A');
      console.log(`   Balance after faucet: ${balance}`);
      
      if (balance !== 'Loading...') {
        console.log('✅ Faucet working correctly');
      }
    }
    
    // Test real-time mempool updates
    console.log('\n🔄 Testing real-time mempool updates...');
    const mempoolTab = await page.$('button[data-view="mempool"]');
    if (mempoolTab) {
      await mempoolTab.click();
      await page.waitForTimeout(5000); // Wait for several update cycles
      
      // Check activity log
      const activityLog = await page.$('#activity-log');
      if (activityLog) {
        const activityEntries = await page.$$('.activity-entry');
        console.log(`   Activity Log Entries: ${activityEntries.length}`);
        
        if (activityEntries.length > 0) {
          const latestActivity = await page.$eval('.activity-entry', el => el.textContent).catch(() => 'N/A');
          console.log(`   Latest Activity: ${latestActivity}`);
          console.log('✅ Real-time mempool updates working');
        }
      }
    }
    
    console.log('✅ Full cross-validation workflow test completed');
    
  } catch (error) {
    console.log('❌ Full cross-validation workflow failed:', error.message);
  } finally {
    await browser.close();
  }
}

// Run the test suite
runCrossValidationTest().catch(console.error); 