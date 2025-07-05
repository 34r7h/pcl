// XMBL Wallet Content Script
console.log('XMBL Wallet: Content script loaded');

// Inject XMBL Wallet API into web pages
(function() {
  'use strict';

  // Create XMBL Wallet API object
  const xmblWallet = {
    // Check if wallet is available
    isConnected: false,
    address: null,
    
    // Connect to wallet
    async connect() {
      try {
        const response = await chrome.runtime.sendMessage({
          type: 'GET_WALLET_STATUS'
        });
        
        if (response.hasWallet) {
          this.isConnected = true;
          this.address = response.wallet.address;
          console.log('PCL Wallet: Connected to wallet', this.address);
          return this.address;
        } else {
          console.log('PCL Wallet: No wallet available');
          return null;
        }
      } catch (error) {
        console.error('PCL Wallet: Connection failed', error);
        return null;
      }
    },
    
    // Request transaction signature
    async signTransaction(transaction) {
      if (!this.isConnected) {
        throw new Error('Wallet not connected');
      }
      
      // This would open the extension popup for user confirmation
      return new Promise((resolve, reject) => {
        chrome.runtime.sendMessage({
          type: 'SIGN_TRANSACTION',
          transaction: transaction
        }, (response) => {
          if (response.success) {
            resolve(response.signature);
          } else {
            reject(new Error(response.error || 'Transaction rejected'));
          }
        });
      });
    },
    
    // Send transaction
    async sendTransaction(to, amount) {
      if (!this.isConnected) {
        throw new Error('Wallet not connected');
      }
      
      const transaction = {
        from: this.address,
        to: to,
        amount: amount,
        timestamp: Date.now()
      };
      
      try {
        const signature = await this.signTransaction(transaction);
        transaction.signature = signature;
        
        // Submit to PCL network
        const response = await fetch('http://localhost:8080/transaction', {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
          },
          body: JSON.stringify(transaction)
        });
        
        if (response.ok) {
          const result = await response.json();
          console.log('PCL Wallet: Transaction sent', result);
          return result;
        } else {
          throw new Error('Transaction failed');
        }
      } catch (error) {
        console.error('PCL Wallet: Send transaction failed', error);
        throw error;
      }
    },
    
    // Get balance
    async getBalance() {
      if (!this.isConnected) {
        throw new Error('Wallet not connected');
      }
      
      try {
        const response = await fetch(`http://localhost:8080/balance/${this.address}`);
        if (response.ok) {
          const data = await response.json();
          return data.balance;
        } else {
          throw new Error('Failed to get balance');
        }
      } catch (error) {
        console.error('PCL Wallet: Get balance failed', error);
        throw error;
      }
    }
  };

  // Inject XMBL Wallet API into window object
  Object.defineProperty(window, 'xmblWallet', {
    value: xmblWallet,
    writable: false,
    configurable: false
  });

  // Dispatch wallet ready event
  window.dispatchEvent(new CustomEvent('xmblWalletReady', {
    detail: { wallet: xmblWallet }
  }));

  console.log('XMBL Wallet: API injected into page');
})(); 