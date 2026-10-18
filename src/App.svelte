<!-- Tesela ya no tiene ventana principal: todo se maneja desde la barra y el panel desplegable. Esta vista solo la usa la
     prueba de extremo a extremo de un módulo (`--selftest-module`), que abre una ventana de pruebas. -->
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import "./lib/styles/nothing.css";
  import ModuleSelfTest from "./lib/views/ModuleSelfTest.svelte";

  let selftest: string | null = $state(null);

  onMount(() => {
    invoke<string | null>("selftest_config").then((m) => (selftest = m)).catch(() => {});
  });
</script>

{#if selftest}
  <ModuleSelfTest moduleId={selftest} />
{/if}
