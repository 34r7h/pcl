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
    console.log('PCL Dashboard: Initializing...');
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
      const result = await chrome.storage.local.get(['pclWallet']);
      if (result.pclWallet) {
        this.wallet = result.pclWallet;
        console.log('PCL Dashboard: Wallet loaded');
        await this.loadWalletData();
      } else {
        console.log('PCL Dashboard: No wallet found');
      }
    } catch (error) {
      console.error('PCL Dashboard: Error loading wallet:', error);
    }
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
      console.error('PCL Dashboard: Error loading wallet data:', error);
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

      // Update UI
      const networkStatus = document.getElementById('networkStatus');
      const networkText = document.getElementById('networkText');
      const nodeStatus = document.getElementById('node-status');

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

    } catch (error) {
      console.error('XMBL Dashboard: Network status check failed:', error);
      const networkStatus = document.getElementById('networkStatus');
      const networkText = document.getElementById('networkText');
      
      if (networkStatus) networkStatus.classList.remove('connected');
      if (networkText) networkText.textContent = 'Offline';
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
      const transaction = {
        from: this.wallet.address,
        to: to,
        amount: amount,
        timestamp: Date.now(),
        nonce: Math.floor(Math.random() * 1000000)
      };

      // Send transaction
      const response = await fetch(`${this.nodeUrl}/transaction`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify(transaction)
      });

      if (response.ok) {
        alert('Transaction sent successfully!');
        this.clearSendForm();
        await this.loadWalletData();
      } else {
        throw new Error('Transaction failed');
      }
    } catch (error) {
      console.error('XMBL Dashboard: Send transaction failed:', error);
      alert('Failed to send transaction: ' + error.message);
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
