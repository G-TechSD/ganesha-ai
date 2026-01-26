<template>
  <div>
    <h1>Hello from MyComponent!</h1>
    <p>{{ message }}</p>
  </div>
</template>

<script>
export default {
  data() {
    return {
      message: 'This is a Vue component.'
    }
  }
}
</script>

<style scoped>
h1 {
  color: blue;
}
</style>
