// src/network/simple_metrics.rs
use std::time::Duration;
use tokio::time::sleep;

pub struct SimpleNetworkSimulator {
    bandwidth_mbps: f64,
    latency_ms: u64,
}

impl SimpleNetworkSimulator {
    pub fn new(bandwidth_mbps: f64, latency_ms: u64) -> Self {
        Self { bandwidth_mbps, latency_ms }
    }
    
    // Send by specifying only the byte length (avoids allocating a buffer)
    pub async fn send_size_with_metrics(&self, bytes: usize) {
        // Simulate latency
        if self.latency_ms > 0 {
            sleep(Duration::from_millis(self.latency_ms)).await;
        }
        
        // Simulate bandwidth
        let transmission_time = self.calculate_transmission_time(bytes);
        sleep(transmission_time).await;
    }
    
    fn calculate_transmission_time(&self, bytes: usize) -> Duration {
        let bits = bytes * 8;
        let seconds = bits as f64 / (self.bandwidth_mbps * 1_000_000.0);
        Duration::from_secs_f64(seconds)
    }
        
}
