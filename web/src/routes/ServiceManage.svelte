<script>
  import { onMount } from 'svelte';
  
  let services = $state([]);
  let loading = $state(true);
  let error = $state(null);
  let summary = $state(null);
  let filterCategory = $state('');
  let filterStatus = $state('');
  let search = $state('');
  let sortKey = $state('name');
  let sortDir = $state('asc');
  let actionLoading = $state({});
  let actionResult = $state({});
  let showLogs = $state(false);
  let logService = $state('');
  let logContent = $state('');
  let logLoading = $state(false);
  let showHealth = $state(false);
  let healthService = $state('');
  let healthResult = $state(null);
  let healthLoading = $state(false);
  let showFilters = $state(false);

  async function load() {
    loading = true;
    error = null;
    try {
      const r = await fetch('/api/checks/service-manage');
      if (!r.ok) { error = '加载失败'; loading = false; return; }
      const d = await r.json();
      const tableSection = d.sections?.find(s => s.title === '服务列表');
      const summarySection = d.sections?.find(s => s.title === '服务汇总');
      if (summarySection?.items) {
        summary = {};
        for (const item of summarySection.items) {
          if (item.type === 'label') summary[item.key] = item.value;
        }
      }
      if (tableSection?.items) {
        const table = tableSection.items.find(i => i.type === 'table');
        if (table) {
          services = table.rows.map((r, i) => ({
            index: r[0],
            pid: r[1],
            name: r[2],
            process: r[3],
            path: r[4],
            ports: r[5],
            status: r[6],
            cpu: r[7],
            memory: r[8],
            category: r[9],
          }));
        }
      }
    } catch (e) { error = e.message; }
    loading = false;
  }

  async function serviceAction(name, action) {
    const key = `${name}-${action}`;
    actionLoading[key] = true;
    actionResult[key] = null;
    try {
      const r = await fetch(`/api/services/${encodeURIComponent(name)}/action`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ action }),
      });
      const d = await r.json();
      actionResult[key] = { success: d.success, message: d.success ? `${action} 成功` : d.stderr || `${action} 失败` };
      if (action !== 'status') setTimeout(load, 1000);
    } catch (e) {
      actionResult[key] = { success: false, message: e.message };
    }
    actionLoading[key] = false;
    setTimeout(() => { actionResult[key] = null; }, 3000);
  }

  function statusClass(s) {
    if (s?.includes('运行中')) return 'status-running';
    return 'status-stopped';
  }

  function changeSort(key) {
    if (sortKey === key) sortDir = sortDir === 'asc' ? 'desc' : 'asc';
    else {
      sortKey = key;
      sortDir = key === 'cpu' || key === 'memory' ? 'desc' : 'asc';
    }
  }

  function sortMark(key) {
    if (sortKey !== key) return '';
    return sortDir === 'asc' ? ' ↑' : ' ↓';
  }

  function parsePercent(v) {
    const n = parseFloat(String(v || '').replace('%', ''));
    return Number.isFinite(n) ? n : 0;
  }

  function parseMemory(v) {
    const raw = String(v || '').trim();
    const n = parseFloat(raw);
    if (!Number.isFinite(n)) return 0;
    if (raw.includes('GB')) return n * 1024;
    if (raw.includes('KB')) return n / 1024;
    return n;
  }

  function matchSearchToken(s, token) {
    const t = token.toLowerCase();
    if (!t) return true;
    const [rawKey, ...rest] = t.split(':');
    const value = rest.join(':');
    if (value) {
      if (rawKey === 'pid') return String(s.pid || '').includes(value);
      if (rawKey === 'port' || rawKey === 'ports') return String(s.ports || '').toLowerCase().includes(value);
      if (rawKey === 'name' || rawKey === 'service') return String(s.name || '').toLowerCase().includes(value);
      if (rawKey === 'path' || rawKey === 'cmd') return String(s.path || '').toLowerCase().includes(value);
      if (rawKey === 'cat' || rawKey === 'category' || rawKey === 'type') return String(s.category || '').toLowerCase().includes(value);
      if (rawKey === 'status') {
        const running = s.status?.includes('运行中');
        if (['running', 'run', 'up', 'active', '运行'].includes(value)) return running;
        if (['stopped', 'stop', 'down', 'inactive', '停止'].includes(value)) return !running;
        return String(s.status || '').toLowerCase().includes(value);
      }
    }
    const cmp = t.match(/^(cpu|mem|memory)(>=|<=|>|<|=)(\d+(?:\.\d+)?)$/);
    if (cmp) {
      const actual = cmp[1] === 'cpu' ? parsePercent(s.cpu) : parseMemory(s.memory);
      const expected = Number(cmp[3]);
      if (cmp[2] === '>') return actual > expected;
      if (cmp[2] === '>=') return actual >= expected;
      if (cmp[2] === '<') return actual < expected;
      if (cmp[2] === '<=') return actual <= expected;
      return actual === expected;
    }
    return [
      s.name,
      s.process,
      s.pid,
      s.path,
      s.ports,
      s.status,
      s.cpu,
      s.memory,
      s.category,
    ].some(v => String(v || '').toLowerCase().includes(t));
  }

  function serviceSortValue(s, key) {
    if (key === 'pid') return Number(s.pid) || 0;
    if (key === 'status') return s.status?.includes('运行中') ? 0 : 1;
    if (key === 'cpu') return parsePercent(s.cpu);
    if (key === 'memory') return parseMemory(s.memory);
    return s[key] || '';
  }

  async function viewLogs(service) {
    const name = typeof service === 'string' ? service : service?.name;
    if (!name) return;
    logService = name;
    showLogs = true;
    logLoading = true;
    logContent = '';
    try {
      const params = new URLSearchParams();
      if (service?.pid) params.set('pid', service.pid);
      if (service?.process) params.set('process', service.process);
      if (service?.path) params.set('path', service.path);
      if (service?.category) params.set('category', service.category);
      const qs = params.toString();
      const r = await fetch(`/api/services/${encodeURIComponent(name)}/logs${qs ? '?' + qs : ''}`);
      if (r.ok) {
        const d = await r.json();
        logContent = d.logs || '暂无日志';
      } else {
        logContent = '获取日志失败';
      }
    } catch (e) { logContent = '错误: ' + e.message; }
    logLoading = false;
  }

  async function checkHealth(name) {
    healthService = name;
    showHealth = true;
    healthLoading = true;
    healthResult = null;
    try {
      const r = await fetch(`/api/services/${encodeURIComponent(name)}/health`);
      if (r.ok) {
        healthResult = await r.json();
      }
    } catch (e) { healthResult = { status: 'error', message: e.message }; }
    healthLoading = false;
  }

  let filtered = $derived.by(() => {
    let result = services;
    if (filterCategory) result = result.filter(s => s.category === filterCategory);
    if (filterStatus === 'running') result = result.filter(s => s.status?.includes('运行中'));
    if (filterStatus === 'stopped') result = result.filter(s => !s.status?.includes('运行中'));
    if (search.trim()) {
      const tokens = search.trim().split(/\s+/);
      result = result.filter(s => tokens.every(token => matchSearchToken(s, token)));
    }
    return [...result].sort((a, b) => {
      const av = serviceSortValue(a, sortKey);
      const bv = serviceSortValue(b, sortKey);
      let cmp = typeof av === 'number' && typeof bv === 'number'
        ? av - bv
        : String(av).localeCompare(String(bv), 'zh-CN');
      if (sortDir === 'desc') cmp = -cmp;
      return cmp;
    });
  });

  let categories = $derived.by(() => {
    const cats = new Set();
    for (const s of services) cats.add(s.category);
    return [...cats].sort();
  });

  let runningCount = $derived(services.filter(s => s.status?.includes('运行中')).length);
  let stoppedCount = $derived(services.length - runningCount);

  onMount(load);
</script>

<div class="service-page">
  <div class="page-header">
    <div class="header-left">
      <span class="service-count">{services.length} 个服务</span>
    </div>
    <div class="header-right">
      <div class="search-wrap">
        <svg class="search-icon" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.35-4.35" stroke-linecap="round"/></svg>
        <input type="text" placeholder="name/nginx pid:123 port:8080 cpu>10 mem>500 status:running" bind:value={search} class="search-input" />
        {#if search}
          <button class="search-clear" onclick={() => search = ''}>✕</button>
        {/if}
      </div>
      <button class="action-btn" onclick={load}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 12a9 9 0 0 0-9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/><path d="M3 12a9 9 0 0 0 9 9 9.75 9.75 0 0 0 6.74-2.74L21 16"/><path d="M16 16h5v5"/></svg>
        <span>刷新</span>
      </button>
      <button class="action-btn" onclick={() => showFilters = !showFilters}>
        <span>筛选{filterCategory || filterStatus || search ? ' *' : ''}</span>
        <span>{showFilters ? '↑' : '↓'}</span>
      </button>
    </div>
  </div>

  {#if showFilters}
  <div class="filter-panel">
    <div class="filter-group quick-query">
      <span class="filter-label">快捷</span>
      <button class="filter-pill" onclick={() => search = 'cpu>10'}>CPU&gt;10%</button>
      <button class="filter-pill" onclick={() => search = 'mem>500'}>内存&gt;500MB</button>
      <button class="filter-pill" onclick={() => search = 'port:'}>端口</button>
      <button class="filter-pill" onclick={() => search = 'status:running'}>运行服务</button>
    </div>
    <div class="filter-group">
      <span class="filter-label">状态</span>
      <button class="filter-pill" class:active={!filterStatus} onclick={() => filterStatus = ''}>
        <span>全部</span>
        <span class="pill-count">{services.length}</span>
      </button>
      <button class="filter-pill filter-running" class:active={filterStatus === 'running'} onclick={() => filterStatus = filterStatus === 'running' ? '' : 'running'}>
        <span>运行中</span>
        <span class="pill-count">{runningCount}</span>
      </button>
      <button class="filter-pill filter-stopped" class:active={filterStatus === 'stopped'} onclick={() => filterStatus = filterStatus === 'stopped' ? '' : 'stopped'}>
        <span>未运行</span>
        <span class="pill-count">{stoppedCount}</span>
      </button>
    </div>
    <div class="filter-group">
      <span class="filter-label">类型</span>
      <button class="filter-pill" class:active={!filterCategory} onclick={() => filterCategory = ''}>
        <span>全部</span>
        <span class="pill-count">{services.length}</span>
      </button>
      {#each categories as cat}
        <button class="filter-pill" class:active={filterCategory === cat} onclick={() => filterCategory = filterCategory === cat ? '' : cat}>
          <span>{cat}</span>
          <span class="pill-count">{services.filter(s => s.category === cat).length}</span>
        </button>
      {/each}
    </div>
    {#if summary}
      <div class="filter-summary">
        <span>总服务 {summary['总服务数'] || services.length}</span>
        <span>运行 {summary['运行中'] || runningCount}</span>
        <span>匹配 {filtered.length}</span>
      </div>
    {/if}
  </div>
  {/if}

  {#if loading}
    <div class="loading"><div class="spinner"></div><span>加载中...</span></div>
  {:else if filtered.length === 0}
    <div class="empty-state">
      <span class="empty-icon">🔧</span>
      <span class="empty-text">没有匹配的服务</span>
    </div>
  {:else}
    <div class="table-wrap">
      <table class="service-table">
        <colgroup>
          <col class="col-index" />
          <col class="col-pid" />
          <col class="col-name" />
          <col class="col-process" />
          <col class="col-path" />
          <col class="col-ports" />
          <col class="col-status" />
          <col class="col-cpu" />
          <col class="col-memory" />
          <col class="col-category" />
          <col class="col-actions" />
        </colgroup>
        <thead>
          <tr>
            <th>#</th>
            <th><button class="th-sort" onclick={() => changeSort('pid')}>PID{sortMark('pid')}</button></th>
            <th><button class="th-sort" onclick={() => changeSort('name')}>服务名{sortMark('name')}</button></th>
            <th><button class="th-sort" onclick={() => changeSort('process')}>进程{sortMark('process')}</button></th>
            <th><button class="th-sort" onclick={() => changeSort('path')}>进程路径{sortMark('path')}</button></th>
            <th><button class="th-sort" onclick={() => changeSort('ports')}>端口{sortMark('ports')}</button></th>
            <th><button class="th-sort" onclick={() => changeSort('status')}>状态{sortMark('status')}</button></th>
            <th><button class="th-sort" onclick={() => changeSort('cpu')}>CPU{sortMark('cpu')}</button></th>
            <th><button class="th-sort" onclick={() => changeSort('memory')}>内存{sortMark('memory')}</button></th>
            <th><button class="th-sort" onclick={() => changeSort('category')}>类型{sortMark('category')}</button></th>
            <th>操作</th>
          </tr>
        </thead>
        <tbody>
          {#each filtered as s, i}
            <tr class:row-running={s.status?.includes('运行中')}>
              <td class="cell-index">{s.index}</td>
              <td class="cell-pid">{s.pid}</td>
              <td class="cell-name">{s.name}</td>
              <td class="cell-process" title={s.process}>{s.process || '-'}</td>
              <td class="cell-path" title={s.path}>{s.path}</td>
              <td class="cell-ports">{s.ports}</td>
              <td class="cell-status">
                <span class="status-dot" class:dot-running={s.status?.includes('运行中')}></span>
                {s.status}
              </td>
              <td class="cell-cpu">{s.cpu}</td>
              <td class="cell-mem">{s.memory}</td>
              <td class="cell-cat"><span class="cat-badge">{s.category}</span></td>
              <td class="cell-actions">
                <div class="action-group">
                  {#if s.status?.includes('运行中')}
                    <button class="act-btn act-restart" onclick={() => serviceAction(s.name, 'restart')} disabled={actionLoading[`${s.name}-restart`]}>
                      {actionLoading[`${s.name}-restart`] ? '...' : '重启'}
                    </button>
                    <button class="act-btn act-stop" onclick={() => serviceAction(s.name, 'stop')} disabled={actionLoading[`${s.name}-stop`]}>
                      {actionLoading[`${s.name}-stop`] ? '...' : '停止'}
                    </button>
                  {:else}
                    <button class="act-btn act-start" onclick={() => serviceAction(s.name, 'start')} disabled={actionLoading[`${s.name}-start`]}>
                      {actionLoading[`${s.name}-start`] ? '...' : '启动'}
                    </button>
                  {/if}
                  <button class="act-btn act-log" onclick={() => viewLogs(s)}>日志</button>
                  <button class="act-btn act-health" onclick={() => checkHealth(s.name)}>健康</button>
                </div>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    <div class="service-card-list">
      {#each filtered as s}
        <div class="service-card" class:card-running={s.status?.includes('运行中')}>
          <div class="service-card-head">
            <div class="service-card-main">
              <span class="status-dot" class:dot-running={s.status?.includes('运行中')}></span>
              <div>
                <div class="service-card-name">{s.name}</div>
                <div class="service-card-sub">PID {s.pid || '-'} · {s.process || '-'} · {s.category || '-'}</div>
              </div>
            </div>
            <span class="service-card-status">{s.status || '-'}</span>
          </div>
          <div class="service-card-grid">
            <div><span>端口</span><strong>{s.ports || '-'}</strong></div>
            <div><span>CPU</span><strong>{s.cpu || '-'}</strong></div>
            <div><span>内存</span><strong>{s.memory || '-'}</strong></div>
          </div>
          <div class="service-card-path" title={s.path}>{s.path || '-'}</div>
          <div class="service-card-actions">
            {#if s.status?.includes('运行中')}
              <button class="act-btn act-restart" onclick={() => serviceAction(s.name, 'restart')} disabled={actionLoading[`${s.name}-restart`]}>
                {actionLoading[`${s.name}-restart`] ? '...' : '重启'}
              </button>
              <button class="act-btn act-stop" onclick={() => serviceAction(s.name, 'stop')} disabled={actionLoading[`${s.name}-stop`]}>
                {actionLoading[`${s.name}-stop`] ? '...' : '停止'}
              </button>
            {:else}
              <button class="act-btn act-start" onclick={() => serviceAction(s.name, 'start')} disabled={actionLoading[`${s.name}-start`]}>
                {actionLoading[`${s.name}-start`] ? '...' : '启动'}
              </button>
            {/if}
            <button class="act-btn act-log" onclick={() => viewLogs(s)}>日志</button>
            <button class="act-btn act-health" onclick={() => checkHealth(s.name)}>健康</button>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

{#if showLogs}
  <div class="modal-overlay" onclick={() => showLogs = false} role="presentation">
    <div class="modal log-modal" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="dialog" aria-modal="true" tabindex="-1">
      <div class="modal-header">
        <h3>服务日志 - {logService}</h3>
        <button class="modal-close" onclick={() => showLogs = false}>✕</button>
      </div>
      <div class="modal-body">
        {#if logLoading}
          <div class="modal-loading"><div class="modal-spinner"></div><span>正在读取服务日志...</span></div>
        {:else}
          <pre class="log-content">{logContent}</pre>
        {/if}
      </div>
    </div>
  </div>
{/if}

{#if showHealth}
  <div class="modal-overlay" onclick={() => showHealth = false} role="presentation">
    <div class="modal modal-large" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="dialog" aria-modal="true" tabindex="-1">
      <div class="modal-header">
        <h3>健康检查 - {healthService}</h3>
        <button class="modal-close" onclick={() => showHealth = false}>✕</button>
      </div>
      <div class="modal-body">
        {#if healthLoading}
          <div class="modal-loading"><div class="modal-spinner"></div><span>正在检查服务状态...</span></div>
        {:else if healthResult}
          <div class="health-result">
            <div class="health-status-card" style="border-color:{healthResult.status === 'ok' ? '#10b981' : '#ef4444'}">
              <div class="health-status-icon" style="color:{healthResult.status === 'ok' ? '#10b981' : '#ef4444'}">
                {healthResult.status === 'ok' ? '✓' : '✗'}
              </div>
              <div class="health-status-info">
                <div class="health-status-text" style="color:{healthResult.status === 'ok' ? '#10b981' : '#ef4444'}">
                  {healthResult.status === 'ok' ? '健康' : '异常'}
                </div>
                <div class="health-message">{healthResult.message || ''}</div>
              </div>
            </div>

            <div class="health-details">
              <div class="health-detail-section">
                <h4>服务状态</h4>
                <div class="detail-rows">
                  <div class="detail-row">
                    <span class="detail-label">Systemd状态</span>
                    <span class="detail-value">{healthResult.systemd_status || '-'}</span>
                  </div>
                  <div class="detail-row">
                    <span class="detail-label">是否激活</span>
                    <span class="detail-value" style="color:{healthResult.is_active ? '#10b981' : '#ef4444'}">{healthResult.is_active ? '是' : '否'}</span>
                  </div>
                  <div class="detail-row">
                    <span class="detail-label">是否运行</span>
                    <span class="detail-value" style="color:{healthResult.is_running ? '#10b981' : '#ef4444'}">{healthResult.is_running ? '是' : '否'}</span>
                  </div>
                </div>
              </div>

              {#if healthResult.pids && healthResult.pids.length > 0}
                <div class="health-detail-section">
                  <h4>进程信息</h4>
                  <div class="detail-rows">
                    <div class="detail-row">
                      <span class="detail-label">进程数</span>
                      <span class="detail-value">{healthResult.process_count || 0}</span>
                    </div>
                    <div class="detail-row">
                      <span class="detail-label">PID列表</span>
                      <span class="detail-value detail-pids">{healthResult.pids.join(', ')}</span>
                    </div>
                  </div>
                </div>
              {/if}

              {#if healthResult.processes && healthResult.processes.length > 0}
                <div class="health-detail-section">
                  <h4>进程详情</h4>
                  <div class="health-process-table">
                    <div class="health-process-head">
                      <span>PID</span>
                      <span>PPID</span>
                      <span>CPU</span>
                      <span>内存</span>
                      <span>运行时长</span>
                      <span>状态</span>
                      <span>命令</span>
                    </div>
                    {#each healthResult.processes as p}
                      <div class="health-process-row">
                        <span>{p.pid || '-'}</span>
                        <span>{p.ppid || '-'}</span>
                        <span class="cpu-val">{p.cpu || '0.0'}%</span>
                        <span>{p.memory || '0.0'}%</span>
                        <span>{p.etime || '-'}</span>
                        <span>{p.stat || '-'}</span>
                        <span class="cmd-val" title={p.args || p.command}>{p.args || p.command || '-'}</span>
                      </div>
                    {/each}
                  </div>
                </div>
              {/if}

              {#if healthResult.listening_ports && healthResult.listening_ports.length > 0}
                <div class="health-detail-section">
                  <h4>监听端口</h4>
                  <div class="ports-list">
                    {#each healthResult.listening_ports as port}
                      <div class="port-item">{port}</div>
                    {/each}
                  </div>
                </div>
              {/if}
            </div>
          </div>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .service-page { width: 100%; max-width: none; margin: 0; }
  .page-header { display: flex; align-items: center; justify-content: space-between; gap: 16px; margin-bottom: 16px; }
  .header-left { display: flex; align-items: baseline; gap: 10px; }
  .service-count { font-size: 14px; color: var(--text-secondary); }
  .header-right { display: flex; align-items: center; gap: 10px; }
  .search-wrap { position: relative; }
  .search-icon { position: absolute; left: 12px; top: 50%; transform: translateY(-50%); color: var(--text-tertiary); }
  .search-input { width: 240px; padding: 10px 14px 10px 36px; background: var(--bg-input); border: 1px solid var(--border-primary); border-radius: 10px; font-size: 14px; color: var(--text-primary); outline: none; box-sizing: border-box; }
  .search-input::placeholder { color: var(--text-tertiary); }
  .search-input:focus { border-color: var(--border-focus); }
  .search-clear { position: absolute; right: 10px; top: 50%; transform: translateY(-50%); background: none; border: none; color: var(--text-tertiary); font-size: 14px; cursor: pointer; }
  .action-btn { display: flex; align-items: center; gap: 6px; padding: 10px 16px; background: var(--bg-secondary); border: 1px solid var(--border-primary); border-radius: 10px; color: var(--text-secondary); font-size: 14px; cursor: pointer; transition: all 0.2s; }
  .action-btn:hover { background: var(--bg-hover); color: var(--text-primary); }

  .filter-panel { display: flex; align-items: center; flex-wrap: wrap; gap: 10px 16px; margin-bottom: 14px; padding: 10px 12px; background: var(--bg-card); border: 1px solid var(--border-primary); border-radius: 10px; }
  .filter-group { display: flex; align-items: center; flex-wrap: wrap; gap: 6px; }
  .filter-label { font-size: 12px; color: var(--text-tertiary); margin-right: 2px; }
  .filter-pill { display: inline-flex; align-items: center; gap: 6px; min-height: 28px; padding: 4px 10px; border-radius: 7px; border: 1px solid var(--border-primary); background: var(--bg-secondary); color: var(--text-secondary); font-size: 12px; cursor: pointer; transition: all 0.15s; }
  .filter-pill:hover { background: var(--bg-hover); color: var(--text-primary); }
  .filter-pill.active { background: var(--accent-primary-light); border-color: var(--accent-primary); color: var(--accent-primary); }
  .filter-running.active { border-color: rgba(52, 211, 153, 0.45); color: #34d399; background: rgba(52, 211, 153, 0.08); }
  .filter-stopped.active { border-color: rgba(248, 113, 113, 0.45); color: #f87171; background: rgba(248, 113, 113, 0.08); }
  .pill-count { min-width: 18px; padding: 1px 6px; border-radius: 999px; background: var(--bg-tertiary); color: var(--text-tertiary); font-size: 10px; font-family: var(--theme-font-family-mono); text-align: center; }
  .filter-pill.active .pill-count { color: inherit; background: rgba(255,255,255,0.08); }
  .filter-summary { display: flex; gap: 10px; margin-left: auto; color: var(--text-tertiary); font-size: 11px; font-family: var(--theme-font-family-mono); }

  .table-wrap { overflow-x: auto; background: var(--bg-card); border: 1px solid var(--border-primary); border-radius: 14px; }
  .service-table { width: 100%; min-width: 930px; table-layout: fixed; border-collapse: collapse; font-size: 13px; }
  .service-table .col-index { width: 32px; }
  .service-table .col-pid { width: 54px; }
  .service-table .col-name { width: 126px; }
  .service-table .col-process { width: 86px; }
  .service-table .col-path { width: 164px; }
  .service-table .col-ports { width: 72px; }
  .service-table .col-status { width: 80px; }
  .service-table .col-cpu { width: 48px; }
  .service-table .col-memory { width: 66px; }
  .service-table .col-category { width: 72px; }
  .service-table .col-actions { width: 182px; }
  .service-table th { text-align: left; padding: 10px 8px; color: var(--text-secondary); font-weight: 600; font-size: 12px; text-transform: uppercase; letter-spacing: 0; border-bottom: 1px solid var(--border-primary); background: var(--bg-secondary); position: sticky; top: 0; z-index: 1; }
  .th-sort { border: none; background: transparent; color: inherit; font: inherit; text-transform: inherit; letter-spacing: inherit; padding: 0; cursor: pointer; }
  .th-sort:hover { color: var(--text-primary); }
  .service-table td { padding: 9px 8px; border-bottom: 1px solid var(--border-secondary); color: var(--text-primary); vertical-align: middle; overflow: hidden; text-overflow: ellipsis; }
  .service-table tr:hover td { background: var(--bg-hover); }
  .service-table .row-running td { background: rgba(52, 211, 153, 0.02); }

  .cell-index { font-family: var(--theme-font-family-mono); color: var(--text-tertiary); text-align: center; width: 30px; }
  .cell-pid { font-family: var(--theme-font-family-mono); color: var(--text-secondary); }
  .cell-name { font-weight: 600; color: var(--text-primary); white-space: nowrap; }
  .cell-process { font-family: var(--theme-font-family-mono); font-size: 11px; color: var(--text-secondary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .cell-path { font-family: var(--theme-font-family-mono); font-size: 11px; color: var(--text-tertiary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .cell-ports { font-family: var(--theme-font-family-mono); color: #22d3ee; white-space: nowrap; }
  .cell-status { display: flex; align-items: center; gap: 6px; white-space: nowrap; }
  .status-dot { width: 6px; height: 6px; border-radius: 50%; background: #ef4444; }
  .dot-running { background: #34d399; box-shadow: 0 0 6px rgba(52, 211, 153, 0.5); }
  .cell-cpu { font-family: var(--theme-font-family-mono); font-variant-numeric: tabular-nums; }
  .cell-mem { font-family: var(--theme-font-family-mono); font-variant-numeric: tabular-nums; }
  .cat-badge { font-size: 10px; padding: 2px 8px; border-radius: 10px; background: var(--bg-secondary); border: 1px solid var(--border-primary); white-space: nowrap; }

  .cell-actions { white-space: nowrap; }
  .action-group { display: flex; align-items: center; flex-wrap: wrap; gap: 4px; }
  .act-btn { min-width: 34px; padding: 4px 5px; border-radius: 4px; border: 1px solid var(--border-primary); background: var(--bg-secondary); color: var(--text-secondary); font-size: 11px; cursor: pointer; transition: all 0.15s; }
  .act-btn:hover { background: var(--bg-hover); color: var(--text-primary); }
  .act-btn:disabled { opacity: 0.5; cursor: not-allowed; }
  .act-restart { border-color: rgba(251, 191, 36, 0.3); }
  .act-restart:hover { background: rgba(251, 191, 36, 0.1); color: #fbbf24; }
  .act-stop { border-color: rgba(248, 113, 113, 0.3); }
  .act-stop:hover { background: rgba(248, 113, 113, 0.1); color: #f87171; }
  .act-start { border-color: rgba(52, 211, 153, 0.3); }
  .act-start:hover { background: rgba(52, 211, 153, 0.1); color: #34d399; }
  .act-log { border-color: rgba(139, 92, 246, 0.3); }
  .act-log:hover { background: rgba(139, 92, 246, 0.1); color: #a78bfa; }
  .act-health { border-color: rgba(16, 185, 129, 0.3); }
  .act-health:hover { background: rgba(16, 185, 129, 0.1); color: #10b981; }

  .service-card-list { display: none; }
  .service-card {
    background: var(--bg-card);
    border: 1px solid var(--border-primary);
    border-left: 4px solid var(--border-primary);
    border-radius: 10px;
    padding: 12px;
    box-shadow: var(--shadow-sm);
  }
  .service-card.card-running { border-left-color: #34d399; }
  .service-card-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 10px; margin-bottom: 10px; }
  .service-card-main { display: flex; align-items: flex-start; gap: 8px; min-width: 0; }
  .service-card-main .status-dot { margin-top: 7px; flex-shrink: 0; }
  .service-card-name { font-weight: 700; color: var(--text-primary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 210px; }
  .service-card-sub { margin-top: 2px; font-size: 11px; color: var(--text-secondary); font-family: var(--theme-font-family-mono); }
  .service-card-status { flex-shrink: 0; padding: 3px 7px; border-radius: 999px; background: rgba(16, 185, 129, 0.1); color: #10b981; font-size: 11px; font-weight: 700; }
  .service-card-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 6px; margin-bottom: 8px; }
  .service-card-grid div { min-width: 0; padding: 8px; border-radius: 8px; background: var(--bg-secondary); }
  .service-card-grid span { display: block; color: var(--text-tertiary); font-size: 10px; margin-bottom: 3px; }
  .service-card-grid strong { display: block; color: var(--text-primary); font-size: 12px; font-family: var(--theme-font-family-mono); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .service-card-path { margin-bottom: 10px; padding: 8px; border-radius: 8px; background: var(--bg-secondary); color: var(--text-secondary); font-family: var(--theme-font-family-mono); font-size: 11px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .service-card-actions { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 6px; }
  .service-card-actions .act-btn { width: 100%; min-height: 32px; font-size: 12px; }

  .modal-overlay { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.56); z-index: 100; display: flex; align-items: center; justify-content: center; padding: 18px; box-sizing: border-box; }
  .modal { width: min(760px, calc(100vw - 36px)); max-height: min(78vh, 760px); background: var(--bg-card); border: 1px solid var(--border-primary); border-radius: 10px; box-shadow: var(--shadow-lg); display: flex; flex-direction: column; overflow: hidden; }
  .log-modal { width: min(960px, calc(100vw - 36px)); height: min(72vh, 720px); }
  .modal-large { width: min(780px, calc(100vw - 36px)); }
  .modal-header { display: flex; justify-content: space-between; align-items: center; padding: 14px 18px; border-bottom: 1px solid var(--border-primary); }
  .modal-header h3 { margin: 0; font-size: 15px; color: var(--text-primary); }
  .modal-close { background: none; border: none; color: var(--text-tertiary); font-size: 18px; cursor: pointer; padding: 4px 8px; border-radius: 6px; }
  .modal-close:hover { background: var(--bg-hover); }
  .modal-body { flex: 1; overflow-y: auto; padding: 16px; min-height: 0; }
  .modal-loading { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 12px; padding: 46px 16px; color: var(--text-secondary); font-size: 13px; }
  .modal-spinner { width: 42px; height: 42px; border-radius: 50%; border: 3px solid rgba(34, 211, 238, 0.14); border-top-color: #22d3ee; border-right-color: rgba(16, 185, 129, 0.75); animation: spin 0.8s linear infinite; box-shadow: 0 0 18px rgba(34, 211, 238, 0.14); }
  .log-content { min-height: 100%; box-sizing: border-box; font-family: var(--theme-font-family-mono); font-size: 12px; line-height: 1.6; color: #d1d5db; white-space: pre-wrap; word-break: break-word; margin: 0; padding: 12px; background: #0b1020; border: 1px solid rgba(148, 163, 184, 0.16); border-radius: 8px; overflow: auto; }
  .health-result { }
  .health-status-card { display: flex; align-items: center; gap: 16px; padding: 16px; border: 1px solid; border-radius: 12px; margin-bottom: 16px; background: linear-gradient(135deg, rgba(15,23,42,.72), rgba(20,184,166,.06)); box-shadow: inset 0 0 28px rgba(34,211,238,.04); }
  .health-status-icon { display: grid; place-items: center; width: 46px; height: 46px; border-radius: 12px; background: rgba(15,23,42,.64); font-size: 22px; font-weight: 900; font-family: var(--theme-font-family-mono); }
  .health-status-info { flex: 1; }
  .health-status-text { font-size: 20px; font-weight: 700; margin-bottom: 4px; }
  .health-message { font-size: 14px; color: var(--text-secondary); }
  .health-details { display: flex; flex-direction: column; gap: 16px; }
  .health-detail-section { background: var(--bg-secondary); border: 1px solid var(--border-primary); border-radius: 10px; padding: 14px; }
  .health-detail-section h4 { font-size: 13px; font-weight: 600; color: var(--text-primary); margin: 0 0 10px; }
  .detail-rows { display: flex; flex-direction: column; gap: 8px; }
  .detail-row { display: flex; justify-content: space-between; align-items: center; }
  .detail-label { font-size: 13px; color: var(--text-secondary); }
  .detail-value { font-size: 13px; color: var(--text-primary); font-family: var(--theme-font-family-mono); }
  .detail-pids { font-size: 12px; }
  .health-process-table { overflow-x: auto; border: 1px solid var(--border-secondary); border-radius: 8px; background: var(--bg-primary); }
  .health-process-head, .health-process-row { display: grid; grid-template-columns: 70px 70px 64px 64px 86px 56px minmax(220px, 1fr); gap: 8px; min-width: 720px; align-items: center; padding: 8px 10px; }
  .health-process-head { color: var(--text-tertiary); font-size: 11px; font-weight: 800; border-bottom: 1px solid var(--border-secondary); text-transform: uppercase; }
  .health-process-row { color: var(--text-secondary); font-family: var(--theme-font-family-mono); font-size: 11px; border-bottom: 1px solid var(--border-secondary); }
  .health-process-row:last-child { border-bottom: none; }
  .cpu-val { color: #67e8f9; font-weight: 800; }
  .cmd-val { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--text-primary); }
  .ports-list { display: flex; flex-wrap: wrap; gap: 6px; }
  .port-item { font-family: var(--theme-font-family-mono); font-size: 12px; color: var(--text-secondary); background: var(--bg-primary); padding: 4px 8px; border-radius: 6px; }

  .loading { display: flex; align-items: center; justify-content: center; gap: 10px; padding: 60px 0; color: var(--text-secondary); font-size: 14px; }
  .spinner { width: 24px; height: 24px; border-radius: 50%; border: 3px solid var(--border-primary); border-top-color: var(--accent-primary); animation: spin 0.8s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
  .empty-state { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px; padding: 80px 0; color: var(--text-secondary); }
  .empty-icon { font-size: 48px; opacity: 0.5; }
  .empty-text { font-size: 15px; color: var(--text-secondary); }

  @media (max-width: 768px) {
    .page-header { align-items: flex-start; }
    .header-left { min-width: 44px; }
    .header-right { flex: 1; min-width: 0; }
    .filter-panel { align-items: stretch; }
    .filter-group { width: 100%; }
    .filter-summary { width: 100%; margin-left: 0; justify-content: space-between; }
    .search-input { width: 100%; }
    .table-wrap { display: none; }
    .service-card-list { display: flex; flex-direction: column; gap: 10px; }
  }
</style>
