const http = require('http');

function makeRequest(url) {
  return new Promise((resolve, reject) => {
    const request = http.get(url, (response) => {
      let data = '';
      response.on('data', (chunk) => data += chunk);
      response.on('end', () => {
        try {
          resolve({ 
            ok: response.statusCode === 200, 
            status: response.statusCode,
            json: () => Promise.resolve(JSON.parse(data))
          });
        } catch (error) {
          resolve({ 
            ok: response.statusCode === 200, 
            status: response.statusCode,
            text: () => Promise.resolve(data)
          });
        }
      });
    });
    request.on('error', reject);
  });
}

async function testBackend() {
  console.log('🧪 Testing backend connection...');
  
  try {
    const response = await makeRequest('http://127.0.0.1:8080/health');
    console.log('✅ Backend status:', response.status);
    
    if (response.ok) {
      const data = await response.json();
      console.log('✅ Backend data:', data);
    }
    
    // Test transaction endpoint
    const txResponse = await makeRequest('http://127.0.0.1:8080/transactions/recent');
    console.log('✅ Transaction endpoint status:', txResponse.status);
    
  } catch (error) {
    console.log('❌ Backend test failed:', error.message);
  }
}

testBackend();
