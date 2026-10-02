import "@fontsource-variable/inter";
import "@fontsource/cormorant-garamond/500.css";
import "@fontsource/cormorant-garamond/600.css";
import "@fontsource/cormorant-garamond/500-italic.css";
import "@fontsource/cormorant-garamond/600-italic.css";
import "./styles/app.css";
import { mount } from "svelte";
import App from "./App.svelte";


export default mount(App, { target: document.getElementById("app")! });
