// XMBL Wallet Fullscreen Dashboard
class XMBLDashboard {
  constructor() {
    this.nodeUrl = 'http://localhost:8080';
    this.wallet = null;
    this.currentView = 'dashboard';
    this.simulatorUrl = 'http://localhost:3000';
    this.mempoolUpdateInterval = null;
    
    // Bind methods to preserve 'this' context
    this.getStoredWallet = this.getStoredWallet.bind(this);
    this.loadWalletData = this.loadWalletData.bind(this);
    this.createWallet = this.createWallet.bind(this);
    this.updateMempoolData = this.updateMempoolData.bind(this);
    this.displayMempoolActivity = this.displayMempoolActivity.bind(this);
    
    console.log('XMBL Dashboard: Initializing...');
    this.init();
  }

  async init() {
    console.log('XMBL Dashboard: Script loaded successfully');
    console.log('XMBL Dashboard: Initializing...');
    
    await this.loadWallet();
    this.setupNavigation();
    this.setupEventListeners();
    
    // Add address creation button to dashboard
    this.addAddressCreationButton();
    
    // Start real-time updates
    this.startMempoolMonitoring();
    this.startTestAddressGeneration();
    
    // Initial network status check
    await this.updateNetworkStatus();
    
    // Set up periodic updates
    setInterval(() => this.updateNetworkStatus(), 5000);
    setInterval(() => this.loadWalletData(), 10000);
    
    // Sync with popup wallet state every 3 seconds
    setInterval(() => this.syncWithPopupWallet(), 3000);
  }

  async loadWallet() {
    try {
      this.wallet = this.getStoredWallet();
      if (this.wallet) {
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

  getStoredWallet() {
    try {
      if (typeof chrome !== 'undefined' && chrome.storage) {
        // Chrome extension context
        return new Promise((resolve) => {
          chrome.storage.local.get(['xmblWallet'], (result) => {
            console.log('XMBL Dashboard: Retrieved wallet from storage:', result.xmblWallet ? 'Found' : 'Not found');
            resolve(result.xmblWallet || null);
          });
        });
      } else {
        // Standalone context
        const stored = localStorage.getItem('xmblWallet');
        const wallet = stored ? JSON.parse(stored) : null;
        console.log('XMBL Dashboard: Retrieved wallet from localStorage:', wallet ? 'Found' : 'Not found');
        return wallet;
      }
    } catch (error) {
      console.log('XMBL Dashboard: Error accessing storage:', error.message);
      return null;
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
      
      const address = this.generateAddress();
      const privateKey = this.generatePrivateKey();
      
      const wallet = {
        address: address,
        privateKey: privateKey,
        balance: 0,
        created: Date.now()
      };
      
      // Store wallet
      if (typeof chrome !== 'undefined' && chrome.storage) {
        chrome.storage.local.set({ xmblWallet: wallet });
      } else {
        localStorage.setItem('xmblWallet', JSON.stringify(wallet));
      }
      
      this.wallet = wallet;
      console.log('XMBL Dashboard: New wallet created successfully');
      this.updateUI();
      await this.loadWalletData();
    } catch (error) {
      console.error('XMBL Dashboard: Error creating wallet:', error);
      // No alert - just log the error
    }
  }

  generateAddress() {
    // Generate truly random address using crypto.getRandomValues
    const hash = new Uint8Array(20);
    crypto.getRandomValues(hash);
    
    // Add additional entropy from timestamp
    const timestamp = Date.now();
    for (let i = 0; i < 20; i++) {
      hash[i] ^= ((timestamp >> (i % 32)) & 0xff);
    }
    
    return Array.from(hash).map(b => b.toString(16).padStart(2, '0')).join('');
  }

  generatePrivateKey() {
    // Generate cryptographically secure private key
    const privateKey = new Uint8Array(32);
    crypto.getRandomValues(privateKey);
    return Array.from(privateKey).map(b => b.toString(16).padStart(2, '0')).join('');
  }

  async loadWalletData() {
    let wallet = this.wallet;
    
    // If wallet is a promise, await it
    if (wallet && typeof wallet.then === 'function') {
      wallet = await wallet;
    }
    
    // If still no wallet, try to get it directly
    if (!wallet) {
      wallet = await this.getStoredWallet();
    }
    
    if (!wallet) return;
    
    try {
      // Load real balance
      const balanceResponse = await fetch(`${this.nodeUrl}/balance/${wallet.address}`);
      if (balanceResponse.ok) {
        const balanceData = await balanceResponse.json();
        const balanceElement = document.getElementById('fullscreen-balance');
        if (balanceElement) {
          balanceElement.textContent = `${balanceData.balance} XMBL`;
        }
      }
      
      // Load real transactions from tx_mempool
      const mempoolResponse = await fetch(`${this.nodeUrl}/mempools`);
      if (mempoolResponse.ok) {
        const mempoolData = await mempoolResponse.json();
        
        // Convert tx_mempool samples to transaction array
        const transactions = Object.values(mempoolData.tx_mempool.samples || {});
        this.updateTransactions(transactions);
      }
      
    } catch (error) {
      console.log('XMBL Dashboard: Error loading wallet data:', error.message);
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
            <h4>Consensus Steps:</h4>
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

      // Send faucet request to backend using new faucet endpoint
      const response = await fetch(`${this.nodeUrl}/faucet`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          address: this.wallet.address,
          amount: 100.0
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

  // Real-time mempool monitoring - shows actual mempool objects
  async updateMempoolData() {
    try {
      const response = await fetch(`${this.nodeUrl}/mempools`);
      if (response.ok) {
        const data = await response.json();
        
        // Update mempool stats with real counts
        const rawTxCount = document.getElementById('raw-tx-count');
        const processingTxCount = document.getElementById('processing-tx-count');
        const validationTaskCount = document.getElementById('validation-task-count');
        const lockedUtxoCount = document.getElementById('locked-utxo-count');
        
        if (rawTxCount) rawTxCount.textContent = data.raw_tx_mempool.count;
        if (processingTxCount) processingTxCount.textContent = data.processing_tx_mempool.count;
        if (validationTaskCount) validationTaskCount.textContent = data.validation_tasks_mempool.count;
        if (lockedUtxoCount) lockedUtxoCount.textContent = data.locked_utxo_mempool.count;
        
        // Display actual mempool objects in activity log
        this.displayMempoolActivity(data);
        
      }
    } catch (error) {
      console.log('XMBL Dashboard: Mempool update failed:', error.message);
    }
  }

  displayMempoolActivity(mempoolData) {
    const activityLog = document.getElementById('activity-log');
    if (!activityLog) return;
    
    // Clear loading message
    activityLog.innerHTML = '';
    
    // Display all 5 mempools as described in README
    const mempoolOrder = [
      {
        key: 'raw_tx_mempool',
        title: 'Raw Transaction Mempool',
        description: 'First entries of tx requests',
        data: mempoolData.raw_tx_mempool
      },
      {
        key: 'validation_tasks_mempool',
        title: 'Validation Tasks Mempool',
        description: 'Validation tasks required to process transactions',
        data: mempoolData.validation_tasks_mempool
      },
      {
        key: 'locked_utxo_mempool',
        title: 'Locked UTXO Mempool',
        description: 'UTXOs invalidated from entry into raw_tx_mempool',
        data: mempoolData.locked_utxo_mempool
      },
      {
        key: 'processing_tx_mempool',
        title: 'Processing Transaction Mempool',
        description: 'Transactions moving through consensus',
        data: mempoolData.processing_tx_mempool
      },
      {
        key: 'tx_mempool',
        title: 'Transaction Mempool',
        description: 'Transactions approved to be blocked or finalized',
        data: mempoolData.tx_mempool
      }
    ];
    
    mempoolOrder.forEach(mempool => {
      const section = document.createElement('div');
      section.className = 'mempool-section';
      
      const count = mempool.data.count || 0;
      const samples = mempool.data.samples || mempool.data.utxos || mempool.data;
      
      section.innerHTML = `
        <div class="mempool-header">
          <h4 class="mempool-title">${mempool.title}</h4>
          <span class="mempool-count">${count} items</span>
        </div>
        <div class="mempool-description">${mempool.description}</div>
        <div class="mempool-content">
          ${count > 0 
            ? `<pre class="mempool-data">${JSON.stringify(samples, null, 2)}</pre>`
            : '<div class="mempool-empty">No items in this mempool</div>'
          }
        </div>
      `;
      
      activityLog.appendChild(section);
    });
    
    // Add timestamp
    const timestamp = document.createElement('div');
    timestamp.className = 'mempool-timestamp';
    timestamp.textContent = `Last updated: ${new Date().toLocaleTimeString()}`;
    activityLog.appendChild(timestamp);
  }

  // New: Dynamic test address generation from simulator
  async startTestAddressGeneration() {
    await this.generateLiveTestAddresses();
    // Regenerate every 30 seconds
    setInterval(() => this.generateLiveTestAddresses(), 30000);
  }

  async generateLiveTestAddresses() {
    try {
      // Get real addresses from actual mempool transactions
      const mempoolResponse = await fetch(`${this.nodeUrl}/mempools`);
      if (mempoolResponse.ok) {
        const mempoolData = await mempoolResponse.json();
        
        // Extract addresses from validation tasks and transactions
        const addresses = [];
        let addressIndex = 0;
        
        // Add faucet address as first test address
        addresses.push({
          name: 'Faucet',
          address: 'faucet_address_123456789',
          balance: 1000000
        });
        
        // Extract addresses from finalized transactions
        Object.values(mempoolData.tx_mempool.samples || {}).forEach(tx => {
          if (tx.to && !addresses.find(a => a.address === tx.to)) {
            const names = ['Alice', 'Bob', 'Charlie', 'Diana', 'Eve'];
            addresses.push({
              name: names[addressIndex % names.length],
              address: tx.to,
              balance: 0 // Will be updated by balance endpoint
            });
            addressIndex++;
          }
        });
        
        // Fill remaining with generated addresses if needed
        while (addresses.length < 4) {
          const names = ['Alice', 'Bob', 'Charlie', 'Diana', 'Eve'];
          const name = names[addressIndex % names.length];
          addresses.push({
            name: name,
            address: `test_user_${name.toLowerCase()}_${Date.now()}`,
            balance: Math.floor(Math.random() * 100)
          });
          addressIndex++;
        }
        
        this.updateLiveTestAddresses(addresses.slice(0, 4));
      }
      
    } catch (error) {
      console.log('XMBL Dashboard: Failed to generate test addresses:', error.message);
    }
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
    }
  }

  addAddressCreationButton() {
    const dashboardView = document.getElementById('dashboard-view');
    if (dashboardView && !document.getElementById('create-address-dashboard-btn')) {
      const createButton = document.createElement('button');
      createButton.id = 'create-address-dashboard-btn';
      createButton.textContent = 'Create New Address';
      createButton.style.cssText = `
        background: #4CAF50;
        color: white;
        border: none;
        padding: 12px 24px;
        border-radius: 6px;
        cursor: pointer;
        font-size: 16px;
        margin: 10px 0;
        display: block;
        width: 200px;
      `;
      
      createButton.addEventListener('click', () => {
        this.createWallet();
      });
      
      // Insert after the wallet info section
      const walletInfoSection = dashboardView.querySelector('.wallet-info');
      if (walletInfoSection) {
        walletInfoSection.appendChild(createButton);
      } else {
        dashboardView.appendChild(createButton);
      }
      
      console.log('XMBL Dashboard: Address creation button added to dashboard');
    }
  }

  async syncWithPopupWallet() {
    try {
      // Check if popup has created/updated wallet
      if (typeof chrome !== 'undefined' && chrome.storage) {
        chrome.storage.local.get(['xmblWallet'], (result) => {
          if (result.xmblWallet) {
            const storedWallet = result.xmblWallet;
            
            // Check if wallet has changed
            if (!this.wallet || this.wallet.address !== storedWallet.address) {
              console.log('XMBL Dashboard: Syncing with popup wallet');
              this.wallet = storedWallet;
              this.updateUI();
              this.loadWalletData();
            }
          }
        });
      } else {
        // Standalone context
        const stored = localStorage.getItem('xmblWallet');
        if (stored) {
          const storedWallet = JSON.parse(stored);
          
          // Check if wallet has changed
          if (!this.wallet || this.wallet.address !== storedWallet.address) {
            console.log('XMBL Dashboard: Syncing with localStorage wallet');
            this.wallet = storedWallet;
            this.updateUI();
            this.loadWalletData();
          }
        }
      }
    } catch (error) {
      console.log('XMBL Dashboard: Error syncing with popup wallet:', error.message);
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
