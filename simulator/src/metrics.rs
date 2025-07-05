use std::collections::HashMap;
use std::time::{Duration, Instant};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SimulationMetrics {
    pub total_transactions: u64,
    pub successful_transactions: u64,
    pub failed_transactions: u64,
    pub start_time: Option<Instant>,
    pub end_time: Option<Instant>,
    pub transaction_latencies: Vec<Duration>,
    pub throughput_samples: Vec<(DateTime<Utc>, u64)>, // (timestamp, tps)
    pub node_metrics: HashMap<Uuid, NodeMetrics>,
    pub leader_election_count: u64,
    pub leader_election_times: Vec<Duration>,
    pub network_messages: u64,
    pub failed_validations: u64,
    pub mempool_sizes: Vec<(DateTime<Utc>, usize)>,
}

#[derive(Debug, Clone)]
pub struct NodeMetrics {
    pub node_id: Uuid,
    pub transactions_processed: u64,
    pub validation_tasks_completed: u64,
    pub uptime: Duration,
    pub response_times: Vec<Duration>,
    pub pulse_count: u64,
    pub last_pulse: Option<DateTime<Utc>>,
}

impl SimulationMetrics {
    pub fn new() -> Self {
        Self {
            total_transactions: 0,
            successful_transactions: 0,
            failed_transactions: 0,
            start_time: None,
            end_time: None,
            transaction_latencies: Vec::new(),
            throughput_samples: Vec::new(),
            node_metrics: HashMap::new(),
            leader_election_count: 0,
            leader_election_times: Vec::new(),
            network_messages: 0,
            failed_validations: 0,
            mempool_sizes: Vec::new(),
        }
    }
    
    pub fn start_simulation(&mut self) {
        self.start_time = Some(Instant::now());
    }
    
    pub fn end_simulation(&mut self) {
        self.end_time = Some(Instant::now());
    }
    
    pub fn record_transaction(&mut self, result: Result<String, Box<dyn std::error::Error + Send + Sync>>) {
        self.total_transactions += 1;
        
        match result {
            Ok(_) => {
                self.successful_transactions += 1;
            },
            Err(_) => {
                self.failed_transactions += 1;
            }
        }
    }
    
    pub fn record_transaction_latency(&mut self, latency: Duration) {
        self.transaction_latencies.push(latency);
    }
    
    pub fn record_throughput_sample(&mut self, tps: u64) {
        self.throughput_samples.push((Utc::now(), tps));
    }
    
    pub fn record_leader_election(&mut self, duration: Duration) {
        self.leader_election_count += 1;
        self.leader_election_times.push(duration);
    }
    
    pub fn record_network_message(&mut self) {
        self.network_messages += 1;
    }
    
    pub fn record_failed_validation(&mut self) {
        self.failed_validations += 1;
    }
    
    pub fn record_mempool_size(&mut self, size: usize) {
        self.mempool_sizes.push((Utc::now(), size));
    }
    
    pub fn get_or_create_node_metrics(&mut self, node_id: Uuid) -> &mut NodeMetrics {
        self.node_metrics.entry(node_id).or_insert_with(|| NodeMetrics::new(node_id))
    }
    
    pub fn average_latency(&self) -> Option<Duration> {
        if self.transaction_latencies.is_empty() {
            return None;
        }
        
        let total: Duration = self.transaction_latencies.iter().sum();
        Some(total / self.transaction_latencies.len() as u32)
    }
    
    pub fn average_throughput(&self) -> Option<f64> {
        if self.throughput_samples.is_empty() {
            return None;
        }
        
        let total: u64 = self.throughput_samples.iter().map(|(_, tps)| tps).sum();
        Some(total as f64 / self.throughput_samples.len() as f64)
    }
    
    pub fn success_rate(&self) -> f64 {
        if self.total_transactions == 0 {
            return 0.0;
        }
        
        (self.successful_transactions as f64 / self.total_transactions as f64) * 100.0
    }
    
    pub fn total_simulation_time(&self) -> Option<Duration> {
        match (self.start_time, self.end_time) {
            (Some(start), Some(end)) => Some(end - start),
            (Some(start), None) => Some(start.elapsed()),
            _ => None,
        }
    }
    
    pub fn average_leader_election_time(&self) -> Option<Duration> {
        if self.leader_election_times.is_empty() {
            return None;
        }
        
        let total: Duration = self.leader_election_times.iter().sum();
        Some(total / self.leader_election_times.len() as u32)
    }
    
    pub fn get_peak_throughput(&self) -> Option<u64> {
        self.throughput_samples.iter().map(|(_, tps)| *tps).max()
    }
    
    pub fn get_min_latency(&self) -> Option<Duration> {
        self.transaction_latencies.iter().min().copied()
    }
    
    pub fn get_max_latency(&self) -> Option<Duration> {
        self.transaction_latencies.iter().max().copied()
    }
    
    pub fn get_percentile_latency(&self, percentile: f64) -> Option<Duration> {
        if self.transaction_latencies.is_empty() {
            return None;
        }
        
        let mut sorted = self.transaction_latencies.clone();
        sorted.sort();
        
        let index = ((sorted.len() as f64) * percentile / 100.0) as usize;
        sorted.get(index).copied()
    }
    
    pub fn get_active_nodes(&self) -> usize {
        self.node_metrics.len()
    }
    
    pub fn get_total_validation_tasks(&self) -> u64 {
        self.node_metrics.values().map(|m| m.validation_tasks_completed).sum()
    }
    
    pub fn get_total_pulse_count(&self) -> u64 {
        self.node_metrics.values().map(|m| m.pulse_count).sum()
    }
    
    pub fn get_network_efficiency(&self) -> f64 {
        if self.network_messages == 0 {
            return 0.0;
        }
        
        (self.successful_transactions as f64 / self.network_messages as f64) * 100.0
    }
    
    pub fn get_validation_failure_rate(&self) -> f64 {
        let total_validations = self.get_total_validation_tasks() + self.failed_validations;
        if total_validations == 0 {
            return 0.0;
        }
        
        (self.failed_validations as f64 / total_validations as f64) * 100.0
    }
    
    pub fn print_summary(&self) {
        println!("=== Simulation Metrics Summary ===");
        println!("Total Transactions: {}", self.total_transactions);
        println!("Successful: {}", self.successful_transactions);
        println!("Failed: {}", self.failed_transactions);
        println!("Success Rate: {:.2}%", self.success_rate());
        
        if let Some(avg_latency) = self.average_latency() {
            println!("Average Latency: {:?}", avg_latency);
        }
        
        if let Some(min_latency) = self.get_min_latency() {
            println!("Min Latency: {:?}", min_latency);
        }
        
        if let Some(max_latency) = self.get_max_latency() {
            println!("Max Latency: {:?}", max_latency);
        }
        
        if let Some(p95_latency) = self.get_percentile_latency(95.0) {
            println!("95th Percentile Latency: {:?}", p95_latency);
        }
        
        if let Some(avg_throughput) = self.average_throughput() {
            println!("Average Throughput: {:.2} TPS", avg_throughput);
        }
        
        if let Some(peak_throughput) = self.get_peak_throughput() {
            println!("Peak Throughput: {} TPS", peak_throughput);
        }
        
        if let Some(sim_time) = self.total_simulation_time() {
            println!("Total Simulation Time: {:?}", sim_time);
        }
        
        println!("Active Nodes: {}", self.get_active_nodes());
        println!("Leader Elections: {}", self.leader_election_count);
        
        if let Some(avg_election_time) = self.average_leader_election_time() {
            println!("Average Leader Election Time: {:?}", avg_election_time);
        }
        
        println!("Network Messages: {}", self.network_messages);
        println!("Network Efficiency: {:.2}%", self.get_network_efficiency());
        println!("Validation Failure Rate: {:.2}%", self.get_validation_failure_rate());
        println!("Total Validation Tasks: {}", self.get_total_validation_tasks());
        println!("Total Pulse Count: {}", self.get_total_pulse_count());
        println!("===================================");
    }
}

impl NodeMetrics {
    pub fn new(node_id: Uuid) -> Self {
        Self {
            node_id,
            transactions_processed: 0,
            validation_tasks_completed: 0,
            uptime: Duration::from_secs(0),
            response_times: Vec::new(),
            pulse_count: 0,
            last_pulse: None,
        }
    }
    
    pub fn record_transaction(&mut self) {
        self.transactions_processed += 1;
    }
    
    pub fn record_validation_task(&mut self) {
        self.validation_tasks_completed += 1;
    }
    
    pub fn record_pulse(&mut self) {
        self.pulse_count += 1;
        self.last_pulse = Some(Utc::now());
    }
    
    pub fn record_response_time(&mut self, response_time: Duration) {
        self.response_times.push(response_time);
    }
    
    pub fn update_uptime(&mut self, uptime: Duration) {
        self.uptime = uptime;
    }
    
    pub fn average_response_time(&self) -> Option<Duration> {
        if self.response_times.is_empty() {
            return None;
        }
        
        let total: Duration = self.response_times.iter().sum();
        Some(total / self.response_times.len() as u32)
    }
    
    pub fn is_active(&self) -> bool {
        if let Some(last_pulse) = self.last_pulse {
            let time_since_pulse = Utc::now() - last_pulse;
            time_since_pulse.num_seconds() < 60 // Consider active if pulse within 60 seconds
        } else {
            false
        }
    }
} 