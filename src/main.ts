import { mount } from "svelte";
import "./lib/styles/fluid.css";
import { installFluid } from "./lib/ui/fluid";
import { installTheme } from "./lib/stores/theme";
import App from "./App.svelte";
import Bar from "./lib/views/Bar.svelte";
import Panel from "./lib/views/Panel.svelte";
import Border from "./lib/views/Border.svelte";
import Taskbar from "./lib/views/Taskbar.svelte";

// Una sola página sirve a todas las ventanas: la de pruebas, la barra y la barra de tareas de cada monitor, el panel y el borde.
const view = new URLSearchParams(window.location.search).get("view");
const root = view === "bar" ? Bar : view === "panel" ? Panel : view === "border" ? Border : view === "taskbar" ? Taskbar : App;

installFluid();
installTheme();
mount(root, { target: document.getElementById("app")! });
