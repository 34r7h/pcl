<template>
  <div class="not-found">
    <div class="error-state">
      <i class="fas fa-exclamation-triangle"></i>
      <h2>Page Not Found</h2>
      <p class="text-muted">The page you're looking for doesn't exist.</p>
      <router-link to="/" class="btn btn-primary">
        <i class="fas fa-home me-2"></i>
        Go to Dashboard
      </router-link>
    </div>
  </div>
</template>

<script>
export default {
  name: 'NotFound'
}
</script>

<style scoped>
.not-found {
  min-height: 60vh;
  display: flex;
  align-items: center;
  justify-content: center;
}
</style> 