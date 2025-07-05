// XMBL Wallet Popup Logic
class XMBLWallet {
  constructor() {
    this.nodeUrl = 'http://localhost:8080';
    this.simulatorUrl = 'http://localhost:3000';
    this.wallet = null;
    this.balance = 0;
    this.transactions = [];
    this.init();
  }

  async init() {
    console.log('XMBL Wallet: Initializing...');
    await this.checkWalletExists();
    await this.checkNodeConnection();
    this.setupEventListeners();
    this.updateUI();
  }

  async checkWalletExists() {
    try {
      const result = await chrome.storage.local.get(['xmblWallet']);
      if (result.xmblWallet) {
        this.wallet = result.xmblWallet;
        console.log('XMBL Wallet: Existing wallet found');
        await this.loadWalletData();
      } else {
        console.log('XMBL Wallet: No existing wallet found');
      }
    } catch (error) {
      console.error('XMBL Wallet: Error checking wallet:', error);
      this.showError('Failed to check wallet');
    }
  }

  async checkNodeConnection() {
    try {
      const response = await fetch(`${this.nodeUrl}/health`);
      if (response.ok) {
        this.updateNetworkStatus('Connected to XMBL Node');
      } else {
        this.updateNetworkStatus('Node connection failed');
      }
    } catch (error) {
      console.log('XMBL Wallet: Node not available');
      this.updateNetworkStatus('Offline - Node unreachable');
    }
  }

  async loadWalletData() {
    try {
      // Load balance from node/simulator
      const balanceResponse = await fetch(`${this.nodeUrl}/balance/${this.wallet.address}`);
      if (balanceResponse.ok) {
        const balanceData = await balanceResponse.json();
        this.balance = balanceData.balance || 0;
      }

      // Load recent transactions
      const txResponse = await fetch(`${this.nodeUrl}/transactions/${this.wallet.address}`);
      if (txResponse.ok) {
        const txData = await txResponse.json();
        this.transactions = txData.transactions || [];
      }
    } catch (error) {
      console.error('XMBL Wallet: Error loading wallet data:', error);
    }
  }

  async createWallet() {
    try {
      console.log('XMBL Wallet: Creating new wallet...');
      
      // Generate wallet keypair (using Web Crypto API for Ed25519)
      const keyPair = await window.crypto.subtle.generateKey(
        {
          name: 'Ed25519',
          namedCurve: 'Ed25519',
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
      
      console.log('XMBL Wallet: New wallet created successfully');
      this.updateUI();
    } catch (error) {
      console.error('XMBL Wallet: Error creating wallet:', error);
      this.showError('Failed to create wallet');
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

  async sendTransaction(to, amount) {
    try {
      console.log('XMBL Wallet: Sending transaction...');
      
      const transaction = {
        from: this.wallet.address,
        to: to,
        amount: amount,
        timestamp: Date.now(),
        nonce: Math.floor(Math.random() * 1000000)
      };

      // Sign transaction (simplified)
      const signature = await this.signTransaction(transaction);
      transaction.signature = signature;

      // Send to node/simulator
      const response = await fetch(`${this.nodeUrl}/transaction`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify(transaction)
      });

      if (response.ok) {
        console.log('XMBL Wallet: Transaction sent successfully');
        await this.loadWalletData();
        this.updateUI();
        return true;
      } else {
        throw new Error('Transaction failed');
      }
    } catch (error) {
      console.error('XMBL Wallet: Error sending transaction:', error);
      this.showError('Failed to send transaction');
      return false;
    }
  }

  async signTransaction(transaction) {
    // Simplified signing (in real implementation, use proper Ed25519 signing)
    const message = JSON.stringify(transaction);
    return btoa(message).substring(0, 32);
  }

  setupEventListeners() {
    // Create wallet button
    document.getElementById('createWalletBtn')?.addEventListener('click', () => {
      this.createWallet();
    });

    // Import wallet button
    document.getElementById('importWalletBtn')?.addEventListener('click', () => {
      this.showError('Import wallet not implemented yet');
    });

    // Send button
    document.getElementById('sendBtn')?.addEventListener('click', () => {
      this.showSendDialog();
    });

    // Receive button
    document.getElementById('receiveBtn')?.addEventListener('click', () => {
      this.showReceiveDialog();
    });

    // Fullscreen button
    document.getElementById('fullscreenBtn')?.addEventListener('click', () => {
      chrome.tabs.create({ url: 'fullscreen.html' });
    });
  }

  showSendDialog() {
    const to = prompt('Enter recipient address:');
    const amount = prompt('Enter amount to send:');
    
    if (to && amount) {
      this.sendTransaction(to, parseFloat(amount));
    }
  }

  showReceiveDialog() {
    if (this.wallet) {
      navigator.clipboard.writeText(this.wallet.address);
      alert(`Address copied to clipboard:\n${this.wallet.address}`);
    }
  }

  updateUI() {
    const walletContent = document.getElementById('walletContent');
    const noWallet = document.getElementById('noWallet');
    const walletExists = document.getElementById('walletExists');

    if (this.wallet) {
      walletContent.style.display = 'none';
      noWallet.style.display = 'none';
      walletExists.style.display = 'block';

      // Update balance
      document.getElementById('balance').textContent = `${this.balance.toFixed(2)} XMBL`;
      
      // Update address
      document.getElementById('address').textContent = 
        `${this.wallet.address.substring(0, 8)}...${this.wallet.address.substring(-8)}`;

      // Update transactions
      this.updateTransactions();
    } else {
      walletContent.style.display = 'none';
      noWallet.style.display = 'block';
      walletExists.style.display = 'none';
    }
  }

  updateTransactions() {
    const transactionsDiv = document.getElementById('transactions');
    
    if (this.transactions.length === 0) {
      transactionsDiv.innerHTML = '<div class="transaction">No transactions yet</div>';
      return;
    }

    transactionsDiv.innerHTML = this.transactions.map(tx => `
      <div class="transaction">
        <div>${tx.type === 'send' ? '↗' : '↙'} ${tx.amount} XMBL</div>
        <div class="tx-hash">${tx.hash.substring(0, 16)}...</div>
      </div>
    `).join('');
  }

  updateNetworkStatus(status) {
    const networkStatus = document.getElementById('networkStatus');
    if (networkStatus) {
      networkStatus.textContent = status;
    }
  }

  showError(message) {
    const errorDiv = document.getElementById('errorMessage');
    if (errorDiv) {
      errorDiv.textContent = message;
      errorDiv.style.display = 'block';
      setTimeout(() => {
        errorDiv.style.display = 'none';
      }, 5000);
    }
  }
}

// Initialize wallet when popup opens
document.addEventListener('DOMContentLoaded', () => {
  new XMBLWallet();
});

// Log successful initialization
console.log('XMBL Wallet: Popup script loaded successfully'); console.log('PCL Wallet: Popup script loaded successfully'); 
