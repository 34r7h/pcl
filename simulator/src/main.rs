use clap::{Parser, Subcommand};
use log::info;
use std::time::Duration;
use tokio::time::sleep;

mod simulation;
mod node_spawner;
mod transaction_generator;
mod metrics;
mod network;

use simulation::Simulation;

#[derive(Parser)]
#[command(name = "pcl-simulator")]
#[command(about = "Peer Consensus Layer Transaction Load Simulator")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a transaction load simulation
    LoadTest {
        /// Number of nodes to spawn
        #[arg(short, long, default_value_t = 10)]
        nodes: u32,
        
        /// Number of leader nodes
        #[arg(short, long, default_value_t = 3)]
        leaders: u32,
        
        /// Transactions per second to generate
        #[arg(short, long, default_value_t = 100)]
        tps: u32,
        
        /// Duration of simulation in seconds
        #[arg(short, long, default_value_t = 60)]
        duration: u64,
        
        /// Enable verbose logging
        #[arg(short, long)]
        verbose: bool,
    },
    /// Stress test the system with high load
    StressTest {
        /// Maximum nodes to spawn
        #[arg(short, long, default_value_t = 100)]
        max_nodes: u32,
        
        /// Maximum transactions per second
        #[arg(short, long, default_value_t = 1000)]
        max_tps: u32,
        
        /// Duration of each test phase in seconds
        #[arg(short, long, default_value_t = 30)]
        phase_duration: u64,
    },
    /// Benchmark specific scenarios
    Benchmark {
        /// Scenario to benchmark
        #[arg(short, long, value_enum)]
        scenario: BenchmarkScenario,
        
        /// Number of iterations
        #[arg(short, long, default_value_t = 5)]
        iterations: u32,
    },
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum BenchmarkScenario {
    LeaderElection,
    TransactionProcessing,
    NetworkGossip,
    MempoolPerformance,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cli = Cli::parse();
    
    // Initialize logging
    env_logger::init();
    
    info!("Starting PCL Simulator");
    
    match cli.command {
        Commands::LoadTest { nodes, leaders, tps, duration, verbose } => {
            info!("Running load test with {} nodes, {} leaders, {} TPS for {} seconds", 
                  nodes, leaders, tps, duration);
            
            let mut simulation = Simulation::new(nodes, leaders, verbose).await?;
            simulation.run_load_test(tps, Duration::from_secs(duration)).await?;
        },
        Commands::StressTest { max_nodes, max_tps, phase_duration } => {
            info!("Running stress test up to {} nodes, {} TPS, {} second phases", 
                  max_nodes, max_tps, phase_duration);
                  
            let mut simulation = Simulation::new(10, 3, true).await?;
            simulation.run_stress_test(max_nodes, max_tps, Duration::from_secs(phase_duration)).await?;
        },
        Commands::Benchmark { scenario, iterations } => {
            info!("Running benchmark for {:?} with {} iterations", scenario, iterations);
            
            let mut simulation = Simulation::new(20, 5, true).await?;
            simulation.run_benchmark(scenario, iterations).await?;
        },
    }
    
    info!("Simulation completed successfully");
    Ok(())
} 