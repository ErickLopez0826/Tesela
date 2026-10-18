<!-- Fecha en tres casillas (día, mes, año) que pasan solas a la siguiente; el año ya viene puesto con el actual. El botón abre el calendario. -->
<script lang="ts">
  let { value = $bindable(""), label, min, max }: { value: string; label: string; min?: string; max?: string } = $props();

  const thisYear = String(new Date().getFullYear());
  let day = $state("");
  let month = $state("");
  let year = $state(thisYear);
  let monthEl: HTMLInputElement | undefined = $state();
  let yearEl: HTMLInputElement | undefined = $state();
  let picker: HTMLInputElement | undefined = $state();

  const pad = (s: string) => s.padStart(2, "0");

  /** «AAAA-MM-DD» si las casillas forman un día que existe; si no, vacío. */
  function compose(d: string, m: string, y: string): string {
    if (!d || !m || y.length !== 4) return "";
    const [dn, mn, yn] = [+d, +m, +y];
    const real = new Date(yn, mn - 1, dn);
    if (real.getFullYear() !== yn || real.getMonth() !== mn - 1 || real.getDate() !== dn) return "";
    return `${y}-${pad(m)}-${pad(d)}`;
  }

  // Si el valor cambia desde fuera (un atajo, «Quitar fechas», el calendario), las casillas lo siguen.
  $effect(() => {
    if (value === compose(day, month, year)) return;
    if (value) {
      const [y, m, d] = value.split("-");
      day = d;
      month = m;
      year = y;
    } else {
      day = "";
      month = "";
      year = thisYear;
    }
  });

  function commit() {
    value = compose(day, month, year);
  }

  const digits = (el: HTMLInputElement, max: number) => (el.value = el.value.replace(/\D/g, "").slice(0, max));

  function onDay(e: Event) {
    day = digits(e.currentTarget as HTMLInputElement, 2);
    // Un 4 en adelante no puede ser el inicio de un día de dos cifras: ya está completo.
    if (day.length === 2 || +day > 3) {
      day = day.length === 1 ? pad(day) : day;
      commit();
      monthEl?.focus();
      monthEl?.select();
    } else commit();
  }

  function onMonth(e: Event) {
    month = digits(e.currentTarget as HTMLInputElement, 2);
    if (month.length === 2 || +month > 1) {
      month = month.length === 1 ? pad(month) : month;
      commit();
      yearEl?.focus();
      yearEl?.select();
    } else commit();
  }

  function onYear(e: Event) {
    year = digits(e.currentTarget as HTMLInputElement, 4);
    commit();
  }

  // Con el año borrado y fuera de la casilla vuelve a ser el actual.
  const settleYear = () => {
    if (year.length !== 4) {
      year = thisYear;
      commit();
    }
  };

  // Retroceso en una casilla vacía vuelve a la anterior.
  function back(e: KeyboardEvent, prev: HTMLInputElement | undefined) {
    if (e.key === "Backspace" && (e.currentTarget as HTMLInputElement).value === "") prev?.focus();
  }

  let dayEl: HTMLInputElement | undefined = $state();

  function openCalendar() {
    try {
      picker?.showPicker();
    } catch {
      picker?.focus();
    }
  }
</script>

<div class="date" role="group" aria-label={label}>
  <input bind:this={dayEl} class="d" inputmode="numeric" autocomplete="off" placeholder="día" value={day} oninput={onDay} aria-label={`${label}: día`} />
  <i aria-hidden="true">/</i>
  <input bind:this={monthEl} class="m" inputmode="numeric" autocomplete="off" placeholder="mes" value={month} oninput={onMonth} onkeydown={(e) => back(e, dayEl)} aria-label={`${label}: mes`} />
  <i aria-hidden="true">/</i>
  <input bind:this={yearEl} class="y" inputmode="numeric" autocomplete="off" placeholder={thisYear} value={year} oninput={onYear} onblur={settleYear} onkeydown={(e) => back(e, monthEl)} aria-label={`${label}: año`} />
  <button type="button" class="cal" aria-label={`${label}: abrir calendario`} onclick={openCalendar}>
    <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <rect x="3.5" y="5" width="17" height="15.5" rx="3" /><path d="M3.5 10h17M8 3v4M16 3v4" />
    </svg>
  </button>
  <input bind:this={picker} class="native" type="date" tabindex="-1" aria-hidden="true" {min} {max} {value} onchange={(e) => (value = e.currentTarget.value)} />
</div>

<style>
  .date { position: relative; display: flex; align-items: center; gap: 2px; flex: 1; min-width: 0; padding: 0 6px 0 10px; border-radius: 12px; background: rgba(0, 0, 0, 0.5); border: 1px solid var(--line); transition: border-color 0.25s; }
  .date:hover, .date:focus-within { border-color: var(--accent); }
  input:not(.native) { width: 4ch; padding: 7px 0; text-align: center; font: 12px var(--mono); color: var(--fg); background: transparent; border: 0; outline: none; }
  input.y { width: 5ch; }
  input::placeholder { color: var(--muted); opacity: 0.7; }
  input:not(.native):focus { color: var(--accent); }
  i { font-style: normal; color: var(--muted); font: 12px var(--mono); }
  .cal { margin-left: auto; display: grid; place-items: center; padding: 4px; color: var(--muted); background: transparent; border: 0; border-radius: 8px; cursor: pointer; transition: color 0.2s; }
  .cal:hover { color: var(--accent); }
  /* El selector nativo solo sirve de calendario: sin verse ni ocupar sitio. */
  .native { position: absolute; right: 0; bottom: 0; width: 1px; height: 1px; opacity: 0; pointer-events: none; color-scheme: dark; }
</style>
