use crate::node_spawner::NodeSpawner;
use crate::transaction_generator::TransactionGenerator;
use crate::metrics::SimulationMetrics;
use crate::network::NetworkSimulator;
use crate::BenchmarkScenario;

use pcl_backend::{Node, NodeKeypair, NodeRole, NodeRegistry};
use log::{info, warn, error, debug};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, mpsc};
use tokio::time::{sleep, interval};
use uuid::Uuid;
use rand::Rng;
use indicatif::{ProgressBar, ProgressStyle};

pub struct Simulation {
    pub node_spawner: NodeSpawner,
    pub transaction_generator: TransactionGenerator,
    pub metrics: Arc<RwLock<SimulationMetrics>>,
    pub network: NetworkSimulator,
    pub node_count: u32,
    pub leader_count: u32,
    pub verbose: bool,
    pub active_nodes: Arc<RwLock<HashMap<Uuid, Node>>>,
    pub node_registry: Arc<RwLock<NodeRegistry>>,
}

impl Simulation {
    pub async fn new(node_count: u32, leader_count: u32, verbose: bool) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        info!("Initializing simulation with {} nodes, {} leaders", node_count, leader_count);
        
        let metrics = Arc::new(RwLock::new(SimulationMetrics::new()));
        let active_nodes = Arc::new(RwLock::new(HashMap::new()));
        let node_registry = Arc::new(RwLock::new(NodeRegistry::new()));
        
        let node_spawner = NodeSpawner::new(active_nodes.clone(), node_registry.clone());
        let transaction_generator = TransactionGenerator::new(active_nodes.clone());
        let network = NetworkSimulator::new(active_nodes.clone());
        
        let mut simulation = Self {
            node_spawner,
            transaction_generator,
            metrics,
            network,
            node_count,
            leader_count,
            verbose,
            active_nodes,
            node_registry,
        };
        
        // Spawn initial nodes
        simulation.spawn_initial_nodes().await?;
        
        Ok(simulation)
    }
    
    async fn spawn_initial_nodes(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Spawning {} initial nodes with {} leaders", self.node_count, self.leader_count);
        
        let progress = ProgressBar::new(self.node_count as u64);
        progress.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} nodes spawned ({msg})")
                .unwrap()
                .progress_chars("#>-")
        );
        
        // Spawn regular nodes
        for _i in 0..(self.node_count - self.leader_count) {
            let node = self.node_spawner.spawn_extension_node().await?;
            progress.set_message(format!("Extension node {}", node.id));
            progress.inc(1);
            
            if self.verbose {
                debug!("Spawned extension node: {}", node.id);
            }
        }
        
        // Spawn leader nodes
        for _i in 0..self.leader_count {
            let node = self.node_spawner.spawn_leader_node().await?;
            progress.set_message(format!("Leader node {}", node.id));
            progress.inc(1);
            
            if self.verbose {
                debug!("Spawned leader node: {}", node.id);
            }
        }
        
        progress.finish_with_message("All nodes spawned successfully");
        
        info!("Successfully spawned {} nodes ({} leaders, {} extensions)", 
              self.node_count, self.leader_count, self.node_count - self.leader_count);
        
        Ok(())
    }
    
    pub async fn run_load_test(&mut self, tps: u32, duration: Duration) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Starting load test: {} TPS for {:?}", tps, duration);
        
        let start_time = Instant::now();
        let mut transaction_interval = interval(Duration::from_millis(1000 / tps as u64));
        let total_transactions = (tps as u64 * duration.as_secs()) as u64;
        
        let progress = ProgressBar::new(total_transactions);
        progress.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} transactions ({per_sec}/s, ETA: {eta})")
                .unwrap()
                .progress_chars("#>-")
        );
        
        let (tx_sender, mut tx_receiver) = mpsc::channel::<Result<String, Box<dyn std::error::Error + Send + Sync>>>(1000);
        
        // Start transaction processing task
        let metrics_clone = self.metrics.clone();
        let tx_processing_task = tokio::spawn(async move {
            while let Some(tx_result) = tx_receiver.recv().await {
                let mut metrics = metrics_clone.write().await;
                metrics.record_transaction(tx_result);
            }
        });
        
        // Generate transactions
        let mut transactions_sent = 0u64;
        while start_time.elapsed() < duration && transactions_sent < total_transactions {
            transaction_interval.tick().await;
            
            match self.transaction_generator.generate_random_transaction().await {
                Ok(tx_id) => {
                    transactions_sent += 1;
                    progress.inc(1);
                    
                    if let Err(e) = tx_sender.send(Ok(tx_id)).await {
                        warn!("Failed to send transaction result: {}", e);
                    }
                    
                    if self.verbose && transactions_sent % 100 == 0 {
                        debug!("Generated {} transactions", transactions_sent);
                    }
                },
                Err(e) => {
                    error!("Failed to generate transaction: {}", e);
                    if let Err(e) = tx_sender.send(Err(e)).await {
                        warn!("Failed to send error result: {}", e);
                    }
                }
            }
        }
        
        progress.finish_with_message("Load test completed");
        
        // Close the channel and wait for processing to complete
        drop(tx_sender);
        tx_processing_task.await?;
        
        // Print results
        self.print_results().await;
        
        Ok(())
    }
    
    pub async fn run_stress_test(&mut self, max_nodes: u32, max_tps: u32, phase_duration: Duration) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Starting stress test: scaling to {} nodes, {} TPS", max_nodes, max_tps);
        
        let phases = vec![
            (self.node_count, 100),
            (max_nodes / 4, max_tps / 4),
            (max_nodes / 2, max_tps / 2),
            (max_nodes * 3 / 4, max_tps * 3 / 4),
            (max_nodes, max_tps),
        ];
        
        for (phase_idx, (target_nodes, target_tps)) in phases.iter().enumerate() {
            info!("Stress test phase {}: {} nodes, {} TPS", phase_idx + 1, target_nodes, target_tps);
            
            // Scale nodes to target
            self.scale_to_node_count(*target_nodes).await?;
            
            // Run load test for this phase
            self.run_load_test(*target_tps, phase_duration).await?;
            
            // Brief pause between phases
            sleep(Duration::from_secs(5)).await;
        }
        
        info!("Stress test completed successfully");
        Ok(())
    }
    
    pub async fn run_benchmark(&mut self, scenario: BenchmarkScenario, iterations: u32) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Running benchmark for {:?} with {} iterations", scenario, iterations);
        
        match scenario {
            BenchmarkScenario::LeaderElection => {
                self.benchmark_leader_election(iterations).await?;
            },
            BenchmarkScenario::TransactionProcessing => {
                self.benchmark_transaction_processing(iterations).await?;
            },
            BenchmarkScenario::NetworkGossip => {
                self.benchmark_network_gossip(iterations).await?;
            },
            BenchmarkScenario::MempoolPerformance => {
                self.benchmark_mempool_performance(iterations).await?;
            },
        }
        
        Ok(())
    }
    
    async fn scale_to_node_count(&mut self, target_count: u32) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let current_count = self.active_nodes.read().await.len() as u32;
        
        if target_count > current_count {
            // Spawn additional nodes
            let additional_nodes = target_count - current_count;
            info!("Scaling up: spawning {} additional nodes", additional_nodes);
            
            for _ in 0..additional_nodes {
                let node = if rand::thread_rng().gen_bool(0.2) {
                    self.node_spawner.spawn_leader_node().await?
                } else {
                    self.node_spawner.spawn_extension_node().await?
                };
                
                if self.verbose {
                    debug!("Spawned additional node: {}", node.id);
                }
            }
        } else if target_count < current_count {
            // Remove excess nodes
            let nodes_to_remove = current_count - target_count;
            info!("Scaling down: removing {} nodes", nodes_to_remove);
            
            self.node_spawner.remove_nodes(nodes_to_remove).await?;
        }
        
        self.node_count = target_count;
        Ok(())
    }
    
    async fn benchmark_leader_election(&mut self, iterations: u32) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Benchmarking leader election process");
        
        let mut election_times = Vec::new();
        
        for i in 0..iterations {
            let start = Instant::now();
            
            // Trigger leader election
            self.network.trigger_leader_election().await?;
            
            // Wait for election to complete
            sleep(Duration::from_secs(5)).await;
            
            let elapsed = start.elapsed();
            election_times.push(elapsed);
            
            info!("Leader election iteration {}: {:?}", i + 1, elapsed);
        }
        
        self.print_benchmark_results("Leader Election", &election_times);
        Ok(())
    }
    
    async fn benchmark_transaction_processing(&mut self, iterations: u32) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Benchmarking transaction processing");
        
        let mut processing_times = Vec::new();
        
        for i in 0..iterations {
            let start = Instant::now();
            
            // Generate and process transaction
            let _tx_id = self.transaction_generator.generate_random_transaction().await?;
            
            // Wait for processing to complete
            sleep(Duration::from_millis(100)).await;
            
            let elapsed = start.elapsed();
            processing_times.push(elapsed);
            
            if self.verbose {
                debug!("Transaction processing iteration {}: {:?}", i + 1, elapsed);
            }
        }
        
        self.print_benchmark_results("Transaction Processing", &processing_times);
        Ok(())
    }
    
    async fn benchmark_network_gossip(&mut self, iterations: u32) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Benchmarking network gossip performance");
        
        let mut gossip_times = Vec::new();
        
        for i in 0..iterations {
            let start = Instant::now();
            
            // Trigger gossip message
            self.network.broadcast_test_message().await?;
            
            // Wait for propagation
            sleep(Duration::from_millis(50)).await;
            
            let elapsed = start.elapsed();
            gossip_times.push(elapsed);
            
            if self.verbose {
                debug!("Gossip iteration {}: {:?}", i + 1, elapsed);
            }
        }
        
        self.print_benchmark_results("Network Gossip", &gossip_times);
        Ok(())
    }
    
    async fn benchmark_mempool_performance(&mut self, iterations: u32) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Benchmarking mempool performance");
        
        // Generate multiple transactions to fill mempool
        for _ in 0..1000 {
            let _tx_id = self.transaction_generator.generate_random_transaction().await?;
        }
        
        let mut lookup_times = Vec::new();
        
        for i in 0..iterations {
            let start = Instant::now();
            
            // Perform mempool operations
            self.network.query_mempool_status().await?;
            
            let elapsed = start.elapsed();
            lookup_times.push(elapsed);
            
            if self.verbose {
                debug!("Mempool query iteration {}: {:?}", i + 1, elapsed);
            }
        }
        
        self.print_benchmark_results("Mempool Performance", &lookup_times);
        Ok(())
    }
    
    fn print_benchmark_results(&self, benchmark_name: &str, times: &[Duration]) {
        if times.is_empty() {
            return;
        }
        
        let total: Duration = times.iter().sum();
        let average = total / times.len() as u32;
        let min = times.iter().min().unwrap();
        let max = times.iter().max().unwrap();
        
        info!("=== {} Benchmark Results ===", benchmark_name);
        info!("Iterations: {}", times.len());
        info!("Average: {:?}", average);
        info!("Min: {:?}", min);
        info!("Max: {:?}", max);
        info!("Total: {:?}", total);
        info!("=======================================");
    }
    
    async fn print_results(&self) {
        let metrics = self.metrics.read().await;
        let active_nodes = self.active_nodes.read().await;
        
        info!("=== Simulation Results ===");
        info!("Active nodes: {}", active_nodes.len());
        info!("Total transactions: {}", metrics.total_transactions);
        info!("Successful transactions: {}", metrics.successful_transactions);
        info!("Failed transactions: {}", metrics.failed_transactions);
        
        if metrics.total_transactions > 0 {
            let success_rate = (metrics.successful_transactions as f64 / metrics.total_transactions as f64) * 100.0;
            info!("Success rate: {:.2}%", success_rate);
        }
        
        if let Some(avg_latency) = metrics.average_latency() {
            info!("Average transaction latency: {:?}", avg_latency);
        }
        
        info!("==========================");
    }
} 