import { mount } from "svelte";
import "./lib/styles/fluid.css";
import { installFluid } from "./lib/ui/fluid";
import { installTheme } from "./lib/stores/theme";
import App from "./App.svelte";
import Bar from "./lib/views/Bar.svelte";
import Panel from "./lib/views/Panel.svelte";
import Border from "./lib/views/Border.svelte";
import Taskbar from "./lib/views/Taskbar.svelte";
import AppPopup from "./lib/views/AppPopup.svelte";
import Search from "./lib/views/Search.svelte";

// Una sola página sirve a todas las ventanas: la de pruebas, la barra y la barra de tareas de cada monitor, el panel, el
// buscador y el borde.
const view = new URLSearchParams(window.location.search).get("view");
const root =
  view === "bar" ? Bar : view === "panel" ? Panel : view === "search" ? Search : view === "border" ? Border : view === "taskbar" ? Taskbar : view === "popup" ? AppPopup : App;

installFluid();
installTheme();
mount(root, { target: document.getElementById("app")! });
