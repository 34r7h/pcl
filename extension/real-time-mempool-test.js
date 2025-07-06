const puppeteer = require('puppeteer');
const { spawn } = require('child_process');
const fs = require('fs');
const path = require('path');

// Real-time mempool monitoring test
async function testRealTimeMempoolWorkflow() {
  console.log('🚀 REAL-TIME MEMPOOL WORKFLOW TEST');
  console.log('=' .repeat(60));

  // Step 1: Start backend node
  console.log('\n1. Starting Backend Node...');
  const nodeProcess = await startBackendNode();
  
  // Wait for node to be ready
  await waitForNodeReady();
  
  // Step 2: Open extension with Puppeteer
  console.log('\n2. Opening Extension Dashboard...');
  const { browser, page } = await openExtensionDashboard();
  
  // Step 3: Monitor mempools and submit transactions
  console.log('\n3. Testing Real-time Mempool Updates...');
  await testMempoolUpdates(page);
  
  // Step 4: Cleanup
  console.log('\n4. Cleanup...');
  await browser.close();
  nodeProcess.kill();
  
  console.log('\n✅ REAL-TIME MEMPOOL TEST COMPLETE');
}

async function startBackendNode() {
  console.log('   Starting backend node...');
  
  const nodeProcess = spawn('./target/release/pcl-node', [], {
    cwd: '/Users/34r7h/Documents/dev/pcl/backend',
    stdio: 'pipe'
  });
  
  nodeProcess.stdout.on('data', (data) => {
    console.log(`   Node: ${data.toString().trim()}`);
  });
  
  nodeProcess.stderr.on('data', (data) => {
    console.log(`   Node Error: ${data.toString().trim()}`);
  });
  
  return nodeProcess;
}

async function waitForNodeReady() {
  console.log('   Waiting for node to be ready...');
  
  for (let i = 0; i < 30; i++) {
    try {
      const { default: fetch } = await import('node-fetch');
      const response = await fetch('http://localhost:8080/health');
      if (response.ok) {
        console.log('   ✅ Node is ready');
        return;
      }
    } catch (error) {
      // Node not ready yet
    }
    await new Promise(resolve => setTimeout(resolve, 1000));
  }
  
  throw new Error('Node failed to start within 30 seconds');
}

async function openExtensionDashboard() {
  const browser = await puppeteer.launch({ 
    headless: false,
    args: ['--no-sandbox', '--disable-setuid-sandbox', '--disable-web-security'],
    defaultViewport: { width: 1200, height: 800 }
  });
  
  const page = await browser.newPage();
  
  // Monitor console logs
  page.on('console', msg => {
    if (msg.text().includes('XMBL') || msg.text().includes('mempool') || msg.text().includes('error')) {
      console.log(`   Extension: ${msg.text()}`);
    }
  });
  
  await page.goto('file://' + path.resolve(__dirname, 'fullscreen.html'));
  
  // Wait for page to load
  await page.waitForSelector('body', { timeout: 10000 });
  await new Promise(resolve => setTimeout(resolve, 3000));
  
  console.log('   ✅ Extension dashboard opened');
  
  return { browser, page };
}

async function testMempoolUpdates(page) {
  // Navigate to mempool tab
  console.log('   Switching to mempool tab...');
  const mempoolBtn = await page.$('button[data-view="mempool"]');
  if (mempoolBtn) {
    await mempoolBtn.click();
    await new Promise(resolve => setTimeout(resolve, 2000));
  }
  
  // Take initial screenshot
  await page.screenshot({ 
    path: path.join(__dirname, 'mempool-initial.png'), 
    fullPage: true 
  });
  console.log('   📸 Initial mempool state captured');
  
  // Submit multiple transactions and monitor changes
  console.log('   Submitting transactions and monitoring mempool changes...');
  
  for (let i = 0; i < 5; i++) {
    console.log(`   \n   Transaction ${i + 1}:`);
    
    // Submit transaction
    await submitTransaction(i);
    
    // Wait for mempool update
    await new Promise(resolve => setTimeout(resolve, 3000));
    
    // Check mempool state
    await checkMempoolState(page, i + 1);
    
    // Take screenshot
    await page.screenshot({ 
      path: path.join(__dirname, `mempool-after-tx-${i + 1}.png`), 
      fullPage: true 
    });
    console.log(`   📸 Mempool state after transaction ${i + 1} captured`);
  }
  
  // Final comprehensive check
  await comprehensiveMempoolCheck(page);
}

async function submitTransaction(index) {
  try {
    const { default: fetch } = await import('node-fetch');
    
    const txData = {
      from: 'faucet_address_123456789',
      to: `test_user_${index}_${Date.now()}`,
      amount: 10.0 + (index * 5),
      user: `user_${index}`,
      stake: 1.0,
      fee: 0.1
    };
    
    console.log(`     Submitting: ${txData.amount} XMBL to ${txData.to}`);
    
    const response = await fetch('http://localhost:8080/transaction', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(txData)
    });
    
    if (response.ok) {
      const result = await response.json();
      console.log(`     ✅ Transaction submitted: ${result.transaction_id}`);
    } else {
      console.log(`     ❌ Transaction failed: ${response.status}`);
    }
  } catch (error) {
    console.log(`     ❌ Transaction error: ${error.message}`);
  }
}

async function checkMempoolState(page, txNumber) {
  try {
    // Get mempool counts
    const rawTxCount = await page.$eval('#raw-tx-count', el => el.textContent).catch(() => '0');
    const processingTxCount = await page.$eval('#processing-tx-count', el => el.textContent).catch(() => '0');
    const validationTaskCount = await page.$eval('#validation-task-count', el => el.textContent).catch(() => '0');
    const lockedUtxoCount = await page.$eval('#locked-utxo-count', el => el.textContent).catch(() => '0');
    
    console.log(`     Mempool state after TX${txNumber}:`);
    console.log(`       Raw Transactions: ${rawTxCount}`);
    console.log(`       Processing Transactions: ${processingTxCount}`);
    console.log(`       Validation Tasks: ${validationTaskCount}`);
    console.log(`       Locked UTXOs: ${lockedUtxoCount}`);
    
    // Check for mempool sections
    const mempoolSections = await page.$$('.mempool-section');
    console.log(`       Mempool Sections Displayed: ${mempoolSections.length}/5`);
    
    if (mempoolSections.length > 0) {
      const sectionTitles = await page.$$eval('.mempool-title', titles => 
        titles.map(title => title.textContent)
      );
      console.log(`       Sections: ${sectionTitles.join(', ')}`);
    }
    
  } catch (error) {
    console.log(`     ❌ Error checking mempool state: ${error.message}`);
  }
}

async function comprehensiveMempoolCheck(page) {
  console.log('\n   Final Comprehensive Mempool Check:');
  
  try {
    // Get all 5 mempool sections
    const mempoolSections = await page.$$('.mempool-section');
    console.log(`   Total Mempool Sections: ${mempoolSections.length}`);
    
    // Expected sections based on README
    const expectedSections = [
      'Raw Transaction Mempool',
      'Validation Tasks Mempool', 
      'Locked UTXO Mempool',
      'Processing Transaction Mempool',
      'Transaction Mempool'
    ];
    
    for (const expectedSection of expectedSections) {
      const sectionExists = await page.$(`h4:contains("${expectedSection}")`);
      console.log(`   ${expectedSection}: ${sectionExists ? '✅ Found' : '❌ Missing'}`);
    }
    
    // Check if there's actual data in sections
    const sectionsWithData = await page.$$eval('.mempool-section', sections => {
      return sections.filter(section => {
        const dataElement = section.querySelector('.mempool-data');
        const emptyElement = section.querySelector('.mempool-empty');
        return dataElement && !emptyElement;
      }).length;
    });
    
    console.log(`   Sections with actual data: ${sectionsWithData}`);
    
    // Take final screenshot
    await page.screenshot({ 
      path: path.join(__dirname, 'mempool-final-comprehensive.png'), 
      fullPage: true 
    });
    console.log(`   📸 Final comprehensive screenshot saved`);
    
  } catch (error) {
    console.log(`   ❌ Error in comprehensive check: ${error.message}`);
  }
}

// Run the test
testRealTimeMempoolWorkflow().catch(console.error); 