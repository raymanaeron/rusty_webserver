# PowerShell script to add circuit_breaker field to all ProxyRoute structs

$files = @(
    "httpserver-config\tests\config_parsing.rs",
    "httpserver-proxy\tests\websocket_advanced.rs", 
    "httpserver-proxy\tests\sticky_session_integration.rs"
)

foreach ($file in $files) {
    if (Test-Path $file) {
        $content = Get-Content $file -Raw
        $newContent = $content -replace 'websocket_health: None,(?!\s*circuit_breaker)', 'websocket_health: None,`r`n            circuit_breaker: None,'
        Set-Content $file $newContent
        Write-Host "Updated $file"
    }
}

Write-Host "Circuit breaker field added to all ProxyRoute structs"
