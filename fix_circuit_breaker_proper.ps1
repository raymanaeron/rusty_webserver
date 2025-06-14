# PowerShell script to fix circuit_breaker fields with proper line endings

$files = @(
    "httpserver-proxy\tests\websocket_sticky_sessions.rs",
    "httpserver-proxy\tests\websocket_advanced.rs", 
    "httpserver-proxy\tests\sticky_session_integration.rs"
)

foreach ($file in $files) {
    if (Test-Path $file) {
        $content = Get-Content $file -Raw
        $newContent = $content -replace 'websocket_health: None,`r`n            circuit_breaker: None,', 'websocket_health: None,
            circuit_breaker: None,'
        Set-Content $file $newContent -NoNewline
        Write-Host "Fixed $file"
    }
}

Write-Host "Circuit breaker fields fixed with proper line endings"
