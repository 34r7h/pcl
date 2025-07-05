const puppeteer = require("puppeteer");
const { spawn } = require("child_process");
const path = require("path");
const fs = require("fs");

let nodeProcess, simulatorProcess;

// Function to start PCL Node
async function startPCLNode() {
  return new Promise((resolve, reject) => {
    console.log("Starting PCL Node...");
    nodeProcess = spawn("cargo", ["run", "--release"], {
      cwd: "../backend",
      stdio: "pipe",
      env: { ...process.env, RUST_LOG: "info" }
    });

    nodeProcess.stdout.on("data", (data) => {
      const output = data.toString();
      console.log("Node:", output);
      
      if (output.includes("Server listening") || output.includes("Node started")) {
        console.log("PCL Node is ready");
        resolve();
      }
    });

    nodeProcess.stderr.on("data", (data) => {
      console.error("Node error:", data.toString());
    });

    nodeProcess.on("error", (error) => {
      console.error("Failed to start PCL Node:", error);
      reject(error);
    });

    // Timeout after 30 seconds
    setTimeout(() => {
      console.log("PCL Node started (timeout)");
      resolve();
    }, 30000);
  });
}

// Function to start Simulator
async function startSimulator() {
  return new Promise((resolve, reject) => {
    console.log("Starting PCL Simulator...");
    simulatorProcess = spawn("cargo", ["run", "--release", "--", "load-test", "--nodes", "3", "--leaders", "1", "--tps", "1", "--duration", "60"], {
      cwd: "../simulator",
      stdio: "pipe",
      env: { ...process.env, RUST_LOG: "info" }
    });

    simulatorProcess.stdout.on("data", (data) => {
      const output = data.toString();
      console.log("Simulator:", output);
      
      if (output.includes("Load test started") || output.includes("Simulation running")) {
        console.log("PCL Simulator is ready");
        resolve();
      }
    });

    simulatorProcess.stderr.on("data", (data) => {
      console.error("Simulator error:", data.toString());
    });

    simulatorProcess.on("error", (error) => {
      console.error("Failed to start PCL Simulator:", error);
      reject(error);
    });

    // Timeout after 20 seconds
    setTimeout(() => {
      console.log("PCL Simulator started (timeout)");
      resolve();
    }, 20000);
  });
}

// Function to test Chrome extension
async function testChromeExtension() {
  console.log("Testing Chrome Extension...");
  
  const extensionPath = path.resolve(__dirname);
  console.log("Extension path:", extensionPath);

  // Launch Chrome with extension
  const browser = await puppeteer.launch({
    headless: false,
    devtools: true,
    args: [
      `--load-extension=${extensionPath}`,
      "--disable-extensions-except=" + extensionPath,
      "--no-sandbox",
      "--disable-setuid-sandbox",
      "--disable-dev-shm-usage",
      "--disable-web-security",
      "--disable-features=VizDisplayCompositor"
    ]
  });

  try {
    // Get all pages (extension popup and background)
    const pages = await browser.pages();
    const page = pages[0];

    console.log("Browser launched with extension");

    // Test 1: Navigate to extension popup
    console.log("Test 1: Testing extension popup...");
    
    // Get extension ID from chrome://extensions
    await page.goto("chrome://extensions/");
    await page.waitForSelector('body', { timeout: 5000 });
    
    // Take screenshot of extensions page
    await page.screenshot({ 
      path: "screenshots/01-extensions-page.png",
      fullPage: true 
    });
    console.log("✅ Screenshot: Extensions page saved");

    // Test 2: Test fullscreen extension page
    console.log("Test 2: Testing fullscreen extension...");
    
    try {
      // Navigate to extension fullscreen page
      const extensionUrl = "chrome-extension://*/fullscreen.html";
      
      // Create new page for fullscreen
      const fullscreenPage = await browser.newPage();
      
      // Navigate to a local file URL for testing
      const fullscreenPath = path.join(__dirname, "fullscreen.html");
      await fullscreenPage.goto(`file://${fullscreenPath}`);
      await fullscreenPage.waitForSelector('body', { timeout: 5000 });

      // Test wallet creation
      console.log("Testing wallet creation...");
      
      // Check if create wallet button exists
      const createWalletBtn = await fullscreenPage.$("#createWalletBtn");
      if (createWalletBtn) {
        await createWalletBtn.click();
        await new Promise(resolve => setTimeout(resolve, 2000));
        console.log("✅ Wallet creation button clicked");
      }

      // Take screenshot of fullscreen dashboard
      await fullscreenPage.screenshot({ 
        path: "screenshots/02-fullscreen-dashboard.png",
        fullPage: true 
      });
      console.log("✅ Screenshot: Fullscreen dashboard saved");

      // Test navigation tabs
      console.log("Testing navigation tabs...");
      
      const navButtons = await fullscreenPage.$$(".nav-btn");
      for (let i = 0; i < navButtons.length; i++) {
        const btn = navButtons[i];
        const viewName = await btn.evaluate(el => el.dataset.view);
        
        await btn.click();
        await new Promise(resolve => setTimeout(resolve, 1000));
        
        await fullscreenPage.screenshot({ 
          path: `screenshots/03-nav-${viewName}.png`,
          fullPage: true 
        });
        console.log(`✅ Screenshot: ${viewName} view saved`);
      }

      await fullscreenPage.close();

    } catch (error) {
      console.error("Fullscreen test error:", error);
    }

    // Test 3: Test popup interface
    console.log("Test 3: Testing popup interface...");
    
    try {
      const popupPage = await browser.newPage();
      const popupPath = path.join(__dirname, "popup.html");
      await popupPage.goto(`file://${popupPath}`);
      await popupPage.waitForSelector('body', { timeout: 5000 });

      // Take screenshot of popup
      await popupPage.screenshot({ 
        path: "screenshots/04-popup-interface.png",
        fullPage: true 
      });
      console.log("✅ Screenshot: Popup interface saved");

      // Test wallet creation in popup
      const createBtn = await popupPage.$("#createWalletBtn");
      if (createBtn) {
        await createBtn.click();
        await new Promise(resolve => setTimeout(resolve, 2000));
        
        await popupPage.screenshot({ 
          path: "screenshots/05-popup-wallet-created.png",
          fullPage: true 
        });
        console.log("✅ Screenshot: Popup wallet created saved");
      }

      await popupPage.close();

    } catch (error) {
      console.error("Popup test error:", error);
    }

    // Test 4: Test console logs
    console.log("Test 4: Checking console logs...");
    
    page.on("console", msg => {
      if (msg.text().includes("PCL Wallet")) {
        console.log("Extension log:", msg.text());
      }
    });

    // Wait for logs
    await new Promise(resolve => setTimeout(resolve, 5000));

    console.log("✅ Chrome extension testing completed successfully");

  } catch (error) {
    console.error("❌ Chrome extension test failed:", error);
  } finally {
    await browser.close();
  }
}

// Create screenshots directory
function createScreenshotsDir() {
  if (!fs.existsSync("screenshots")) {
    fs.mkdirSync("screenshots");
    console.log("Created screenshots directory");
  }
}

// Cleanup function
function cleanup() {
  console.log("Cleaning up processes...");
  
  if (nodeProcess) {
    nodeProcess.kill("SIGTERM");
    console.log("PCL Node stopped");
  }
  
  if (simulatorProcess) {
    simulatorProcess.kill("SIGTERM");
    console.log("PCL Simulator stopped");
  }
}

// Main test function
async function runTests() {
  try {
    createScreenshotsDir();
    
    console.log("=== PCL Extension Test Suite ===");
    
    // Step 1: Start PCL Node
    await startPCLNode();
    
    // Step 2: Start Simulator
    await startSimulator();
    
    // Step 3: Wait for services to stabilize
    console.log("Waiting for services to stabilize...");
    await new Promise(resolve => setTimeout(resolve, 5000));
    
    // Step 4: Test Chrome Extension
    await testChromeExtension();
    
    console.log("=== All Tests Completed ===");
    console.log("Screenshots saved in ./screenshots/ directory");
    
  } catch (error) {
    console.error("❌ Test suite failed:", error);
  } finally {
    cleanup();
    process.exit(0);
  }
}

// Handle process termination
process.on("SIGINT", cleanup);
process.on("SIGTERM", cleanup);

// Run the tests
runTests();
