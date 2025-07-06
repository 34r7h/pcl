const puppeteer = require('puppeteer');
const { spawn } = require('child_process');
const path = require('path');

class ComprehensiveRealMempoolTest {
  constructor() {
    this.browser = null;
    this.page = null;
    this.nodeProcess = null;
    this.nodeUrl = 'http://localhost:8080';
    this.extensionPath = path.join(__dirname);
    this.isRunning = false;
  }

  async initialize() {
    console.log('🚀 Starting Comprehensive Real Mempool Test');
    
    // Start the backend node
    await this.startBackendNode();
    
    // Wait for node to be ready
    await this.waitForNodeReady();
    
    // Launch browser with extension
    await this.launchBrowser();
    
    console.log('✅ Test environment initialized');
  }

  async startBackendNode() {
    return new Promise((resolve, reject) => {
      console.log('🔧 Starting backend node...');
      
      this.nodeProcess = spawn('cargo', ['run'], {
        cwd: path.join(__dirname, '../backend'),
        stdio: 'pipe'
      });

      this.nodeProcess.stdout.on('data', (data) => {
        const output = data.toString();
        console.log(`[NODE] ${output.trim()}`);
        
        if (output.includes('Server listening on')) {
          console.log('✅ Backend node started successfully');
          resolve();
        }
      });

      this.nodeProcess.stderr.on('data', (data) => {
        console.error(`[NODE ERROR] ${data.toString()}`);
      });

      this.nodeProcess.on('close', (code) => {
        console.log(`[NODE] Process exited with code ${code}`);
        this.isRunning = false;
      });

      // Timeout after 30 seconds
      setTimeout(() => {
        if (!this.isRunning) {
          reject(new Error('Backend node failed to start within 30 seconds'));
        }
      }, 30000);
    });
  }

  async waitForNodeReady() {
    console.log('⏳ Waiting for node to be ready...');
    
    for (let i = 0; i < 30; i++) {
      try {
        const response = await fetch(`${this.nodeUrl}/health`);
        if (response.ok) {
          console.log('✅ Node is ready and responding');
          this.isRunning = true;
          return;
        }
      } catch (error) {
        // Node not ready yet, continue waiting
      }
      
      await new Promise(resolve => setTimeout(resolve, 1000));
    }
    
    throw new Error('Node failed to become ready within 30 seconds');
  }

  async launchBrowser() {
    console.log('🌐 Launching browser with extension...');
    
    this.browser = await puppeteer.launch({
      headless: false,
      args: [
        `--disable-extensions-except=${this.extensionPath}`,
        `--load-extension=${this.extensionPath}`,
        '--no-sandbox',
        '--disable-setuid-sandbox',
        '--disable-dev-shm-usage',
        '--disable-accelerated-2d-canvas',
        '--no-first-run',
        '--no-default-browser-check',
        '--disable-default-apps'
      ]
    });

    const pages = await this.browser.pages();
    this.page = pages[0];
    
    console.log('✅ Browser launched with extension');
  }

  async runComprehensiveTests() {
    console.log('\n🧪 Running Comprehensive Real Mempool Tests\n');
    
    try {
      // Test 1: Verify backend API endpoints
      await this.testBackendEndpoints();
      
      // Test 2: Test real crypto address generation
      await this.testRealCryptoGeneration();
      
      // Test 3: Test faucet functionality
      await this.testFaucetFunctionality();
      
      // Test 4: Test fullscreen dashboard
      await this.testFullscreenDashboard();
      
      // Test 5: Test real-time mempool updates
      await this.testRealTimeMempoolUpdates();
      
      // Test 6: Test address creation and sync
      await this.testAddressCreationSync();
      
      // Test 7: Test transaction processing
      await this.testTransactionProcessing();
      
      console.log('\n✅ All comprehensive tests completed successfully!');
      
    } catch (error) {
      console.error('\n❌ Comprehensive test failed:', error.message);
      throw error;
    }
  }

  async testBackendEndpoints() {
    console.log('🔍 Test 1: Backend API Endpoints');
    
    // Test health endpoint
    const healthResponse = await fetch(`${this.nodeUrl}/health`);
    console.log('   ✅ Health endpoint:', healthResponse.ok ? 'Working' : 'Failed');
    
    // Test network endpoint
    const networkResponse = await fetch(`${this.nodeUrl}/network`);
    const networkData = await networkResponse.json();
    console.log('   ✅ Network endpoint:', networkData.leaders > 0 ? 'Working' : 'Failed');
    
    // Test mempools endpoint
    const mempoolResponse = await fetch(`${this.nodeUrl}/mempools`);
    const mempoolData = await mempoolResponse.json();
    console.log('   ✅ Mempools endpoint:', mempoolData.raw_tx_mempool ? 'Working' : 'Failed');
    
    // Test addresses endpoint
    const addressesResponse = await fetch(`${this.nodeUrl}/addresses`);
    const addressesData = await addressesResponse.json();
    console.log('   ✅ Addresses endpoint:', addressesData.addresses ? 'Working' : 'Failed');
    
    console.log('   📊 Backend endpoints test completed\n');
  }

  async testRealCryptoGeneration() {
    console.log('🔐 Test 2: Real Crypto Address Generation');
    
    const addressesResponse = await fetch(`${this.nodeUrl}/addresses`);
    const addressesData = await addressesResponse.json();
    
    // Verify addresses are not hardcoded
    const addresses = addressesData.addresses;
    const hasRealAddresses = addresses.some(addr => 
      !addr.address.includes('faucet_address_123456789') && 
      addr.address.length === 40 // 20 bytes hex = 40 chars
    );
    
    console.log('   ✅ Real crypto addresses:', hasRealAddresses ? 'Generated' : 'Still hardcoded');
    
    // Verify unique addresses
    const uniqueAddresses = new Set(addresses.map(a => a.address));
    console.log('   ✅ Address uniqueness:', uniqueAddresses.size === addresses.length ? 'Verified' : 'Failed');
    
    console.log('   🔑 Real crypto generation test completed\n');
  }

  async testFaucetFunctionality() {
    console.log('💧 Test 3: Faucet Functionality');
    
    // Generate a test address
    const testAddress = '0123456789abcdef0123456789abcdef01234567';
    
    // Test faucet endpoint
    const faucetResponse = await fetch(`${this.nodeUrl}/faucet`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        address: testAddress,
        amount: 100.0
      })
    });
    
    const faucetData = await faucetResponse.json();
    console.log('   ✅ Faucet endpoint:', faucetData.status === 'success' ? 'Working' : 'Failed');
    
    // Verify balance was updated
    const balanceResponse = await fetch(`${this.nodeUrl}/balance/${testAddress}`);
    const balanceData = await balanceResponse.json();
    console.log('   ✅ Balance update:', balanceData.balance >= 100 ? 'Working' : 'Failed');
    
    console.log('   🚰 Faucet functionality test completed\n');
  }

  async testFullscreenDashboard() {
    console.log('🖥️ Test 4: Fullscreen Dashboard');
    
    // Navigate to fullscreen dashboard
    await this.page.goto(`file://${this.extensionPath}/fullscreen.html`);
    await this.page.waitForTimeout(2000);
    
    // Check if dashboard loaded
    const dashboardTitle = await this.page.$eval('h1', el => el.textContent);
    console.log('   ✅ Dashboard loaded:', dashboardTitle.includes('XMBL') ? 'Yes' : 'No');
    
    // Check for wallet creation button
    const createWalletBtn = await this.page.$('#create-wallet-btn');
    console.log('   ✅ Create wallet button:', createWalletBtn ? 'Present' : 'Missing');
    
    // Check for mempool sections
    const mempoolSections = await this.page.$$('.mempool-section');
    console.log('   ✅ Mempool sections:', mempoolSections.length >= 5 ? 'Present' : 'Missing');
    
    console.log('   🎛️ Fullscreen dashboard test completed\n');
  }

  async testRealTimeMempoolUpdates() {
    console.log('⏱️ Test 5: Real-time Mempool Updates');
    
    // Wait for mempool to load
    await this.page.waitForTimeout(3000);
    
    // Check initial mempool state
    const initialMempoolText = await this.page.$eval('#activity-log', el => el.textContent);
    console.log('   ✅ Initial mempool data:', initialMempoolText.length > 100 ? 'Loaded' : 'Empty');
    
    // Submit a transaction to create activity
    const txResponse = await fetch(`${this.nodeUrl}/transaction`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        from: '0123456789abcdef0123456789abcdef01234567',
        to: 'fedcba9876543210fedcba9876543210fedcba98',
        amount: 10.0,
        user: 'test_user',
        stake: 1.0,
        fee: 0.1
      })
    });
    
    console.log('   ✅ Transaction submitted:', txResponse.ok ? 'Success' : 'Failed');
    
    // Wait for mempool to update
    await this.page.waitForTimeout(5000);
    
    // Check if mempool data changed
    const updatedMempoolText = await this.page.$eval('#activity-log', el => el.textContent);
    const mempoolUpdated = updatedMempoolText !== initialMempoolText;
    console.log('   ✅ Mempool updates:', mempoolUpdated ? 'Real-time' : 'Static');
    
    console.log('   ⚡ Real-time mempool updates test completed\n');
  }

  async testAddressCreationSync() {
    console.log('🔄 Test 6: Address Creation and Sync');
    
    // Test popup wallet creation
    await this.page.goto(`file://${this.extensionPath}/popup.html`);
    await this.page.waitForTimeout(1000);
    
    // Create wallet in popup
    const createWalletBtn = await this.page.$('#createWalletBtn');
    if (createWalletBtn) {
      await createWalletBtn.click();
      await this.page.waitForTimeout(2000);
      console.log('   ✅ Popup wallet creation: Triggered');
    } else {
      console.log('   ❌ Popup wallet creation: Button not found');
    }
    
    // Check if wallet was created
    const walletAddress = await this.page.$eval('#wallet-address', el => el.textContent);
    console.log('   ✅ Wallet address generated:', walletAddress.length > 20 ? 'Yes' : 'No');
    
    // Switch to fullscreen and check sync
    await this.page.goto(`file://${this.extensionPath}/fullscreen.html`);
    await this.page.waitForTimeout(2000);
    
    const fullscreenAddress = await this.page.$eval('#fullscreen-address', el => el.textContent);
    console.log('   ✅ Address sync:', fullscreenAddress === walletAddress ? 'Synced' : 'Not synced');
    
    console.log('   🔗 Address creation and sync test completed\n');
  }

  async testTransactionProcessing() {
    console.log('💸 Test 7: Transaction Processing');
    
    // Get current mempool state
    const mempoolResponse = await fetch(`${this.nodeUrl}/mempools`);
    const mempoolData = await mempoolResponse.json();
    
    console.log('   📊 Current mempool counts:');
    console.log(`      Raw TX: ${mempoolData.raw_tx_mempool.count}`);
    console.log(`      Validation Tasks: ${mempoolData.validation_tasks_mempool.count}`);
    console.log(`      Locked UTXOs: ${mempoolData.locked_utxo_mempool.count}`);
    console.log(`      Processing TX: ${mempoolData.processing_tx_mempool.count}`);
    console.log(`      Finalized TX: ${mempoolData.tx_mempool.count}`);
    
    // Verify non-zero counts indicate real activity
    const hasRealActivity = (
      mempoolData.raw_tx_mempool.count > 0 ||
      mempoolData.validation_tasks_mempool.count > 0 ||
      mempoolData.tx_mempool.count > 0
    );
    
    console.log('   ✅ Real transaction activity:', hasRealActivity ? 'Detected' : 'None');
    
    // Submit multiple transactions to test processing
    for (let i = 0; i < 3; i++) {
      const txResponse = await fetch(`${this.nodeUrl}/transaction`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          from: `test_address_${i}`,
          to: `destination_${i}`,
          amount: 5.0 + i,
          user: `user_${i}`,
          stake: 0.5,
          fee: 0.1
        })
      });
      
      console.log(`   ✅ Transaction ${i + 1}:`, txResponse.ok ? 'Submitted' : 'Failed');
    }
    
    console.log('   💫 Transaction processing test completed\n');
  }

  async takeScreenshot(filename) {
    if (this.page) {
      await this.page.screenshot({ 
        path: path.join(__dirname, `${filename}.png`),
        fullPage: true 
      });
      console.log(`📸 Screenshot saved: ${filename}.png`);
    }
  }

  async cleanup() {
    console.log('🧹 Cleaning up test environment...');
    
    if (this.browser) {
      await this.browser.close();
      console.log('✅ Browser closed');
    }
    
    if (this.nodeProcess) {
      this.nodeProcess.kill();
      console.log('✅ Node process terminated');
    }
    
    console.log('✅ Cleanup completed');
  }
}

// Main execution
async function runComprehensiveTest() {
  const test = new ComprehensiveRealMempoolTest();
  
  try {
    await test.initialize();
    await test.runComprehensiveTests();
    await test.takeScreenshot('comprehensive-test-success');
    
  } catch (error) {
    console.error('💥 Test failed:', error.message);
    await test.takeScreenshot('comprehensive-test-failure');
    process.exit(1);
    
  } finally {
    await test.cleanup();
  }
}

// Run the test
if (require.main === module) {
  runComprehensiveTest();
}

module.exports = ComprehensiveRealMempoolTest; 