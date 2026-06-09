<script>
  import { onMount, tick } from 'svelte';
  import ConfirmDialog from '../lib/ConfirmDialog.svelte';

  let alerts = $state([]);
  let loading = $state(true);
  let search = $state('');
  let filterLevel = $state('');
  let filterType = $state('');
  let selectedAlertId = $state('');
  let sortKey = $state('time');
  let sortDir = $state('desc');
  let showHistory = $state(false);
  let scans = $state([]);
  let summary = $state(null);
  let timestamp = $state('');
  let showTypeSelect = $state(false);
  let typeSearch = $state('');
  let typeHighlightIndex = $state(0);
  let typeOptionsList = $state(null);
  let typeSearchInput = $state(null);
  let clearing = $state(false);
  let showClearConfirm = $state(false);
  let actionMessage = $state('');
  let actionError = $state('');
  let refreshTimer = null;

  async function load(silent = false) {
    if (!silent) loading = true;
    try {
      const r = await fetch('/api/alerts?limit=500&history=' + showHistory + '&ts=' + Date.now(), { cache: 'no-store' });
      if (r.ok) {
        const d = await r.json();
        alerts = d.alerts || [];
        scans = d.scans || [];
        summary = d.summary || null;
        timestamp = d.timestamp || '';
      }
    } catch (e) { console.warn('加载告警失败:', e); }
    if (!silent) loading = false;
  }

  function levelColor(l) {
    if (l === 'error') return '#ef4444';
    if (l === 'warn') return '#f59e0b';
    return '#94a3b8';
  }

  function levelBg(l) {
    if (l === 'error') return 'rgba(239, 68, 68, 0.08)';
    if (l === 'warn') return 'rgba(245, 158, 11, 0.08)';
    return 'rgba(148, 163, 184, 0.08)';
  }

  function levelIcon(l) {
    if (l === 'error') return 'FAIL';
    if (l === 'warn') return 'WARN';
    return 'INFO';
  }

  function typeLabel(t) {
    if (t === 'resource') return '资源';
    if (t === 'service') return '服务';
    if (t === 'log') return '日志';
    if (t === 'process') return '进程';
    if (t === 'script') return '脚本';
    return t;
  }

  function levelLabel(l) {
    if (l === 'error') return '错误';
    if (l === 'warn') return '警告';
    return '信息';
  }

  function compactCount(value) {
    const n = Number(value || 0);
    return n > 500 ? '500+' : String(n);
  }

  function fieldValue(alert, key) {
    if (key === 'level') return alert.level === 'error' ? 0 : alert.level === 'warn' ? 1 : 2;
    if (key === 'active') return alert.active ? 0 : 1;
    if (key === 'target') return alert.service_name || alert.pid || alert.type || '';
    if (key === 'time') return alert.last_seen || alert.timestamp || '';
    return alert[key] || '';
  }

  function changeSort(key) {
    if (sortKey === key) sortDir = sortDir === 'asc' ? 'desc' : 'asc';
    else {
      sortKey = key;
      sortDir = key === 'time' ? 'desc' : 'asc';
    }
  }

  function sortMark(key) {
    if (sortKey !== key) return '';
    return sortDir === 'asc' ? ' ↑' : ' ↓';
  }

  function primaryTarget(alert) {
    return alert.service_name || (alert.pid ? `PID ${alert.pid}` : typeLabel(alert.type));
  }

  async function clearAllAlerts() {
    if (clearing) return;
    if (alerts.length > 0) {
      showClearConfirm = true;
      return;
    }
    await confirmClearAllAlerts();
  }

  async function confirmClearAllAlerts() {
    if (clearing) return;
    clearing = true;
    showClearConfirm = false;
    actionMessage = '';
    actionError = '';
    try {
      const r = await fetch('/api/alerts', { method: 'DELETE' });
      if (!r.ok) throw new Error('清理失败: ' + r.status);
      alerts = [];
      scans = [];
      summary = { checked: 0, skipped: 0 };
      selectedAlertId = '';
      timestamp = new Date().toLocaleString('zh-CN', { hour12: false });
      actionMessage = '系统告警已全部清理';
      window.dispatchEvent(new CustomEvent('dm-alerts-refresh'));
    } catch (e) {
      actionError = e.message || String(e);
    }
    clearing = false;
  }

  let filtered = $derived.by(() => {
    let result = alerts;
    if (filterLevel) result = result.filter(a => a.level === filterLevel);
    if (filterType) result = result.filter(a => a.type === filterType);
    if (search.trim()) {
      const q = search.toLowerCase();
      result = result.filter(a => JSON.stringify(a).toLowerCase().includes(q));
    }
    return [...result].sort((a, b) => {
      const av = fieldValue(a, sortKey);
      const bv = fieldValue(b, sortKey);
      let cmp = typeof av === 'number' && typeof bv === 'number'
        ? av - bv
        : String(av).localeCompare(String(bv), 'zh-CN');
      if (sortDir === 'desc') cmp = -cmp;
      return cmp;
    });
  });

  let types = $derived.by(() => {
    const t = new Set();
    for (const a of alerts) t.add(a.type);
    return [...t].sort();
  });

  let typeOptions = $derived.by(() => {
    const q = typeSearch.trim().toLowerCase();
    const allOption = { value: '', label: '全部类型', detail: '', count: alerts.length };
    const mapped = types
      .filter(t => !q || typeLabel(t).toLowerCase().includes(q) || String(t).toLowerCase().includes(q))
      .map(t => ({
        value: t,
        label: typeLabel(t),
        detail: String(t),
        count: alerts.filter(a => a.type === t).length,
      }));
    if (!q) return [allOption, ...mapped];
    const allMatches = allOption.label.toLowerCase().includes(q) || 'all'.includes(q);
    return allMatches ? [allOption, ...mapped] : mapped;
  });

  function clampTypeIndex(index) {
    if (typeOptions.length === 0) return 0;
    return Math.max(0, Math.min(index, typeOptions.length - 1));
  }

  function scrollTypeHighlightIntoView() {
    requestAnimationFrame(() => {
      const el = typeOptionsList?.querySelector(`[data-type-index="${typeHighlightIndex}"]`);
      el?.scrollIntoView({ block: 'nearest' });
    });
  }

  function setTypeHighlight(index) {
    typeHighlightIndex = clampTypeIndex(index);
    scrollTypeHighlightIntoView();
  }

  function chooseType(value) {
    filterType = value;
    showTypeSelect = false;
    typeSearch = '';
    typeHighlightIndex = 0;
  }

  async function toggleTypeSelect() {
    showTypeSelect = !showTypeSelect;
    typeSearch = '';
    if (showTypeSelect) {
      const currentIndex = typeOptions.findIndex(o => o.value === filterType);
      typeHighlightIndex = currentIndex >= 0 ? currentIndex : 0;
      await tick();
      typeSearchInput?.focus();
      scrollTypeHighlightIntoView();
    }
  }

  function onTypeSearchInput() {
    setTypeHighlight(0);
  }

  function onTypeKeydown(e) {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setTypeHighlight(typeHighlightIndex + 1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setTypeHighlight(typeHighlightIndex - 1);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const option = typeOptions[clampTypeIndex(typeHighlightIndex)];
      if (option) chooseType(option.value);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      showTypeSelect = false;
      typeSearch = '';
      typeHighlightIndex = 0;
    }
  }

  let errorCount = $derived(alerts.filter(a => a.level === 'error').length);
  let warnCount = $derived(alerts.filter(a => a.level === 'warn').length);

  onMount(() => {
    load();
    refreshTimer = setInterval(() => load(true), 8000);
    return () => { if (refreshTimer) clearInterval(refreshTimer); };
  });
</script>

<div class="alerts-page">
  <div class="alerts-header">
    <div class="header-stats">
      <button class="stat-card stat-total" class:active={!filterLevel} onclick={() => filterLevel = ''}>
        <span class="stat-num">{compactCount(alerts.length)}</span>
        <span class="stat-label">总告警</span>
      </button>
      <button class="stat-card stat-error" class:active={filterLevel === 'error'} onclick={() => filterLevel = filterLevel === 'error' ? '' : 'error'}>
        <span class="stat-num">{compactCount(errorCount)}</span>
        <span class="stat-label">错误</span>
      </button>
      <button class="stat-card stat-warn" class:active={filterLevel === 'warn'} onclick={() => filterLevel = filterLevel === 'warn' ? '' : 'warn'}>
        <span class="stat-num">{compactCount(warnCount)}</span>
        <span class="stat-label">警告</span>
      </button>
      {#if summary}
        <div class="stat-card stat-scan">
          <span class="stat-num">{summary.checked || 0}</span>
          <span class="stat-label">已检查</span>
        </div>
      {/if}
      <div class="type-select inline-type-select">
        <button class="type-select-btn" class:active={!!filterType} onclick={toggleTypeSelect}>
          <span>{filterType ? typeLabel(filterType) : '全部类型'}</span>
          <span class="type-select-count">{compactCount(filterType ? alerts.filter(a => a.type === filterType).length : alerts.length)}</span>
          <span class="type-select-arrow">{showTypeSelect ? '↑' : '↓'}</span>
        </button>
        {#if showTypeSelect}
          <div class="type-select-menu">
            <input class="type-select-search" bind:this={typeSearchInput} bind:value={typeSearch} oninput={onTypeSearchInput} onkeydown={onTypeKeydown} placeholder="搜索类型..." autocomplete="off" />
            <div class="type-select-options" bind:this={typeOptionsList} role="listbox" aria-label="告警类型">
              {#each typeOptions as option, i}
              <button
                class="type-select-option"
                class:active={filterType === option.value}
                class:highlighted={i === typeHighlightIndex}
                data-type-index={i}
                onclick={() => chooseType(option.value)}
                onmouseenter={() => typeHighlightIndex = i}
                role="option"
                aria-selected={filterType === option.value}>
                <span>{option.label}</span>
                {#if option.detail}<small>{option.detail}</small>{/if}
                <span>{compactCount(option.count)}</span>
              </button>
              {/each}
              {#if typeOptions.length === 0}
                <div class="type-select-empty">没有匹配类型</div>
              {/if}
            </div>
          </div>
        {/if}
      </div>
    </div>
    <div class="header-actions">
      <div class="search-wrap">
        <svg class="search-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.35-4.35" stroke-linecap="round"/></svg>
        <input type="text" placeholder="搜索告警..." bind:value={search} class="search-input" />
      </div>
      <button class="history-btn" class:active={showHistory} onclick={() => { showHistory = !showHistory; load(); }}>
        {showHistory ? '只看活跃' : '包含历史'}
      </button>
      <button class="clear-alerts-btn" onclick={clearAllAlerts} disabled={clearing || loading}>
        {clearing ? '清理中' : '清理告警'}
      </button>
    </div>
  </div>

  {#if actionMessage}<div class="action-notice ok">{actionMessage}</div>{/if}
  {#if actionError}<div class="action-notice error">{actionError}</div>{/if}

  {#if timestamp}
    <div class="scan-compact">后台异步分析：{timestamp}，已检查 {summary?.checked || scans.length || 0} 项</div>
  {/if}

  {#if loading}
    <div class="loading-state">
      <div class="loading-pulse"></div>
      <span>加载告警信息...</span>
    </div>
  {:else if filtered.length === 0}
    <div class="empty-state">
      <div class="empty-title">未发现当前筛选条件下的告警</div>
      <div class="empty-desc">已完成资源、服务、日志、Journal 和脚本执行历史检查；如现场仍有异常，请结合具体业务日志继续定位。</div>
    </div>
  {:else}
    <div class="alerts-list" class:with-history={showHistory}>
      <div class="alerts-table-head">
        <button onclick={() => changeSort('time')}>时间{sortMark('time')}</button>
        <button onclick={() => changeSort('level')}>级别{sortMark('level')}</button>
        {#if showHistory}<button onclick={() => changeSort('active')}>状态{sortMark('active')}</button>{/if}
        <button onclick={() => changeSort('target')}>对象{sortMark('target')}</button>
        <button onclick={() => changeSort('pid')}>PID{sortMark('pid')}</button>
        <button onclick={() => changeSort('log_path')}>日志路径{sortMark('log_path')}</button>
        <button onclick={() => changeSort('summary')}>告警概要{sortMark('summary')}</button>
        <span class="head-static">详情</span>
      </div>
      {#each filtered as alert}
        <div class="alert-row-wrap" class:row-error={alert.level === 'error'} class:row-warn={alert.level === 'warn'}>
          <button class="alert-row" onclick={() => selectedAlertId = selectedAlertId === alert.id ? '' : alert.id}>
            <span class="time-cell">{alert.last_seen || alert.timestamp}</span>
            <span class="level-pill" style="color:{levelColor(alert.level)};background:{levelBg(alert.level)}">{levelIcon(alert.level)}</span>
            {#if showHistory}<span class="active-cell" class:inactive={!alert.active}>{alert.active ? '活跃' : '历史'}</span>{/if}
            <span class="target-cell" title={primaryTarget(alert)}>{primaryTarget(alert)}<small>{typeLabel(alert.type)}</small></span>
            <span class="pid-cell">{alert.pid || '-'}</span>
            <span class="path-cell" title={alert.log_path || ''}>{alert.log_path || '-'}</span>
            <span class="summary-cell" title={alert.summary || alert.message}>{alert.summary || alert.message}</span>
            <span class="detail-action-cell">
              <span class="detail-open-btn">{selectedAlertId === alert.id ? '收起' : '详情'}</span>
            </span>
          </button>
          {#if selectedAlertId === alert.id}
            <div class="row-details">
              <div class="detail-grid">
                <div><span class="detail-label">级别</span><span class="detail-value" style="color:{levelColor(alert.level)}">{levelLabel(alert.level)}</span></div>
                <div><span class="detail-label">类型</span><span class="detail-value">{typeLabel(alert.type)}</span></div>
                <div><span class="detail-label">ID</span><span class="detail-value detail-id">{alert.id}</span></div>
                <div><span class="detail-label">首次发现</span><span class="detail-value">{alert.first_seen || alert.timestamp}</span></div>
                <div><span class="detail-label">最后发现</span><span class="detail-value">{alert.last_seen || alert.timestamp}</span></div>
                <div><span class="detail-label">出现次数</span><span class="detail-value">{alert.occurrence_count || 1}</span></div>
              </div>
              <div class="detail-message">{alert.message}</div>
              {#if alert.handling}
                <div class="detail-block">
                  <div class="detail-block-title">处理意见</div>
                  <div class="suggestion-line">{alert.handling}</div>
                </div>
              {/if}
              {#if alert.evidence?.length}
                <div class="detail-block">
                  <div class="detail-block-title">异常详情 / 证据</div>
                  {#each alert.evidence as line}
                    <pre class="evidence-line">{line}</pre>
                  {/each}
                </div>
              {/if}
              {#if alert.suggestions?.length}
                <div class="detail-block">
                  <div class="detail-block-title">处理建议</div>
                  {#each alert.suggestions as suggestion}
                    <div class="suggestion-line">{suggestion}</div>
                  {/each}
                </div>
              {/if}
              {#if alert.commands?.length}
                <div class="detail-block">
                  <div class="detail-block-title">建议命令</div>
                  {#each alert.commands as cmd}
                    <code class="command-line">{cmd}</code>
                  {/each}
                </div>
              {/if}
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>

<ConfirmDialog
  open={showClearConfirm}
  title="清理系统告警"
  message="确认清理所有系统告警？当前活跃和历史告警都会被删除，后台后续命中会重新生成。"
  detail={`当前列表: ${alerts.length} 条\n历史视图: ${showHistory ? '开启' : '关闭'}`}
  confirmText="清理告警"
  loading={clearing}
  onCancel={() => showClearConfirm = false}
  onConfirm={confirmClearAllAlerts}
/>

<style>
  .alerts-page { width: 100%; max-width: none; margin: 0; }
  .alerts-header { display: flex; justify-content: space-between; align-items: center; gap: 12px; margin-bottom: 10px; }
  .header-stats { display: flex; gap: 6px; flex-wrap: wrap; }
  .stat-card { padding: 6px 10px; border-radius: 8px; background: var(--bg-card); border: 1px solid var(--border-primary); text-align: center; min-width: 58px; cursor: pointer; }
  .stat-card:hover, .stat-card.active { border-color: var(--border-focus); background: var(--accent-primary-light); }
  .stat-card.stat-error { cursor: pointer; }
  .stat-card.stat-error:hover, .stat-card.stat-error.active { border-color: #ef4444; background: rgba(239, 68, 68, 0.05); }
  .stat-card.stat-warn { cursor: pointer; }
  .stat-card.stat-warn:hover, .stat-card.stat-warn.active { border-color: #f59e0b; background: rgba(245, 158, 11, 0.05); }
  .stat-scan .stat-num { color: #22d3ee; }
  .stat-num { display: block; font-size: 15px; font-weight: 700; color: var(--text-primary); font-family: var(--theme-font-family-mono); line-height: 1.1; }
  .stat-error .stat-num { color: #ef4444; }
  .stat-warn .stat-num { color: #f59e0b; }
  .stat-label { font-size: 11px; color: var(--text-secondary); }
  .header-actions { display: flex; align-items: center; gap: 10px; }
  .search-wrap { position: relative; }
  .search-icon { position: absolute; left: 12px; top: 50%; transform: translateY(-50%); color: var(--text-tertiary); }
  .search-input { width: 220px; padding: 10px 14px 10px 38px; background: var(--bg-input); border: 1px solid var(--border-primary); border-radius: 10px; font-size: 14px; color: var(--text-primary); outline: none; }
  .search-input:focus { border-color: var(--border-focus); }
  .history-btn { display: flex; align-items: center; gap: 6px; padding: 10px 14px; background: var(--bg-secondary); border: 1px solid var(--border-primary); border-radius: 10px; color: var(--text-secondary); font-size: 13px; cursor: pointer; }
  .history-btn:hover { background: var(--bg-hover); }
  .history-btn.active { border-color: var(--accent-primary); color: var(--accent-primary); background: var(--accent-primary-light); }
  .clear-alerts-btn { display: flex; align-items: center; gap: 6px; padding: 10px 12px; background: rgba(239, 68, 68, 0.08); border: 1px solid rgba(239, 68, 68, 0.18); border-radius: 10px; color: #f87171; font-size: 13px; font-weight: 700; cursor: pointer; white-space: nowrap; }
  .clear-alerts-btn:hover:not(:disabled) { background: rgba(239, 68, 68, 0.14); border-color: rgba(239, 68, 68, 0.32); }
  .clear-alerts-btn:disabled { opacity: .55; cursor: not-allowed; }
  .action-notice { margin: 0 0 10px; padding: 8px 11px; border-radius: 9px; font-size: 12px; font-weight: 700; }
  .action-notice.ok { color: #34d399; background: rgba(52, 211, 153, 0.08); border: 1px solid rgba(52, 211, 153, 0.18); }
  .action-notice.error { color: #f87171; background: rgba(239, 68, 68, 0.08); border: 1px solid rgba(239, 68, 68, 0.18); }
  .type-select { position: relative; width: 220px; max-width: 100%; }
  .inline-type-select { width: 190px; }
  .type-select-btn { width: 100%; display: flex; align-items: center; gap: 8px; justify-content: space-between; padding: 9px 11px; border-radius: 9px; border: 1px solid var(--border-primary); background: var(--bg-card); color: var(--text-secondary); cursor: pointer; font-size: 12px; }
  .type-select-btn:hover, .type-select-btn.active { border-color: var(--accent-primary); color: var(--accent-primary); background: var(--accent-primary-light); }
  .type-select-count { padding: 1px 6px; border-radius: 999px; background: var(--bg-tertiary); color: var(--text-tertiary); font-family: var(--theme-font-family-mono); font-size: 10px; }
  .type-select-arrow { color: var(--text-tertiary); font-size: 11px; }
  .type-select-menu { position: absolute; top: calc(100% + 6px); left: 0; width: 280px; max-width: min(90vw, 320px); z-index: 40; padding: 8px; border-radius: 10px; border: 1px solid rgba(34,211,238,.2); background: #0b1020; box-shadow: 0 18px 38px rgba(0,0,0,.42); }
  .type-select-search { width: 100%; margin-bottom: 6px; padding: 8px 9px; border-radius: 7px; border: 1px solid rgba(148,163,184,.16); background: #111827; color: #f8fafc; outline: none; box-sizing: border-box; font-size: 12px; }
  .type-select-search:focus { border-color: rgba(34,211,238,.42); box-shadow: 0 0 0 3px rgba(34,211,238,.08); }
  .type-select-options { max-height: 382px; overflow-y: auto; padding-right: 2px; overscroll-behavior: contain; scrollbar-width: thin; scrollbar-color: rgba(34,211,238,.42) rgba(15,23,42,.9); }
  .type-select-options::-webkit-scrollbar { width: 8px; }
  .type-select-options::-webkit-scrollbar-track { background: rgba(15,23,42,.9); border-radius: 999px; }
  .type-select-options::-webkit-scrollbar-thumb { background: rgba(34,211,238,.38); border-radius: 999px; }
  .type-select-option { width: 100%; display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 4px 8px; align-items: center; padding: 8px 9px; border: none; border-radius: 7px; background: transparent; color: #e2e8f0; cursor: pointer; text-align: left; font-size: 12px; }
  .type-select-option small { grid-column: 1; color: #64748b; font-size: 10px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .type-select-option span:last-child { grid-row: 1 / span 2; grid-column: 2; color: #94a3b8; font-family: var(--theme-font-family-mono); font-size: 11px; }
  .type-select-option:hover, .type-select-option.highlighted { background: rgba(34,211,238,.1); color: #67e8f9; }
  .type-select-option.active { background: rgba(34,211,238,.16); color: #67e8f9; box-shadow: inset 2px 0 0 #22d3ee; }
  .type-select-option.highlighted small, .type-select-option.active small { color: #94a3b8; }
  .type-select-empty { padding: 10px; color: #64748b; font-size: 12px; text-align: center; }
  .scan-compact { margin-bottom: 10px; color: var(--text-tertiary); font-size: 12px; text-align: right; }
  .alerts-list { background: var(--bg-card); border: 1px solid var(--border-primary); border-radius: 10px; overflow-x: auto; overflow-y: hidden; }
  .alerts-table-head { display: grid; grid-template-columns: 132px 66px 132px 62px 150px minmax(360px, 1.8fr) 74px; gap: 8px; padding: 10px 12px; background: var(--bg-secondary); border-bottom: 1px solid var(--border-primary); }
  .alerts-table-head,
  .alert-row { min-width: 980px; }
  .alerts-list.with-history .alerts-table-head,
  .alerts-list.with-history .alert-row { grid-template-columns: 132px 66px 58px 132px 62px 150px minmax(360px, 1.8fr) 74px; min-width: 1120px; }
  .alerts-table-head button, .head-static { border: none; background: transparent; color: var(--text-secondary); font-size: 12px; font-weight: 700; text-align: left; cursor: pointer; padding: 0; }
  .head-static { cursor: default; text-align: center; }
  .alerts-table-head button:hover { color: var(--text-primary); }
  .alert-row-wrap { border-bottom: 1px solid var(--border-secondary); border-left: 4px solid transparent; transition: background .2s ease, border-color .2s ease, opacity .2s ease; }
  .alert-row-wrap:last-child { border-bottom: none; }
  .alert-row-wrap.row-error { border-left-color: #ef4444; }
  .alert-row-wrap.row-warn { border-left-color: #f59e0b; }
  .alert-row { display: grid; grid-template-columns: 132px 66px 132px 62px 150px minmax(360px, 1.8fr) 74px; gap: 8px; align-items: center; width: 100%; padding: 11px 12px; border: none; background: transparent; color: inherit; text-align: left; cursor: pointer; transition: background .18s ease; }
  .alert-row:hover { background: var(--bg-hover); }
  .level-pill { display: inline-flex; align-items: center; justify-content: center; width: fit-content; min-width: 48px; padding: 4px 8px; border-radius: 7px; font-size: 10px; font-weight: 800; font-family: var(--theme-font-family-mono); }
  .active-cell { width: fit-content; padding: 3px 7px; border-radius: 999px; color: #10b981; background: rgba(16, 185, 129, 0.08); font-size: 11px; font-weight: 700; }
  .active-cell.inactive { color: var(--text-tertiary); background: var(--bg-secondary); }
  .target-cell { min-width: 0; color: var(--text-primary); font-size: 13px; font-weight: 700; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .target-cell small { display: block; margin-top: 2px; color: var(--text-tertiary); font-size: 10px; font-weight: 500; }
  .pid-cell, .path-cell, .time-cell { min-width: 0; color: var(--text-secondary); font-size: 12px; font-family: var(--theme-font-family-mono); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .summary-cell { min-width: 0; color: var(--text-secondary); font-size: 12px; line-height: 1.4; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .detail-action-cell { display: flex; justify-content: center; }
  .detail-open-btn { display: inline-flex; align-items: center; justify-content: center; min-width: 52px; height: 26px; padding: 0 9px; border-radius: 8px; border: 1px solid rgba(34,211,238,.24); background: rgba(34,211,238,.08); color: #67e8f9; font-size: 11px; font-weight: 800; }
  .row-details { padding: 12px 16px 16px; background: color-mix(in srgb, var(--bg-secondary) 70%, transparent); border-top: 1px solid var(--border-secondary); }
  .detail-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); gap: 8px 16px; margin-bottom: 10px; }
  .detail-grid > div { display: flex; justify-content: space-between; gap: 12px; padding: 7px 8px; background: var(--bg-card); border: 1px solid var(--border-secondary); border-radius: 7px; }
  .detail-label { font-size: 12px; color: var(--text-secondary); }
  .detail-value { min-width: 0; font-size: 12px; color: var(--text-primary); font-weight: 500; overflow-wrap: anywhere; text-align: right; }
  .detail-id { font-family: var(--theme-font-family-mono); }
  .detail-message { margin: 8px 0 4px; padding: 9px 10px; color: var(--text-primary); background: var(--bg-card); border: 1px solid var(--border-secondary); border-radius: 7px; font-size: 13px; line-height: 1.5; }
  .detail-block { margin-top: 12px; padding-top: 10px; border-top: 1px solid var(--border-secondary); }
  .detail-block-title { margin-bottom: 7px; color: var(--text-secondary); font-size: 12px; font-weight: 700; }
  .evidence-line { margin: 0 0 6px; padding: 7px 8px; background: var(--bg-secondary); border: 1px solid var(--border-secondary); border-radius: 6px; color: var(--text-primary); white-space: pre-wrap; overflow-wrap: anywhere; font-family: var(--theme-font-family-mono); font-size: 11px; line-height: 1.45; }
  .suggestion-line { margin-bottom: 6px; padding: 7px 8px; background: rgba(34, 211, 238, 0.06); border: 1px solid rgba(34, 211, 238, 0.12); border-radius: 6px; color: var(--text-secondary); font-size: 12px; line-height: 1.45; }
  .command-line { display: block; margin-bottom: 6px; padding: 7px 8px; background: #0b1020; border: 1px solid rgba(148, 163, 184, 0.16); border-radius: 6px; color: #d1d5db; font-family: var(--theme-font-family-mono); font-size: 11px; white-space: pre-wrap; overflow-wrap: anywhere; }
  .loading-state { display: flex; flex-direction: column; align-items: center; gap: 12px; padding: 60px 0; color: var(--text-secondary); }
  .loading-pulse { width: 40px; height: 40px; border-radius: 50%; background: var(--accent-primary); animation: pulse 1.5s ease-in-out infinite; }
  @keyframes pulse { 0%, 100% { transform: scale(0.8); opacity: 0.5; } 50% { transform: scale(1.2); opacity: 1; } }
  .empty-state { display: flex; flex-direction: column; align-items: center; gap: 8px; padding: 44px 16px; background: var(--bg-card); border: 1px solid var(--border-primary); border-radius: 10px; text-align: center; }
  .empty-title { font-size: 18px; font-weight: 600; color: var(--text-primary); }
  .empty-desc { max-width: 760px; font-size: 13px; color: var(--text-secondary); line-height: 1.6; }
  @media (max-width: 768px) {
    .alerts-header { flex-direction: column; }
    .header-stats { width: 100%; }
    .stat-card { flex: 1; }
    .header-actions { width: 100%; }
    .search-input { width: 100%; }
    .alerts-table-head { display: none; }
    .alert-row { grid-template-columns: 68px 1fr; }
    .pid-cell, .path-cell, .time-cell { display: none; }
    .summary-cell { grid-column: 2; white-space: normal; }
  }
</style>
