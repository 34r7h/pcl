import { createRouter, createWebHistory } from 'vue-router'
import Dashboard from '../views/Dashboard.vue'

const routes = [
  {
    path: '/',
    name: 'Dashboard',
    component: Dashboard
  },
  {
    path: '/transactions',
    name: 'Transactions',
    component: () => import('../views/Transactions.vue')
  },
  {
    path: '/transaction/:id',
    name: 'TransactionDetail',
    component: () => import('../views/TransactionDetail.vue')
  },
  {
    path: '/nodes',
    name: 'Nodes',
    component: () => import('../views/Nodes.vue')
  },
  {
    path: '/node/:id',
    name: 'NodeDetail',
    component: () => import('../views/NodeDetail.vue')
  },
  {
    path: '/mempool',
    name: 'Mempool',
    component: () => import('../views/Mempool.vue')
  },
  {
    path: '/consensus',
    name: 'Consensus',
    component: () => import('../views/Consensus.vue')
  },
  {
    path: '/network',
    name: 'Network',
    component: () => import('../views/Network.vue')
  },
  {
    path: '/settings',
    name: 'Settings',
    component: () => import('../views/Settings.vue')
  },
  {
    path: '/create-transaction',
    name: 'CreateTransaction',
    component: () => import('../views/CreateTransaction.vue')
  },
  {
    path: '/:pathMatch(.*)*',
    name: 'NotFound',
    component: () => import('../views/NotFound.vue')
  }
]

const router = createRouter({
  history: createWebHistory(process.env.BASE_URL),
  routes,
  linkActiveClass: 'router-link-active'
})

export default router 