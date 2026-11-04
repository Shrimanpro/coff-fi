import "@fontsource-variable/inter";
import "@fontsource/cormorant-garamond/500.css";
import "@fontsource/cormorant-garamond/600.css";
import "@fontsource/cormorant-garamond/500-italic.css";
import "@fontsource/cormorant-garamond/600-italic.css";
import "./styles/app.css";
import { invoke } from "@tauri-apps/api/core";
import { mount } from "svelte";

// The app has its own context menus; suppress the webview's default one.
document.addEventListener("contextmenu", (e) => e.preventDefault());

// iOS gets the touch layout (src/mobile); desktop keeps the original window layout.
const platform = await invoke<string>("get_platform").catch(() => "desktop");
document.documentElement.dataset.platform = platform;
// iOS WebKit only applies :active (press states) when a touchstart listener exists.
if (platform === "ios") document.addEventListener("touchstart", () => {}, { passive: true });
const Root = platform === "ios" ? (await import("./mobile/MobileApp.svelte")).default : (await import("./App.svelte")).default;

export default mount(Root, { target: document.getElementById("app")! });
