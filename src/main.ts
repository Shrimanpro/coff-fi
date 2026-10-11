import "@fontsource-variable/inter";
import "@fontsource/cormorant-garamond/500.css";
import "@fontsource/cormorant-garamond/600.css";
import "@fontsource/cormorant-garamond/500-italic.css";
import "@fontsource/cormorant-garamond/600-italic.css";
import "./styles/app.css";
import { mount } from "svelte";
import App from "./App.svelte";

// The app has its own context menus; suppress the webview's default one.
document.addEventListener("contextmenu", (e) => e.preventDefault());


export default mount(App, { target: document.getElementById("app")! });
