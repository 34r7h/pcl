import { createApp } from "vue";
import App from "./App.vue";
import router from "./router";
import store from "./store";

// Bootstrap CSS
import "bootstrap/dist/css/bootstrap.min.css";
import "bootstrap";

// Font Awesome
import "@fortawesome/fontawesome-free/css/all.min.css";

// Global CSS
import "./assets/styles.css";

const app = createApp(App);

app.use(store);
app.use(router);

app.mount("#app");
