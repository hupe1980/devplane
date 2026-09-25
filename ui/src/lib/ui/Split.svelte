<script lang="ts">
  // A pane a person sizes by dragging its edge; the size is remembered per
  // pane (`id`) in this browser only. The handle is a focusable separator the
  // arrow keys move.
  import type { Snippet } from "svelte";

  let {
    id,
    axis = "x",
    size: initial = 280,
    min = 160,
    max = 640,
    /// Which side of the handle the sized pane is on: `start` = left/top.
    side = "start",
    collapsed = false,
    pane,
    children,
  }: {
    id: string;
    axis?: "x" | "y";
    size?: number;
    min?: number;
    max?: number;
    side?: "start" | "end";
    collapsed?: boolean;
    pane: Snippet;
    children: Snippet;
  } = $props();

  const store = () => `vp-split:${id}`;
  function remembered(): number {
    try {
      const v = Number(localStorage.getItem(store()));
      return Number.isFinite(v) && v >= min && v <= max ? v : initial;
    } catch {
      return initial;
    }
  }
  let size = $state(remembered());
  let dragging = $state(false);

  function keep() {
    try {
      localStorage.setItem(store(), String(Math.round(size)));
    } catch {
      /* private window: the size lasts the tab */
    }
  }

  let start = 0;
  let from = 0;
  function down(e: PointerEvent) {
    dragging = true;
    start = axis === "x" ? e.clientX : e.clientY;
    from = size;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }
  function move(e: PointerEvent) {
    if (!dragging) return;
    const delta = (axis === "x" ? e.clientX : e.clientY) - start;
    size = Math.min(max, Math.max(min, from + (side === "start" ? delta : -delta)));
  }
  function up() {
    if (!dragging) return;
    dragging = false;
    keep();
  }
  function key(e: KeyboardEvent) {
    const grow = axis === "x" ? ["ArrowRight", "ArrowLeft"] : ["ArrowDown", "ArrowUp"];
    const i = grow.indexOf(e.key);
    if (i === -1) return;
    e.preventDefault();
    const step = e.shiftKey ? 48 : 16;
    const sign = (i === 0 ? 1 : -1) * (side === "start" ? 1 : -1);
    size = Math.min(max, Math.max(min, size + sign * step));
    keep();
  }
</script>

<div class="split {axis}" class:dragging class:end={side === "end"}>
  {#if !collapsed}
    <div class="pane" style="{axis === 'x' ? 'width' : 'height'}: {size}px">
      {@render pane()}
    </div>
    <!-- A focusable separator is the ARIA pattern for a resizable pane. -->
    <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
    <div
      class="handle"
      role="separator"
      tabindex="0"
      aria-orientation={axis === "x" ? "vertical" : "horizontal"}
      aria-valuenow={Math.round(size)}
      aria-valuemin={min}
      aria-valuemax={max}
      aria-label="resize"
      onpointerdown={down}
      onpointermove={move}
      onpointerup={up}
      onpointercancel={up}
      onkeydown={key}
    ></div>
  {/if}
  <div class="rest">{@render children()}</div>
</div>

<style>
  .split {
    display: flex;
    min-width: 0;
    min-height: 0;
    flex: 1;
  }
  .split.y {
    flex-direction: column;
  }
  .split.end {
    flex-direction: row-reverse;
  }
  .split.y.end {
    flex-direction: column-reverse;
  }
  .pane {
    flex: none;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .rest {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .handle {
    flex: none;
    position: relative;
    background: var(--line);
    z-index: 2;
  }
  .x > .handle {
    width: 1px;
    cursor: col-resize;
  }
  .y > .handle {
    height: 1px;
    cursor: row-resize;
  }
  /* A wider hit area than the visible line. */
  .handle::after {
    content: "";
    position: absolute;
    inset: -3px;
  }
  .handle:hover,
  .handle:focus-visible,
  .dragging > .handle {
    background: var(--accent);
    outline: none;
  }
  .dragging {
    user-select: none;
  }
</style>
