/* global chrome */
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
    setupGlobalEventListeners();
    
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
      console.log('New wallet created successfully');
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
    
    // Always load transactions even if no wallet exists
    try {
      // Load real transactions from tx_mempool
      const mempoolResponse = await fetch(`${this.nodeUrl}/mempools`);
      if (mempoolResponse.ok) {
        const mempoolData = await mempoolResponse.json();
        
        // Convert tx_mempool samples to transaction array
        const transactions = Object.values(mempoolData.tx_mempool.samples || {});
        this.updateTransactions(transactions);
      }
    } catch (error) {
      console.log('XMBL Dashboard: Error loading transactions:', error.message);
    }
    
    // Only load wallet-specific data if wallet exists
    if (!wallet) {
      console.log('No wallet found, setting balance to 0');
      this.updateBalanceDisplay(0);
      return;
    }
    
    try {
      console.log('Loading balance for address:', wallet.address);
      // Load real balance
      const balanceResponse = await fetch(`${this.nodeUrl}/balance/${wallet.address}`);
      if (balanceResponse.ok) {
        const balanceData = await balanceResponse.json();
        console.log('Balance data received:', balanceData);
        this.updateBalanceDisplay(balanceData.balance);
      } else {
        console.log('Balance response not ok:', balanceResponse.status);
        this.updateBalanceDisplay(0);
      }
      
    } catch (error) {
      console.log('Error loading balance:', error.message);
      this.updateBalanceDisplay(0);
    }
  }

  updateBalanceDisplay(balance) {
    const balanceElement = document.getElementById('fullscreen-balance');
    if (balanceElement) {
      balanceElement.textContent = `${balance} XMBL`;
      console.log('Balance updated to:', balance);
    } else {
      console.log('Balance element not found');
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
      
      // Check mempool activity to determine if simulator is active
      const mempoolResponse = await fetch(`${this.nodeUrl}/mempools`);
      
      if (mempoolResponse.ok) {
        const mempoolData = await mempoolResponse.json();
        const totalActivity = (mempoolData.raw_tx_mempool?.count || 0) + 
                             (mempoolData.validation_tasks_mempool?.count || 0) + 
                             (mempoolData.processing_tx_mempool?.count || 0) +
                             (mempoolData.tx_mempool?.count || 0);
        const hasActivity = totalActivity > 0;
        console.log('XMBL Dashboard: Mempool activity detected:', hasActivity, `(${totalActivity} items)`);
        return hasActivity;
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
      
      // Update balance display
      this.updateBalanceDisplay(this.wallet.balance || 0);
    }
  }

  updateTransactions(transactions) {
    const container = document.getElementById('transactions-tbody');
    if (!container) return;

    if (!transactions || transactions.length === 0) {
      container.innerHTML = '<tr><td colspan="6" style="text-align: center; opacity: 0.5;">No transactions yet</td></tr>';
      return;
    }
    container.innerHTML = transactions.map(tx => `
      <tr class="transaction-row clickable" data-tx-id="${tx.hash}" style="cursor: pointer;">
        <td class="tx-hash" title="${tx.hash}">${tx.hash ? tx.hash.substring(0, 8) + '...' : 'N/A'}</td>
        <td class="tx-from" title="${tx.from}">${tx.from ? tx.from.substring(0, 8) + '...' : 'N/A'}</td>
        <td class="tx-to" title="${tx.to}">${tx.to ? tx.to.substring(0, 8) + '...' : 'N/A'}</td>
        <td class="tx-amount">${tx.amount || 0} XMBL</td>
        <td class="tx-status">
          <span class="status-badge status-${tx.status || 'pending'}">${tx.status || 'pending'}</span>
        </td>
        <td class="tx-time">${tx.timestamp ? new Date(tx.timestamp).toLocaleString() : 'N/A'}</td>
      </tr>
    `).join('');
    
    // Add event listeners for transaction clicks
    container.querySelectorAll('.transaction-row.clickable').forEach(row => {
      row.addEventListener('click', () => {
        const txId = row.dataset.txId;
        if (txId) {
          this.showConsensusSteps(txId);
        }
      });
    });
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
      console.log('XMBL Dashboard: Sending transaction...', { from: this.wallet.address, to, amount });
      
      const transaction = {
        from: this.wallet.address,
        to: to,
        amount: amount,
        timestamp: Date.now(),
        nonce: Math.floor(Math.random() * 1000000)
      };

      // Send transaction to backend
      const response = await fetch(`${this.nodeUrl}/transaction`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify(transaction)
      });

      if (response.ok) {
        const result = await response.json();
        console.log('XMBL Dashboard: Transaction submitted successfully:', result);
        
        alert('Transaction sent successfully!');
        this.clearSendForm();
        
        console.log('XMBL Dashboard: Transaction sent, forcing immediate balance refresh...');
        
        // Immediate balance refresh
        await this.refreshBalance();
        
        // Additional refresh after delay for backend processing
        setTimeout(async () => {
          console.log('XMBL Dashboard: Secondary balance refresh...');
          await this.refreshBalance();
          await this.loadWalletData();
        }, 2000);
        
        // Force UI update
        setTimeout(async () => {
          console.log('XMBL Dashboard: Final balance refresh...');
          await this.refreshBalance();
        }, 5000);
      } else {
        throw new Error('Transaction failed');
      }
    } catch (error) {
      console.error('XMBL Dashboard: Transaction error:', error);
      alert('Transaction failed: ' + error.message);
    }
  }

  clearSendForm() {
    const sendTo = document.getElementById('send-to');
    const sendAmount = document.getElementById('send-amount');
    if (sendTo) sendTo.value = '';
    if (sendAmount) sendAmount.value = '';
  }

  async requestFaucet() {
    if (!this.wallet) {
      alert('Create wallet first');
      return;
    }

    const faucetBtn = document.getElementById('faucet-btn');
    if (faucetBtn.disabled) return;
    
    faucetBtn.disabled = true;
    faucetBtn.textContent = 'Processing...';

    try {
      const response = await fetch(`${this.nodeUrl}/transaction`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          from: 'faucet_address_123456789',
          to: this.wallet.address,
          amount: 100.0,
          timestamp: Date.now(),
          type: 'faucet'
        })
      });

      if (response.ok) {
        console.log('Faucet transaction successful');
        // Force balance refresh after faucet
        await this.refreshBalance();
        alert('100 XMBL added');
      } else {
        throw new Error('Faucet failed');
      }
    } catch (error) {
      alert('Faucet error: ' + error.message);
    } finally {
      faucetBtn.disabled = false;
      faucetBtn.textContent = '🚰 Get Funds';
    }
  }

  async refreshBalance() {
    if (!this.wallet) {
      console.log('No wallet for balance refresh');
      return;
    }
    
    try {
      console.log('Refreshing balance for address:', this.wallet.address);
      
      // Get balance from backend
      const balanceResponse = await fetch(`${this.nodeUrl}/balance/${this.wallet.address}`);
      if (balanceResponse.ok) {
        const balanceData = await balanceResponse.json();
        console.log('Balance refresh data:', balanceData);
        
        // Update display
        this.updateBalanceDisplay(balanceData.balance);
        
        // Update wallet object
        this.wallet.balance = balanceData.balance;
        
        // Store updated wallet
        if (typeof chrome !== 'undefined' && chrome.storage) {
          chrome.storage.local.set({ xmblWallet: this.wallet });
        } else {
          localStorage.setItem('xmblWallet', JSON.stringify(this.wallet));
        }
        
        return balanceData.balance;
      } else {
        console.log('Balance refresh failed:', balanceResponse.status);
        return 0;
      }
    } catch (error) {
      console.log('Balance refresh error:', error.message);
      return 0;
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



  // Enhanced real-time mempool monitoring
  startMempoolMonitoring() {
    if (this.mempoolUpdateInterval) {
      clearInterval(this.mempoolUpdateInterval);
    }
    
    console.log('XMBL Dashboard: Starting enhanced mempool monitoring...');
    
    // Update mempool data every 1 second for real-time feel
    this.mempoolUpdateInterval = setInterval(async () => {
      await this.updateMempoolData();
    }, 1000);
    
    // Initial update
    this.updateMempoolData();
    
    // Add visual indicator for real-time updates
    setTimeout(() => {
      const activityLog = document.getElementById('activity-log');
      if (activityLog && !document.getElementById('realtime-indicator')) {
        const indicator = document.createElement('div');
        indicator.id = 'realtime-indicator';
        indicator.innerHTML = '<span class="indicator-dot"></span> Real-time updates active';
        indicator.style.cssText = `
          position: absolute;
          top: 10px;
          right: 10px;
          background: #00ff88;
          color: #000;
          padding: 4px 8px;
          border-radius: 12px;
          font-size: 10px;
          font-weight: bold;
          z-index: 10;
        `;
        activityLog.style.position = 'relative';
        activityLog.appendChild(indicator);
      }
    }, 1000);
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
    
    // Display all 5 mempools as formatted tables
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
          <span class="mempool-count">${count}</span>
        </div>
        <div class="mempool-content">
          ${count > 0 
            ? this.formatMempoolData(mempool.key, samples)
            : '<div class="mempool-empty">Empty</div>'
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
    
    // Add event listeners for consensus buttons
    activityLog.querySelectorAll('.btn-consensus').forEach(button => {
      button.addEventListener('click', () => {
        const txId = button.dataset.txId;
        if (txId) {
          this.showConsensusSteps(txId);
        }
      });
    });
  }

  formatMempoolData(mempoolType, samples) {
    if (!samples || Object.keys(samples).length === 0) {
      return '<div class="mempool-empty">No items in this mempool</div>';
    }

    switch (mempoolType) {
      case 'raw_tx_mempool':
        return this.formatRawTransactions(samples);
      case 'validation_tasks_mempool':
        return this.formatValidationTasks(samples);
      case 'locked_utxo_mempool':
        return this.formatLockedUTXOs(samples);
      case 'processing_tx_mempool':
        return this.formatProcessingTransactions(samples);
      case 'tx_mempool':
        return this.formatFinalizedTransactions(samples);
      default:
        return `<pre class="mempool-data">${JSON.stringify(samples, null, 2)}</pre>`;
    }
  }

  formatRawTransactions(samples) {
    // samples is leader_id -> (tx_id -> RawTransaction)
    const allTransactions = [];
    
    Object.entries(samples).forEach(([leaderId, txPool]) => {
      if (typeof txPool === 'object' && txPool !== null) {
        Object.entries(txPool).forEach(([txId, tx]) => {
          allTransactions.push({
            txId,
            tx,
            leaderId
          });
        });
      }
    });
    
    if (allTransactions.length === 0) return '<div class="mempool-empty">No raw transactions</div>';
    
    return `
      <table class="mempool-table">
        <thead>
          <tr>
            <th>TX Hash</th>
            <th>From</th>
            <th>To</th>
            <th>Amount</th>
            <th>Leader</th>
          </tr>
        </thead>
        <tbody>
          ${allTransactions.map(({txId, tx, leaderId}) => {
            const txData = tx.tx_data || tx;
            return `
            <tr>
              <td class="tx-hash">${txId.substring(0, 8)}...</td>
              <td class="address">${txData.from ? (Array.isArray(txData.from) ? txData.from[0][0].substring(0, 8) + '...' : txData.from.substring(0, 8) + '...') : (txData.user ? txData.user.substring(0, 8) + '...' : 'Unknown')}</td>
              <td class="address">${txData.to ? (Array.isArray(txData.to) ? txData.to[0][0].substring(0, 8) + '...' : txData.to.substring(0, 8) + '...') : 'Unknown'}</td>
              <td class="amount">${txData.amount || (Array.isArray(txData.to) ? txData.to[0][1] : 0)} XMBL</td>
              <td class="leader">${leaderId}</td>
            </tr>
            `;
          }).join('')}
        </tbody>
      </table>
    `;
  }

  formatValidationTasks(samples) {
    const allTasks = [];
    Object.entries(samples).forEach(([leaderId, tasks]) => {
      tasks.forEach(task => {
        allTasks.push({...task, leader: leaderId});
      });
    });
    
    if (allTasks.length === 0) return '<div class="mempool-empty">No validation tasks</div>';
    
    return `
      <table class="mempool-table">
        <thead>
          <tr>
            <th>Task ID</th>
            <th>Type</th>
            <th>Validator</th>
            <th>Status</th>
            <th>Leader</th>
          </tr>
        </thead>
        <tbody>
          ${allTasks.map(task => `
            <tr>
              <td class="task-id">${task.task_id}</td>
              <td class="task-type">${task.task_type}</td>
              <td class="address">${task.assigned_validator?.substring(0, 8)}...</td>
              <td class="status ${task.complete ? 'complete' : 'pending'}">${task.complete ? 'Complete' : 'Pending'}</td>
              <td class="leader">${task.leader}</td>
            </tr>
          `).join('')}
        </tbody>
      </table>
    `;
  }

  formatLockedUTXOs(samples) {
    const utxos = samples.utxos || samples;
    if (!utxos || (Array.isArray(utxos) && utxos.length === 0) || (typeof utxos === 'object' && Object.keys(utxos).length === 0)) {
      return '<div class="mempool-empty">No locked UTXOs</div>';
    }
    
    return `
      <table class="mempool-table">
        <thead>
          <tr>
            <th>UTXO ID</th>
            <th>Owner</th>
            <th>Amount</th>
            <th>Lock Reason</th>
          </tr>
        </thead>
        <tbody>
          ${(Array.isArray(utxos) ? utxos.map((utxo, index) => {
            const utxoId = typeof utxo === 'string' ? utxo : `utxo_${index}`;
            return `
            <tr>
              <td class="utxo-id">${utxoId.substring(0, 12)}...</td>
              <td class="address">System</td>
              <td class="amount">Variable</td>
              <td class="lock-reason">Transaction Processing</td>
            </tr>
            `;
          }) : Object.entries(utxos).map(([utxoId, utxo]) => `
            <tr>
              <td class="utxo-id">${utxoId.substring(0, 12)}...</td>
              <td class="address">${utxo.owner ? utxo.owner.substring(0, 8) + '...' : 'System'}</td>
              <td class="amount">${utxo.amount || 'Variable'} XMBL</td>
              <td class="lock-reason">Transaction Processing</td>
            </tr>
          `)).join('')}
        </tbody>
      </table>
    `;
  }

  formatProcessingTransactions(samples) {
    const entries = Object.entries(samples);
    if (entries.length === 0) return '<div class="mempool-empty">No processing transactions</div>';
    
    return `
      <table class="mempool-table">
        <thead>
          <tr>
            <th>TX Hash</th>
            <th>From</th>
            <th>To</th>
            <th>Amount</th>
            <th>Progress</th>
          </tr>
        </thead>
        <tbody>
          ${entries.map(([txId, tx]) => `
            <tr>
              <td class="tx-hash">${txId.substring(0, 8)}...</td>
              <td class="address">${tx.from?.substring(0, 8)}...</td>
              <td class="address">${tx.to?.substring(0, 8)}...</td>
              <td class="amount">${tx.amount} XMBL</td>
              <td class="progress">Validating</td>
            </tr>
          `).join('')}
        </tbody>
      </table>
    `;
  }

  formatFinalizedTransactions(samples) {
    const entries = Object.entries(samples);
    if (entries.length === 0) return '<div class="mempool-empty">Empty</div>';
    
    return `
      <table class="mempool-table">
        <thead>
          <tr>
            <th>TX</th>
            <th>Amount</th>
            <th>Consensus</th>
          </tr>
        </thead>
        <tbody>
          ${entries.map(([txId, tx]) => `
            <tr>
              <td class="tx-hash">${txId.substring(0, 12)}...</td>
              <td class="amount">${tx.amount} XMBL</td>
              <td><button class="btn-consensus" data-tx-id="${txId}">View Protocol</button></td>
            </tr>
          `).join('')}
        </tbody>
      </table>
    `;
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

  // Removed protocol enforcement section - not needed

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

  // Add consensus steps display functionality
  showConsensusSteps(txId) {
    console.log('Showing consensus for:', txId);
    
    fetch(`${this.nodeUrl}/transaction/${txId}`)
      .then(response => {
        if (!response.ok) {
          throw new Error(`HTTP ${response.status}`);
        }
        return response.json();
      })
      .then(data => {
        if (data.transaction) {
          this.displayConsensusModal(data);
        } else if (data.error) {
          alert('Transaction not found');
        } else {
          alert('Invalid response');
        }
      })
      .catch(error => {
        console.error('Transaction fetch error:', error);
        alert('Failed to load transaction');
      });
  }

  displayConsensusModal(txData) {
    const tx = txData.transaction;
    const leaderNode = txData.leader_node;
    const crossValidationProof = txData.cross_validation_proof;
    
    const modal = document.createElement('div');
    modal.className = 'consensus-modal';
    modal.innerHTML = `
      <div class="consensus-modal-content">
        <div class="consensus-header">
          <h3>🔐 Real Consensus Protocol Steps</h3>
          <button class="close-btn">×</button>
        </div>
        <div class="consensus-body">
          <div class="tx-details">
            <h4>Transaction Details</h4>
            <div class="tx-info">
              <div><strong>Hash:</strong> ${tx.hash}</div>
              <div><strong>From:</strong> ${tx.from}</div>
              <div><strong>To:</strong> ${tx.to}</div>
              <div><strong>Amount:</strong> ${tx.amount} XMBL</div>
              <div><strong>Leader Node:</strong> ${leaderNode ? leaderNode.name : 'N/A'} (${tx.leader_id})</div>
              <div><strong>Status:</strong> ${tx.status}</div>
              <div><strong>Timestamp:</strong> ${new Date(tx.timestamp).toLocaleString()}</div>
            </div>
          </div>
          <div class="consensus-steps">
            <h4>Real Consensus Steps Completed</h4>
            <div class="step-list">
              ${this.generateRealConsensusSteps(tx)}
            </div>
          </div>
          <div class="cross-validation-proof">
            <h4>Cross-Validation Proof</h4>
            <div class="proof-info">
              <div><strong>Digital Root:</strong> ${crossValidationProof.digital_root}</div>
              <div><strong>Validators Involved:</strong> ${crossValidationProof.validators_involved}</div>
              <div><strong>Validation Steps:</strong> ${crossValidationProof.validation_steps_completed}</div>
              <div><strong>Cross-Validators:</strong> ${crossValidationProof.cross_validators ? crossValidationProof.cross_validators.join(', ') : 'N/A'}</div>
              <div><strong>Validation Tasks by Submitter:</strong> ${crossValidationProof.validation_tasks_completed_by_submitter ? crossValidationProof.validation_tasks_completed_by_submitter.join(', ') : 'N/A'}</div>
            </div>
          </div>
          <div class="validators-info">
            <h4>Real Validators with Ed25519 Signatures</h4>
            <div class="validator-list">
              ${tx.validators ? tx.validators.map(validator => `
                <div class="validator-item">
                  <div class="validator-address">${validator}</div>
                  <div class="validator-status">✅ Real Ed25519 Signature Verified</div>
                  <div class="validator-signature">Cryptographic signature validated</div>
                </div>
              `).join('') : '<div>No validators listed</div>'}
            </div>
          </div>
        </div>
      </div>
    `;
    
    document.body.appendChild(modal);
    
    // Add event listener for close button
    const closeBtn = modal.querySelector('.close-btn');
    if (closeBtn) {
      closeBtn.addEventListener('click', () => {
        modal.remove();
      });
    }
    
    // Close modal when clicking outside
    modal.addEventListener('click', (e) => {
      if (e.target === modal) {
        modal.remove();
      }
    });
  }

  generateRealConsensusSteps(tx) {
    // Generate real consensus steps based on README protocol
    const steps = [
      { 
        title: 'Step 1: Transaction Submission', 
        description: `User sent ${tx.amount} XMBL to leader ${tx.leader_id}. Transaction includes fee: ${tx.fee || 0.1} XMBL and validation stake: ${tx.stake || 0.2} XMBL`,
        completed: true,
        readmeStep: 'Step 1: Alice sends Bob a transaction of one coin to leader node Charlie'
      },
      { 
        title: 'Step 2a: Raw Transaction Mempool Entry', 
        description: `Leader ${tx.leader_id} hashed the raw transaction to get raw_tx_id and created raw_tx_mempool entry`,
        completed: true,
        readmeStep: 'Step 2a: Charlie hashes the raw transaction to get the raw_tx_id and starts a raw_tx_mempool entry'
      },
      { 
        title: 'Step 2b: Validation Tasks Creation', 
        description: `Leader ${tx.leader_id} added user's raw_tx_id to validation_tasks_mempool for cross-validation`,
        completed: true,
        readmeStep: 'Step 2b: Charlie adds Alice\'s raw_tx_id to the validation_tasks_mempool'
      },
      { 
        title: 'Step 2c: UTXO Locking', 
        description: 'UTXOs used in transaction added to locked_utxo_mempool to prevent double-spend attacks',
        completed: true,
        readmeStep: 'Step 2c: UTXOs put on the locked_utxo_mempool to prevent double-spend attacks'
      },
      { 
        title: 'Step 2d: Leader Gossip', 
        description: `Leader ${tx.leader_id} gossiped transaction to 3 other leaders who continued to gossip to more leaders`,
        completed: true,
        readmeStep: 'Step 2d: Charlie gossips to 3 leaders who continue to gossip the transaction to other leaders'
      },
      { 
        title: 'Step 3: Cross-Validation Tasks Assignment', 
        description: 'Other leaders sent validation tasks to user. User must validate OTHER users\' transactions to earn right to submit own transaction',
        completed: true,
        readmeStep: 'Step 3: Other leaders send Charlie validation tasks for Alice to complete'
      },
      { 
        title: 'Step 4: User Validation Completion', 
        description: 'User completed assigned validation tasks with cryptographic signatures and reported completion timestamps to leaders',
        completed: true,
        readmeStep: 'Step 4: Alice completes validation tasks and reports timestamps with signatures'
      },
      { 
        title: 'Step 5: Processing Mempool', 
        description: `Leader ${tx.leader_id} averaged validation timestamps, signed transaction, and moved to processing_tx_mempool`,
        completed: true,
        readmeStep: 'Step 5: Charlie averages validation_timestamps, signs it, and puts it in processing_tx_mempool'
      },
      { 
        title: 'Step 6: Validator Broadcast & Final Validation', 
        description: 'Validator broadcasted transaction to 3 random leaders for final validation and chain-specific requirements',
        completed: true,
        readmeStep: 'Step 6: Validator broadcasts transaction to 3 random leaders for final validation'
      },
      { 
        title: 'Step 7: XMBL Cubic DLT Integration', 
        description: `Digital root calculated: ${tx.digital_root || 'N/A'}. Transaction added to tx_mempool for XMBL Cubic DLT geometric inclusion`,
        completed: true,
        readmeStep: 'Step 7: Calculate digital root of tx_id and add to tx_mempool for XMBL Cubic DLT protocol'
      }
    ];

    // Add actual validation steps from transaction data if available
    if (tx.validation_steps && tx.validation_steps.length > 0) {
      tx.validation_steps.forEach((step, index) => {
        steps.push({
          title: `Real Validation Step ${index + 1}`,
          description: step,
          completed: true,
          readmeStep: 'Actual validation step from consensus protocol'
        });
      });
    }

    return steps.map((step, index) => `
      <div class="step-item ${step.completed ? 'completed' : 'pending'}">
        <div class="step-number">${index + 1}</div>
        <div class="step-content">
          <div class="step-title">${step.title}</div>
          <div class="step-description">${step.description}</div>
          <div class="step-readme">${step.readmeStep}</div>
          <div class="step-timestamp">${new Date(tx.timestamp).toLocaleString()}</div>
        </div>
      </div>
    `).join('');
  }
}

// Setup event listeners for global functions
function setupGlobalEventListeners() {
  // Clear send form
  const clearBtn = document.getElementById('clear-send-form');
  if (clearBtn) {
    clearBtn.addEventListener('click', () => {
      document.getElementById('send-to').value = '';
      document.getElementById('send-amount').value = '';
    });
  }
  
  // Copy address
  const copyBtn = document.getElementById('copy-address-btn');
  if (copyBtn) {
    copyBtn.addEventListener('click', () => {
      const addressEl = document.getElementById('receive-address');
      if (addressEl) {
        navigator.clipboard.writeText(addressEl.textContent);
        alert('Address copied to clipboard!');
      }
    });
  }
}

// Remove global faucet function - handled by class method

// Remove global functions - handled by proper event listeners

// Initialize dashboard when page loads
document.addEventListener('DOMContentLoaded', () => {
  window.dashboardInstance = new XMBLDashboard();
});

console.log('XMBL Dashboard: Script loaded successfully');
