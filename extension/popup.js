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

  generateAddress(publicKeyBytes) {
    // Use the hex representation of the raw public key as the address
    return Array.from(new Uint8Array(publicKeyBytes)).map(b => b.toString(16).padStart(2, '0')).join('');
  }

  async sendTransaction(toAddress, amountToSend) {
    try {
      if (!this.wallet) {
        this.showError("Wallet not created or loaded.");
        return false;
      }
      console.log('XMBL Wallet: Preparing transaction...');

      // This is a simplified transaction structure for the popup.
      // The backend will need to resolve UTXOs for self.wallet.address.
      // Fees and stake are hardcoded for now.
      const fee = 0.1;
      const stake = 0.2;
      const totalFromAmount = amountToSend + fee + stake; // Simplified: assume one UTXO covers this

      const txDataPayload = {
        to: [[toAddress, amountToSend]],
        // from: [[ "alice_utxo_placeholder_id", totalFromAmount ]], // Placeholder for UTXO management
        // For now, the backend will need to figure out Alice's UTXOs.
        // Or, the simulator should be used for creating well-formed transactions.
        // For the popup, we'll let the backend derive 'from' or use a default.
        // Sending an empty 'from' or a special marker might be necessary.
        // Let's send a simplified format and expect the backend to adapt or simulate UTXO usage.
        from_address: this.wallet.address, // Sending from address instead of specific UTXOs
        amount: amountToSend, // Total amount being sent to recipients
        user: this.wallet.address, // Alice's address (public key hex)
        stake: stake,
        fee: fee,
        timestamp: new Date().toISOString(), // Use ISO string for backend compatibility
        // Nonce can be handled by backend or incremented in wallet state
        nonce: Math.floor(Math.random() * 1000000000)
      };

      // Sign the txDataPayload (without the 'sig' field itself)
      const signatureHex = await this.signData(txDataPayload);
      
      const transactionToSend = {
        ...txDataPayload,
        sig: signatureHex, // Add hex signature
      };

      console.log('XMBL Wallet: Sending transaction:', JSON.stringify(transactionToSend, null, 2));

      // Send to node/simulator
      // The endpoint /transaction needs to be able to handle this structure
      // and create a RawTransaction from it.
      const response = await fetch(`${this.nodeUrl}/create_transaction`, { // Changed endpoint
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

  async signData(dataToSign) {
    try {
      if (!this.wallet || !this.wallet.privateKey) {
        throw new Error("Private key not available for signing.");
      }

      const privateKeyBytes = new Uint8Array(this.wallet.privateKey);

      const importedPrivateKey = await window.crypto.subtle.importKey(
        'pkcs8', // Private key format
        privateKeyBytes.buffer,
        {
          name: 'Ed25519',
          namedCurve: 'Ed25519',
        },
        true, // extractable
        ['sign'] // key usages
      );

      // Prepare message: Convert data to JSON string, then to Uint8Array
      // Ensure the 'sig' field is not part of the payload being signed.
      const payloadToSign = { ...dataToSign };
      if ('sig' in payloadToSign) {
        delete payloadToSign.sig;
      }
      const messageString = JSON.stringify(payloadToSign);
      const messageBytes = new TextEncoder().encode(messageString);

      const signatureBytes = await window.crypto.subtle.sign(
        'Ed25519',
        importedPrivateKey,
        messageBytes
      );

      // Convert signature to hex string
      return Array.from(new Uint8Array(signatureBytes)).map(b => b.toString(16).padStart(2, '0')).join('');
    } catch (error) {
      console.error('XMBL Wallet: Error signing data:', error);
      this.showError('Failed to sign transaction data.');
      throw error; // Re-throw to be caught by sender
    }
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
