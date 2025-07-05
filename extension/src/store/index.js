import { createStore } from 'vuex'
import axios from 'axios'

// API configuration
const API_BASE_URL = process.env.VUE_APP_API_URL || 'http://localhost:8080/api'

const api = axios.create({
  baseURL: API_BASE_URL,
  timeout: 10000
})

export default createStore({
  state: {
    // System state
    loading: false,
    error: null,
    
    // Node status
    nodeStatus: {
      connected: false,
      nodeId: null,
      role: null,
      ip: null,
      uptime: null
    },
    
    // System statistics
    systemStats: {
      activeTransactions: null,
      connectedPeers: null,
      mempoolSize: null,
      consensusPhase: null,
      networkHealth: null
    },
    
    // Data collections
    transactions: [],
    nodes: [],
    mempoolData: {
      rawTx: [],
      validationTasks: [],
      lockedUtxo: [],
      processingTx: [],
      finalizedTx: [],
      uptime: []
    },
    
    // Consensus data
    consensusData: {
      currentLeaders: [],
      electionRound: 0,
      lastElection: null,
      pulseData: []
    },
    
    // Network data
    networkData: {
      peers: [],
      messageHistory: [],
      networkStats: {}
    },
    
    // UI state
    toasts: []
  },
  
  mutations: {
    SET_LOADING(state, loading) {
      state.loading = loading
    },
    
    SET_ERROR(state, error) {
      state.error = error
    },
    
    SET_NODE_STATUS(state, status) {
      state.nodeStatus = { ...state.nodeStatus, ...status }
    },
    
    SET_SYSTEM_STATS(state, stats) {
      state.systemStats = { ...state.systemStats, ...stats }
    },
    
    SET_TRANSACTIONS(state, transactions) {
      state.transactions = transactions
    },
    
    ADD_TRANSACTION(state, transaction) {
      const index = state.transactions.findIndex(tx => tx.id === transaction.id)
      if (index >= 0) {
        state.transactions.splice(index, 1, transaction)
      } else {
        state.transactions.push(transaction)
      }
    },
    
    SET_NODES(state, nodes) {
      state.nodes = nodes
    },
    
    ADD_NODE(state, node) {
      const index = state.nodes.findIndex(n => n.id === node.id)
      if (index >= 0) {
        state.nodes.splice(index, 1, node)
      } else {
        state.nodes.push(node)
      }
    },
    
    SET_MEMPOOL_DATA(state, data) {
      state.mempoolData = { ...state.mempoolData, ...data }
    },
    
    SET_CONSENSUS_DATA(state, data) {
      state.consensusData = { ...state.consensusData, ...data }
    },
    
    SET_NETWORK_DATA(state, data) {
      state.networkData = { ...state.networkData, ...data }
    },
    
    ADD_TOAST(state, toast) {
      const id = Date.now()
      state.toasts.push({
        id,
        time: new Date().toLocaleTimeString(),
        ...toast
      })
    },
    
    REMOVE_TOAST(state, id) {
      state.toasts = state.toasts.filter(toast => toast.id !== id)
    }
  },
  
  actions: {
    async fetchNodeStatus({ commit }) {
      try {
        commit('SET_LOADING', true)
        const response = await api.get('/node/status')
        commit('SET_NODE_STATUS', {
          connected: true,
          ...response.data
        })
      } catch (error) {
        commit('SET_NODE_STATUS', { connected: false })
        commit('SET_ERROR', error.message)
      } finally {
        commit('SET_LOADING', false)
      }
    },
    
    async fetchSystemStats({ commit }) {
      try {
        const response = await api.get('/system/stats')
        commit('SET_SYSTEM_STATS', response.data)
      } catch (error) {
        commit('SET_ERROR', error.message)
      }
    },
    
    async fetchTransactions({ commit }) {
      try {
        commit('SET_LOADING', true)
        const response = await api.get('/transactions')
        commit('SET_TRANSACTIONS', response.data)
      } catch (error) {
        commit('SET_ERROR', error.message)
      } finally {
        commit('SET_LOADING', false)
      }
    },
    
    async fetchTransaction({ commit }, id) {
      try {
        const response = await api.get(`/transactions/${id}`)
        commit('ADD_TRANSACTION', response.data)
        return response.data
      } catch (error) {
        commit('SET_ERROR', error.message)
        throw error
      }
    },
    
    async createTransaction({ commit, dispatch }, transactionData) {
      try {
        commit('SET_LOADING', true)
        const response = await api.post('/transactions', transactionData)
        commit('ADD_TRANSACTION', response.data)
        dispatch('showToast', {
          type: 'success',
          title: 'Transaction Created',
          message: `Transaction ${response.data.id} created successfully`
        })
        return response.data
      } catch (error) {
        commit('SET_ERROR', error.message)
        dispatch('showToast', {
          type: 'error',
          title: 'Transaction Failed',
          message: error.message
        })
        throw error
      } finally {
        commit('SET_LOADING', false)
      }
    },
    
    async fetchNodes({ commit }) {
      try {
        commit('SET_LOADING', true)
        const response = await api.get('/nodes')
        commit('SET_NODES', response.data)
      } catch (error) {
        commit('SET_ERROR', error.message)
      } finally {
        commit('SET_LOADING', false)
      }
    },
    
    async fetchNode({ commit }, id) {
      try {
        const response = await api.get(`/nodes/${id}`)
        commit('ADD_NODE', response.data)
        return response.data
      } catch (error) {
        commit('SET_ERROR', error.message)
        throw error
      }
    },
    
    async fetchMempoolData({ commit }) {
      try {
        const response = await api.get('/mempool')
        commit('SET_MEMPOOL_DATA', response.data)
      } catch (error) {
        commit('SET_ERROR', error.message)
      }
    },
    
    async fetchConsensusData({ commit }) {
      try {
        const response = await api.get('/consensus')
        commit('SET_CONSENSUS_DATA', response.data)
      } catch (error) {
        commit('SET_ERROR', error.message)
      }
    },
    
    async fetchNetworkData({ commit }) {
      try {
        const response = await api.get('/network')
        commit('SET_NETWORK_DATA', response.data)
      } catch (error) {
        commit('SET_ERROR', error.message)
      }
    },
    
    showToast({ commit }, toast) {
      commit('ADD_TOAST', toast)
      // Auto-remove toast after 5 seconds
      setTimeout(() => {
        commit('REMOVE_TOAST', toast.id)
      }, 5000)
    },
    
    removeToast({ commit }, id) {
      commit('REMOVE_TOAST', id)
    }
  },
  
  getters: {
    isConnected: state => state.nodeStatus.connected,
    
    activeTransactionCount: state => state.transactions.filter(tx => 
      tx.status === 'processing' || tx.status === 'validating'
    ).length,
    
    completedTransactionCount: state => state.transactions.filter(tx => 
      tx.status === 'completed'
    ).length,
    
    failedTransactionCount: state => state.transactions.filter(tx => 
      tx.status === 'failed'
    ).length,
    
    leaderNodes: state => state.nodes.filter(node => node.role === 'leader'),
    
    validatorNodes: state => state.nodes.filter(node => node.role === 'validator'),
    
    extensionNodes: state => state.nodes.filter(node => node.role === 'extension'),
    
    totalMempoolSize: state => {
      const mempool = state.mempoolData
      return (mempool.rawTx?.length || 0) +
             (mempool.validationTasks?.length || 0) +
             (mempool.lockedUtxo?.length || 0) +
             (mempool.processingTx?.length || 0) +
             (mempool.finalizedTx?.length || 0)
    },
    
    networkHealth: state => {
      const stats = state.networkData.networkStats
      if (!stats.connectedPeers) return 0
      return Math.min(100, (stats.connectedPeers / 10) * 100) // Assume 10 is good
    }
  }
}) 