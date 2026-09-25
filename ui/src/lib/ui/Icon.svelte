<script lang="ts" module>
  // The icon set, inlined and fetched from nowhere: one 24-unit grid, one
  // stroke weight. An icon never carries meaning alone — every use sits beside
  // a word or has an `aria-label`.
  export const PATHS: Record<string, string> = {
    inbox: "M3 13h5l1.5 3h5L16 13h5M5.5 5h13L21 13v6a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1v-6z",
    change: "M6 3v12M18 9a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM6 21a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM18 9a9 9 0 0 1-9 9",
    sessions: "M3 12h4l3-8 4 16 3-8h4",
    spec: "M14 3H6a1 1 0 0 0-1 1v16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V8zM14 3v5h5M9 13h6M9 17h6M9 9h2",
    ledger: "M12 3v18M5 7h14M5 7l-2.5 6a3 3 0 0 0 5 0zM19 7l-2.5 6a3 3 0 0 0 5 0zM8 21h8",
    report: "M5 21V4M5 4h11l-2 4 2 4H5",
    forge: "M6 3v12M6 21a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM18 21a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM18 15V9a3 3 0 0 0-3-3h-4M13 3l-2 3 2 3",
    settings: "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-2.9 1.2V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-2.9-1.2l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1A1.7 1.7 0 0 0 3.9 14H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.2-2.9l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1A1.7 1.7 0 0 0 10 3.1V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 2.9 1.2l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0 1.2 2.9H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z",
    search: "M11 19a8 8 0 1 0 0-16 8 8 0 0 0 0 16zM21 21l-4.3-4.3",
    plus: "M12 5v14M5 12h14",
    play: "M7 4l13 8-13 8z",
    stop: "M6 6h12v12H6z",
    check: "M4 12.5l5 5L20 6.5",
    x: "M6 6l12 12M18 6L6 18",
    alert: "M12 9v4M12 17h.01M10.3 3.9L2.4 18a2 2 0 0 0 1.7 3h15.8a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z",
    question: "M9.1 9a3 3 0 0 1 5.8 1c0 2-3 3-3 3M12 17h.01M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18z",
    clock: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM12 7v5l3 2",
    right: "M9 6l6 6-6 6",
    down: "M6 9l6 6 6-6",
    left: "M15 6l-6 6 6 6",
    dot: "M12 13a1 1 0 1 0 0-2 1 1 0 0 0 0 2z",
    sidebar: "M4 4h16v16H4zM9 4v16",
    panel: "M4 4h16v16H4zM4 15h16",
    split: "M4 4h16v16H4zM12 4v16",
    unified: "M4 4h16v16H4zM4 12h16",
    filter: "M3 5h18l-7 8v6l-4 2v-8z",
    refresh: "M21 12a9 9 0 1 1-2.6-6.4M21 4v5h-5",
    external: "M14 4h6v6M20 4l-9 9M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5",
    terminal: "M4 17l6-5-6-5M12 19h8",
    file: "M14 3H6a1 1 0 0 0-1 1v16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V8zM14 3v5h5",
    folder: "M3 6a1 1 0 0 1 1-1h5l2 2h9a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1z",
    person: "M12 12a4 4 0 1 0 0-8 4 4 0 0 0 0 8zM4 21a8 8 0 0 1 16 0",
    agent: "M12 8V4M8 4h8M6 8h12a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2v-8a2 2 0 0 1 2-2zM9 13v1M15 13v1",
    shield: "M12 21s8-4 8-10V5l-8-3-8 3v6c0 6 8 10 8 10z",
    rule: "M4 6h16M4 12h10M4 18h7",
    gate: "M4 21V8l8-5 8 5v13M9 21v-6h6v6",
    timeline: "M3 12h18M7 8v8M12 5v14M17 9v6",
    keyboard: "M3 6h18v12H3zM7 10h.01M11 10h.01M15 10h.01M7 14h10",
    sun: "M12 17a5 5 0 1 0 0-10 5 5 0 0 0 0 10zM12 1v2M12 21v2M4.2 4.2l1.4 1.4M18.4 18.4l1.4 1.4M1 12h2M21 12h2M4.2 19.8l1.4-1.4M18.4 5.6l1.4-1.4",
    moon: "M21 12.8A9 9 0 1 1 11.2 3 7 7 0 0 0 21 12.8z",
    more: "M5 12h.01M12 12h.01M19 12h.01",
    pin: "M12 17v5M9 3h6l-1 6 3 3H7l3-3z",
    eye: "M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12zM12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z",
    skip: "M5 4l10 8-10 8zM19 5v14",
  };
</script>

<script lang="ts">
  let {
    name,
    size = 16,
    label = "",
    stroke = 1.75,
  }: { name: string; size?: number; label?: string; stroke?: number } = $props();
  const d = $derived(PATHS[name] ?? PATHS.dot);
</script>

<svg
  class="icon"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={stroke}
  stroke-linecap="round"
  stroke-linejoin="round"
  role={label ? "img" : undefined}
  aria-label={label || undefined}
  aria-hidden={label ? undefined : "true"}
>
  <path {d} />
</svg>

<style>
  .icon {
    flex: none;
    display: inline-block;
    vertical-align: -0.2em;
  }
</style>
