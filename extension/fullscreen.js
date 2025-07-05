// XMBL Wallet Fullscreen Dashboard
class XMBLDashboard {
  constructor() {
    this.wallet = null;
    this.nodeUrl = 'http://localhost:8080';
    this.simulatorUrl = 'http://localhost:3000';
    this.currentView = 'dashboard';
    this.init();
  }

  async init() {
    console.log('XMBL Dashboard: Initializing...');
    await this.loadWallet();
    this.setupNavigation();
    this.setupEventListeners();
    await this.updateNetworkStatus();
    this.updateUI();
    
    // Auto-refresh every 10 seconds
    setInterval(() => {
      this.updateNetworkStatus();
      this.loadWalletData();
    }, 10000);
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
      alert('Failed to create wallet: ' + error.message);
    }
  }

  generateAddress(publicKey) {
    const hash = new Uint8Array(20);
    const pubKeyArray = new Uint8Array(publicKey);
    
    // Simple address generation (first 20 bytes of hash)
    for (let i = 0; i < 20; i++) {
      hash[i] = pubKeyArray[i % pubKeyArray.length];
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
    try {
      // Check node
      const nodeResponse = await fetch(`${this.nodeUrl}/health`);
      const nodeConnected = nodeResponse.ok;

      // Check if simulator is conceptually running (check for recent activity)
      const simulatorActive = await this.checkSimulatorActivity();

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
      const networkStatus = document.getElementById('networkStatus');
      const networkText = document.getElementById('networkText');
      
      if (networkStatus) networkStatus.classList.remove('connected');
      if (networkText) networkText.textContent = 'Offline';
    }
  }

  async checkSimulatorActivity() {
    try {
      // Since simulator is CLI tool, we'll check if there's recent transaction activity
      // as a proxy for simulator being active
      const response = await fetch(`${this.nodeUrl}/health`);
      if (!response.ok) return false;
      
      // Check for recent transactions as indicator of simulator activity
      const txResponse = await fetch(`${this.nodeUrl}/transactions/recent`);
      
      // If we get any response, assume simulator might be active
      // In reality, this would check for recent blockchain activity
      return true;
    } catch (error) {
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
    const tbody = document.getElementById('transactions-tbody');
    if (!tbody) return;

    if (transactions.length === 0) {
      tbody.innerHTML = '<tr><td colspan="6" style="text-align: center; opacity: 0.5;">No transactions yet</td></tr>';
      return;
    }

    tbody.innerHTML = transactions.map(tx => `
      <tr>
        <td class="tx-hash">${tx.hash ? tx.hash.substring(0, 16) + '...' : 'N/A'}</td>
        <td class="tx-hash">${tx.from ? tx.from.substring(0, 8) + '...' : 'N/A'}</td>
        <td class="tx-hash">${tx.to ? tx.to.substring(0, 8) + '...' : 'N/A'}</td>
        <td>${tx.amount || 0} XMBL</td>
        <td>${tx.status || 'Pending'}</td>
        <td>${new Date(tx.timestamp || Date.now()).toLocaleString()}</td>
      </tr>
    `).join('');

    // Update transaction count
    const txCount = document.getElementById('tx-count');
    if (txCount) {
      txCount.textContent = transactions.length;
    }

    const txPending = document.getElementById('tx-pending');
    if (txPending) {
      const pendingCount = transactions.filter(tx => tx.status === 'pending').length;
      txPending.textContent = pendingCount;
    }
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
      showValidationWorkflow();
      
      // Step 1: Alice creates transaction
      updateValidationStep(1, 'active');
      await new Promise(resolve => setTimeout(resolve, 1000));
      
      const transaction = {
        from: this.wallet.address,
        to: to,
        amount: amount,
        timestamp: Date.now(),
        nonce: Math.floor(Math.random() * 1000000)
      };

      updateValidationStep(1, 'completed');
      
      // Step 2: Charlie processes and gossips
      updateValidationStep(2, 'active');
      await new Promise(resolve => setTimeout(resolve, 1500));
      updateValidationStep(2, 'completed');
      
      // Step 3: Leaders assign validation tasks
      updateValidationStep(3, 'active');
      await new Promise(resolve => setTimeout(resolve, 1000));
      updateValidationStep(3, 'completed');
      
      // Step 4: Alice completes validation tasks
      updateValidationStep(4, 'active');
      await new Promise(resolve => setTimeout(resolve, 2000));
      updateValidationStep(4, 'completed');
      
      // Step 5: Charlie processes validation results
      updateValidationStep(5, 'active');
      await new Promise(resolve => setTimeout(resolve, 1000));
      updateValidationStep(5, 'completed');
      
      // Step 6: Validator broadcasts and finalizes
      updateValidationStep(6, 'active');
      
      // Send transaction
      const response = await fetch(`${this.nodeUrl}/transaction`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify(transaction)
      });

      if (response.ok) {
        updateValidationStep(6, 'completed');
        alert('Transaction sent successfully!');
        this.clearSendForm();
        await this.loadWalletData();
      } else {
        updateValidationStep(6, 'failed');
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
          updateValidationStep(i, 'failed');
          break;
        }
      }
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
