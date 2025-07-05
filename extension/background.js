// XMBL Wallet Background Service Worker
console.log('XMBL Wallet: Background service worker starting...');

// Install event
chrome.runtime.onInstalled.addListener((details) => {
  console.log('XMBL Wallet: Extension installed/updated', details);
  
  // Set up initial state
  chrome.storage.local.set({
    xmblWalletInstalled: true,
    installDate: Date.now()
  });
});

// Handle extension startup
chrome.runtime.onStartup.addListener(() => {
  console.log('XMBL Wallet: Extension startup');
});

// Handle messages from content scripts or popup
chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
  console.log('PCL Wallet: Message received', message);
  
  switch (message.type) {
    case 'GET_WALLET_STATUS':
      getWalletStatus().then(sendResponse);
      return true; // Will respond asynchronously
      
    case 'NODE_HEALTH_CHECK':
      checkNodeHealth().then(sendResponse);
      return true;
      
    case 'SIMULATOR_HEALTH_CHECK':
      checkSimulatorHealth().then(sendResponse);
      return true;
      
    default:
      console.log('PCL Wallet: Unknown message type', message.type);
  }
});

// Check wallet status
async function getWalletStatus() {
  try {
    const result = await chrome.storage.local.get(['pclWallet']);
    return {
      hasWallet: !!result.pclWallet,
      wallet: result.pclWallet || null
    };
  } catch (error) {
    console.error('PCL Wallet: Error getting wallet status', error);
    return { hasWallet: false, wallet: null };
  }
}

// Check PCL node health
async function checkNodeHealth() {
  try {
    const response = await fetch('http://localhost:8080/health');
    return {
      connected: response.ok,
      status: response.status
    };
  } catch (error) {
    console.log('PCL Wallet: Node health check failed', error);
    return {
      connected: false,
      error: error.message
    };
  }
}

// Check simulator health
async function checkSimulatorHealth() {
  try {
    const response = await fetch('http://localhost:3000/health');
    return {
      connected: response.ok,
      status: response.status
    };
  } catch (error) {
    console.log('PCL Wallet: Simulator health check failed', error);
    return {
      connected: false,
      error: error.message
    };
  }
}

// Periodic health checks (every 30 seconds)
setInterval(async () => {
  const nodeHealth = await checkNodeHealth();
  const simulatorHealth = await checkSimulatorHealth();
  
  // Store connection status
  chrome.storage.local.set({
    nodeConnected: nodeHealth.connected,
    simulatorConnected: simulatorHealth.connected,
    lastHealthCheck: Date.now()
  });
}, 30000);

console.log('PCL Wallet: Background service worker loaded successfully'); 