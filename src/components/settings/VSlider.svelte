<script lang="ts">
  let {
    value,
    min = -12,
    max = 12,
    label,
    onchange,
  }: { value: number; min?: number; max?: number; label: string; onchange: (v: number) => void } = $props();

  let track: HTMLDivElement;
  let dragging = false;
  const frac = $derived((value - min) / (max - min));

  function set(e: PointerEvent) {
    const r = track.getBoundingClientRect();
    const f = 1 - Math.max(0, Math.min(1, (e.clientY - r.top) / r.height));
    onchange(Math.round((min + f * (max - min)) * 2) / 2);
  }
</script>

<div class="vs">
  <span class="val">{value > 0 ? "+" : ""}{value.toFixed(1)}</span>
  <div
    class="track"
    bind:this={track}
    role="slider"
    tabindex="0"
    aria-label={label}
    aria-valuemin={min}
    aria-valuemax={max}
    aria-valuenow={value}
    onpointerdown={(e) => {
      dragging = true;
      track.setPointerCapture(e.pointerId);
      set(e);
    }}
    onpointermove={(e) => dragging && set(e)}
    onpointerup={() => (dragging = false)}
    style:touch-action="none"
    ondblclick={() => onchange(0)}
    onkeydown={(e) => {
      if (e.key === "ArrowUp") onchange(Math.min(max, value + 0.5));
      if (e.key === "ArrowDown") onchange(Math.max(min, value - 0.5));
    }}
  >
    <div class="mid"></div>
    <div
      class="fill"
      style:top="{Math.min(frac, 0.5) * 100}%"
      style:bottom="{Math.min(1 - frac, 0.5) * 100}%"
    ></div>
    <div class="knob" style:bottom="calc({frac * 100}% - 8px)"></div>
  </div>
  <span class="lab">{label}</span>
</div>

<style>
  .vs {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
  }
  .val,
  .lab {
    font-size: 11px;
    color: var(--text-3);
    font-variant-numeric: tabular-nums;
  }
  .track {
    position: relative;
    width: 26px;
    height: 170px;
    cursor: ns-resize;
    touch-action: none;
  }
  .track::before {
    content: "";
    position: absolute;
    left: 50%;
    top: 0;
    bottom: 0;
    width: 4px;
    translate: -50% 0;
    border-radius: 4px;
    background: color-mix(in srgb, var(--text) 14%, transparent);
  }
  .mid {
    position: absolute;
    left: 4px;
    right: 4px;
    top: 50%;
    height: 1px;
    background: var(--rule);
  }
  .fill {
    position: absolute;
    left: 50%;
    width: 4px;
    translate: -50% 0;
    border-radius: 4px;
    background: var(--accent);
  }
  .knob {
    position: absolute;
    left: 50%;
    width: 16px;
    height: 16px;
    translate: -50% 0;
    border-radius: 50%;
    background: var(--text);
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.35);
  }
</style>
