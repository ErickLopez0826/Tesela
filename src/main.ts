import { mount } from "svelte";
import "./lib/styles/fluid.css";
import { installFluid } from "./lib/ui/fluid";
import { installTheme } from "./lib/stores/theme";
import App from "./App.svelte";
import Bar from "./lib/views/Bar.svelte";
import Panel from "./lib/views/Panel.svelte";
import Border from "./lib/views/Border.svelte";

// Una sola página sirve a las tres ventanas: la principal, la barra de cada monitor y el panel.
const view = new URLSearchParams(window.location.search).get("view");
const root = view === "bar" ? Bar : view === "panel" ? Panel : view === "border" ? Border : App;

installFluid();
installTheme();
mount(root, { target: document.getElementById("app")! });
