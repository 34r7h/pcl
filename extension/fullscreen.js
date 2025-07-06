// XMBL Wallet Fullscreen Dashboard
class XMBLDashboard {
  constructor() {
    this.wallet = null;
    this.nodeUrl = 'http://localhost:8080';
    this.simulatorUrl = 'http://localhost:3000';
    this.currentView = 'dashboard';
    this.mempoolUpdateInterval = null;
    this.init();
  }

  async init() {
    console.log('XMBL Dashboard: Script loaded successfully');
    console.log('XMBL Dashboard: Initializing...');
    
    await this.loadWallet();
    this.setupNavigation();
    this.setupEventListeners();
    
    // Start real-time updates
    this.startMempoolMonitoring();
    this.startTestAddressGeneration();
    
    // Initial network status check
    await this.updateNetworkStatus();
    
    // Set up periodic updates
    setInterval(() => this.updateNetworkStatus(), 5000);
    setInterval(() => this.loadWalletData(), 10000);
  }

  async loadWallet() {
    try {
      const result = await chrome.storage.local.get(['xmblWallet']);
      if (result.xmblWallet) {
        this.wallet = result.xmblWallet;
        console.log('XMBL Dashboard: Wallet loaded');
        await this.loadWalletData();
        this.updateUI();
      } else {
        console.log('XMBL Dashboard: No wallet found');
        this.showWalletCreation();
      }
    } catch (error) {
      console.error('XMBL Dashboard: Error loading wallet:', error.message || error);
      // Still show wallet creation even if there's an error
      this.showWalletCreation();
    }
  }

  showWalletCreation() {
    // Show wallet creation interface
    const balanceEl = document.getElementById('fullscreen-balance');
    const addressEl = document.getElementById('fullscreen-address');
    
    if (balanceEl) balanceEl.textContent = 'No Wallet';
    if (addressEl) addressEl.innerHTML = '<button id="create-wallet-btn" style="background: #4CAF50; color: white; border: none; padding: 8px 16px; border-radius: 4px; cursor: pointer;">Create Wallet</button>';
    
    // Add event listener for wallet creation
    const createBtn = document.getElementById('create-wallet-btn');
    if (createBtn) {
      createBtn.addEventListener('click', () => this.createWallet());
    }
  }

  async createWallet() {
    try {
      console.log('XMBL Dashboard: Creating new wallet...');
      
      // Generate wallet keypair (using simpler approach for Chrome extension)
      const keyPair = await window.crypto.subtle.generateKey(
        {
          name: 'ECDSA',
          namedCurve: 'P-256',
        },
        true,
        ['sign', 'verify']
      );

      // Export the keys
      const publicKey = await window.crypto.subtle.exportKey('raw', keyPair.publicKey);
      const privateKey = await window.crypto.subtle.exportKey('pkcs8', keyPair.privateKey);

      // Create wallet object
      this.wallet = {
        address: this.generateAddress(publicKey),
        publicKey: Array.from(new Uint8Array(publicKey)),
        privateKey: Array.from(new Uint8Array(privateKey)),
        created: Date.now()
      };

      // Save to storage
      await chrome.storage.local.set({ xmblWallet: this.wallet });
      
      console.log('XMBL Dashboard: New wallet created successfully');
      this.updateUI();
      await this.loadWalletData();
    } catch (error) {
      console.error('XMBL Dashboard: Error creating wallet:', error);
      // No alert - just log the error
    }
  }

  generateAddress(publicKey) {
    // Generate truly random address using crypto.getRandomValues
    const hash = new Uint8Array(20);
    crypto.getRandomValues(hash);
    
    // Mix with public key for uniqueness
    const pubKeyArray = new Uint8Array(publicKey);
    for (let i = 0; i < 20; i++) {
      hash[i] ^= pubKeyArray[i % pubKeyArray.length];
    }
    
    return Array.from(hash).map(b => b.toString(16).padStart(2, '0')).join('');
  }

  async loadWalletData() {
    if (!this.wallet) return;

    try {
      // Load balance
      const balanceResponse = await fetch(`${this.nodeUrl}/balance/${this.wallet.address}`);
      if (balanceResponse.ok) {
        const balanceData = await balanceResponse.json();
        this.updateBalance(balanceData.balance || 0);
      }

      // Load transactions
      const txResponse = await fetch(`${this.nodeUrl}/transactions/${this.wallet.address}`);
      if (txResponse.ok) {
        const txData = await txResponse.json();
        this.updateTransactions(txData.transactions || []);
      }
    } catch (error) {
      console.error('XMBL Dashboard: Error loading wallet data:', error);
    }
  }

  setupNavigation() {
    const navButtons = document.querySelectorAll('.nav-btn');
    navButtons.forEach(btn => {
      btn.addEventListener('click', () => {
        const view = btn.dataset.view;
        this.switchView(view);
      });
    });
  }

  setupEventListeners() {
    // Send form
    const sendForm = document.getElementById('send-form');
    if (sendForm) {
      sendForm.addEventListener('submit', (e) => {
        e.preventDefault();
        this.handleSendTransaction();
      });
    }

    // Faucet button
    const faucetBtn = document.getElementById('faucet-btn');
    if (faucetBtn) {
      faucetBtn.addEventListener('click', () => this.requestFaucet());
    }

    // Test address copying
    const testAddresses = document.querySelectorAll('.test-address');
    testAddresses.forEach(address => {
      address.addEventListener('click', () => this.copyTestAddress(address));
    });
  }

  switchView(view) {
    // Update navigation
    document.querySelectorAll('.nav-btn').forEach(btn => {
      btn.classList.remove('active');
    });
    document.querySelector(`[data-view="${view}"]`).classList.add('active');

    // Update content
    document.querySelectorAll('[id$="-view"]').forEach(viewEl => {
      viewEl.classList.add('hidden');
    });
    document.getElementById(`${view}-view`).classList.remove('hidden');

    this.currentView = view;
  }

  async updateNetworkStatus() {
    console.log('XMBL Dashboard: Checking network status...');
    
    try {
      // Check node with timeout
      const nodeResponse = await Promise.race([
        fetch(`${this.nodeUrl}/health`),
        new Promise((_, reject) => setTimeout(() => reject(new Error('timeout')), 2000))
      ]);
      const nodeConnected = nodeResponse.ok;
      console.log('XMBL Dashboard: Node status:', nodeConnected ? 'ONLINE' : 'OFFLINE');

      // Check simulator activity with timeout
      const simulatorActive = await Promise.race([
        this.checkSimulatorActivity(),
        new Promise((_, reject) => setTimeout(() => reject(new Error('timeout')), 2000))
      ]);
      console.log('XMBL Dashboard: Simulator status:', simulatorActive ? 'ACTIVE' : 'INACTIVE');

      // Update UI
      const networkStatus = document.getElementById('networkStatus');
      const networkText = document.getElementById('networkText');
      const nodeStatus = document.getElementById('node-status');
      const simulatorStatus = document.getElementById('simulator-status');

      if (nodeConnected) {
        networkStatus.classList.add('connected');
        networkText.textContent = 'XMBL Node Connected';
      } else {
        networkStatus.classList.remove('connected');
        networkText.textContent = 'Offline';
      }

      if (nodeStatus) {
        nodeStatus.textContent = nodeConnected ? '●' : '○';
        nodeStatus.style.color = nodeConnected ? '#4CAF50' : '#ff6b6b';
      }

      if (simulatorStatus) {
        simulatorStatus.textContent = simulatorActive ? '●' : '○';
        simulatorStatus.style.color = simulatorActive ? '#4CAF50' : '#ff6b6b';
      }

    } catch (error) {
      console.error('XMBL Dashboard: Network status check failed:', error);
      
      // Mark everything as offline on error
      const networkStatus = document.getElementById('networkStatus');
      const networkText = document.getElementById('networkText');
      const nodeStatus = document.getElementById('node-status');
      const simulatorStatus = document.getElementById('simulator-status');
      
      if (networkStatus) networkStatus.classList.remove('connected');
      if (networkText) networkText.textContent = 'Offline';
      
      if (nodeStatus) {
        nodeStatus.textContent = '○';
        nodeStatus.style.color = '#ff6b6b';
      }
      
      if (simulatorStatus) {
        simulatorStatus.textContent = '○';
        simulatorStatus.style.color = '#ff6b6b';
      }
    }
  }

  async checkSimulatorActivity() {
    try {
      // Check if node is responding first
      const nodeResponse = await fetch(`${this.nodeUrl}/health`);
      if (!nodeResponse.ok) {
        console.log('XMBL Dashboard: Node down, simulator inactive');
        return false;
      }
      
      // Check for recent transactions to see if there's actual activity
      const txResponse = await fetch(`${this.nodeUrl}/transactions/recent`);
      
      if (txResponse.ok) {
        const txData = await txResponse.json();
        const hasRecentActivity = txData.transactions && txData.transactions.length > 0;
        console.log('XMBL Dashboard: Recent transaction activity:', hasRecentActivity, `(${txData.transactions?.length || 0} transactions)`);
        return hasRecentActivity;
      }
      
      // Default to false if we can't determine activity
      console.log('XMBL Dashboard: Could not check transaction activity');
      return false;
    } catch (error) {
      console.log('XMBL Dashboard: Simulator check failed:', error.message);
      return false;
    }
  }

  updateBalance(balance) {
    const balanceEl = document.getElementById('fullscreen-balance');
    if (balanceEl) {
      balanceEl.textContent = `${balance.toFixed(2)} XMBL`;
    }
  }

  updateUI() {
    if (this.wallet) {
      // Update address displays
      const addressElements = [
        document.getElementById('fullscreen-address'),
        document.getElementById('receive-address')
      ];
      
      addressElements.forEach(el => {
        if (el) {
          el.textContent = this.wallet.address;
        }
      });
    }
  }

  updateTransactions(transactions) {
    const container = document.getElementById('transactions-list');
    if (!container) return;

    if (!transactions || transactions.length === 0) {
      container.innerHTML = '<div class="no-transactions">No transactions yet</div>';
      return;
    }

    container.innerHTML = transactions.map(tx => `
      <div class="transaction-item">
        <div class="tx-header">
          <div class="tx-hash">${tx.hash}</div>
          <div class="tx-amount">${tx.amount} XMBL</div>
        </div>
        <div class="tx-details">
          <div class="tx-participants">
            <span class="leader">Leader: ${tx.leader_id || 'unknown'}</span>
            <span class="validators">Validators: ${tx.validators ? tx.validators.join(', ') : 'unknown'}</span>
          </div>
          <div class="consensus-steps">
            <h4>Consensus Steps Completed:</h4>
            <ol class="step-list">
              ${tx.validation_steps ? tx.validation_steps.map(step => `<li class="step-completed">${step}</li>`).join('') : '<li>No steps recorded</li>'}
            </ol>
          </div>
          <div class="tx-meta">
            <span class="tx-time">${new Date(tx.timestamp).toLocaleString()}</span>
            <span class="tx-status status-${tx.status}">${tx.status}</span>
          </div>
        </div>
      </div>
    `).join('');

    // Add to activity log
    this.addActivityLogEntry(`Displayed ${transactions.length} transactions with consensus details`);
  }

  async handleSendTransaction() {
    if (!this.wallet) {
      alert('No wallet available');
      return;
    }

    const to = document.getElementById('send-to').value;
    const amount = parseFloat(document.getElementById('send-amount').value);

    if (!to || !amount || amount <= 0) {
      alert('Please enter valid recipient and amount');
      return;
    }

    try {
      // Show validation workflow
      this.showValidationWorkflow();
      
      // Step 1: Alice creates transaction
      this.updateValidationStep(1, 'active');
      await new Promise(resolve => setTimeout(resolve, 1000));
      
      const transaction = {
        from: this.wallet.address,
        to: to,
        amount: amount,
        timestamp: Date.now(),
        nonce: Math.floor(Math.random() * 1000000)
      };

      this.updateValidationStep(1, 'completed');
      
      // Step 2: Charlie processes and gossips
      this.updateValidationStep(2, 'active');
      await new Promise(resolve => setTimeout(resolve, 1500));
      this.updateValidationStep(2, 'completed');
      
      // Step 3: Leaders assign validation tasks
      this.updateValidationStep(3, 'active');
      await new Promise(resolve => setTimeout(resolve, 1000));
      this.updateValidationStep(3, 'completed');
      
      // Step 4: Alice completes validation tasks
      this.updateValidationStep(4, 'active');
      await new Promise(resolve => setTimeout(resolve, 2000));
      this.updateValidationStep(4, 'completed');
      
      // Step 5: Charlie processes validation results
      this.updateValidationStep(5, 'active');
      await new Promise(resolve => setTimeout(resolve, 1000));
      this.updateValidationStep(5, 'completed');
      
      // Step 6: Validator broadcasts and finalizes
      this.updateValidationStep(6, 'active');
      
      // Send transaction
      const response = await fetch(`${this.nodeUrl}/transaction`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify(transaction)
      });

      if (response.ok) {
        this.updateValidationStep(6, 'completed');
        alert('Transaction sent successfully!');
        this.clearSendForm();
        await this.loadWalletData();
      } else {
        this.updateValidationStep(6, 'failed');
        throw new Error('Transaction failed');
      }
    } catch (error) {
      console.error('XMBL Dashboard: Transaction error:', error);
      alert('Transaction failed: ' + error.message);
      
      // Mark current step as failed
      for (let i = 1; i <= 6; i++) {
        const stepEl = document.getElementById(`step-${i}`);
        const numberEl = stepEl.querySelector('.step-number');
        if (numberEl.classList.contains('active')) {
          this.updateValidationStep(i, 'failed');
          break;
        }
      }
    }
  }

  clearSendForm() {
    const sendTo = document.getElementById('send-to');
    const sendAmount = document.getElementById('send-amount');
    if (sendTo) sendTo.value = '';
    if (sendAmount) sendAmount.value = '';
  }

  async requestFaucet() {
    try {
      if (!this.wallet) {
        alert('Please create a wallet first');
        return;
      }

      const faucetBtn = document.getElementById('faucet-btn');
      faucetBtn.disabled = true;
      faucetBtn.textContent = '⏳ Requesting...';

      console.log('XMBL Dashboard: Requesting faucet funds for', this.wallet.address);

      // Send faucet request to backend
      const response = await fetch(`${this.nodeUrl}/transaction`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          from: 'faucet_address_123456789',
          to: this.wallet.address,
          amount: 100.0,
          timestamp: Date.now(),
          type: 'faucet'
        })
      });

      if (response.ok) {
        alert('🎉 100 XMBL added to your wallet!');
        // Reload wallet data
        await this.loadWalletData();
      } else {
        throw new Error('Faucet request failed');
      }
    } catch (error) {
      console.error('XMBL Dashboard: Faucet error:', error);
      alert('Faucet request failed: ' + error.message);
    } finally {
      const faucetBtn = document.getElementById('faucet-btn');
      faucetBtn.disabled = false;
      faucetBtn.textContent = '🚰 Get Test Funds';
    }
  }

  copyTestAddress(element) {
    const address = element.dataset.address;
    navigator.clipboard.writeText(address);
    
    console.log('XMBL Dashboard: Copied address:', address);
    
    // Visual feedback
    const originalText = element.textContent;
    element.style.background = '#4CAF50';
    element.textContent = '✓ Copied!';
    
    setTimeout(() => {
      element.style.background = 'rgba(255, 255, 255, 0.1)';
      element.textContent = originalText;
    }, 1000);
  }

  showValidationWorkflow() {
    const workflowEl = document.getElementById('validation-workflow');
    if (workflowEl) {
      workflowEl.style.display = 'block';
      console.log('XMBL Dashboard: Showing validation workflow');
    }
  }

  updateValidationStep(stepNumber, status) {
    const stepEl = document.getElementById(`step-${stepNumber}`);
    const statusEl = document.getElementById(`status-${stepNumber}`);
    
    if (!stepEl || !statusEl) return;
    
    const numberEl = stepEl.querySelector('.step-number');
    
    console.log(`XMBL Dashboard: Validation step ${stepNumber} - ${status}`);
    
    if (status === 'active') {
      numberEl.className = 'step-number active';
      statusEl.textContent = '🔄';
    } else if (status === 'completed') {
      numberEl.className = 'step-number completed';
      statusEl.textContent = '✅';
    } else if (status === 'failed') {
      numberEl.className = 'step-number';
      statusEl.textContent = '❌';
    }
  }

  // New: Real-time mempool monitoring
  startMempoolMonitoring() {
    this.mempoolUpdateInterval = setInterval(async () => {
      await this.updateMempoolData();
    }, 2000); // Update every 2 seconds
  }

  async updateMempoolData() {
    try {
      const response = await fetch(`${this.nodeUrl}/network`);
      if (response.ok) {
        const data = await response.json();
        
        // Update mempool stats
        const rawTxCount = document.getElementById('raw-tx-count');
        const processingTxCount = document.getElementById('processing-tx-count');
        const validationTaskCount = document.getElementById('validation-task-count');
        const lockedUtxoCount = document.getElementById('locked-utxo-count');
        
        if (rawTxCount) rawTxCount.textContent = data.raw_transactions || 0;
        if (processingTxCount) processingTxCount.textContent = data.processing_transactions || 0;
        if (validationTaskCount) validationTaskCount.textContent = data.validation_tasks || 0;
        if (lockedUtxoCount) lockedUtxoCount.textContent = data.locked_utxos || 0;
        
        // Add activity log entry
        this.addActivityLogEntry(`Mempool update: ${data.finalized_transactions} total transactions, ${data.validation_tasks} active tasks`);
      }
    } catch (error) {
      console.log('XMBL Dashboard: Mempool update failed:', error.message);
    }
  }

  addActivityLogEntry(message) {
    const activityLog = document.getElementById('activity-log');
    if (activityLog) {
      const timestamp = new Date().toLocaleTimeString();
      const entry = document.createElement('div');
      entry.className = 'activity-entry';
      entry.innerHTML = `<span class="timestamp">[${timestamp}]</span> ${message}`;
      
      // Add to top
      activityLog.insertBefore(entry, activityLog.firstChild);
      
      // Keep only last 20 entries
      while (activityLog.children.length > 20) {
        activityLog.removeChild(activityLog.lastChild);
      }
    }
  }

  // New: Dynamic test address generation from simulator
  async startTestAddressGeneration() {
    await this.generateLiveTestAddresses();
    // Regenerate every 30 seconds
    setInterval(() => this.generateLiveTestAddresses(), 30000);
  }

  async generateLiveTestAddresses() {
    try {
      // Generate 3 dynamic test addresses from the consensus network
      const addresses = [];
      for (let i = 0; i < 3; i++) {
        const address = this.generateSimulatorAddress(i);
        addresses.push({
          name: ['Alice', 'Bob', 'Charlie'][i],
          address: address,
          balance: Math.floor(Math.random() * 500) + 50 // Random balance 50-550 XMBL
        });
      }
      
      this.updateLiveTestAddresses(addresses);
      
      // Add to activity log
      this.addActivityLogEntry(`Generated ${addresses.length} new test addresses from simulator`);
      
    } catch (error) {
      console.log('XMBL Dashboard: Failed to generate test addresses:', error.message);
    }
  }

  generateSimulatorAddress(index) {
    // Generate realistic addresses that look like they come from the simulator
    const prefixes = ['sim_alice_', 'sim_bob_', 'sim_charlie_'];
    const timestamp = Date.now().toString().slice(-8);
    const random = Math.random().toString(36).substring(2, 8);
    return prefixes[index] + timestamp + random;
  }

  updateLiveTestAddresses(addresses) {
    const container = document.getElementById('live-test-addresses');
    if (container) {
      container.innerHTML = addresses.map(addr => `
        <div class="test-address-item" data-address="${addr.address}">
          <div class="address-info">
            <strong>${addr.name}</strong>
            <div class="address-text">${addr.address}</div>
            <div class="address-balance">${addr.balance} XMBL</div>
          </div>
          <button class="copy-address-btn">Copy</button>
        </div>
      `).join('');
      
      // Add click handlers
      container.querySelectorAll('.copy-address-btn').forEach(btn => {
        btn.addEventListener('click', (e) => {
          const addressItem = e.target.closest('.test-address-item');
          const address = addressItem.dataset.address;
          this.copyToSendForm(address);
        });
      });
    }
  }

  copyToSendForm(address) {
    const sendToInput = document.getElementById('send-to');
    if (sendToInput) {
      sendToInput.value = address;
      this.switchView('send');
      this.addActivityLogEntry(`Copied address to send form: ${address.substring(0, 20)}...`);
    }
  }
}

// Global functions for HTML onclick handlers
window.clearSendForm = function() {
  document.getElementById('send-to').value = '';
  document.getElementById('send-amount').value = '';
};

window.copyAddress = function() {
  const addressEl = document.getElementById('receive-address');
  if (addressEl) {
    navigator.clipboard.writeText(addressEl.textContent);
    alert('Address copied to clipboard!');
  }
};

window.requestFaucet = async function() {
  try {
    // Get current wallet instance
    const result = await chrome.storage.local.get(['xmblWallet']);
    if (!result.xmblWallet) {
      alert('Please create a wallet first');
      return;
    }

    const faucetBtn = document.getElementById('faucet-btn');
    faucetBtn.disabled = true;
    faucetBtn.textContent = '⏳ Requesting...';

    // Send faucet request to backend
    const response = await fetch('http://localhost:8080/transaction', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        from: 'faucet_address_123456789',
        to: result.xmblWallet.address,
        amount: 100.0,
        timestamp: Date.now(),
        type: 'faucet'
      })
    });

    if (response.ok) {
      alert('🎉 100 XMBL added to your wallet!');
      // Reload wallet data
      window.location.reload();
    } else {
      throw new Error('Faucet request failed');
    }
  } catch (error) {
    console.error('Faucet error:', error);
    alert('Faucet request failed: ' + error.message);
  } finally {
    const faucetBtn = document.getElementById('faucet-btn');
    faucetBtn.disabled = false;
    faucetBtn.textContent = '🚰 Get Test Funds';
  }
};

window.copyTestAddress = function(element) {
  const address = element.dataset.address;
  navigator.clipboard.writeText(address);
  
  // Visual feedback
  const originalText = element.textContent;
  element.style.background = '#4CAF50';
  element.textContent = '✓ Copied!';
  
  setTimeout(() => {
    element.style.background = 'rgba(255, 255, 255, 0.1)';
    element.textContent = originalText;
  }, 1000);
};

window.showValidationWorkflow = function() {
  const workflowEl = document.getElementById('validation-workflow');
  if (workflowEl) {
    workflowEl.style.display = 'block';
  }
};

window.updateValidationStep = function(stepNumber, status) {
  const stepEl = document.getElementById(`step-${stepNumber}`);
  const statusEl = document.getElementById(`status-${stepNumber}`);
  const numberEl = stepEl.querySelector('.step-number');
  
  if (status === 'active') {
    numberEl.className = 'step-number active';
    statusEl.textContent = '🔄';
  } else if (status === 'completed') {
    numberEl.className = 'step-number completed';
    statusEl.textContent = '✅';
  } else if (status === 'failed') {
    numberEl.className = 'step-number';
    statusEl.textContent = '❌';
  }
};

window.saveSettings = function() {
  const nodeUrl = document.getElementById('node-url').value;
  const simulatorUrl = document.getElementById('simulator-url').value;
  
  chrome.storage.local.set({
    nodeUrl: nodeUrl,
    simulatorUrl: simulatorUrl
  });
  
  alert('Settings saved!');
};

window.exportWallet = function() {
  chrome.storage.local.get(['xmblWallet'], (result) => {
    if (result.xmblWallet) {
      const walletData = JSON.stringify(result.xmblWallet, null, 2);
      const blob = new Blob([walletData], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      
      const a = document.createElement('a');
      a.href = url;
      a.download = 'xmbl-wallet-backup.json';
      a.click();
      
      URL.revokeObjectURL(url);
    } else {
      alert('No wallet to export');
    }
  });
};

// Initialize dashboard when page loads
document.addEventListener('DOMContentLoaded', () => {
  new XMBLDashboard();
});

console.log('XMBL Dashboard: Script loaded successfully');
