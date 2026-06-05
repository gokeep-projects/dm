<script>
  import { onMount, onDestroy } from 'svelte';

  let stats = $state(null);
  let sys = $state(null);
  let refreshTimer = null;
  let refreshInterval = $state(15);
  let isRefreshing = $state(false);
  let lastUpdate = $state(new Date());
  let updateCount = $state(0);
  let lastUpdateText = $state('刚刚');
  let scripts = $state([]);
  let checks = $state([]);
  let quickRunItem = $state('');
  let showScriptSelect = $state(false);
  let scriptSearch = $state('');
  let dropdownIndex = $state(0);
  let searchInput = $state(null);
  let showStatList = $state(null);
  let statListData = $state([]);
  let showAllProcesses = $state(false);
  let allProcesses = $state([]);
  let processSortKey = $state('cpu_usage');
  let processSortDir = $state('desc');
  let historySortKey = $state('timestamp');
  let historySortDir = $state('desc');
  let dashboardWs = null;
  let metricTimer = null;
  let clockTimer = null;
  let wsConnected = $state(false);
  let metricHistory = $state([]);
  let lastMetricSampleAt = 0;
  let trendMinutes = $state(30);
  let trendHover = $state(null);
  let trendLoading = $state(false);
  let metricRequestSeq = 0;
  let dropdownList = $state(null);
  let trendRangeEnd = $state(Date.now());

  const SAMPLE_MIN_MS = 14 * 1000;
  const trendOptions = [
    { value: 3, label: '3分钟' },
    { value: 30, label: '30分钟' },
    { value: 60, label: '1小时' },
    { value: 120, label: '2小时' },
  ];

  const refreshOptions = [
    { value: 0, label: '关闭' },
    { value: 15, label: '15s' },
    { value: 30, label: '30s' },
    { value: 60, label: '60s' },
  ];

  function setRefreshInterval(val) {
    refreshInterval = val;
    if (refreshTimer) { clearInterval(refreshTimer); refreshTimer = null; }
    if (val > 0) refreshTimer = setInterval(loadData, val * 1000);
  }

  function connectDashboardWs() {
    const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
    dashboardWs = new WebSocket(`${proto}//${location.host}/ws/dashboard`);
    dashboardWs.onopen = () => {
      wsConnected = true;
    };
    dashboardWs.onmessage = (e) => {
      try {
        const d = JSON.parse(e.data);
        if (d.type === 'update') {
          sys = d.system;
          stats = d.stats;
          scripts = d.scripts || scripts;
          recordMetricSample(sys);
          lastUpdate = new Date();
          updateCount++;
          lastUpdateText = '刚刚';
        }
      } catch (_) {}
    };
    dashboardWs.onclose = () => {
      wsConnected = false;
      setTimeout(connectDashboardWs, 3000);
    };
    dashboardWs.onerror = () => {
      wsConnected = false;
    };
  }

  async function loadData() {
    isRefreshing = true;
    try {
      const [s, i, sc, ck] = await Promise.all([
        fetch('/api/dashboard/stats').then(r => r.ok ? r.json() : null),
        fetch('/api/system/info').then(r => r.ok ? r.json() : null),
        fetch('/api/scripts').then(r => r.ok ? r.json() : null),
        fetch('/api/checks').then(r => r.ok ? r.json() : null),
      ]);
      if (s) stats = s;
      if (i) {
        sys = i;
        recordMetricSample(sys);
      }
      if (sc) scripts = sc.scripts || [];
      if (ck) checks = ck.checks || [];
    } catch (e) { console.warn('加载仪表盘数据失败:', e); }
    lastUpdate = new Date();
    updateCount++;
    setTimeout(() => isRefreshing = false, 300);
  }

  function fmt(b) { if (!b) return '0 B'; const k = 1024, s = ['B','KB','MB','GB']; let i = 0, v = b; while (v >= k && i < 3) { v /= k; i++; } return v.toFixed(1) + ' ' + s[i]; }
  function upt(s) { const d = Math.floor(s / 86400), h = Math.floor((s % 86400) / 3600), m = Math.floor((s % 3600) / 60); return d + '天 ' + h + '时 ' + m + '分'; }
  function barColor(v) { if (v == null) return '#3b82f6'; if (v < 60) return '#3b82f6'; if (v < 80) return '#f59e0b'; return '#ef4444'; }
  function formatTime(ts) { if (!ts) return '-'; try { const d = new Date(ts.replace(' ', 'T')); const diff = Math.floor((Date.now() - d.getTime()) / 1000); if (diff < 60) return diff + '秒前'; if (diff < 3600) return Math.floor(diff/60) + '分钟前'; if (diff < 86400) return Math.floor(diff/3600) + '小时前'; return d.toLocaleDateString('zh-CN', { month: '2-digit', day: '2-digit' }); } catch { return ts; } }
  function formatRelative(date) { const diff = Math.floor((Date.now() - date.getTime()) / 1000); if (diff < 5) return '刚刚'; if (diff < 60) return diff + '秒前'; if (diff < 3600) return Math.floor(diff/60) + '分钟前'; return date.toLocaleTimeString('zh-CN', { hour12: false }); }
  function healthScore() {
    if (!sys) return { score: 0, level: 'unknown', label: '未知' };
    const cpu = sys.cpu_usage || 0, mem = sys.memory_usage || 0, disk = sys.disk_usage || 0;
    const loadPct = Math.min(((sys.load_avg?.one || 0) / (sys.cpu_count || 1)) * 100, 100);
    const score = Math.round((Math.max(0, 100 - cpu) * 0.3 + Math.max(0, 100 - mem) * 0.3 + Math.max(0, 100 - disk) * 0.2 + Math.max(0, 100 - loadPct) * 0.2));
    if (score >= 80) return { score, level: 'good', label: '健康' };
    if (score >= 60) return { score, level: 'warn', label: '注意' };
    return { score, level: 'bad', label: '警告' };
  }
  function healthColor(level) { if (level === 'good') return '#10b981'; if (level === 'warn') return '#f59e0b'; return '#ef4444'; }

  function visibleNetworks() {
    return (sys?.networks || []).filter(n => n.name !== 'lo' && !n.name.includes('docker') && !n.name.includes('br-') && !n.name.includes('veth'));
  }

  function networkTotals(system = sys) {
    const nets = (system?.networks || []).filter(n => n.name !== 'lo' && !n.name.includes('docker') && !n.name.includes('br-') && !n.name.includes('veth'));
    return nets.reduce((acc, n) => {
      acc.rx += Number(n.received_bytes || 0);
      acc.tx += Number(n.transmitted_bytes || 0);
      return acc;
    }, { rx: 0, tx: 0 });
  }

  function trendWindowMs() {
    return trendMinutes * 60 * 1000;
  }

  function trendRangeStart() {
    return trendRangeEnd - trendWindowMs();
  }

  function quickItemMatches(item, q) {
    if (!q) return true;
    const number = item.number;
    const label = item.numberLabel;
    return String(number).includes(q) ||
      label.includes(q) ||
      ('#' + number).includes(q) ||
      ('#' + label).includes(q) ||
      item.name?.toLowerCase().includes(q) ||
      item.id?.toLowerCase().includes(q) ||
      item.description?.toLowerCase().includes(q) ||
      item.category?.toLowerCase().includes(q) ||
      item.kindLabel?.toLowerCase().includes(q);
  }

  async function loadMetricHistory(showLoading = false) {
    const minutes = trendMinutes;
    const seq = ++metricRequestSeq;
    const rangeEnd = Date.now();
    trendRangeEnd = rangeEnd;
    if (showLoading) trendLoading = true;
    try {
      const r = await fetch(`/api/dashboard/metrics?minutes=${minutes}&ts=${Date.now()}`, { cache: 'no-store' });
      if (!r.ok) return;
      const d = await r.json();
      if (seq !== metricRequestSeq || minutes !== trendMinutes) return;
      const cutoff = rangeEnd - minutes * 60 * 1000;
      metricHistory = (d.points || []).map(p => ({
        t: Number(p.ts_ms || 0),
        timestamp: p.timestamp,
        cpu: Number(p.cpu_usage || 0),
        mem: Number(p.memory_usage || 0),
        load: Number(p.load_ratio || 0),
        load_one: Number(p.load_one || 0),
        rx: Number(p.rx_bytes || 0),
        tx: Number(p.tx_bytes || 0),
      })).filter(p => p.t >= cutoff);
    } catch (_) {
    } finally {
      if (showLoading && seq === metricRequestSeq) trendLoading = false;
    }
  }

  function recordMetricSample(system) {
    if (!system) return;
    const now = Date.now();
    if (lastMetricSampleAt && now - lastMetricSampleAt < SAMPLE_MIN_MS) return;
    lastMetricSampleAt = now;
    trendRangeEnd = now;
    const totals = networkTotals(system);
    const loadRatio = system.cpu_count ? (Number(system.load_avg?.one || 0) / Number(system.cpu_count || 1)) * 100 : 0;
    const point = {
      t: now,
      cpu: Number(system.cpu_usage || 0),
      mem: Number(system.memory_usage || 0),
      load: Math.min(Math.max(loadRatio, 0), 300),
      rx: totals.rx,
      tx: totals.tx,
    };
    const cutoff = trendRangeStart();
    metricHistory = [...metricHistory.filter(p => p.t >= cutoff), point].slice(-520);
  }

  async function setTrendMinutes(value) {
    if (trendMinutes === value) return;
    trendMinutes = value;
    trendHover = null;
    trendRangeEnd = Date.now();
    const cutoff = trendRangeStart();
    metricHistory = metricHistory.filter(p => p.t >= cutoff);
    await loadMetricHistory(true);
  }

  function windowedMetricHistory() {
    const start = trendRangeStart();
    return metricHistory.filter(p => p.t >= start && p.t <= trendRangeEnd);
  }

  function seriesPath(key, maxValue = 100, height = 150, width = 620) {
    const history = windowedMetricHistory();
    if (!history.length) return '';
    const pad = 10;
    const start = trendRangeStart();
    const points = history.map(p => {
      const x = pad + Math.max(0, Math.min(1, (p.t - start) / trendWindowMs())) * (width - pad * 2);
      const y = pad + (1 - Math.max(0, Math.min(1, Number(p[key] || 0) / maxValue))) * (height - pad * 2);
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    });
    if (points.length === 1) points.push(`${(width - pad).toFixed(1)},${points[0].split(',')[1]}`);
    return points.join(' ');
  }

  function networkRatePoints(key) {
    const points = [];
    const history = windowedMetricHistory();
    for (let i = 1; i < history.length; i++) {
      const prev = history[i - 1];
      const cur = history[i];
      const seconds = Math.max(1, (cur.t - prev.t) / 1000);
      points.push({ t: cur.t, value: Math.max(0, (Number(cur[key] || 0) - Number(prev[key] || 0)) / seconds) });
    }
    return points;
  }

  function netPath(key, height = 150, width = 620) {
    const points = networkRatePoints(key);
    if (!points.length) return '';
    const maxValue = Math.max(1, ...points.map(p => p.value));
    const pad = 10;
    const start = trendRangeStart();
    return points.map(p => {
      const x = pad + Math.max(0, Math.min(1, (p.t - start) / trendWindowMs())) * (width - pad * 2);
      const y = pad + (1 - Math.max(0, Math.min(1, p.value / maxValue))) * (height - pad * 2);
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    }).join(' ');
  }

  function latestRate(key) {
    const points = networkRatePoints(key);
    return points.length ? points[points.length - 1].value : 0;
  }

  function timeRangeLabel() {
    const end = new Date(trendRangeEnd);
    const start = new Date(trendRangeStart());
    const opt = { hour: '2-digit', minute: '2-digit', hour12: false };
    return `${start.toLocaleTimeString('zh-CN', opt)} - ${end.toLocaleTimeString('zh-CN', opt)}`;
  }

  function selectedTrendLabel() {
    return trendOptions.find(o => o.value === trendMinutes)?.label || '30分钟';
  }

  function chartHover(event, chart) {
    const history = windowedMetricHistory();
    if (!history.length) return;
    const rect = event.currentTarget.getBoundingClientRect();
    const x = Math.max(0, Math.min(rect.width, event.clientX - rect.left));
    const ratio = rect.width ? x / rect.width : 0;
    const target = trendRangeStart() + trendWindowMs() * ratio;
    let nearest = history[0];
    let distance = Math.abs(nearest.t - target);
    for (const point of history) {
      const d = Math.abs(point.t - target);
      if (d < distance) {
        nearest = point;
        distance = d;
      }
    }
    trendHover = {
      chart,
      x,
      y: Math.max(12, Math.min(rect.height - 12, event.clientY - rect.top)),
      point: nearest,
    };
  }

  function hideTrendHover(chart) {
    if (trendHover?.chart === chart) trendHover = null;
  }

  function hoverRate(key) {
    if (!trendHover?.point) return 0;
    const history = windowedMetricHistory();
    const idx = history.findIndex(p => p.t === trendHover.point.t);
    if (idx <= 0) return 0;
    const prev = history[idx - 1];
    const seconds = Math.max(1, (trendHover.point.t - prev.t) / 1000);
    return Math.max(0, (Number(trendHover.point[key] || 0) - Number(prev[key] || 0)) / seconds);
  }

  function hoverTime(point) {
    if (!point) return '-';
    const d = new Date(point.t);
    return d.toLocaleTimeString('zh-CN', { hour12: false });
  }

  function moveDropdownSelection(delta) {
    dropdownIndex = Math.max(0, Math.min(dropdownIndex + delta, filteredQuickItems.length - 1));
    setTimeout(() => {
      dropdownList?.querySelector('.item-active')?.scrollIntoView({ block: 'nearest' });
    }, 0);
  }

  function changeProcessSort(key) {
    if (processSortKey === key) processSortDir = processSortDir === 'asc' ? 'desc' : 'asc';
    else {
      processSortKey = key;
      processSortDir = key === 'cpu_usage' || key === 'memory_bytes' ? 'desc' : 'asc';
    }
  }

  function changeHistorySort(key) {
    if (historySortKey === key) historySortDir = historySortDir === 'asc' ? 'desc' : 'asc';
    else {
      historySortKey = key;
      historySortDir = key === 'timestamp' ? 'desc' : 'asc';
    }
  }

  function sortMark(activeKey, activeDir, key) {
    if (activeKey !== key) return '';
    return activeDir === 'asc' ? ' ↑' : ' ↓';
  }

  function processValue(p, key) {
    if (key === 'pid') return Number(p.pid || 0);
    if (key === 'cpu_usage') return Number(p.cpu_usage || 0);
    if (key === 'memory_bytes') return Number(p.memory_bytes || 0);
    if (key === 'status') return p.status || '';
    if (key === 'cmd') return p.cmd || p.name || '';
    return p.name || '';
  }

  function historyValue(r, key) {
    if (key === 'exit_code') return Number(r.exit_code ?? 9999);
    if (key === 'script_name') return r.script_name || '';
    return r.timestamp || '';
  }

  function getPublicIp() {
    if (!sys?.networks) return '-';
    for (const n of sys.networks) {
      if (n.name === 'lo' || n.name.includes('docker') || n.name.includes('br-') || n.name.includes('veth')) continue;
      if (n.ip && !n.ip.startsWith('127.') && !n.ip.startsWith('172.')) return n.ip;
    }
    for (const n of sys.networks) {
      if (n.name === 'lo' || n.name.includes('docker') || n.name.includes('br-')) continue;
      if (n.ip) return n.ip;
    }
    return '-';
  }

  function loadLevel(load, cores) {
    if (!load || !cores) return 'normal';
    const ratio = load / cores;
    if (ratio > 2) return 'critical';
    if (ratio > 1) return 'warning';
    return 'normal';
  }

  let quickItems = $derived.by(() => {
    const checkItems = checks.map((c, index) => {
      const number = index + 1;
      return {
        ...c,
        kind: 'check',
        kindLabel: '检查',
        value: 'check:' + c.id,
        number,
        numberLabel: String(number).padStart(2, '0'),
        description: c.description || '',
      };
    });
    const scriptItems = scripts.map((s, index) => {
      const number = checks.length + index + 1;
      return {
        ...s,
        kind: 'script',
        kindLabel: '脚本',
        value: 'script:' + s.id,
        number,
        numberLabel: String(number).padStart(2, '0'),
        description: s.description || s.feature || '',
      };
    });
    return [...checkItems, ...scriptItems];
  });

  let selectedQuickItem = $derived.by(() => quickItems.find(item => item.value === quickRunItem));

  let filteredQuickItems = $derived.by(() => {
    if (!scriptSearch.trim()) return quickItems;
    const q = scriptSearch.trim().toLowerCase();
    return quickItems.filter(item => quickItemMatches(item, q));
  });

  let sortedProcesses = $derived.by(() => {
    return [...allProcesses].sort((a, b) => {
      const av = processValue(a, processSortKey);
      const bv = processValue(b, processSortKey);
      let cmp = typeof av === 'number' && typeof bv === 'number'
        ? av - bv
        : String(av).localeCompare(String(bv), 'zh-CN');
      if (processSortDir === 'desc') cmp = -cmp;
      return cmp;
    });
  });

  let sortedHistory = $derived.by(() => {
    return [...statListData].sort((a, b) => {
      const av = historyValue(a, historySortKey);
      const bv = historyValue(b, historySortKey);
      let cmp = typeof av === 'number' && typeof bv === 'number'
        ? av - bv
        : String(av).localeCompare(String(bv), 'zh-CN');
      if (historySortDir === 'desc') cmp = -cmp;
      return cmp;
    });
  });

  async function showStatListPanel(type) {
    showStatList = type;
    try {
      const r = await fetch('/api/dashboard/history?limit=100');
      if (r.ok) {
        const d = await r.json();
        statListData = d.records || [];
      }
    } catch (_) {}
  }

  let processRefreshTimer = $state(null);

  async function showAllProcessesPanel() {
    showAllProcesses = true;
    await loadAllProcesses();
    if (processRefreshTimer) clearInterval(processRefreshTimer);
    processRefreshTimer = setInterval(loadAllProcesses, 2000);
  }

  async function loadAllProcesses() {
    try {
      const r = await fetch('/api/system/processes');
      if (r.ok) {
        const d = await r.json();
        allProcesses = d.processes || [];
      }
    } catch (_) {}
  }

  function closeAllProcesses() {
    showAllProcesses = false;
    if (processRefreshTimer) { clearInterval(processRefreshTimer); processRefreshTimer = null; }
  }

  function startQuickRun(value) {
    if (!value) return;
    showScriptSelect = false;
    const [kind, ...rest] = value.split(':');
    const targetId = rest.join(':');
    if (kind === 'check') location.hash = '#/check/' + encodeURIComponent(targetId);
    else location.hash = '#/script/' + encodeURIComponent(targetId) + '/run';
  }

  onMount(() => { 
    loadMetricHistory();
    loadData(); 
    connectDashboardWs();
    if (refreshInterval > 0) refreshTimer = setInterval(loadData, refreshInterval * 1000); 
    metricTimer = setInterval(loadMetricHistory, 30000);
    clockTimer = setInterval(() => { lastUpdateText = formatRelative(lastUpdate); }, 1000);
  });
  onDestroy(() => { 
    if (refreshTimer) clearInterval(refreshTimer); 
    if (metricTimer) clearInterval(metricTimer);
    if (clockTimer) clearInterval(clockTimer);
    if (processRefreshTimer) clearInterval(processRefreshTimer);
    if (dashboardWs) {
      dashboardWs.onclose = null;
      dashboardWs.close();
    }
  });
</script>

<div class="dashboard">
  <div class="dash-top">
    <div class="health-card">
      {#if sys}
        {@const h = healthScore()}
        <div class="health-score" style="color:{healthColor(h.level)}">{h.score}</div>
        <div class="health-label" style="color:{healthColor(h.level)}">{h.label}</div>
      {/if}
    </div>
    <div class="stat-cards">
      <div class="mini-stat">
        <span class="mini-val">{stats?.total_scripts ?? 0}</span>
        <span class="mini-label">脚本</span>
      </div>
      <div class="mini-stat">
        <span class="mini-val">{stats?.total_executions ?? 0}</span>
        <span class="mini-label">执行</span>
      </div>
      <div class="mini-stat">
        <span class="mini-val" style="color:#10b981">{stats?.success_count ?? 0}</span>
        <span class="mini-label">成功</span>
      </div>
      <div class="mini-stat">
        <span class="mini-val" style="color:#ef4444">{stats?.failure_count ?? 0}</span>
        <span class="mini-label">失败</span>
      </div>
    </div>
    <div class="meta-bar">
      <div class="meta-item"><span class="meta-dot" class:active={isRefreshing}></span><span>{lastUpdateText}</span></div>
      <div class="refresh-select">{#each refreshOptions as opt}<button class="refresh-opt" class:active={refreshInterval === opt.value} onclick={() => setRefreshInterval(opt.value)}>{opt.label}</button>{/each}</div>
    </div>
  </div>

  <div class="quick-run-section">
    <div class="qr-header">
      <h2>快速执行</h2>
      <div class="qr-status ready">选择脚本或检查后进入结果页执行</div>
    </div>
    <div class="qr-controls">
      <div class="qr-select-wrap">
        <button class="qr-select-btn" onclick={() => { showScriptSelect = !showScriptSelect; if (showScriptSelect) setTimeout(() => searchInput?.focus(), 100); }}>
          {selectedQuickItem ? `${selectedQuickItem.kindLabel} · ${selectedQuickItem.name || selectedQuickItem.id}` : '选择脚本或检查...'}
          <span class="qr-select-arrow">▼</span>
        </button>
        {#if showScriptSelect}
          <div class="qr-dropdown">
            <input type="text" placeholder="输入编号、名称、ID 搜索脚本或检查..." bind:value={scriptSearch} bind:this={searchInput} class="qr-dropdown-search" oninput={() => dropdownIndex = 0} onkeydown={(e) => {
              if (e.key === 'ArrowDown') { e.preventDefault(); moveDropdownSelection(1); }
              else if (e.key === 'ArrowUp') { e.preventDefault(); moveDropdownSelection(-1); }
              else if (e.key === 'Enter') { e.preventDefault(); if (filteredQuickItems[dropdownIndex]) { quickRunItem = filteredQuickItems[dropdownIndex].value; showScriptSelect = false; } }
              else if (e.key === 'Escape') { showScriptSelect = false; }
            }} />
            <div class="qr-dropdown-list" bind:this={dropdownList}>
              {#each filteredQuickItems as s, i}
                <button class="qr-dropdown-item" class:item-active={i === dropdownIndex} onclick={() => { quickRunItem = s.value; showScriptSelect = false; }}>
                  <span class="qr-item-number">#{s.numberLabel}</span>
                  <span class="qr-item-type" class:check-type={s.kind === 'check'}>{s.kindLabel}</span>
                  <span class="qr-item-icon">{s.kind === 'check' ? 'CHK' : s.category === '系统检查' ? 'SYS' : s.category === '系统安全' ? 'SEC' : 'RUN'}</span>
                  <div class="qr-item-info">
                    <span class="qr-item-name">{s.name}</span>
                    <span class="qr-item-id">{s.id}</span>
                    <span class="qr-item-desc">{s.description || ''}</span>
                  </div>
                </button>
              {:else}
                <div class="qr-empty">没有匹配的脚本或检查项</div>
              {/each}
            </div>
          </div>
        {/if}
      </div>
      <button class="qr-btn qr-start" onclick={() => startQuickRun(quickRunItem)} disabled={!quickRunItem}>执行并查看结果</button>
    </div>
  </div>

  <div class="main-grid">
    <div class="left-col">
      <div class="resource-section"><h3>系统资源</h3>
        <div class="resource-bars">
          <div class="res-item"><div class="res-header"><span>CPU</span><span style="color:{barColor(sys?.cpu_usage)}">{(sys?.cpu_usage || 0).toFixed(1)}%</span></div><div class="res-bar"><div class="res-fill" style="width:{Math.min(sys?.cpu_usage || 0, 100)}%;background:{barColor(sys?.cpu_usage)}"></div></div><div class="res-detail">{sys?.cpu_brand || '-'} · {sys?.cpu_count || 0} 核</div></div>
          <div class="res-item"><div class="res-header"><span>内存</span><span style="color:{barColor(sys?.memory_usage)}">{(sys?.memory_usage || 0).toFixed(1)}%</span></div><div class="res-bar"><div class="res-fill" style="width:{Math.min(sys?.memory_usage || 0, 100)}%;background:{barColor(sys?.memory_usage)}"></div></div><div class="res-detail">{fmt(sys?.memory_used)} / {fmt(sys?.memory_total)}</div></div>
          <div class="res-item"><div class="res-header"><span>磁盘</span><span style="color:{barColor(sys?.disk_usage)}">{(sys?.disk_usage || 0).toFixed(1)}%</span></div><div class="res-bar"><div class="res-fill" style="width:{Math.min(sys?.disk_usage || 0, 100)}%;background:{barColor(sys?.disk_usage)}"></div></div><div class="res-detail">{fmt(sys?.disk_used)} / {fmt(sys?.disk_total)}</div></div>
        </div>
      </div>
      <div class="load-section trend-card">
        <div class="trend-head">
          <h3>系统趋势 <span class="load-hint">最近 {selectedTrendLabel()} · {timeRangeLabel()}</span></h3>
          <div class="trend-values">
            <div class="trend-range">
              {#each trendOptions as opt}
                <button class:active={trendMinutes === opt.value} disabled={trendLoading && trendMinutes === opt.value} onclick={() => setTrendMinutes(opt.value)}>{opt.label}</button>
              {/each}
            </div>
            <span class="trend-chip cpu">CPU {(sys?.cpu_usage || 0).toFixed(1)}%</span>
            <span class="trend-chip mem">内存 {(sys?.memory_usage || 0).toFixed(1)}%</span>
            <span class="trend-chip load">负载 {sys?.load_avg?.one?.toFixed(2) || '0.00'}</span>
            <span class="trend-chip rx">↓ {fmt(latestRate('rx'))}/s</span>
            <span class="trend-chip tx">↑ {fmt(latestRate('tx'))}/s</span>
            <span class="trend-chip total">总量 {fmt(networkTotals().rx + networkTotals().tx)}</span>
          </div>
        </div>
        <div class="trend-chart" role="img" aria-label="系统趋势悬停查看数值" onmousemove={(e) => chartHover(e, 'system')} onmouseleave={() => hideTrendHover('system')}>
          <svg viewBox="0 0 620 150" preserveAspectRatio="none" aria-label="CPU、内存、负载、网络收发趋势">
            <defs>
              <linearGradient id="trendCpu" x1="0" x2="1"><stop offset="0%" stop-color="#22d3ee"/><stop offset="100%" stop-color="#38bdf8"/></linearGradient>
              <linearGradient id="trendMem" x1="0" x2="1"><stop offset="0%" stop-color="#a78bfa"/><stop offset="100%" stop-color="#f472b6"/></linearGradient>
              <linearGradient id="trendLoad" x1="0" x2="1"><stop offset="0%" stop-color="#f59e0b"/><stop offset="100%" stop-color="#f97316"/></linearGradient>
              <linearGradient id="trendRx" x1="0" x2="1"><stop offset="0%" stop-color="#34d399"/><stop offset="100%" stop-color="#22c55e"/></linearGradient>
              <linearGradient id="trendTx" x1="0" x2="1"><stop offset="0%" stop-color="#60a5fa"/><stop offset="100%" stop-color="#818cf8"/></linearGradient>
            </defs>
            <g class="chart-grid">
              <line x1="10" y1="10" x2="610" y2="10"/><line x1="10" y1="75" x2="610" y2="75"/><line x1="10" y1="140" x2="610" y2="140"/>
              <line x1="10" y1="10" x2="10" y2="140"/><line x1="210" y1="10" x2="210" y2="140"/><line x1="410" y1="10" x2="410" y2="140"/><line x1="610" y1="10" x2="610" y2="140"/>
            </g>
            <polyline class="trend-line cpu-line" points={seriesPath('cpu', 100)} />
            <polyline class="trend-line mem-line" points={seriesPath('mem', 100)} />
            <polyline class="trend-line load-line" points={seriesPath('load', 300)} />
            <polyline class="trend-line rx-line" points={netPath('rx')} />
            <polyline class="trend-line tx-line" points={netPath('tx')} />
          </svg>
          {#if trendLoading}
            <div class="trend-loading"><span></span><em>切换 {selectedTrendLabel()} 数据</em></div>
          {/if}
          {#if trendHover?.chart === 'system'}
            <div class="trend-tooltip" style="left:{trendHover.x}px;top:{trendHover.y}px">
              <strong>{hoverTime(trendHover.point)}</strong>
              <span>CPU {trendHover.point.cpu.toFixed(1)}%</span>
              <span>内存 {trendHover.point.mem.toFixed(1)}%</span>
              <span>负载 {trendHover.point.load_one?.toFixed?.(2) || '-'}</span>
              <span>接收 {fmt(hoverRate('rx'))}/s</span>
              <span>发送 {fmt(hoverRate('tx'))}/s</span>
            </div>
          {/if}
          <div class="trend-axis">
            <span>{new Date(trendRangeStart()).toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit', hour12: false })}</span>
            <span>{new Date(trendRangeStart() + trendWindowMs() / 2).toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit', hour12: false })}</span>
            <span>{new Date(trendRangeEnd).toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit', hour12: false })}</span>
          </div>
        </div>
      </div>
    </div>
    <div class="right-col">
      <div class="sys-info-section"><h3>ℹ️ 系统信息</h3><div class="info-rows">
        <div class="info-row"><span>主机</span><span>{sys?.hostname || '-'}</span></div>
        <div class="info-row"><span>IP</span><span class="ip-value">{getPublicIp()}</span></div>
        <div class="info-row"><span>系统</span><span>{sys?.os || '-'}</span></div>
        <div class="info-row"><span>内核</span><span>{sys?.kernel || '-'}</span></div>
        <div class="info-row"><span>架构</span><span>{sys?.arch || '-'} · {sys?.cpu_count || 0}核</span></div>
        <div class="info-row"><span>运行</span><span>{sys?.uptime ? upt(sys.uptime) : '-'}</span></div>
        <div class="info-row"><span>进程</span><span>{sys?.process_count || 0}</span></div>
      </div></div>
      {#if sys?.top_processes?.length}
        <div class="proc-section">
          <h3>Top 进程 <span class="proc-hint">(CPU+内存综合)</span></h3>
          <div class="proc-table">
            <div class="proc-header">
              <span class="proc-col-rank">#</span>
              <span class="proc-col-name">进程名</span>
              <span class="proc-col-cpu">CPU</span>
              <span class="proc-col-mem">内存</span>
            </div>
            {#each sys.top_processes.slice(0, 6) as p, i}
              <div class="proc-row" class:proc-warning={p.cpu_usage > 50} class:proc-critical={p.cpu_usage > 80}>
                <span class="proc-col-rank">{i + 1}</span>
                <span class="proc-col-name">{p.name}</span>
                <span class="proc-col-cpu" style="color:{barColor(p.cpu_usage)}">{p.cpu_usage.toFixed(1)}%</span>
                <span class="proc-col-mem">{fmt(p.memory_bytes)}</span>
              </div>
            {/each}
          </div>
          <button class="proc-more-btn" onclick={showAllProcessesPanel}>查看所有进程 →</button>
        </div>
      {/if}
    </div>
  </div>
</div>

{#if showStatList}
  <div class="modal-overlay" onclick={() => showStatList = null} role="presentation">
    <div class="modal" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="dialog" aria-modal="true" tabindex="-1">
      <div class="modal-header"><h3>{showStatList === 'scripts' ? '脚本列表' : showStatList === 'executions' ? '执行记录' : showStatList === 'success' ? '成功记录' : '失败记录'}</h3><button class="modal-close" onclick={() => showStatList = null}>✕</button></div>
      <div class="modal-body">
        <div class="stat-list-head">
          <button onclick={() => changeHistorySort('script_name')}>脚本{sortMark(historySortKey, historySortDir, 'script_name')}</button>
          <button onclick={() => changeHistorySort('timestamp')}>时间{sortMark(historySortKey, historySortDir, 'timestamp')}</button>
          <button onclick={() => changeHistorySort('exit_code')}>结果{sortMark(historySortKey, historySortDir, 'exit_code')}</button>
        </div>
        <div class="stat-list">{#each sortedHistory as r}<div class="stat-list-item"><span class="stat-list-name">{r.script_name}</span><span class="stat-list-time">{formatTime(r.timestamp)}</span><span class="stat-list-badge" style="background:{r.exit_code === 0 ? '#10b981' : '#ef4444'}22;color:{r.exit_code === 0 ? '#10b981' : '#ef4444'}">{r.exit_code === 0 ? 'OK' : 'FAIL'}</span></div>{/each}</div>
      </div>
    </div>
  </div>
{/if}

{#if showAllProcesses}
  <div class="modal-overlay" onclick={closeAllProcesses} role="presentation">
    <div class="modal modal-xl" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="dialog" aria-modal="true" tabindex="-1">
      <div class="modal-header">
        <h3>所有进程 ({allProcesses.length})</h3>
        <div class="modal-actions">
          <button class="modal-refresh" onclick={loadAllProcesses}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 12a9 9 0 0 0-9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/><path d="M3 12a9 9 0 0 0 9 9 9.75 9.75 0 0 0 6.74-2.74L21 16"/><path d="M16 16h5v5"/></svg>
            刷新
          </button>
          <button class="modal-close" onclick={closeAllProcesses}>✕</button>
        </div>
      </div>
      <div class="modal-body">
        <div class="process-table">
          <div class="process-header">
            <span class="col-rank">#</span>
            <button class="process-sort" onclick={() => changeProcessSort('pid')}>PID{sortMark(processSortKey, processSortDir, 'pid')}</button>
            <button class="process-sort" onclick={() => changeProcessSort('name')}>进程名{sortMark(processSortKey, processSortDir, 'name')}</button>
            <button class="process-sort" onclick={() => changeProcessSort('cmd')}>命令行{sortMark(processSortKey, processSortDir, 'cmd')}</button>
            <button class="process-sort" onclick={() => changeProcessSort('cpu_usage')}>CPU%{sortMark(processSortKey, processSortDir, 'cpu_usage')}</button>
            <button class="process-sort" onclick={() => changeProcessSort('memory_bytes')}>内存{sortMark(processSortKey, processSortDir, 'memory_bytes')}</button>
            <button class="process-sort" onclick={() => changeProcessSort('status')}>状态{sortMark(processSortKey, processSortDir, 'status')}</button>
          </div>
          <div class="process-list">
            {#each sortedProcesses as p, i}
              <div class="process-row" class:proc-warning={p.cpu_usage > 50} class:proc-critical={p.cpu_usage > 80}>
                <span class="col-rank">{i + 1}</span>
                <span class="col-pid">{p.pid}</span>
                <span class="col-name">{p.name}</span>
                <span class="col-cmd" title={p.cmd || p.name}>{p.cmd || p.name}</span>
                <span class="col-cpu" style="color:{barColor(p.cpu_usage)}">{p.cpu_usage.toFixed(1)}%</span>
                <span class="col-mem">{fmt(p.memory_bytes)}</span>
                <span class="col-status">{p.status || '运行中'}</span>
              </div>
            {/each}
          </div>
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .dashboard { width: 100%; max-width: none; margin: 0; }
  .dash-top { display: flex; align-items: center; gap: 16px; margin-bottom: 16px; padding: 12px 16px; background: var(--bg-card); border: 1px solid var(--border-primary); border-radius: 12px; }
  .health-card { display: flex; flex-direction: column; align-items: center; min-width: 60px; }
  .health-score { font-size: 28px; font-weight: 800; font-family: var(--theme-font-family-mono); }
  .health-label { font-size: 11px; font-weight: 600; }
  .stat-cards { display: flex; gap: 12px; flex: 1; }
  .mini-stat { display: flex; flex-direction: column; align-items: center; padding: 4px 12px; background: none; border: none; cursor: pointer; }
  .mini-stat:hover { background: var(--bg-hover); border-radius: 8px; }
  .mini-val { font-size: 20px; font-weight: 700; color: var(--text-primary); font-family: var(--theme-font-family-mono); }
  .mini-label { font-size: 10px; color: var(--text-tertiary); }
  .meta-bar { display: flex; align-items: center; gap: 10px; margin-left: auto; }
  .meta-item { display: flex; align-items: center; gap: 4px; font-size: 11px; color: var(--text-tertiary); }
  .meta-dot { width: 6px; height: 6px; border-radius: 50%; background: var(--text-tertiary); }
  .meta-dot.active { background: #3b82f6; box-shadow: 0 0 6px rgba(59,130,246,0.6); }
  .refresh-select { display: flex; gap: 1px; background: var(--bg-secondary); border-radius: 6px; padding: 2px; }
  .refresh-opt { padding: 3px 6px; border: none; background: transparent; color: var(--text-tertiary); font-size: 10px; cursor: pointer; border-radius: 4px; }
  .refresh-opt.active { background: var(--bg-card); color: var(--accent-primary); }
  .quick-run-section { background: var(--bg-card); border: 1px solid var(--border-primary); border-radius: 12px; padding: 14px; margin-bottom: 16px; }
  .qr-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 10px; }
  .qr-header h2 { font-size: 14px; font-weight: 700; color: var(--text-primary); margin: 0; }
  .qr-status { font-size: 11px; padding: 3px 8px; border-radius: 6px; display: flex; align-items: center; gap: 4px; }
  .qr-status.ready { background: var(--bg-secondary); color: var(--text-secondary); border: 1px solid var(--border-primary); }
  .qr-controls { display: flex; gap: 8px; align-items: center; }
  .qr-select-wrap { flex: 1; position: relative; }
  .qr-select-btn { width: 100%; padding: 8px 12px; background: var(--bg-input); border: 1px solid var(--border-primary); border-radius: 8px; font-size: 12px; color: var(--text-primary); cursor: pointer; text-align: left; display: flex; justify-content: space-between; align-items: center; }
  .qr-select-btn:hover { border-color: var(--border-focus); }
  .qr-select-arrow { font-size: 10px; color: var(--text-tertiary); }
  .qr-dropdown { position: absolute; top: 100%; left: 0; right: 0; background: var(--bg-card); border: 1px solid rgba(34, 211, 238, .22); border-radius: 8px; margin-top: 4px; z-index: 30; box-shadow: var(--shadow-lg); overflow: hidden; }
  .qr-dropdown-search { width: 100%; padding: 9px 12px; background: var(--bg-input); border: none; border-bottom: 1px solid var(--border-primary); font-size: 12px; color: var(--text-primary); outline: none; box-sizing: border-box; }
  .qr-dropdown-search::placeholder { color: var(--text-tertiary); }
  .qr-dropdown-list { max-height: 300px; overflow-y: auto; }
  .qr-dropdown-item { display: flex; align-items: center; gap: 10px; padding: 10px 12px; background: transparent; border: none; border-bottom: 1px solid var(--border-secondary); width: 100%; cursor: pointer; text-align: left; color: var(--text-primary); }
  .qr-dropdown-item:hover { background: rgba(34, 211, 238, .08); }
  .qr-dropdown-item.item-active { background: rgba(20, 184, 166, .18); }
  .qr-item-number { display: inline-grid; place-items: center; min-width: 38px; height: 24px; border-radius: 7px; border: 1px solid rgba(34,211,238,.22); background: rgba(34,211,238,.08); color: var(--accent-primary); font-family: var(--theme-font-family-mono); font-size: 10px; font-weight: 900; flex-shrink: 0; }
  .qr-item-type { display: inline-grid; place-items: center; min-width: 36px; height: 22px; border-radius: 999px; border: 1px solid rgba(96,165,250,.28); background: rgba(96,165,250,.12); color: #60a5fa; font-size: 10px; font-weight: 900; flex-shrink: 0; }
  .qr-item-type.check-type { border-color: rgba(52,211,153,.28); background: rgba(52,211,153,.12); color: #34d399; }
  .qr-item-icon { display: inline-flex; align-items: center; justify-content: center; width: 34px; height: 34px; border-radius: 8px; background: rgba(34,211,238,.12); border: 1px solid rgba(34,211,238,.18); color: #67e8f9; font-family: var(--theme-font-family-mono); font-size: 11px; font-weight: 900; flex-shrink: 0; }
  .qr-item-info { flex: 1; }
  .qr-item-name { display: block; font-size: 13px; font-weight: 600; color: var(--text-primary); }
  .qr-item-id { display: block; font-size: 10px; color: var(--text-tertiary); font-family: var(--theme-font-family-mono); }
  .qr-item-desc { display: block; font-size: 11px; color: var(--text-secondary); margin-top: 2px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .qr-btn { padding: 8px 16px; border: none; border-radius: 8px; font-size: 12px; font-weight: 600; cursor: pointer; }
  .qr-empty { padding: 18px 12px; color: var(--text-tertiary); font-size: 12px; text-align: center; }
  .qr-start { background: #3b82f6; color: white; }
  .qr-start:hover { background: #2563eb; }
  .qr-start:disabled { opacity: 0.5; cursor: not-allowed; }
  .main-grid { display: grid; grid-template-columns: minmax(0, 1fr) minmax(300px, 360px); gap: 12px; }
  .left-col, .right-col { display: flex; flex-direction: column; gap: 12px; }
  .resource-section, .load-section, .sys-info-section, .proc-section { background: var(--bg-card); border: 1px solid var(--border-primary); border-radius: 10px; padding: 12px; }
  h3 { font-size: 12px; font-weight: 700; color: var(--text-secondary); margin: 0 0 10px; text-transform: uppercase; letter-spacing: 0.5px; }
  .load-hint, .proc-hint { font-size: 10px; color: var(--text-tertiary); font-weight: normal; text-transform: none; }
  .resource-bars { display: flex; flex-direction: column; gap: 8px; }
  .res-header { display: flex; justify-content: space-between; font-size: 11px; color: var(--text-secondary); margin-bottom: 3px; }
  .res-bar { height: 5px; background: var(--bg-secondary); border-radius: 3px; overflow: hidden; }
  .res-fill { height: 100%; border-radius: 3px; transition: width 0.5s; }
  .res-detail { font-size: 10px; color: var(--text-tertiary); margin-top: 2px; }
  .trend-card { position: relative; overflow: hidden; background: linear-gradient(180deg, rgba(17, 24, 39, .96), rgba(8, 13, 24, .98)); border-color: rgba(34, 211, 238, .14); }
  .trend-card::before { content: ''; position: absolute; inset: 0; background: radial-gradient(circle at 20% 0%, rgba(34,211,238,.14), transparent 34%), radial-gradient(circle at 90% 20%, rgba(167,139,250,.12), transparent 30%); pointer-events: none; }
  .trend-head { position: relative; z-index: 1; display: flex; align-items: center; justify-content: space-between; gap: 10px; margin-bottom: 10px; }
  .trend-head h3 { margin: 0; }
  .trend-values { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; justify-content: flex-end; }
  .trend-range { display: inline-flex; gap: 2px; padding: 2px; border-radius: 8px; border: 1px solid rgba(148, 163, 184, .14); background: rgba(15, 23, 42, .62); }
  .trend-range button { min-height: 24px; padding: 0 8px; border: none; border-radius: 6px; background: transparent; color: var(--text-tertiary); font-size: 10px; font-weight: 800; cursor: pointer; }
  .trend-range button:hover { color: var(--text-primary); background: rgba(148, 163, 184, .08); }
  .trend-range button.active { color: #67e8f9; background: rgba(34, 211, 238, .12); box-shadow: inset 0 0 14px rgba(34, 211, 238, .08); }
  .trend-range button:disabled { opacity: .7; cursor: wait; }
  .trend-chip { display: inline-flex; align-items: center; gap: 4px; padding: 4px 8px; border-radius: 999px; border: 1px solid rgba(148, 163, 184, .16); background: rgba(15, 23, 42, .55); color: var(--text-secondary); font-family: var(--theme-font-family-mono); font-size: 10px; font-weight: 800; white-space: nowrap; }
  .trend-chip.cpu { color: #67e8f9; border-color: rgba(34,211,238,.28); }
  .trend-chip.mem { color: #c4b5fd; border-color: rgba(167,139,250,.28); }
  .trend-chip.load { color: #fbbf24; border-color: rgba(245,158,11,.28); }
  .trend-chip.rx { color: #34d399; border-color: rgba(52,211,153,.28); }
  .trend-chip.tx { color: #60a5fa; border-color: rgba(96,165,250,.28); }
  .trend-chip.total { color: var(--text-primary); }
  .trend-chart { position: relative; z-index: 1; height: 168px; padding: 8px; border: 1px solid rgba(148, 163, 184, .12); border-radius: 10px; background: linear-gradient(180deg, rgba(2, 6, 23, .46), rgba(15, 23, 42, .26)); box-shadow: inset 0 0 28px rgba(34, 211, 238, .04); }
  .trend-chart svg { display: block; width: 100%; height: 100%; overflow: visible; }
  .trend-loading { position: absolute; inset: 8px; display: grid; place-items: center; gap: 8px; border-radius: 8px; background: rgba(2, 6, 23, .58); backdrop-filter: blur(2px); color: #67e8f9; font-size: 11px; font-weight: 800; pointer-events: none; }
  .trend-loading span { width: 28px; height: 28px; border-radius: 50%; border: 2px solid rgba(34, 211, 238, .18); border-top-color: #22d3ee; box-shadow: 0 0 24px rgba(34,211,238,.18); animation: spin .8s linear infinite; }
  .trend-loading em { font-style: normal; font-family: var(--theme-font-family-mono); }
  .trend-tooltip { position: absolute; z-index: 6; min-width: 124px; pointer-events: none; transform: translate(-50%, calc(-100% - 12px)); padding: 9px 10px; border-radius: 9px; border: 1px solid rgba(34, 211, 238, .28); background: rgba(3, 7, 18, .94); color: #e2e8f0; box-shadow: 0 16px 36px rgba(0,0,0,.38), 0 0 28px rgba(34,211,238,.12); font-family: var(--theme-font-family-mono); }
  .trend-tooltip::after { content: ''; position: absolute; left: 50%; bottom: -7px; width: 10px; height: 10px; transform: translateX(-50%) rotate(45deg); background: rgba(3, 7, 18, .94); border-right: 1px solid rgba(34, 211, 238, .22); border-bottom: 1px solid rgba(34, 211, 238, .22); }
  .trend-tooltip strong { display: block; margin-bottom: 5px; color: #67e8f9; font-size: 11px; }
  .trend-tooltip span { display: block; margin-top: 3px; color: #cbd5e1; font-size: 11px; }
  .trend-axis { position: absolute; left: 10px; right: 10px; bottom: 6px; display: flex; justify-content: space-between; color: var(--text-tertiary); font-family: var(--theme-font-family-mono); font-size: 9px; pointer-events: none; }
  .chart-grid line { stroke: rgba(148, 163, 184, .14); stroke-width: 1; vector-effect: non-scaling-stroke; }
  .trend-line { fill: none; stroke-width: 2.8; stroke-linecap: round; stroke-linejoin: round; vector-effect: non-scaling-stroke; filter: drop-shadow(0 0 8px currentColor); }
  .cpu-line { stroke: url(#trendCpu); color: #22d3ee; }
  .mem-line { stroke: url(#trendMem); color: #a78bfa; }
  .load-line { stroke: url(#trendLoad); color: #f59e0b; }
  .rx-line { stroke: url(#trendRx); color: #34d399; stroke-dasharray: 7 5; }
  .tx-line { stroke: url(#trendTx); color: #60a5fa; stroke-dasharray: 7 5; }
  .info-rows { display: flex; flex-direction: column; gap: 4px; }
  .info-row { display: flex; justify-content: space-between; font-size: 11px; }
  .info-row span:first-child { color: var(--text-tertiary); }
  .info-row span:last-child { color: var(--text-primary); font-family: var(--theme-font-family-mono); }
  .ip-value { color: #3b82f6 !important; font-weight: 600; }
  .proc-section { min-height: 264px; display: flex; flex-direction: column; }
  .proc-table { flex: 1; }
  .proc-more-btn { width: 100%; padding: 8px; background: var(--bg-secondary); border: 1px solid var(--border-primary); border-radius: 6px; color: var(--text-secondary); font-size: 11px; cursor: pointer; margin-top: 8px; }
  .proc-more-btn:hover { background: var(--bg-hover); color: var(--text-primary); }
  .modal-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.6); z-index: 100; display: flex; align-items: center; justify-content: center; }
  .modal { width: 500px; max-width: 90vw; max-height: 80vh; background: var(--bg-card); border: 1px solid var(--border-primary); border-radius: 14px; display: flex; flex-direction: column; overflow: hidden; }
  .modal-xl { width: 95vw; max-width: 1200px; height: 85vh; }
  .modal-header { display: flex; justify-content: space-between; align-items: center; padding: 14px 18px; border-bottom: 1px solid var(--border-primary); }
  .modal-header h3 { margin: 0; font-size: 15px; color: var(--text-primary); }
  .modal-close { background: none; border: none; color: var(--text-tertiary); font-size: 18px; cursor: pointer; padding: 4px 8px; border-radius: 6px; }
  .modal-close:hover { background: var(--bg-hover); }
  .proc-table { width: 100%; }
  .proc-header { display: grid; grid-template-columns: 40px 1fr 80px 100px; gap: 8px; padding: 8px 0; border-bottom: 1px solid var(--border-primary); font-size: 11px; font-weight: 600; color: var(--text-tertiary); text-transform: uppercase; }
  .proc-row { display: grid; grid-template-columns: 40px 1fr 80px 100px; gap: 8px; padding: 6px 0; border-bottom: 1px solid var(--border-secondary); font-size: 12px; }
  .proc-row:hover { background: var(--bg-hover); }
  .proc-row.proc-warning { background: rgba(245,158,11,0.05); }
  .proc-row.proc-critical { background: rgba(239,68,68,0.05); }
  .proc-col-rank { color: var(--text-tertiary); font-family: var(--theme-font-family-mono); }
  .proc-col-name { color: var(--text-primary); font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .proc-col-cpu { font-weight: 600; font-family: var(--theme-font-family-mono); }
  .proc-col-mem { color: var(--text-secondary); font-family: var(--theme-font-family-mono); }
  .process-table { width: 100%; }
  .process-header { display: grid; grid-template-columns: 40px 80px 120px 1fr 80px 100px 80px; gap: 8px; padding: 8px 0; border-bottom: 1px solid var(--border-primary); font-size: 11px; font-weight: 600; color: var(--text-tertiary); text-transform: uppercase; }
  .process-sort { border: none; background: transparent; color: inherit; font: inherit; text-transform: inherit; padding: 0; text-align: left; cursor: pointer; }
  .process-sort:hover { color: var(--text-primary); }
  .process-list { max-height: 60vh; overflow-y: auto; }
  .process-row { display: grid; grid-template-columns: 40px 80px 120px 1fr 80px 100px 80px; gap: 8px; padding: 6px 0; border-bottom: 1px solid var(--border-secondary); font-size: 12px; }
  .process-row:hover { background: var(--bg-hover); }
  .process-row.proc-warning { background: rgba(245,158,11,0.05); }
  .process-row.proc-critical { background: rgba(239,68,68,0.05); }
  .col-rank { color: var(--text-tertiary); font-family: var(--theme-font-family-mono); }
  .col-pid { color: var(--text-secondary); font-family: var(--theme-font-family-mono); }
  .col-name { color: var(--text-primary); font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .col-cmd { color: var(--text-tertiary); font-size: 11px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .col-cpu { font-weight: 600; font-family: var(--theme-font-family-mono); }
  .col-mem { color: var(--text-secondary); font-family: var(--theme-font-family-mono); }
  .col-status { font-size: 11px; color: var(--text-tertiary); }
  .modal-actions { display: flex; align-items: center; gap: 8px; }
  .modal-refresh { display: flex; align-items: center; gap: 4px; padding: 4px 10px; background: var(--bg-secondary); border: 1px solid var(--border-primary); border-radius: 6px; color: var(--text-secondary); font-size: 12px; cursor: pointer; }
  .modal-refresh:hover { background: var(--bg-hover); color: var(--text-primary); }
  .modal-body { flex: 1; overflow-y: auto; padding: 12px; }
  .stat-list-head { display: grid; grid-template-columns: 1fr 92px 42px; gap: 10px; padding: 0 10px 6px; color: var(--text-tertiary); font-size: 11px; }
  .stat-list-head button { border: none; background: transparent; color: inherit; font: inherit; padding: 0; text-align: left; cursor: pointer; }
  .stat-list-head button:hover { color: var(--text-primary); }
  .stat-list { display: flex; flex-direction: column; gap: 4px; }
  .stat-list-item { display: flex; align-items: center; gap: 10px; padding: 8px 10px; background: var(--bg-secondary); border-radius: 6px; }
  .stat-list-name { flex: 1; font-size: 13px; color: var(--text-primary); }
  .stat-list-time { font-size: 11px; color: var(--text-tertiary); }
  .stat-list-badge { width: 20px; height: 20px; border-radius: 50%; display: flex; align-items: center; justify-content: center; font-size: 10px; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (max-width: 900px) { .main-grid { grid-template-columns: 1fr; } }
</style>
