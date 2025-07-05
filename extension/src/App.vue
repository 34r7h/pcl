<template>
  <div id="app">
    <!-- Navigation Bar -->
    <nav class="navbar navbar-expand-lg navbar-dark bg-primary">
      <div class="container-fluid">
        <router-link class="navbar-brand" to="/">
          <i class="fas fa-network-wired me-2"></i>
          PCL Extension
        </router-link>
        
        <button class="navbar-toggler" type="button" data-bs-toggle="collapse" data-bs-target="#navbarNav">
          <span class="navbar-toggler-icon"></span>
        </button>
        
        <div class="collapse navbar-collapse" id="navbarNav">
          <ul class="navbar-nav me-auto">
            <li class="nav-item">
              <router-link class="nav-link" to="/">
                <i class="fas fa-tachometer-alt me-1"></i>
                Dashboard
              </router-link>
            </li>
            <li class="nav-item">
              <router-link class="nav-link" to="/transactions">
                <i class="fas fa-exchange-alt me-1"></i>
                Transactions
              </router-link>
            </li>
            <li class="nav-item">
              <router-link class="nav-link" to="/nodes">
                <i class="fas fa-server me-1"></i>
                Nodes
              </router-link>
            </li>
            <li class="nav-item">
              <router-link class="nav-link" to="/mempool">
                <i class="fas fa-database me-1"></i>
                Mempool
              </router-link>
            </li>
            <li class="nav-item">
              <router-link class="nav-link" to="/consensus">
                <i class="fas fa-vote-yea me-1"></i>
                Consensus
              </router-link>
            </li>
            <li class="nav-item">
              <router-link class="nav-link" to="/network">
                <i class="fas fa-project-diagram me-1"></i>
                Network
              </router-link>
            </li>
          </ul>
          
          <!-- Node Status -->
          <div class="navbar-text me-3">
            <span class="badge bg-success me-2" v-if="nodeStatus.connected">
              <i class="fas fa-circle"></i> Connected
            </span>
            <span class="badge bg-danger me-2" v-else>
              <i class="fas fa-circle"></i> Disconnected
            </span>
            Node: {{ nodeStatus.nodeId || 'Unknown' }}
          </div>
          
          <!-- Settings Dropdown -->
          <div class="dropdown">
            <button class="btn btn-outline-light dropdown-toggle" type="button" data-bs-toggle="dropdown">
              <i class="fas fa-cog"></i>
            </button>
            <ul class="dropdown-menu dropdown-menu-end">
              <li><router-link class="dropdown-item" to="/settings">Settings</router-link></li>
              <li><hr class="dropdown-divider"></li>
              <li><a class="dropdown-item" href="#" @click="exportData">Export Data</a></li>
              <li><a class="dropdown-item" href="#" @click="clearCache">Clear Cache</a></li>
            </ul>
          </div>
        </div>
      </div>
    </nav>

    <!-- Main Content -->
    <main class="container-fluid py-4">
      <router-view />
    </main>

    <!-- Footer -->
    <footer class="bg-light text-center py-3 mt-5">
      <div class="container">
        <span class="text-muted">
          PCL Extension v{{ version }} | 
          <span v-if="systemStats.activeTransactions !== null">
            Active Transactions: {{ systemStats.activeTransactions }} | 
          </span>
          <span v-if="systemStats.connectedPeers !== null">
            Connected Peers: {{ systemStats.connectedPeers }}
          </span>
        </span>
      </div>
    </footer>

    <!-- Loading Overlay -->
    <div v-if="loading" class="loading-overlay">
      <div class="spinner-border text-primary" role="status">
        <span class="visually-hidden">Loading...</span>
      </div>
    </div>

    <!-- Toast Container -->
    <div class="toast-container position-fixed bottom-0 end-0 p-3">
      <div v-for="toast in toasts" :key="toast.id" class="toast show" role="alert">
        <div class="toast-header">
          <i :class="getToastIcon(toast.type)" class="me-2"></i>
          <strong class="me-auto">{{ toast.title }}</strong>
          <small>{{ toast.time }}</small>
          <button type="button" class="btn-close" @click="removeToast(toast.id)"></button>
        </div>
        <div class="toast-body">
          {{ toast.message }}
        </div>
      </div>
    </div>
  </div>
</template>

<script>
import { mapState, mapActions } from 'vuex'

export default {
  name: 'App',
  data() {
    return {
      version: '0.1.0'
    }
  },
  computed: {
    ...mapState(['nodeStatus', 'systemStats', 'loading', 'toasts'])
  },
  methods: {
    ...mapActions(['fetchNodeStatus', 'fetchSystemStats', 'showToast', 'removeToast']),
    
    getToastIcon(type) {
      const icons = {
        success: 'fas fa-check-circle text-success',
        error: 'fas fa-exclamation-circle text-danger',
        warning: 'fas fa-exclamation-triangle text-warning',
        info: 'fas fa-info-circle text-info'
      }
      return icons[type] || icons.info
    },
    
    exportData() {
      this.showToast({
        type: 'info',
        title: 'Export Data',
        message: 'Data export functionality will be implemented.'
      })
    },
    
    clearCache() {
      localStorage.clear()
      this.showToast({
        type: 'success',
        title: 'Cache Cleared',
        message: 'Local cache has been cleared successfully.'
      })
    }
  },
  async mounted() {
    // Initialize app data
    try {
      await this.fetchNodeStatus()
      await this.fetchSystemStats()
      
      // Set up periodic updates
      setInterval(() => {
        this.fetchNodeStatus()
        this.fetchSystemStats()
      }, 5000) // Update every 5 seconds
      
    } catch (error) {
      this.showToast({
        type: 'error',
        title: 'Connection Error',
        message: 'Failed to connect to PCL backend.'
      })
    }
  }
}
</script>

<style scoped>
#app {
  font-family: Avenir, Helvetica, Arial, sans-serif;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  min-height: 100vh;
  display: flex;
  flex-direction: column;
}

main {
  flex: 1;
}

.loading-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  background-color: rgba(0, 0, 0, 0.5);
  display: flex;
  justify-content: center;
  align-items: center;
  z-index: 9999;
}

.navbar-brand {
  font-weight: bold;
}

.nav-link.router-link-active {
  font-weight: bold;
  background-color: rgba(255, 255, 255, 0.1);
  border-radius: 0.25rem;
}

.toast {
  min-width: 300px;
}
</style> 