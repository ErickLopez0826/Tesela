import { mount } from "svelte";
import App from "./App.svelte";
import Bar from "./lib/views/Bar.svelte";
import Panel from "./lib/views/Panel.svelte";

// Una sola página sirve a las tres ventanas: la principal, la barra de cada monitor y el panel.
const view = new URLSearchParams(window.location.search).get("view");
const root = view === "bar" ? Bar : view === "panel" ? Panel : App;

mount(root, { target: document.getElementById("app")! });
