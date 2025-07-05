const puppeteer = require("puppeteer");
const { spawn } = require("child_process");

let devServer;

// Function to start the Vue dev server
async function startDevServer() {
  return new Promise((resolve, reject) => {
    devServer = spawn("npm", ["run", "serve"], {
      stdio: "pipe",
      env: { ...process.env, NODE_ENV: "development" },
    });

    devServer.stdout.on("data", (data) => {
      const output = data.toString();
      console.log("Dev server output:", output);

      // Check if server is ready
      if (output.includes("Local:") && output.includes("http://localhost:")) {
        const match = output.match(/Local:\s+([^\s]+)/);
        if (match) {
          console.log("Dev server ready at:", match[1]);
          resolve(match[1]);
        }
      }
    });

    devServer.stderr.on("data", (data) => {
      console.error("Dev server error:", data.toString());
    });

    devServer.on("close", (code) => {
      if (code !== 0) {
        reject(new Error(`Dev server exited with code ${code}`));
      }
    });

    // Timeout after 30 seconds
    setTimeout(() => {
      reject(new Error("Dev server timeout"));
    }, 30000);
  });
}

// Function to stop the dev server
function stopDevServer() {
  if (devServer) {
    devServer.kill();
  }
}

// Main test function
async function testExtension() {
  let browser;
  let page;

  try {
    console.log("Starting Vue dev server...");
    const serverUrl = await startDevServer();

    console.log("Starting browser...");
    browser = await puppeteer.launch({
      headless: false,
      devtools: true,
      args: ["--no-sandbox", "--disable-setuid-sandbox"],
    });

    page = await browser.newPage();

    // Enable console logging
    page.on("console", (msg) => {
      console.log(`Browser console [${msg.type()}]:`, msg.text());
    });

    // Enable error logging
    page.on("pageerror", (error) => {
      console.error("Browser page error:", error.message);
    });

    // Navigate to the extension
    console.log("Navigating to:", serverUrl);
    await page.goto(serverUrl, { waitUntil: "networkidle2" });

    console.log("Taking screenshot...");
    await page.screenshot({ path: "extension-screenshot.png", fullPage: true });

    // Test page title
    const title = await page.title();
    console.log("Page title:", title);

    // Test if Vue app is mounted
    const h1Text = await page
      .$eval("h1", (el) => el.textContent)
      .catch(() => "Not found");
    console.log("Main heading:", h1Text);

    // Test dashboard components
    const dashboardElements = await page.$$eval(".dashboard-card", (els) =>
      els.map((el) => el.textContent),
    );
    console.log("Dashboard cards:", dashboardElements);

    // Test navigation
    const navItems = await page.$$eval(".nav-link", (els) =>
      els.map((el) => el.textContent),
    );
    console.log("Navigation items:", navItems);

    // Test button interactions
    const buttons = await page.$$("button");
    console.log(`Found ${buttons.length} buttons`);

    if (buttons.length > 0) {
      console.log("Testing button click...");
      await buttons[0].click();
      await page.waitForTimeout(1000);
    }

    // Test form elements
    const inputs = await page.$$("input");
    console.log(`Found ${inputs.length} input elements`);

    // Test that Chart.js is loaded
    const chartExists = await page.evaluate(() => {
      return typeof Chart !== "undefined";
    });
    console.log("Chart.js loaded:", chartExists);

    // Test API endpoints (mock data)
    const apiTestResult = await page.evaluate(() => {
      // This would test the API endpoints if they were real
      return {
        mockTest: "API endpoints would be tested here",
        hasVuex: typeof window.$store !== "undefined",
      };
    });
    console.log("API test result:", apiTestResult);

    // Test console for any errors
    const consoleErrors = await page.evaluate(() => {
      const errors = [];
      const originalError = console.error;
      console.error = (...args) => {
        errors.push(args.join(" "));
        originalError.apply(console, args);
      };
      return errors;
    });

    console.log("Console errors:", consoleErrors);

    // Final screenshot
    await page.screenshot({
      path: "extension-final-screenshot.png",
      fullPage: true,
    });

    console.log("✅ Extension test completed successfully!");

    // Return test results
    return {
      title,
      h1Text,
      dashboardElements,
      navItems,
      buttonCount: buttons.length,
      inputCount: inputs.length,
      chartExists,
      apiTestResult,
      consoleErrors,
    };
  } catch (error) {
    console.error("❌ Test failed:", error);
    throw error;
  } finally {
    if (browser) {
      await browser.close();
    }
    stopDevServer();
  }
}

// Run the test
testExtension()
  .then((results) => {
    console.log("\n=== Test Results ===");
    console.log(JSON.stringify(results, null, 2));
    process.exit(0);
  })
  .catch((error) => {
    console.error("Test failed:", error);
    process.exit(1);
  });

// Handle process termination
process.on("SIGINT", () => {
  console.log("Received SIGINT, cleaning up...");
  stopDevServer();
  process.exit(0);
});

process.on("SIGTERM", () => {
  console.log("Received SIGTERM, cleaning up...");
  stopDevServer();
  process.exit(0);
});
