// Test JavaScript file for HTTP Server
console.log('🚀 HTTP Server JavaScript test file loaded successfully!');

// Test CORS by making a fetch request
async function testCORS() {
    try {
        const response = await fetch('/test.json');
        const data = await response.json();
        console.log('✅ CORS test successful:', data);
        
        // Update the page with the result
        const resultDiv = document.getElementById('cors-result');
        if (resultDiv) {
            resultDiv.innerHTML = `
                <div style="background: rgba(76, 175, 80, 0.2); padding: 15px; border-radius: 8px; margin: 10px 0;">
                    ✅ CORS Test Successful!<br>
                    Server: ${data.server}<br>
                    Timestamp: ${data.timestamp}
                </div>
            `;
        }
    } catch (error) {
        console.error('❌ CORS test failed:', error);
        const resultDiv = document.getElementById('cors-result');
        if (resultDiv) {
            resultDiv.innerHTML = `
                <div style="background: rgba(244, 67, 54, 0.2); padding: 15px; border-radius: 8px; margin: 10px 0;">
                    ❌ CORS Test Failed: ${error.message}
                </div>
            `;
        }
    }
}

// Test different file types
function testFileTypes() {
    const fileTypes = [
        { url: '/test.css', type: 'CSS' },
        { url: '/test.json', type: 'JSON' },
        { url: '/README.md', type: 'Markdown' }
    ];

    fileTypes.forEach(async (file) => {
        try {
            const response = await fetch(file.url);
            const contentType = response.headers.get('content-type');
            console.log(`📄 ${file.type} file served with Content-Type: ${contentType}`);
        } catch (error) {
            console.log(`❌ Failed to fetch ${file.type} file:`, error);
        }
    });
}

// Run tests when the page loads
document.addEventListener('DOMContentLoaded', () => {
    console.log('🎯 Starting HTTP Server tests...');
    
    // Add a test button
    const button = document.createElement('button');
    button.textContent = 'Test CORS & File Types';
    button.style.cssText = `
        background: linear-gradient(45deg, #ff6b6b, #ee5a24);
        color: white;
        border: none;
        padding: 12px 24px;
        border-radius: 25px;
        font-size: 16px;
        cursor: pointer;
        margin: 20px 0;
        box-shadow: 0 4px 15px rgba(0, 0, 0, 0.2);
        transition: transform 0.2s;
    `;
    
    button.onmouseover = () => button.style.transform = 'translateY(-2px)';
    button.onmouseout = () => button.style.transform = 'translateY(0)';
    
    button.onclick = () => {
        testCORS();
        testFileTypes();
    };
    
    // Insert the button after the main heading
    const container = document.querySelector('.container');
    if (container) {
        const corsResult = document.createElement('div');
        corsResult.id = 'cors-result';
        container.appendChild(button);
        container.appendChild(corsResult);
    }
    
    // Auto-run tests after 2 seconds
    setTimeout(() => {
        testCORS();
        testFileTypes();
    }, 2000);
});
