<script>
  import { onMount, onDestroy } from 'svelte';

  let checks = $state([]);
  let loading = $state(true);
  let search = $state('');
  let selectedCategory = $state('');
  let healthCheckRunning = $state(false);
  let healthCheckResult = $state(null);
  let healthCheckError = $state(null);
  let healthProgress = $state(null);
  let healthPollTimer = null;
  let healthFinishTimer = null;
  let healthStartedAt = 0;
  let exporting = $state(false);
  let importing = $state(false);
  let exportError = $state(null);
  let importInput = $state(null);
  let expandedChecks = $state(new Set());
  let viewMode = $state('grid');
  let sortKey = $state('name');
  let sortDir = $state('asc');
  let showFilters = $state(false);
  let showHealthLogs = $state(false);
  let showHealthModal = $state(false);

  async function load() {
    loading = true;
    try {
      const r = await fetch('/api/checks');
      if (r.ok) {
        const d = await r.json();
        checks = d.checks || [];
      }
    } catch (e) { console.warn('加载检查项失败:', e); }
    loading = false;
  }

  async function runFullHealthCheck() {
    showHealthModal = true;
    showHealthLogs = false;
    healthCheckRunning = true;
    healthCheckResult = null;
    healthCheckError = null;
    exportError = null;
    healthStartedAt = Date.now();
    if (healthFinishTimer) clearTimeout(healthFinishTimer);
    healthFinishTimer = null;
    healthProgress = {
      percent: 0,
      current_step: '准备启动体检任务',
      logs: ['正在创建后台体检任务...'],
      completed: 0,
      total: 0,
      warnings: 0,
      errors: 0,
      status: 'running',
    };
    try {
      const r = await fetch('/api/health/full/start', { method: 'POST' });
      if (!r.ok) {
        healthCheckError = '体检失败';
        healthCheckRunning = false;
        return;
      }
      const d = await r.json();
      pollHealthProgress(d.task_id);
    } catch (e) {
      healthCheckError = e.message;
      healthCheckRunning = false;
    }
  }

  async function pollHealthProgress(taskId) {
    if (healthPollTimer) clearInterval(healthPollTimer);
    const loadProgress = async () => {
      try {
        const r = await fetch(`/api/health/full/${encodeURIComponent(taskId)}?ts=${Date.now()}`, { cache: 'no-store' });
        if (!r.ok) throw new Error('读取体检进度失败');
        const d = await r.json();
        const elapsed = Date.now() - healthStartedAt;
        const minVisibleMs = 10000;
        if (d.status === 'done' && elapsed < minVisibleMs) {
          healthProgress = {
            ...d,
            status: 'running',
            percent: Math.min(d.percent || 100, 96),
            current_step: '正在生成体检报告与同步告警',
            logs: [...(d.logs || []), '正在整理结构化检查结果、告警命中和导出数据...'],
          };
          if (!healthFinishTimer) {
            healthFinishTimer = setTimeout(() => {
              healthFinishTimer = null;
              if (healthPollTimer) clearInterval(healthPollTimer);
              healthPollTimer = null;
              healthProgress = { ...d, percent: 100 };
              healthCheckRunning = false;
              showHealthLogs = false;
              if (d.result) healthCheckResult = d.result;
              window.dispatchEvent(new CustomEvent('dm-alerts-refresh'));
            }, minVisibleMs - elapsed);
          }
          return;
        }
        healthProgress = d;
        if (d.status === 'done' || d.status === 'error') {
          if (healthPollTimer) clearInterval(healthPollTimer);
          healthPollTimer = null;
          healthCheckRunning = false;
          showHealthLogs = false;
          if (d.result) healthCheckResult = d.result;
          window.dispatchEvent(new CustomEvent('dm-alerts-refresh'));
          if (d.status === 'error') healthCheckError = d.current_step || '体检失败';
        }
      } catch (e) {
        if (healthPollTimer) clearInterval(healthPollTimer);
        healthPollTimer = null;
        healthCheckRunning = false;
        healthCheckError = e.message;
      }
    };
    await loadProgress();
    healthPollTimer = setInterval(loadProgress, 700);
  }

  async function exportAllChecks() {
    exporting = true;
    exportError = null;
    try {
      const r = await fetch('/api/checks/export', { cache: 'no-store' });
      if (!r.ok) throw new Error('导出失败: ' + r.status);
      const data = await r.json();
      const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json;charset=utf-8' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      const ts = new Date().toISOString().replace(/[:.]/g, '-');
      a.href = url;
      a.download = `dm-checks-export-${ts}.json`;
      document.body.appendChild(a);
      a.click();
      a.remove();
      URL.revokeObjectURL(url);
      healthCheckResult = {
        overall_status: data.overall_status,
        total_checks: data.total,
        total_warnings: data.summary?.warnings || 0,
        total_errors: data.summary?.errors || 0,
        checks: (data.checks || []).map(c => ({
          id: c.id,
          name: c.name,
          status: c.status,
          duration_ms: c.duration_ms,
          warnings: c.warning_count || 0,
          errors: c.error_count || 0,
        })),
        alerts: data.alerts || [],
        timestamp: data.exported_at,
      };
      window.dispatchEvent(new CustomEvent('dm-alerts-refresh'));
    } catch (e) {
      exportError = e.message;
    }
    exporting = false;
  }

  function importAllChecks() {
    importInput?.click();
  }

  function closeHealthPanels() {
    showHealthModal = false;
    showHealthLogs = false;
    healthCheckResult = null;
    healthCheckError = null;
    exportError = null;
  }

  function closeHealthModal() {
    showHealthModal = false;
  }

  async function importCheckFile(event) {
    const file = event.currentTarget.files?.[0];
    if (!file) return;
    importing = true;
    exportError = null;
    try {
      const text = await file.text();
      const data = JSON.parse(text);
      if (!data || typeof data !== 'object' || !Array.isArray(data.checks)) {
        throw new Error('导入文件格式不正确，必须包含 checks 数组');
      }
      const payload = {
        ...data,
        imported_at: new Date().toISOString(),
        imported_file: file.name,
      };
      sessionStorage.setItem('dm-imported-check-report', JSON.stringify(payload));
      location.hash = '#/check-import';
    } catch (e) {
      exportError = '导入失败: ' + (e.message || e);
    } finally {
      importing = false;
      event.currentTarget.value = '';
    }
  }

  function toggleCheck(id) {
    const next = new Set(expandedChecks);
    if (next.has(id)) next.delete(id); else next.add(id);
    expandedChecks = next;
  }

  function changeSort(key) {
    if (sortKey === key) sortDir = sortDir === 'asc' ? 'desc' : 'asc';
    else {
      sortKey = key;
      sortDir = 'asc';
    }
  }

  function sortMark(key) {
    if (sortKey !== key) return '';
    return sortDir === 'asc' ? ' ↑' : ' ↓';
  }

  function iconPath(id) {
    const normalized = String(id || '').toLowerCase();
    if (normalized.includes('elasticsearch')) return 'M5 8.5 12 4l7 4.5v7L12 20l-7-4.5v-7Zm3.1 1.8h7.8M8.1 13.7h7.8M12 4v16';
    if (normalized.includes('redis')) return 'M12 3 4.5 6.8 12 10.6l7.5-3.8L12 3Zm-7.5 8.4L12 15.2l7.5-3.8M4.5 16 12 20l7.5-4';
    if (normalized.includes('nginx')) return 'M12 3.2 4.6 7.5v9L12 20.8l7.4-4.3v-9L12 3.2Zm-3.5 6.3v5m7-5v5m-7-5 7 5m0-5-7 5';
    if (normalized.includes('mysql')) return 'M4 6c0-1.7 3.6-3 8-3s8 1.3 8 3-3.6 3-8 3-8-1.3-8-3Zm0 0v6c0 1.7 3.6 3 8 3s8-1.3 8-3V6M4 12v6c0 1.7 3.6 3 8 3s8-1.3 8-3v-6';
    if (normalized.includes('java')) return 'M8 18h8M9 14h6M10 3.8c2.3 2.1-2.1 3.3.2 5.5m4-5.5c2.3 2.1-2.1 3.3.2 5.5M6.5 10.5h11l-.8 3.7A5 5 0 0 1 12 18a5 5 0 0 1-4.7-3.8l-.8-3.7Z';
    if (normalized.includes('keep')) return 'M12 3l7 4v5c0 4.5-3 7.5-7 9-4-1.5-7-4.5-7-9V7l7-4Zm-3 9 2 2 4-5';
    if (normalized.includes('security')) return 'M12 3l7 4v5c0 4.5-3 7.5-7 9-4-1.5-7-4.5-7-9V7l7-4Zm0 5v5m0 3h.01';
    if (normalized.includes('network')) return 'M6 18h12M12 6v8m-5.5-5.5L12 3l5.5 5.5M4 14h4l2-3 3 6 2-3h5';
    if (normalized.includes('service')) return 'M4 7h10M4 12h16M4 17h10M17 5l3 2-3 2M17 15l3 2-3 2';
    if (normalized.includes('resource') || normalized.includes('system')) return 'M4 13h4l2-7 4 14 2-7h4M4 20h16';
    if (normalized.includes('container')) return 'M3 7l9-4 9 4-9 4-9-4Zm0 4l9 4 9-4M3 15l9 4 9-4';
    if (normalized.includes('middleware')) return 'M5 5h6v6H5V5Zm8 0h6v6h-6V5ZM5 13h6v6H5v-6Zm8 0h6v6h-6v-6Z';
    if (normalized.includes('schedule')) return 'M7 3v3m10-3v3M4 8h16M6 5h12a2 2 0 0 1 2 2v11a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2Zm3 8h3v3H9v-3Z';
    return 'M5 4h14v16H5V4Zm4 4h6m-7 4h8m-8 4h5';
  }

  function iconTone(id) {
    const normalized = String(id || '').toLowerCase();
    if (normalized.includes('security')) return 'security';
    if (normalized.includes('mysql') || normalized.includes('redis') || normalized.includes('elasticsearch')) return 'data';
    if (normalized.includes('nginx') || normalized.includes('network')) return 'network';
    if (normalized.includes('java') || normalized.includes('service')) return 'service';
    if (normalized.includes('resource') || normalized.includes('system')) return 'resource';
    return 'default';
  }

  function statusColor(s) {
    if (s === 'ok') return '#10b981';
    if (s === 'warn') return '#f59e0b';
    if (s === 'error') return '#ef4444';
    return '#94a3b8';
  }

  function statusBg(s) {
    if (s === 'ok') return 'rgba(16, 185, 129, 0.1)';
    if (s === 'warn') return 'rgba(245, 158, 11, 0.1)';
    if (s === 'error') return 'rgba(239, 68, 68, 0.1)';
    return 'rgba(148, 163, 184, 0.1)';
  }

  let categories = $derived.by(() => {
    const cats = new Set();
    for (const c of checks) cats.add(c.category);
    return [...cats].sort();
  });

  let numberedChecks = $derived.by(() => checks.map((check, index) => ({
    ...check,
    number: index + 1,
    numberLabel: String(index + 1).padStart(2, '0'),
  })));

  let filtered = $derived.by(() => {
    let result = numberedChecks;
    if (selectedCategory) result = result.filter(c => c.category === selectedCategory);
    if (search.trim()) {
      const q = search.trim().toLowerCase();
      result = result.filter(c =>
        String(c.number).includes(q) ||
        c.numberLabel.includes(q) ||
        ('#' + c.number).includes(q) ||
        ('#' + c.numberLabel).includes(q) ||
        c.name.toLowerCase().includes(q) ||
        c.id.toLowerCase().includes(q) ||
        c.description.toLowerCase().includes(q)
      );
    }
    return [...result].sort((a, b) => {
      const av = sortKey === 'number' ? a.number : (a[sortKey] || '');
      const bv = sortKey === 'number' ? b.number : (b[sortKey] || '');
      let cmp = typeof av === 'number' && typeof bv === 'number'
        ? av - bv
        : String(av).localeCompare(String(bv), 'zh-CN', { numeric: true });
      if (sortDir === 'desc') cmp = -cmp;
      return cmp;
    });
  });

  let grouped = $derived.by(() => {
    const map = {};
    for (const c of filtered) {
      if (!map[c.category]) map[c.category] = [];
      map[c.category].push(c);
    }
    return map;
  });

  onMount(load);
  onDestroy(() => {
    if (healthPollTimer) clearInterval(healthPollTimer);
    if (healthFinishTimer) clearTimeout(healthFinishTimer);
  });
</script>

<div class="checks-page">
  <div class="page-header">
    <div class="header-left">
      <span class="check-count">{filtered.length} / {checks.length} 项检查</span>
    </div>
    <div class="header-right">
      <div class="view-toggle">
        <button class="sort-chip" onclick={() => changeSort('number')}>编号{sortMark('number')}</button>
        <button class="sort-chip" onclick={() => changeSort('name')}>名称{sortMark('name')}</button>
        <button class="sort-chip" onclick={() => changeSort('category')}>分类{sortMark('category')}</button>
        <button class="sort-chip" onclick={() => changeSort('id')}>ID{sortMark('id')}</button>
        <button class="view-btn" class:active={viewMode === 'list'} onclick={() => viewMode = 'list'} title="列表视图">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="8" y1="6" x2="21" y2="6"/><line x1="8" y1="12" x2="21" y2="12"/><line x1="8" y1="18" x2="21" y2="18"/><line x1="3" y1="6" x2="3.01" y2="6"/><line x1="3" y1="12" x2="3.01" y2="12"/><line x1="3" y1="18" x2="3.01" y2="18"/></svg>
        </button>
        <button class="view-btn" class:active={viewMode === 'grid'} onclick={() => viewMode = 'grid'} title="卡片视图">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/></svg>
        </button>
      </div>
      <div class="search-wrap">
        <svg class="search-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.35-4.35" stroke-linecap="round"/></svg>
        <input type="text" placeholder="输入编号、名称、ID 快搜检查项..." bind:value={search} class="search-input" />
        {#if search}
          <button class="search-clear" onclick={() => search = ''}>✕</button>
        {/if}
      </div>
      <button class="tool-btn" onclick={() => showFilters = !showFilters}>
        筛选{selectedCategory || search ? ' *' : ''} {showFilters ? '↑' : '↓'}
      </button>
      <button class="tool-btn" onclick={() => showHealthLogs = !showHealthLogs}>
        体检日志 {showHealthLogs ? '↑' : '↓'}
      </button>
      <button class="tool-btn export-mini" onclick={exportAllChecks} disabled={exporting || healthCheckRunning}>
        {exporting ? '导出中' : '导出'}
      </button>
      <button class="tool-btn import-mini" onclick={importAllChecks} disabled={importing || healthCheckRunning}>
        {importing ? '导入中' : '导入'}
      </button>
      <input bind:this={importInput} class="hidden-file" type="file" accept="application/json,.json" onchange={importCheckFile} />
    </div>
  </div>

  <div class="health-check-section">
    <div class="health-check-card">
      <div class="health-check-left">
        <div class="health-check-icon">🏥</div>
        <div class="health-check-info">
          <h2 class="health-check-title">系统体检</h2>
          <p class="health-check-desc">一键检查所有系统指标，发现潜在问题</p>
        </div>
      </div>
      <button class="health-check-btn" onclick={runFullHealthCheck} disabled={healthCheckRunning}>
        {#if healthCheckRunning}
          <div class="btn-spinner"></div>
          <span>{healthProgress?.percent || 0}%</span>
        {:else}
          <span class="btn-icon">▶</span>
          <span>开始体检</span>
        {/if}
      </button>
    </div>

    {#if exportError}
      <div class="health-error">
        <span class="error-icon">✗</span>
        <span>{exportError}</span>
        <button class="error-close" onclick={() => exportError = null}>✕</button>
      </div>
    {/if}
  </div>

  {#if showFilters}
    <div class="category-chips">
      <button class="chip" class:active={!selectedCategory} onclick={() => selectedCategory = ''}>
        <span>全部</span>
        <span class="chip-count">{checks.length}</span>
      </button>
      {#each categories as cat}
        <button class="chip" class:active={selectedCategory === cat} onclick={() => selectedCategory = cat}>
          <span>{cat}</span>
          <span class="chip-count">{checks.filter(c => c.category === cat).length}</span>
        </button>
      {/each}
    </div>
  {/if}

  {#if loading}
    <div class="loading">
      <div class="checks-loader">
        <i></i><i></i><i></i><i></i><i></i><i></i>
      </div>
      <div class="loading-text">正在加载检查目录...</div>
    </div>
  {:else if filtered.length === 0}
    <div class="empty-state">
      <span class="empty-icon">🔍</span>
      <span class="empty-text">没有匹配的检查项</span>
    </div>
  {:else}
    {#each Object.entries(grouped) as [cat, items]}
      <div class="category-section">
        <h2 class="category-title">{cat}</h2>
        {#if viewMode === 'list'}
          <div class="checks-list">
            {#each items as check}
              <a href="#/check/{check.id}" class="check-row">
                <span class="check-number">#{check.numberLabel}</span>
                <span class="check-icon {iconTone(check.id)}">
                  <svg viewBox="0 0 24 24" aria-hidden="true"><path d={iconPath(check.id)} /></svg>
                </span>
                <span class="check-name">{check.name}</span>
                <span class="check-desc">{check.description}</span>
                <span class="check-id">{check.id}</span>
                <span class="check-arrow">→</span>
              </a>
            {/each}
          </div>
        {:else}
          <div class="checks-grid">
            {#each items as check}
              <a href="#/check/{check.id}" class="check-card">
                <span class="card-number">#{check.numberLabel}</span>
                <div class="card-icon {iconTone(check.id)}">
                  <svg viewBox="0 0 24 24" aria-hidden="true"><path d={iconPath(check.id)} /></svg>
                </div>
                <div class="card-info">
                  <h3 class="card-title">{check.name}</h3>
                  <p class="card-desc">{check.description}</p>
                  <span class="card-id">{check.id}</span>
                </div>
                <div class="card-arrow">→</div>
              </a>
            {/each}
          </div>
        {/if}
      </div>
    {/each}
  {/if}
</div>

{#if showHealthModal}
  <div class="health-modal-overlay" onclick={closeHealthModal} role="presentation">
    <div class="health-modal" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="dialog" aria-modal="true" tabindex="-1">
      <div class="health-modal-head">
        <div>
          <h2>系统体检</h2>
          <p>{healthProgress?.current_step || (healthCheckResult ? '体检报告已生成' : '正在准备检查任务')}</p>
        </div>
        <div class="health-modal-actions">
          {#if (healthProgress?.logs || []).length || healthCheckResult}
            <button class="modal-log-toggle" onclick={() => showHealthLogs = !showHealthLogs}>
              {showHealthLogs ? '收起日志' : '查看日志'}
            </button>
          {/if}
          {#if healthCheckRunning}
            <span class="modal-running">{healthProgress?.percent || 0}%</span>
          {/if}
          <button class="modal-icon-btn" onclick={closeHealthModal} aria-label="关闭体检窗口">✕</button>
        </div>
      </div>

      <div class="health-modal-body">
        {#if healthCheckRunning}
          <div class="health-loading modal-loading">
            <div class="scanner-grid"></div>
            <div class="health-progress-head">
              <div class="progress-orb" style="--p:{healthProgress?.percent || 0}">
                <div class="progress-orb-inner">
                  <span>{healthProgress?.percent || 0}%</span>
                </div>
              </div>
              <div class="progress-meta">
                <div class="progress-title">{healthProgress?.current_step || '正在检查系统健康状态'}</div>
                <div class="progress-sub">
                  已完成 {healthProgress?.completed || 0}/{healthProgress?.total || 0}
                  · 警告 {healthProgress?.warnings || 0}
                  · 错误 {healthProgress?.errors || 0}
                </div>
                <div class="loading-bar">
                  <div class="loading-progress" style="width:{healthProgress?.percent || 0}%"></div>
                </div>
              </div>
            </div>
            <div class="step-rail" aria-label="体检步骤">
              {#each ['启动', '采集', '规则命中', '告警同步', '报告'] as step, i}
                <div class="step-dot" class:active={(healthProgress?.percent || 0) >= i * 24}>
                  <span>{i + 1}</span>
                  <em>{step}</em>
                </div>
              {/each}
            </div>
            <div class="loading-actions">
              <button class="show-log-btn" onclick={() => showHealthLogs = !showHealthLogs}>
                {showHealthLogs ? '收起执行日志' : '查看执行日志'}
              </button>
            </div>
          </div>
        {/if}

        {#if showHealthLogs}
          <div class="health-log-panel modal-log-panel">
            <div class="health-log-head">
              <div class="health-log-title">体检执行日志</div>
              <div class="panel-actions">
                <button class="panel-close" onclick={() => showHealthLogs = false}>隐藏日志</button>
                {#if healthCheckResult || healthCheckError || exportError}
                  <button class="panel-close muted" onclick={closeHealthPanels}>关闭全部</button>
                {/if}
              </div>
            </div>
            {#if (healthProgress?.logs || []).length}
              {#each (healthProgress?.logs || []) as line}
                <div class="log-line">{line}</div>
              {/each}
            {:else if healthCheckResult}
              <div class="log-line">{healthCheckResult.timestamp || ''} 体检完成：{healthCheckResult.total_checks || 0} 项，警告 {healthCheckResult.total_warnings || 0}，错误 {healthCheckResult.total_errors || 0}</div>
            {:else}
              <div class="log-line">暂无体检日志，点击开始体检后会实时记录步骤。</div>
            {/if}
          </div>
        {/if}

        {#if healthCheckResult}
          <div class="health-result-panel modal-result-panel">
            <div class="result-header">
              <div class="result-status" style="color:{statusColor(healthCheckResult.overall_status)}">
                {#if healthCheckResult.overall_status === 'ok'}
                  <span class="result-icon">✓</span>
                  <span>系统健康</span>
                {:else if healthCheckResult.overall_status === 'warn'}
                  <span class="result-icon">⚠</span>
                  <span>有警告</span>
                {:else}
                  <span class="result-icon">✗</span>
                  <span>有问题</span>
                {/if}
              </div>
              <div class="result-stats">
                <div class="stat-item">
                  <span class="stat-value">{healthCheckResult.total_checks}</span>
                  <span class="stat-label">检查项</span>
                </div>
                <div class="stat-item stat-warn">
                  <span class="stat-value">{healthCheckResult.total_warnings}</span>
                  <span class="stat-label">警告</span>
                </div>
                <div class="stat-item stat-error">
                  <span class="stat-value">{healthCheckResult.total_errors}</span>
                  <span class="stat-label">错误</span>
                </div>
              </div>
              <div class="result-actions">
                <button class="result-close-all" onclick={closeHealthPanels}>关闭全部</button>
              </div>
            </div>

            <div class="result-checks">
              {#each healthCheckResult.checks as check}
                <a href="#/check/{check.id}" class="result-check-item" class:check-ok={check.status === 'ok'} class:check-warn={check.status === 'warn'} class:check-error={check.status === 'error'}>
                  <div class="check-main">
                    <span class="check-status-dot" style="background:{statusColor(check.status)}"></span>
                    <span class="check-name">{check.name}</span>
                    {#if check.warnings || check.errors}
                      <span class="check-findings">{check.warnings || 0} 警告 / {check.errors || 0} 错误</span>
                    {/if}
                    <span class="check-duration">{check.duration_ms}ms</span>
                    <span class="check-arrow">→</span>
                  </div>
                </a>
              {/each}
            </div>
            {#if (healthCheckResult.alerts || []).length}
              <div class="result-alerts">
                <div class="result-alerts-title">规则命中告警 {(healthCheckResult.alerts || []).length} 条</div>
                {#each (healthCheckResult.alerts || []).slice(0, 12) as alert}
                  <a href="#/alerts" class="result-alert-item">
                    <span class="alert-level" class:error={alert.level === 'error'}>{alert.level || 'warn'}</span>
                    <span class="alert-title">{alert.title || alert.summary || alert.message}</span>
                    <span class="alert-target">{alert.service_name || alert.target || alert.pid || alert.log_path || '系统'}</span>
                    <span class="alert-handling">{alert.handling || (alert.suggestions || [])[0] || '进入告警页查看处理建议'}</span>
                  </a>
                {/each}
                {#if (healthCheckResult.alerts || []).length > 12}
                  <a href="#/alerts" class="more-alerts">查看全部告警 →</a>
                {/if}
              </div>
            {/if}
          </div>
        {/if}

        {#if healthCheckError}
          <div class="health-error">
            <span class="error-icon">✗</span>
            <span>{healthCheckError}</span>
            <button class="error-close" onclick={() => healthCheckError = null}>✕</button>
          </div>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .checks-page { width: 100%; max-width: none; margin: 0; }
  .page-header { display: flex; align-items: center; justify-content: space-between; gap: 16px; margin-bottom: 24px; }
  .header-left { display: flex; align-items: baseline; gap: 10px; }
  .check-count { font-size: 14px; color: var(--text-secondary); }
  .header-right { display: flex; align-items: center; gap: 10px; }
  .search-wrap { position: relative; }
  .search-icon { position: absolute; left: 12px; top: 50%; transform: translateY(-50%); color: var(--text-tertiary); }
  .search-input { width: 260px; padding: 10px 14px 10px 38px; background: var(--bg-input); border: 1px solid var(--border-primary); border-radius: 10px; font-size: 14px; color: var(--text-primary); outline: none; transition: all 0.2s; box-sizing: border-box; }
  .search-input:focus { border-color: var(--border-focus); box-shadow: 0 0 0 3px var(--accent-primary-light); }
  .search-clear { position: absolute; right: 10px; top: 50%; transform: translateY(-50%); background: none; border: none; color: var(--text-tertiary); cursor: pointer; font-size: 14px; }

  .health-check-section { margin-bottom: 24px; }
  .health-check-card { display: flex; align-items: center; justify-content: space-between; padding: 20px 24px; background: linear-gradient(135deg, #3b82f6 0%, #2563eb 100%); border-radius: 16px; color: white; }
  .health-check-left { display: flex; align-items: center; gap: 16px; }
  .health-check-icon { font-size: 40px; }
  .health-check-info {}
  .health-check-title { font-size: 20px; font-weight: 700; margin: 0 0 4px; }
  .health-check-desc { font-size: 14px; opacity: 0.9; margin: 0; }
  .health-check-btn { display: flex; align-items: center; gap: 8px; padding: 12px 24px; background: white; color: #2563eb; border: none; border-radius: 12px; font-size: 15px; font-weight: 600; cursor: pointer; transition: all 0.2s; }
  .tool-btn { height: 34px; padding: 0 11px; border: 1px solid var(--border-primary); border-radius: 8px; background: var(--bg-secondary); color: var(--text-secondary); font-size: 12px; font-weight: 700; cursor: pointer; }
  .tool-btn:hover { border-color: var(--accent-primary); color: var(--accent-primary); background: var(--accent-primary-light); }
  .tool-btn:disabled { opacity: .55; cursor: not-allowed; }
  .export-mini { color: #67e8f9; border-color: rgba(34,211,238,.24); }
  .import-mini { color: #a7f3d0; border-color: rgba(52,211,153,.2); }
  .hidden-file { display: none; }
  .health-check-btn:hover:not(:disabled) { transform: translateY(-2px); box-shadow: 0 8px 20px rgba(0,0,0,0.2); }
  .health-check-btn:disabled { opacity: 0.7; cursor: not-allowed; }
  .health-close-all { margin-left: 10px; padding: 10px 14px; border-radius: 10px; border: 1px solid rgba(255,255,255,.28); background: rgba(15, 23, 42, .18); color: white; font-size: 12px; font-weight: 800; cursor: pointer; }
  .health-close-all:hover { background: rgba(15, 23, 42, .3); }
  .btn-icon { font-size: 16px; }
  .btn-spinner { width: 18px; height: 18px; border-radius: 50%; border: 2px solid rgba(37, 99, 235, 0.22); border-top-color: #2563eb; animation: spin 0.8s linear infinite; }
  @keyframes pulse { 0%, 100% { transform: scale(0.8); opacity: 0.5; } 50% { transform: scale(1.2); opacity: 1; } }

  .health-loading { position: relative; overflow: hidden; background: linear-gradient(180deg, rgba(15, 23, 42, 0.96), rgba(8, 13, 24, 0.98)); border: 1px solid rgba(34, 211, 238, 0.2); border-radius: 12px; padding: 18px; margin-top: 12px; box-shadow: 0 18px 48px rgba(15, 23, 42, 0.24); }
  .health-loading::before { content: ''; position: absolute; inset: 0; background: linear-gradient(90deg, transparent, rgba(34,211,238,0.12), transparent); transform: translateX(-100%); animation: scanSweep 2.4s linear infinite; pointer-events: none; }
  .scanner-grid { position: absolute; inset: 0; opacity: .22; background-image: linear-gradient(rgba(34,211,238,.16) 1px, transparent 1px), linear-gradient(90deg, rgba(34,211,238,.12) 1px, transparent 1px); background-size: 28px 28px; animation: gridDrift 5s linear infinite; pointer-events: none; }
  .health-progress-head { display: flex; align-items: center; gap: 18px; }
  .progress-orb { --p: 0; width: 86px; height: 86px; border-radius: 50%; flex-shrink: 0; display: grid; place-items: center; background: conic-gradient(#22d3ee calc(var(--p) * 1%), rgba(148, 163, 184, 0.18) 0); position: relative; animation: orbSpin 1.6s linear infinite; }
  .progress-orb::after { content: ''; position: absolute; inset: -6px; border-radius: 50%; border: 1px solid rgba(34, 211, 238, 0.16); border-top-color: rgba(34, 211, 238, 0.72); animation: spin 1.1s linear infinite; }
  .progress-orb-inner { width: 66px; height: 66px; border-radius: 50%; display: grid; place-items: center; background: var(--bg-card); color: var(--text-primary); font-family: var(--theme-font-family-mono); font-size: 18px; font-weight: 800; }
  .progress-meta { flex: 1; min-width: 0; }
  .progress-title { color: var(--text-primary); font-size: 15px; font-weight: 700; margin-bottom: 6px; }
  .progress-sub { color: var(--text-secondary); font-size: 12px; margin-bottom: 10px; }
  .loading-bar { height: 7px; background: var(--bg-secondary); border-radius: 999px; overflow: hidden; }
  .loading-progress { height: 100%; background: linear-gradient(90deg, #22d3ee, #10b981, #f59e0b); background-size: 200% 100%; animation: shimmer 1.6s linear infinite; border-radius: 999px; transition: width .35s ease; }
  .loading-logs { margin-top: 14px; max-height: 178px; overflow: auto; padding: 10px; border-radius: 9px; background: #0b1020; border: 1px solid rgba(148, 163, 184, 0.16); }
  .log-line { color: #cbd5e1; font-family: var(--theme-font-family-mono); font-size: 11px; line-height: 1.65; white-space: pre-wrap; overflow-wrap: anywhere; }
  .health-log-panel { margin: 12px 0 0; padding: 12px; max-height: 240px; overflow: auto; border-radius: 10px; border: 1px solid rgba(34,211,238,.16); background: #0b1020; }
  .health-log-head { display: flex; justify-content: space-between; align-items: center; gap: 10px; margin-bottom: 8px; }
  .health-log-title { color: #67e8f9; font-size: 12px; font-weight: 800; }
  .loading-actions { position: relative; z-index: 1; display: flex; justify-content: flex-end; margin-top: 12px; }
  .show-log-btn, .modal-log-toggle { min-height: 30px; padding: 0 11px; border-radius: 8px; border: 1px solid rgba(34,211,238,.24); background: rgba(34,211,238,.08); color: #67e8f9; font-size: 12px; font-weight: 800; cursor: pointer; }
  .show-log-btn:hover, .modal-log-toggle:hover { background: rgba(34,211,238,.14); border-color: rgba(34,211,238,.38); }
  .panel-actions { display: flex; align-items: center; gap: 6px; }
  .panel-close { padding: 4px 8px; border-radius: 7px; border: 1px solid rgba(34,211,238,.18); background: transparent; color: #67e8f9; font-size: 11px; cursor: pointer; }
  .panel-close:hover { background: rgba(34,211,238,.08); }
  .panel-close.muted { color: var(--text-secondary); border-color: var(--border-primary); }
  .step-rail { position: relative; z-index: 1; display: grid; grid-template-columns: repeat(5, minmax(0, 1fr)); gap: 8px; margin-top: 16px; }
  .step-dot { min-width: 0; padding: 8px; border-radius: 8px; background: rgba(15, 23, 42, 0.55); border: 1px solid rgba(148, 163, 184, 0.16); color: var(--text-tertiary); transition: all .25s ease; }
  .step-dot.active { color: #67e8f9; border-color: rgba(34,211,238,.38); box-shadow: inset 0 0 18px rgba(34,211,238,.08); }
  .step-dot span { display: inline-grid; place-items: center; width: 18px; height: 18px; margin-right: 6px; border-radius: 50%; background: rgba(148, 163, 184, .14); font-family: var(--theme-font-family-mono); font-size: 10px; }
  .step-dot.active span { background: rgba(34,211,238,.18); color: #a5f3fc; }
  .step-dot em { font-style: normal; font-size: 11px; font-weight: 700; }
  @keyframes orbSpin { 0%,100% { filter: drop-shadow(0 0 0 rgba(34,211,238,0)); } 50% { filter: drop-shadow(0 0 14px rgba(34,211,238,.32)); } }
  @keyframes shimmer { 0% { background-position: -200% 0; } 100% { background-position: 200% 0; } }
  @keyframes stepPulse { 0%, 100% { opacity: 0.7; } 50% { opacity: 1; } }
  @keyframes scanSweep { to { transform: translateX(100%); } }
  @keyframes gridDrift { to { background-position: 28px 28px; } }

  .health-result-panel { background: var(--bg-card); border: 1px solid var(--border-primary); border-radius: 16px; margin-top: 16px; overflow: hidden; }
  .result-header { display: flex; align-items: center; justify-content: space-between; padding: 20px 24px; border-bottom: 1px solid var(--border-primary); }
  .result-status { display: flex; align-items: center; gap: 10px; font-size: 20px; font-weight: 700; }
  .result-icon { font-size: 24px; }
  .result-actions { display: flex; align-items: center; gap: 8px; }
  .result-close-all { height: 30px; padding: 0 10px; border-radius: 8px; border: 1px solid var(--border-primary); background: var(--bg-secondary); color: var(--text-secondary); font-size: 12px; font-weight: 700; cursor: pointer; }
  .result-close-all:hover { border-color: var(--border-focus); color: var(--accent-primary); background: var(--accent-primary-light); }
  .result-stats { display: flex; gap: 20px; }
  .stat-item { text-align: center; }
  .stat-value { display: block; font-size: 24px; font-weight: 700; color: var(--text-primary); font-family: var(--theme-font-family-mono); }
  .stat-label { font-size: 12px; color: var(--text-secondary); }
  .stat-warn .stat-value { color: #f59e0b; }
  .stat-error .stat-value { color: #ef4444; }
  .result-close { background: none; border: none; color: var(--text-tertiary); font-size: 20px; cursor: pointer; padding: 4px 8px; border-radius: 6px; }
  .result-close:hover { background: var(--bg-hover); }

  .result-checks { padding: 12px; }
  .result-check-item { display: flex; width: 100%; background: var(--bg-secondary); border: 1px solid var(--border-primary); border-radius: 10px; margin-bottom: 8px; overflow: hidden; text-decoration: none; color: inherit; transition: all 0.15s; }
  .result-check-item:hover { border-color: var(--accent-primary); transform: translateX(4px); }
  .result-check-item.check-ok { border-left: 3px solid #10b981; }
  .result-check-item.check-warn { border-left: 3px solid #f59e0b; }
  .result-check-item.check-error { border-left: 3px solid #ef4444; }
  .check-main { display: flex; align-items: center; gap: 10px; padding: 12px 14px; width: 100%; }
  .check-status-dot { width: 10px; height: 10px; border-radius: 50%; flex-shrink: 0; }
  .check-name { flex: 1; font-size: 14px; font-weight: 600; color: var(--text-primary); }
  .check-findings { flex-shrink: 0; padding: 3px 7px; border-radius: 999px; color: #f59e0b; background: rgba(245, 158, 11, 0.08); border: 1px solid rgba(245, 158, 11, 0.16); font-size: 11px; font-family: var(--theme-font-family-mono); }
  .check-duration { font-size: 12px; color: var(--text-tertiary); font-family: var(--theme-font-family-mono); }
  .check-arrow { color: var(--text-tertiary); font-size: 14px; transition: all 0.2s; }
  .result-check-item:hover .check-arrow { color: var(--accent-primary); transform: translateX(4px); }
  .result-alerts { padding: 0 12px 12px; }
  .result-alerts-title { margin: 4px 0 8px; color: var(--text-primary); font-size: 13px; font-weight: 800; }
  .result-alert-item { display: grid; grid-template-columns: 70px minmax(160px, 1fr) minmax(90px, 160px) minmax(160px, 1.2fr); gap: 10px; align-items: center; margin-bottom: 6px; padding: 10px 12px; border: 1px solid var(--border-primary); border-radius: 9px; background: var(--bg-secondary); color: inherit; text-decoration: none; transition: all .16s ease; }
  .result-alert-item:hover { border-color: rgba(245, 158, 11, .45); transform: translateX(3px); }
  .alert-level { width: fit-content; min-width: 46px; padding: 3px 7px; border-radius: 7px; background: rgba(245, 158, 11, .12); color: #fbbf24; font-family: var(--theme-font-family-mono); font-size: 10px; font-weight: 900; text-align: center; text-transform: uppercase; }
  .alert-level.error { background: rgba(239, 68, 68, .13); color: #f87171; }
  .alert-title { min-width: 0; color: var(--text-primary); font-size: 12px; font-weight: 700; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .alert-target, .alert-handling { min-width: 0; color: var(--text-secondary); font-size: 11px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .more-alerts { display: inline-flex; margin-top: 6px; color: var(--accent-primary); font-size: 12px; font-weight: 700; text-decoration: none; }

  .health-error { display: flex; align-items: center; gap: 10px; padding: 14px 18px; background: rgba(239, 68, 68, 0.1); border: 1px solid rgba(239, 68, 68, 0.2); border-radius: 12px; margin-top: 12px; color: #ef4444; font-size: 14px; }
  .error-icon { font-size: 18px; }
  .error-close { background: none; border: none; color: #ef4444; cursor: pointer; margin-left: auto; font-size: 16px; }

  .health-modal-overlay { position: fixed; inset: 0; z-index: 120; display: flex; align-items: center; justify-content: center; padding: 24px; background: rgba(2, 6, 23, .72); backdrop-filter: blur(8px); }
  .health-modal { width: min(980px, 96vw); max-height: min(860px, 92vh); display: flex; flex-direction: column; overflow: hidden; border-radius: 18px; border: 1px solid rgba(34, 211, 238, .24); background: linear-gradient(180deg, rgba(8, 13, 24, .98), rgba(2, 6, 23, .98)); box-shadow: 0 30px 90px rgba(0,0,0,.45), 0 0 60px rgba(34,211,238,.12); color: #e2e8f0; }
  .health-modal-head { display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 18px 20px; border-bottom: 1px solid rgba(148, 163, 184, .16); background: linear-gradient(90deg, rgba(34,211,238,.1), rgba(52,211,153,.05), transparent); }
  .health-modal-head h2 { margin: 0; color: #f8fafc; font-size: 18px; font-weight: 800; }
  .health-modal-head p { margin: 4px 0 0; color: #94a3b8; font-size: 12px; }
  .health-modal-actions { display: flex; align-items: center; gap: 10px; }
  .modal-running { min-width: 54px; padding: 5px 9px; border-radius: 999px; border: 1px solid rgba(34,211,238,.28); background: rgba(34,211,238,.1); color: #67e8f9; font-family: var(--theme-font-family-mono); font-size: 12px; font-weight: 900; text-align: center; }
  .modal-icon-btn { width: 34px; height: 34px; border-radius: 9px; border: 1px solid rgba(148, 163, 184, .18); background: rgba(15, 23, 42, .7); color: #cbd5e1; cursor: pointer; }
  .modal-icon-btn:hover { color: #fff; border-color: rgba(34,211,238,.38); background: rgba(34,211,238,.12); }
  .health-modal-body { min-height: 0; overflow-y: auto; padding: 16px; }
  .modal-loading { margin-top: 0; }
  .modal-log-panel { max-height: 260px; }
  .modal-result-panel { margin-top: 12px; background: rgba(15, 23, 42, .72); border-color: rgba(148, 163, 184, .16); }
  .health-modal .result-header { border-color: rgba(148, 163, 184, .16); }
  .health-modal .result-check-item,
  .health-modal .result-alert-item { background: rgba(15, 23, 42, .68); border-color: rgba(148, 163, 184, .16); }
  .health-modal .stat-value,
  .health-modal .check-name,
  .health-modal .alert-title { color: #f8fafc; }
  .health-modal .stat-label,
  .health-modal .check-duration,
  .health-modal .alert-target,
  .health-modal .alert-handling { color: #94a3b8; }

  .category-chips { display: flex; flex-wrap: wrap; gap: 8px; margin-bottom: 20px; }
  .chip { display: flex; align-items: center; gap: 6px; padding: 8px 16px; border-radius: 20px; border: 1px solid var(--border-primary); background: var(--bg-card); color: var(--text-secondary); font-size: 13px; cursor: pointer; transition: all 0.2s; }
  .chip:hover { background: var(--bg-hover); color: var(--text-primary); }
  .chip.active { background: var(--accent-primary-light); border-color: var(--accent-primary); color: var(--accent-primary); }
  .chip-count { font-size: 11px; color: var(--text-tertiary); background: var(--bg-tertiary); padding: 2px 6px; border-radius: 6px; }
  .chip.active .chip-count { background: var(--accent-primary-light); color: var(--accent-primary); }

  .category-section { margin-bottom: 24px; }
  .category-title { font-size: 16px; font-weight: 700; color: var(--text-primary); margin: 0 0 14px; }
  .checks-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); gap: 12px; }
  .checks-list { display: flex; flex-direction: column; gap: 6px; }
  .check-row { display: flex; align-items: center; gap: 12px; padding: 14px 16px; background: var(--bg-card); border: 1px solid var(--border-primary); border-radius: 10px; text-decoration: none; color: inherit; transition: all 0.2s; }
  .check-row:hover { transform: translateX(4px); border-color: var(--accent-primary); box-shadow: var(--shadow-md); }
  .check-number, .card-number { display: inline-grid; place-items: center; border-radius: 7px; border: 1px solid rgba(34,211,238,.24); background: rgba(34,211,238,.08); color: var(--accent-primary); font-family: var(--theme-font-family-mono); font-size: 11px; font-weight: 900; letter-spacing: 0; flex-shrink: 0; }
  .check-number { min-width: 42px; height: 26px; }
  .check-row .check-icon { display: inline-grid; place-items: center; width: 34px; height: 34px; border-radius: 9px; color: #67e8f9; background: rgba(34,211,238,.1); border: 1px solid rgba(34,211,238,.18); flex-shrink: 0; }
  .check-row .check-icon svg { width: 18px; height: 18px; fill: none; stroke: currentColor; stroke-width: 1.8; stroke-linecap: round; stroke-linejoin: round; }
  .check-row .check-name { font-size: 14px; font-weight: 600; color: var(--text-primary); min-width: 120px; }
  .check-row .check-desc { font-size: 12px; color: var(--text-secondary); flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .check-row .check-id { font-size: 10px; color: var(--text-tertiary); font-family: var(--theme-font-family-mono); }
  .check-row .check-arrow { color: var(--text-tertiary); font-size: 14px; transition: all 0.2s; }
  .check-row:hover .check-arrow { color: var(--accent-primary); transform: translateX(4px); }

  .view-toggle { display: flex; gap: 2px; background: var(--bg-secondary); border-radius: 8px; padding: 3px; }
  .sort-chip { min-height: 32px; padding: 0 10px; border: none; border-radius: 6px; background: transparent; color: var(--text-secondary); font-size: 12px; cursor: pointer; }
  .sort-chip:hover { color: var(--text-primary); background: var(--bg-hover); }
  .view-btn { display: flex; align-items: center; justify-content: center; width: 32px; height: 32px; border: none; background: transparent; color: var(--text-tertiary); border-radius: 6px; cursor: pointer; transition: all 0.15s; }
  .view-btn:hover { color: var(--text-primary); }
  .view-btn.active { background: var(--bg-card); color: var(--accent-primary); box-shadow: var(--shadow-sm); }
  .check-card { position: relative; overflow: hidden; display: flex; align-items: center; gap: 14px; padding: 16px; background: linear-gradient(135deg, rgba(15, 23, 42, .94), rgba(8, 13, 24, .98)); border: 1px solid rgba(148, 163, 184, .14); border-radius: 12px; text-decoration: none; color: inherit; transition: transform 0.2s, border-color 0.2s, box-shadow 0.2s; }
  .card-number { position: absolute; top: 10px; right: 10px; min-width: 42px; height: 24px; z-index: 2; }
  .check-card::before { content: ''; position: absolute; inset: 0; background: radial-gradient(circle at 8% 0%, rgba(34,211,238,.1), transparent 34%), linear-gradient(90deg, rgba(255,255,255,.03), transparent 45%); opacity: .75; pointer-events: none; }
  .check-card:hover { transform: translateY(-2px); box-shadow: 0 16px 40px rgba(0,0,0,.22), 0 0 28px rgba(34,211,238,.08); border-color: rgba(34,211,238,.34); }
  .card-icon { position: relative; z-index: 1; display: grid; place-items: center; width: 56px; height: 56px; border-radius: 14px; background: radial-gradient(circle at 35% 20%, rgba(255,255,255,.14), transparent 38%), rgba(34,211,238,0.1); border: 1px solid rgba(34,211,238,0.18); color: #67e8f9; flex-shrink: 0; box-shadow: inset 0 0 26px rgba(34,211,238,.08), 0 10px 28px rgba(2,6,23,.18); }
  .card-icon::after { content: ''; position: absolute; inset: 8px; border-radius: 10px; border: 1px solid currentColor; opacity: .12; }
  .card-icon svg { width: 29px; height: 29px; fill: none; stroke: currentColor; stroke-width: 1.75; stroke-linecap: round; stroke-linejoin: round; filter: drop-shadow(0 0 10px currentColor); }
  .card-icon.security, .check-icon.security { color: #fbbf24; background: rgba(245,158,11,.1); border-color: rgba(245,158,11,.2); }
  .card-icon.data, .check-icon.data { color: #a7f3d0; background: rgba(52,211,153,.1); border-color: rgba(52,211,153,.2); }
  .card-icon.network, .check-icon.network { color: #93c5fd; background: rgba(59,130,246,.1); border-color: rgba(59,130,246,.2); }
  .card-icon.service, .check-icon.service { color: #c4b5fd; background: rgba(167,139,250,.1); border-color: rgba(167,139,250,.2); }
  .card-icon.resource, .check-icon.resource { color: #67e8f9; background: rgba(34,211,238,.1); border-color: rgba(34,211,238,.2); }
  .card-info { flex: 1; min-width: 0; }
  .card-title { font-size: 15px; font-weight: 600; color: var(--text-primary); margin: 0 0 4px; }
  .card-desc { font-size: 12px; color: var(--text-secondary); margin: 0 0 6px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .card-id { font-size: 10px; color: var(--text-tertiary); font-family: var(--theme-font-family-mono); }
  .card-arrow { color: var(--text-tertiary); font-size: 16px; transition: all 0.2s; }
  .check-card:hover .card-arrow { color: var(--accent-primary); transform: translateX(4px); }

  .loading { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 14px; padding: 60px 0; color: var(--text-secondary); font-size: 14px; }
  .checks-loader { display: grid; grid-template-columns: repeat(3, 34px); gap: 8px; }
  .checks-loader i { display: block; width: 34px; height: 28px; border-radius: 8px; background: linear-gradient(135deg, rgba(34,211,238,0.24), rgba(52,211,153,0.14)); border: 1px solid rgba(34,211,238,0.18); animation: tileLoad 1.1s ease-in-out infinite; }
  .checks-loader i:nth-child(2) { animation-delay: .08s; }
  .checks-loader i:nth-child(3) { animation-delay: .16s; }
  .checks-loader i:nth-child(4) { animation-delay: .24s; }
  .checks-loader i:nth-child(5) { animation-delay: .32s; }
  .checks-loader i:nth-child(6) { animation-delay: .40s; }
  .loading-text { color: var(--text-secondary); font-size: 12px; }
  @keyframes tileLoad { 0%,100% { opacity: .42; transform: translateY(0); } 50% { opacity: 1; transform: translateY(-4px); } }
  @keyframes spin { to { transform: rotate(360deg); } }

  .empty-state { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px; padding: 80px 0; color: var(--text-secondary); }
  .empty-icon { font-size: 48px; opacity: 0.5; }
  .empty-text { font-size: 15px; }

  @media (max-width: 768px) {
    .page-header { flex-direction: column; align-items: stretch; }
    .header-right { flex-wrap: wrap; }
    .search-input { width: 100%; }
    .health-check-card { flex-direction: column; gap: 16px; text-align: center; }
    .health-check-left { flex-direction: column; }
    .result-header { flex-direction: column; gap: 16px; }
    .checks-grid { grid-template-columns: 1fr; }
  }
</style>
