const $ = (id) => document.getElementById(id);
let wasm, raw, timer, lastVersion, lastSnapshot;
let toastTimer;
const startedAt = performance.now();

function toast(message) {
  $("toast").textContent = message;
  $("toast").hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => { $("toast").hidden = true; }, 6000);
}

function apply(action, value = "") {
  try {
    wasm.command(action, String(value));
    refresh(true);
    return true;
  } catch (error) {
    toast(String(error));
    return false;
  }
}

function refresh(force = false) {
  if (!wasm) return;
  const state = JSON.parse(wasm.snapshot());
  if (state.total == null) return;
  lastSnapshot = state;
  if (!force && state.version === lastVersion) return;
  lastVersion = state.version;
  const queryMs = state.query_ms == null ? "" : ` · Query ${Number(state.query_ms).toFixed(2)} ms`;
  const summary = `${Number(state.matched).toLocaleString()} of ${Number(state.total).toLocaleString()} events${queryMs}`;
  $("live-summary").textContent = summary;
  if ($("text-view").hidden) return;
  $("text-summary").textContent = summary;
  if (document.activeElement !== $("text-search")) $("text-search").value = state.query_text ?? "";
  $("text-level").value = (state.level ?? "all").toLowerCase();
  $("text-slow").value = state.slow ? "on" : "off";
  const focusedPosition = document.activeElement?.closest(".event-row")?.dataset.position;
  $("text-rows").replaceChildren();
  for (const [position, row] of (state.first_rows ?? []).entries()) {
    const button = document.createElement("button");
    button.className = "event-row";
    button.dataset.position = String(position);
    const meta = document.createElement("small");
    meta.textContent = `${row.timestamp_utc ?? row.timestamp_ms} · ${row.service} · ${row.level} · ${row.duration_ms} ms`;
    const message = document.createElement("span");
    message.textContent = row.message;
    button.append(meta, message);
    button.addEventListener("click", () => apply("inspect", position));
    $("text-rows").append(button);
  }
  if (focusedPosition != null) {
    $("text-rows").querySelector(`[data-position="${Number(focusedPosition)}"]`)?.focus();
  }
  if (!(state.first_rows ?? []).length) $("text-rows").textContent = "No matching events. Try a different search or reset the filters.";
  $("text-detail").textContent = state.selected ? JSON.stringify(state.selected, null, 2) : "Select an event.";
}

function toggleText(show) {
  $("text-view").hidden = !show;
  $("text-toggle").setAttribute("aria-expanded", String(show));
  if (show) {
    refresh(true);
    $("text-search").focus();
  } else $("text-toggle").focus();
}

$("text-toggle").addEventListener("click", () => toggleText($("text-view").hidden));
$("close-text").addEventListener("click", () => toggleText(false));
$("retry").addEventListener("click", () => location.reload());
$("filters").addEventListener("submit", (event) => event.preventDefault());
$("text-search").addEventListener("input", (event) => apply("search", event.target.value));
$("text-level").addEventListener("change", (event) => apply("level", event.target.value));
$("text-slow").addEventListener("change", (event) => apply("slow", event.target.value));
$("text-reset").addEventListener("click", () => {
  apply("reset");
});
document.querySelectorAll("[data-demo]").forEach(button => button.addEventListener("click", () => apply("demo", button.dataset.demo)));
$("text-import").addEventListener("click", () => $("jsonl-file").click());
$("jsonl-file").addEventListener("change", async (event) => {
  const file = event.target.files?.[0];
  if (!file) return;
  try {
    if (file.size > 32 * 1024 * 1024) throw new Error("This file is larger than 32 MiB. Choose a smaller JSONL file.");
    const text = await file.text();
    if (apply("import", text)) toast(`Opened ${file.name} locally. No file data was uploaded.`);
  } catch (error) { toast(String(error)); }
  event.target.value = "";
});

// Keep the standard HTML controls out of the canvas's keyboard/pointer handling.
for (const type of ["keydown","keyup","pointerdown","pointerup"]) {
  $("text-view").addEventListener(type, (event) => {
    if (type === "keydown" && event.key === "Escape") toggleText(false);
    event.stopPropagation();
  });
}
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && !$("text-view").hidden) toggleText(false);
});

// A bounded diagnostic interface: it reflects actual Rust state and executes the
// same commands as the UI. It does not send data or metrics anywhere.
window.traceglass = {
  snapshot: () => wasm ? JSON.parse(wasm.snapshot()) : {},
  command: (action, value = "") => apply(action, value),
  memoryBytes: () => raw?.memory?.buffer?.byteLength ?? null,
  get ready() { return !!lastSnapshot; },
  get startupMs() { return this._startupMs ?? null; }
};

async function start() {
  try {
    if (!("WebAssembly" in window)) throw new Error("WebAssembly is not available in this browser.");
    wasm = await import("/wasm/traceglass.js");
    raw = await wasm.default();
    $("boot-status").textContent = "Starting graphics and preparing events…";
    wasm.run();
    const deadline = performance.now() + 45000;
    const waitForApp = () => {
      refresh();
      if (lastSnapshot && document.querySelector("body > canvas")) {
        // Leave two browser frames for the first submitted GPUI frame.
        requestAnimationFrame(() => requestAnimationFrame(() => {
          $("boot").hidden = true;
          $("text-toggle").hidden = false;
          window.traceglass._startupMs = performance.now() - startedAt;
          document.documentElement.dataset.ready = "true";
        }));
        timer = setInterval(() => { if (!document.hidden) refresh(); }, 1000);
      } else if (performance.now() < deadline) {
        setTimeout(waitForApp, 100);
      } else {
        fail("Graphics did not start. Try a recent desktop browser with hardware acceleration enabled.");
      }
    };
    waitForApp();
  } catch (error) { fail(error); }
}

function fail(error) {
  clearInterval(timer);
  $("boot").hidden = false;
  $("boot-status").textContent = String(error);
  $("retry").hidden = false;
  console.error("Traceglass startup failed:", error);
}
window.addEventListener("error", (event) => {
  if (!lastSnapshot) fail(event.message);
});
start();
