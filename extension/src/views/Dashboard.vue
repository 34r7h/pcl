<template>
  <div class="dashboard">
    <!-- Header -->
    <div class="row mb-4">
      <div class="col">
        <h1 class="h2 mb-1">
          <i class="fas fa-tachometer-alt me-2"></i>
          Dashboard
        </h1>
        <p class="text-muted">Real-time overview of the Peer Consensus Layer system</p>
      </div>
      <div class="col-auto">
        <button class="btn btn-primary" @click="refreshData">
          <i class="fas fa-sync-alt" :class="{ 'fa-spin': loading }"></i>
          Refresh
        </button>
      </div>
    </div>

    <!-- Status Cards -->
    <div class="row mb-4">
      <div class="col-lg-3 col-md-6 mb-3">
        <div class="card status-card bg-primary text-white">
          <div class="card-body">
            <div class="d-flex align-items-center">
              <div class="flex-grow-1">
                <h6 class="card-subtitle mb-1">Node Status</h6>
                <h3 class="card-title mb-0">
                  {{ nodeStatus.connected ? 'Connected' : 'Disconnected' }}
                </h3>
              </div>
              <div class="icon">
                <i class="fas fa-server fa-2x opacity-75"></i>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div class="col-lg-3 col-md-6 mb-3">
        <div class="card status-card bg-success text-white">
          <div class="card-body">
            <div class="d-flex align-items-center">
              <div class="flex-grow-1">
                <h6 class="card-subtitle mb-1">Active Transactions</h6>
                <h3 class="card-title mb-0">{{ activeTransactionCount }}</h3>
              </div>
              <div class="icon">
                <i class="fas fa-exchange-alt fa-2x opacity-75"></i>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div class="col-lg-3 col-md-6 mb-3">
        <div class="card status-card bg-info text-white">
          <div class="card-body">
            <div class="d-flex align-items-center">
              <div class="flex-grow-1">
                <h6 class="card-subtitle mb-1">Connected Peers</h6>
                <h3 class="card-title mb-0">{{ systemStats.connectedPeers || 0 }}</h3>
              </div>
              <div class="icon">
                <i class="fas fa-users fa-2x opacity-75"></i>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div class="col-lg-3 col-md-6 mb-3">
        <div class="card status-card bg-warning text-white">
          <div class="card-body">
            <div class="d-flex align-items-center">
              <div class="flex-grow-1">
                <h6 class="card-subtitle mb-1">Mempool Size</h6>
                <h3 class="card-title mb-0">{{ totalMempoolSize }}</h3>
              </div>
              <div class="icon">
                <i class="fas fa-database fa-2x opacity-75"></i>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Charts Row -->
    <div class="row mb-4">
      <div class="col-lg-8 mb-3">
        <div class="card h-100">
          <div class="card-header">
            <h5 class="card-title mb-0">
              <i class="fas fa-chart-line me-2"></i>
              Transaction Throughput
            </h5>
          </div>
          <div class="card-body">
            <canvas ref="throughputChart" height="300"></canvas>
          </div>
        </div>
      </div>

      <div class="col-lg-4 mb-3">
        <div class="card h-100">
          <div class="card-header">
            <h5 class="card-title mb-0">
              <i class="fas fa-chart-pie me-2"></i>
              Node Distribution
            </h5>
          </div>
          <div class="card-body">
            <canvas ref="nodeChart" height="300"></canvas>
          </div>
        </div>
      </div>
    </div>

    <!-- System Info Row -->
    <div class="row mb-4">
      <div class="col-lg-6 mb-3">
        <div class="card">
          <div class="card-header">
            <h5 class="card-title mb-0">
              <i class="fas fa-vote-yea me-2"></i>
              Consensus Status
            </h5>
          </div>
          <div class="card-body">
            <div class="row">
              <div class="col-sm-6">
                <strong>Phase:</strong>
                <span class="badge bg-success ms-2">
                  {{ consensusData.phase || 'Normal Operation' }}
                </span>
              </div>
              <div class="col-sm-6">
                <strong>Election Round:</strong>
                <span class="ms-2">{{ consensusData.electionRound || 0 }}</span>
              </div>
            </div>
            
            <hr>
            
            <h6>Current Leaders:</h6>
            <div v-if="consensusData.currentLeaders && consensusData.currentLeaders.length">
              <div v-for="leader in consensusData.currentLeaders" :key="leader" class="badge bg-primary me-1 mb-1">
                {{ leader }}
              </div>
            </div>
            <div v-else>
              <span class="text-muted">No leaders elected</span>
            </div>
          </div>
        </div>
      </div>

      <div class="col-lg-6 mb-3">
        <div class="card">
          <div class="card-header">
            <h5 class="card-title mb-0">
              <i class="fas fa-heartbeat me-2"></i>
              System Health
            </h5>
          </div>
          <div class="card-body">
            <div class="mb-3">
              <label class="form-label">Network Health</label>
              <div class="progress">
                <div 
                  class="progress-bar" 
                  :class="getHealthClass(networkHealth)"
                  :style="{ width: networkHealth + '%' }"
                >
                  {{ networkHealth }}%
                </div>
              </div>
            </div>
            
            <div class="mb-3">
              <label class="form-label">System Load</label>
              <div class="progress">
                <div 
                  class="progress-bar bg-info" 
                  :style="{ width: (systemStats.systemLoad || 0) + '%' }"
                >
                  {{ (systemStats.systemLoad || 0) }}%
                </div>
              </div>
            </div>
            
            <div class="row text-center">
              <div class="col">
                <small class="text-muted">Uptime</small>
                <div class="h6">{{ formatUptime(nodeStatus.uptime) }}</div>
              </div>
              <div class="col">
                <small class="text-muted">Response Time</small>
                <div class="h6">{{ formatResponseTime(50) }}ms</div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Recent Activity -->
    <div class="row">
      <div class="col-lg-6 mb-3">
        <div class="card">
          <div class="card-header d-flex justify-content-between align-items-center">
            <h5 class="card-title mb-0">
              <i class="fas fa-history me-2"></i>
              Recent Transactions
            </h5>
            <router-link to="/transactions" class="btn btn-sm btn-outline-primary">
              View All
            </router-link>
          </div>
          <div class="card-body p-0">
            <div v-if="recentTransactions.length" class="list-group list-group-flush">
              <div 
                v-for="tx in recentTransactions" 
                :key="tx.id" 
                class="list-group-item d-flex justify-content-between align-items-center"
              >
                <div>
                  <strong>{{ tx.id.substring(0, 8) }}...</strong>
                  <br>
                  <small class="text-muted">{{ formatTime(tx.timestamp) }}</small>
                </div>
                <span class="badge" :class="getStatusClass(tx.status)">
                  {{ tx.status }}
                </span>
              </div>
            </div>
            <div v-else class="p-3 text-center text-muted">
              No recent transactions
            </div>
          </div>
        </div>
      </div>

      <div class="col-lg-6 mb-3">
        <div class="card">
          <div class="card-header d-flex justify-content-between align-items-center">
            <h5 class="card-title mb-0">
              <i class="fas fa-bell me-2"></i>
              System Events
            </h5>
            <button class="btn btn-sm btn-outline-secondary" @click="clearEvents">
              Clear
            </button>
          </div>
          <div class="card-body p-0">
            <div v-if="systemEvents.length" class="list-group list-group-flush">
              <div 
                v-for="event in systemEvents" 
                :key="event.id" 
                class="list-group-item"
              >
                <div class="d-flex align-items-center">
                  <i :class="getEventIcon(event.type)" class="me-2"></i>
                  <div class="flex-grow-1">
                    <strong>{{ event.title }}</strong>
                    <br>
                    <small class="text-muted">{{ event.message }}</small>
                  </div>
                  <small class="text-muted">{{ formatTime(event.timestamp) }}</small>
                </div>
              </div>
            </div>
            <div v-else class="p-3 text-center text-muted">
              No recent events
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script>
import { mapState, mapGetters, mapActions } from 'vuex'
import Chart from 'chart.js/auto'
import moment from 'moment'

export default {
  name: 'Dashboard',
  data() {
    return {
      charts: {},
      systemEvents: [
        {
          id: 1,
          type: 'info',
          title: 'Node Connected',
          message: 'Successfully connected to peer network',
          timestamp: new Date()
        },
        {
          id: 2,
          type: 'success',
          title: 'Leader Election',
          message: 'New leaders elected for round 42',
          timestamp: new Date(Date.now() - 300000)
        }
      ]
    }
  },
  computed: {
    ...mapState(['nodeStatus', 'systemStats', 'loading', 'transactions', 'consensusData']),
    ...mapGetters([
      'activeTransactionCount', 
      'totalMempoolSize', 
      'networkHealth'
    ]),
    
    recentTransactions() {
      return this.transactions
        .slice(0, 5)
        .sort((a, b) => new Date(b.timestamp) - new Date(a.timestamp))
    }
  },
  methods: {
    ...mapActions([
      'fetchNodeStatus', 
      'fetchSystemStats', 
      'fetchTransactions', 
      'fetchConsensusData'
    ]),
    
    async refreshData() {
      await Promise.all([
        this.fetchNodeStatus(),
        this.fetchSystemStats(),
        this.fetchTransactions(),
        this.fetchConsensusData()
      ])
      this.updateCharts()
    },
    
    initCharts() {
      this.initThroughputChart()
      this.initNodeChart()
    },
    
    initThroughputChart() {
      const ctx = this.$refs.throughputChart.getContext('2d')
      this.charts.throughput = new Chart(ctx, {
        type: 'line',
        data: {
          labels: Array.from({ length: 10 }, (_, i) => moment().subtract(i * 5, 'minutes').format('HH:mm')).reverse(),
          datasets: [{
            label: 'Transactions/min',
            data: Array.from({ length: 10 }, () => Math.floor(Math.random() * 20) + 5),
            borderColor: 'rgb(75, 192, 192)',
            backgroundColor: 'rgba(75, 192, 192, 0.1)',
            tension: 0.4
          }]
        },
        options: {
          responsive: true,
          maintainAspectRatio: false,
          scales: {
            y: {
              beginAtZero: true
            }
          }
        }
      })
    },
    
    initNodeChart() {
      const ctx = this.$refs.nodeChart.getContext('2d')
      this.charts.nodes = new Chart(ctx, {
        type: 'doughnut',
        data: {
          labels: ['Leaders', 'Validators', 'Extensions'],
          datasets: [{
            data: [3, 2, 8],
            backgroundColor: [
              '#FF6384',
              '#36A2EB',
              '#FFCE56'
            ]
          }]
        },
        options: {
          responsive: true,
          maintainAspectRatio: false
        }
      })
    },
    
    updateCharts() {
      // Update with real data when available
      if (this.charts.throughput) {
        // Update throughput chart with real data
      }
      if (this.charts.nodes) {
        // Update node distribution with real data
      }
    },
    
    getHealthClass(health) {
      if (health >= 80) return 'bg-success'
      if (health >= 60) return 'bg-warning'
      return 'bg-danger'
    },
    
    getStatusClass(status) {
      const classes = {
        'completed': 'bg-success',
        'processing': 'bg-warning',
        'validating': 'bg-info',
        'failed': 'bg-danger',
        'pending': 'bg-secondary'
      }
      return classes[status] || 'bg-secondary'
    },
    
    getEventIcon(type) {
      const icons = {
        'success': 'fas fa-check-circle text-success',
        'error': 'fas fa-exclamation-circle text-danger',
        'warning': 'fas fa-exclamation-triangle text-warning',
        'info': 'fas fa-info-circle text-info'
      }
      return icons[type] || icons.info
    },
    
    formatTime(timestamp) {
      return moment(timestamp).fromNow()
    },
    
    formatUptime(seconds) {
      if (!seconds) return '0h 0m'
      const hours = Math.floor(seconds / 3600)
      const minutes = Math.floor((seconds % 3600) / 60)
      return `${hours}h ${minutes}m`
    },
    
    formatResponseTime(ms) {
      return ms ? ms.toFixed(0) : '0'
    },
    
    clearEvents() {
      this.systemEvents = []
    }
  },
  
  async mounted() {
    await this.refreshData()
    this.initCharts()
    
    // Set up auto-refresh
    this.refreshInterval = setInterval(() => {
      this.refreshData()
    }, 30000) // Refresh every 30 seconds
  },
  
  beforeUnmount() {
    if (this.refreshInterval) {
      clearInterval(this.refreshInterval)
    }
    
    // Destroy charts
    Object.values(this.charts).forEach(chart => {
      if (chart) chart.destroy()
    })
  }
}
</script>

<style scoped>
.status-card {
  border: none;
  border-radius: 0.5rem;
  box-shadow: 0 0.125rem 0.25rem rgba(0, 0, 0, 0.075);
}

.status-card .icon {
  flex-shrink: 0;
}

.card {
  border: none;
  box-shadow: 0 0.125rem 0.25rem rgba(0, 0, 0, 0.075);
}

.card-header {
  background-color: #f8f9fa;
  border-bottom: 1px solid #dee2e6;
}

.progress {
  height: 1rem;
}

.list-group-item {
  border-left: none;
  border-right: none;
}

.list-group-item:first-child {
  border-top: none;
}

.list-group-item:last-child {
  border-bottom: none;
}

canvas {
  max-height: 300px;
}
</style> 