mod config_tests;
mod core_tests;
mod balancer_tests;
mod static_tests;
mod proxy_tests;
mod tunnel_tests;

fn main() {
    println!("Hello from httpserver-tests!");
    println!("Available test modules:");
    println!("- config_tests: Configuration parsing and validation tests");
    println!("- core_tests: Core server functionality tests");
    println!("- balancer_tests: Load balancing and circuit breaker tests");
    println!("- static_tests: Static file serving tests");
    println!("- proxy_tests: Proxy handling and WebSocket tests");
    println!("- tunnel_tests: Tunneling and connection tests");
}
