<script>
  import { onMount } from 'svelte';

  let { script, view = 'list', running = false, onQuickRun, onRunWithParams } = $props();

  let open = $state(false);
  let pos = $state({ top: 0, left: 0 });
  let containerEl = $state(null);

  function toggle(evt) {
    if (running) return;
    // 阻止外层 <a> 触发的页面跳转
    evt?.preventDefault?.();
    evt?.stopPropagation?.();
    if (open) { open = false; return; }
    const t = evt?.currentTarget;
    if (t) {
      const r = t.getBoundingClientRect();
      pos = { top: Math.round(r.bottom + 6), left: Math.round(r.right - 220) };
    }
    open = true;
  }

  function pickQuick(evt) {
    evt?.preventDefault?.();
    evt?.stopPropagation?.();
    open = false;
    onQuickRun?.(script);
  }

  function pickParams(evt) {
    evt?.preventDefault?.();
    evt?.stopPropagation?.();
    open = false;
    onRunWithParams?.(script);
  }

  function close() { open = false; }




  function onKey(event) {
    if (open && event.key === 'Escape') {
      open = false;
      event.stopPropagation();
    }
  }

  onMount(() => {
    window.addEventListener('mousedown', onGlobal);
    window.addEventListener('keydown', onKey);
    return () => {
      window.removeEventListener('mousedown', onGlobal);
      window.removeEventListener('keydown', onKey);
    };
  });

  const hasParams = $derived((script?.metadata?.params?.length || 0) > 0);
  const cls = $derived(`run-dropdown-wrap run-dropdown-${view}`);
</script>

<div class={cls} bind:this={containerEl}>
  <button
    class="run-trigger"
    class:open
    class:list={view === 'list'}
    onclick={toggle}
    disabled={running}
    aria-haspopup="menu"
    aria-expanded={open}
    title="执行 {script?.name || ''}">
    {#if running}
      <span class="btn-spinner"></span>
      <span class="run-label">执行中</span>
    {:else}
      <svg width="11" height="11" viewBox="0 0 24 24" fill="currentColor"><path d="M8 5v14l11-7z"/></svg>
      {#if view === 'list'}<span class="run-label">执行</span>{/if}
      <svg class="run-caret" width="8" height="8" viewBox="0 0 24 24" fill="currentColor"><path d="M7 10l5 5 5-5z"/></svg>
    {/if}
  </button>
  {#if open}
    <div class="run-menu" role="menu" style="top:{pos.top}px;left:{pos.left}px" onclick={(e) => e.stopPropagation()}>
      <button type="button" class="run-menu-item" role="menuitem" onclick={pickQuick}>
        <span class="run-menu-icon">▶</span>
        <span class="run-menu-text"><strong>快速执行</strong><em>不带参数，直接运行</em></span>
      </button>
      {#if hasParams}
        <button type="button" class="run-menu-item" role="menuitem" onclick={pickParams}>
          <span class="run-menu-icon">⚙</span>
          <span class="run-menu-text"><strong>带参数执行</strong><em>填写 {script.metadata.params.length} 个参数后运行</em></span>
        </button>
      {/if}
    </div>
  {/if}
</div>

<style>
  .run-dropdown-wrap {
    position: relative;
    display: inline-flex;
    overflow: visible !important;
  }
  .run-trigger {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 1px solid rgba(34, 211, 238, 0.12);
    background: rgba(34, 211, 238, 0.08);
    color: #22d3ee;
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.2s;
    flex-shrink: 0;
  }
  .run-trigger.list {
    width: auto;
    height: 24px;
    padding: 0 8px;
    margin-left: 4px;
    gap: 4px;
    font-size: 12px;
  }
  .run-dropdown-card .run-trigger {
    width: 26px;
    height: 26px;
  }
  .run-trigger.open,
  .run-trigger:hover {
    background: linear-gradient(135deg, #22d3ee, #0891b2);
    color: #fff;
    transform: scale(1.08);
    box-shadow: 0 4px 14px rgba(34, 211, 238, 0.4);
  }
  .run-caret {
    margin-left: 2px;
    opacity: 0.85;
    transition: transform 0.15s;
  }
  .run-trigger.open .run-caret {
    transform: rotate(180deg);
  }
  .run-label {
    font-weight: 600;
  }
  .btn-spinner {
    width: 12px;
    height: 12px;
    border: 2px solid currentColor;
    border-top-color: transparent;
    border-radius: 50%;
    animation: spin 0.6s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }

  .run-menu {
    position: fixed;
    z-index: 9999;
    min-width: 220px;
    padding: 6px;
    background: #0f172a;
    border: 1px solid rgba(34, 211, 238, 0.35);
    border-radius: 10px;
    box-shadow: 0 12px 36px rgba(0, 0, 0, 0.6), 0 0 0 1px rgba(34, 211, 238, 0.15);
    display: flex;
    flex-direction: column;
    gap: 2px;
    animation: runMenuIn 0.12s ease-out;
  }
  @keyframes runMenuIn {
    from { opacity: 0; transform: translateY(-4px); }
    to { opacity: 1; transform: translateY(0); }
  }
  .run-menu-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border: none;
    background: transparent;
    color: var(--text-primary, #e2e8f0);
    text-align: left;
    cursor: pointer;
    border-radius: 7px;
    transition: background 0.15s;
    font: inherit;
  }
  .run-menu-item:hover {
    background: linear-gradient(135deg, rgba(34, 211, 238, 0.12), rgba(6, 182, 212, 0.08));
  }
  .run-menu-icon {
    width: 22px;
    height: 22px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: rgba(34, 211, 238, 0.12);
    color: #22d3ee;
    border-radius: 6px;
    font-size: 12px;
    flex-shrink: 0;
  }
  .run-menu-text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .run-menu-text strong {
    font-size: 12px;
    font-weight: 600;
    color: #e2e8f0;
  }
  .run-menu-text em {
    font-size: 10.5px;
    font-style: normal;
    color: #94a3b8;
    margin-top: 1px;
  }
</style>
