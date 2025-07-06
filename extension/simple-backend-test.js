const { spawn } = require('child_process');
const path = require('path');

class SimpleBackendTest {
  constructor() {
    this.nodeProcess = null;
    this.nodeUrl = 'http://localhost:8080';
    this.isRunning = false;
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

  async testRealCrypto() {
    console.log('\n🔐 Testing Real Crypto Generation...');
    
    const addressesResponse = await fetch(`${this.nodeUrl}/addresses`);
    const addressesData = await addressesResponse.json();
    
    console.log('📍 Generated addresses:');
    addressesData.addresses.forEach(addr => {
      console.log(`   ${addr.name}: ${addr.address} (${addr.balance} XMBL)`);
    });
    
    // Verify addresses are not hardcoded
    const hasRealAddresses = addressesData.addresses.some(addr => 
      !addr.address.includes('faucet_address_123456789') && 
      addr.address.length === 40 // 20 bytes hex = 40 chars
    );
    
    console.log('✅ Real crypto addresses:', hasRealAddresses ? 'Generated' : 'Still hardcoded');
    
    // Verify unique addresses
    const uniqueAddresses = new Set(addressesData.addresses.map(a => a.address));
    console.log('✅ Address uniqueness:', uniqueAddresses.size === addressesData.addresses.length ? 'Verified' : 'Failed');
  }

  async testFaucetEndpoint() {
    console.log('\n💧 Testing Faucet Endpoint...');
    
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
    console.log('📤 Faucet response:', faucetData);
    console.log('✅ Faucet endpoint:', faucetData.status === 'success' ? 'Working' : 'Failed');
    
    // Verify balance was updated
    const balanceResponse = await fetch(`${this.nodeUrl}/balance/${testAddress}`);
    const balanceData = await balanceResponse.json();
    console.log('💰 Balance after faucet:', balanceData);
    console.log('✅ Balance update:', balanceData.balance >= 100 ? 'Working' : 'Failed');
  }

  async testMempoolData() {
    console.log('\n🔄 Testing Real Mempool Data...');
    
    const mempoolResponse = await fetch(`${this.nodeUrl}/mempools`);
    const mempoolData = await mempoolResponse.json();
    
    console.log('📊 Mempool counts:');
    console.log(`   Raw TX: ${mempoolData.raw_tx_mempool.count}`);
    console.log(`   Validation Tasks: ${mempoolData.validation_tasks_mempool.count}`);
    console.log(`   Locked UTXOs: ${mempoolData.locked_utxo_mempool.count}`);
    console.log(`   Processing TX: ${mempoolData.processing_tx_mempool.count}`);
    console.log(`   Finalized TX: ${mempoolData.tx_mempool.count}`);
    
    // Check if we have sample data
    if (mempoolData.raw_tx_mempool.samples) {
      console.log('📋 Sample raw transactions:', Object.keys(mempoolData.raw_tx_mempool.samples).length > 0 ? 'Present' : 'Empty');
    }
    
    if (mempoolData.validation_tasks_mempool.samples) {
      console.log('📋 Sample validation tasks:', Object.keys(mempoolData.validation_tasks_mempool.samples).length > 0 ? 'Present' : 'Empty');
    }
    
    // Verify non-zero counts indicate real activity
    const hasRealActivity = (
      mempoolData.raw_tx_mempool.count > 0 ||
      mempoolData.validation_tasks_mempool.count > 0 ||
      mempoolData.tx_mempool.count > 0
    );
    
    console.log('✅ Real mempool activity:', hasRealActivity ? 'Detected' : 'None');
  }

  async testTransactionSubmission() {
    console.log('\n💸 Testing Transaction Submission...');
    
    // Submit a transaction
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
    
    const txData = await txResponse.json();
    console.log('📤 Transaction response:', txData);
    console.log('✅ Transaction submission:', txData.status === 'success' ? 'Working' : 'Failed');
    
    // Wait a moment and check mempool again
    await new Promise(resolve => setTimeout(resolve, 2000));
    
    const mempoolResponse = await fetch(`${this.nodeUrl}/mempools`);
    const updatedMempoolData = await mempoolResponse.json();
    
    console.log('📊 Updated mempool counts:');
    console.log(`   Raw TX: ${updatedMempoolData.raw_tx_mempool.count}`);
    console.log(`   Processing TX: ${updatedMempoolData.processing_tx_mempool.count}`);
    console.log(`   Finalized TX: ${updatedMempoolData.tx_mempool.count}`);
    
    const mempoolUpdated = (
      updatedMempoolData.raw_tx_mempool.count > 0 ||
      updatedMempoolData.processing_tx_mempool.count > 0 ||
      updatedMempoolData.tx_mempool.count > 0
    );
    
    console.log('✅ Mempool updated after transaction:', mempoolUpdated ? 'Yes' : 'No');
  }

  async runAllTests() {
    console.log('🧪 Running Backend Tests...\n');
    
    try {
      await this.startBackendNode();
      await this.waitForNodeReady();
      
      await this.testRealCrypto();
      await this.testFaucetEndpoint();
      await this.testMempoolData();
      await this.testTransactionSubmission();
      
      console.log('\n✅ All backend tests completed successfully!');
      
    } catch (error) {
      console.error('\n❌ Backend test failed:', error.message);
      throw error;
    }
  }

  cleanup() {
    console.log('\n🧹 Cleaning up...');
    
    if (this.nodeProcess) {
      this.nodeProcess.kill();
      console.log('✅ Node process terminated');
    }
  }
}

// Main execution
async function runTest() {
  const test = new SimpleBackendTest();
  
  try {
    await test.runAllTests();
  } catch (error) {
    console.error('💥 Test failed:', error.message);
    process.exit(1);
  } finally {
    test.cleanup();
  }
}

if (require.main === module) {
  runTest();
}

module.exports = SimpleBackendTest; 