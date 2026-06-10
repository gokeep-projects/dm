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
  let topProcessSortKey = $state('cpu_usage');
  let topProcessSortDir = $state('desc');
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
  let trendVisible = $state({ cpu: true, mem: true, load: true, rx: true, tx: true });

  const SAMPLE_MIN_MS = 14 * 1000;
  const TOP_PROCESS_DISPLAY_LIMIT = 10;
  const trendSeries = [
    { key: 'cpu', label: 'CPU', cls: 'cpu' },
    { key: 'mem', label: '内存', cls: 'mem' },
    { key: 'load', label: '负载', cls: 'load' },
    { key: 'rx', label: '接收', cls: 'rx' },
    { key: 'tx', label: '发送', cls: 'tx' },
  ];
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

  function isObject(value) {
    return value && typeof value === 'object' && !Array.isArray(value);
  }

  function hasSystemPayload(value) {
    return isObject(value) && (
      value.hostname ||
      value.os ||
      Number(value.cpu_count || 0) > 0 ||
      Number(value.memory_total || 0) > 0 ||
      Number(value.process_count || 0) > 0
    );
  }

  function mergeTopProcesses(nextRows, prevRows = []) {
    if (!Array.isArray(nextRows) || nextRows.length === 0) return prevRows;
    const prevByPid = new Map((prevRows || []).map(p => [String(p.pid), p]));
    return nextRows.map(row => {
      const prev = prevByPid.get(String(row.pid)) || {};
      return {
        ...prev,
        ...row,
        path: row.path || prev.path || row.cmd || prev.cmd || row.exe_path || prev.exe_path || row.name || prev.name || 'unknown',
        status: row.status || prev.status || '未知',
        ports: Array.isArray(row.ports) && row.ports.length ? row.ports : (prev.ports || []),
      };
    });
  }

  function mergeSystemPayload(next) {
    if (!hasSystemPayload(next)) return;
    const prev = sys || {};
    sys = {
      ...prev,
      ...next,
      load_avg: isObject(next.load_avg) ? { ...(prev.load_avg || {}), ...next.load_avg } : prev.load_avg,
      networks: Array.isArray(next.networks) && next.networks.length ? next.networks : (prev.networks || []),
      disks: Array.isArray(next.disks) && next.disks.length ? next.disks : (prev.disks || []),
      top_processes: mergeTopProcesses(next.top_processes, prev.top_processes),
    };
    recordMetricSample(sys);
  }

  function mergeStatsPayload(next) {
    if (!isObject(next)) return;
    const prev = stats || {};
    stats = {
      ...prev,
      ...next,
      categories: isObject(next.categories) ? next.categories : (prev.categories || {}),
      recent_execs: Array.isArray(next.recent_execs) ? next.recent_execs : (prev.recent_execs || []),
    };
  }

  function updateListIfReady(payload, key, current) {
    const rows = payload?.[key];
    if (Array.isArray(rows) && (rows.length > 0 || current.length === 0)) return rows;
    return current;
  }

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
          mergeSystemPayload(d.system);
          mergeStatsPayload(d.stats);
          scripts = updateListIfReady(d, 'scripts', scripts);
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
      mergeStatsPayload(s);
      mergeSystemPayload(i);
      if (sc) scripts = updateListIfReady(sc, 'scripts', scripts);
      if (ck) checks = updateListIfReady(ck, 'checks', checks);
    } catch (e) { console.warn('加载仪表盘数据失败:', e); }
    lastUpdate = new Date();
    updateCount++;
    setTimeout(() => isRefreshing = false, 300);
  }

  function fmt(b) { if (!b) return '0 B'; const k = 1024, s = ['B','KB','MB','GB','TB']; let i = 0, v = b; while (v >= k && i < s.length - 1) { v /= k; i++; } return v.toFixed(1) + ' ' + s[i]; }
  function num(n) { return Number(n || 0).toLocaleString('zh-CN'); }
  function pct(v) { return Math.max(0, Math.min(100, Number(v || 0))); }
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
      const points = (d.points || []).map(normalizeMetricPoint).filter(p => p.t >= cutoff);
      if (points.length === 0) {
        if (metricHistory.length === 0 && sys) recordMetricSample(sys, true);
        return;
      }
      mergeMetricHistory(points, cutoff);
    } catch (_) {
    } finally {
      if (showLoading && seq === metricRequestSeq) trendLoading = false;
    }
  }

  function normalizeMetricPoint(p) {
    return {
      t: Number(p.ts_ms || p.t || 0),
      timestamp: p.timestamp,
      cpu: Number(p.cpu_usage ?? p.cpu ?? 0),
      mem: Number(p.memory_usage ?? p.mem ?? 0),
      load: Number(p.load_ratio ?? p.load ?? 0),
      load_one: Number(p.load_one ?? p.load ?? 0),
      rx: Number(p.rx_bytes ?? p.rx ?? 0),
      tx: Number(p.tx_bytes ?? p.tx ?? 0),
    };
  }

  function mergeMetricHistory(points, cutoff = trendRangeStart()) {
    const byTs = new Map();
    for (const point of [...metricHistory, ...points]) {
      if (!point?.t || point.t < cutoff) continue;
      byTs.set(String(point.t), normalizeMetricPoint(point));
    }
    metricHistory = [...byTs.values()].sort((a, b) => a.t - b.t).slice(-520);
  }

  function recordMetricSample(system, force = false) {
    if (!system) return;
    const now = Date.now();
    if (!force && lastMetricSampleAt && now - lastMetricSampleAt < SAMPLE_MIN_MS) return;
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
    mergeMetricHistory([point], cutoff);
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
    if (history.length === 1) return [{ t: history[0].t, value: 0 }];
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
    const coords = points.map(p => {
      const x = pad + Math.max(0, Math.min(1, (p.t - start) / trendWindowMs())) * (width - pad * 2);
      const y = pad + (1 - Math.max(0, Math.min(1, p.value / maxValue))) * (height - pad * 2);
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    });
    if (coords.length === 1) coords.push(`${(width - pad).toFixed(1)},${coords[0].split(',')[1]}`);
    return coords.join(' ');
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

  function toggleTrendSeries(key) {
    trendVisible = { ...trendVisible, [key]: !trendVisible[key] };
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

  function changeTopProcessSort(key) {
    if (topProcessSortKey === key) topProcessSortDir = topProcessSortDir === 'asc' ? 'desc' : 'asc';
    else {
      topProcessSortKey = key;
      topProcessSortDir = key === 'cpu_usage' || key === 'memory_bytes' ? 'desc' : 'asc';
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
    if (key === 'status') return procStatus(p);
    if (key === 'cmd') return p.cmd || p.name || '';
    if (key === 'path') return procPath(p);
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

  function loadRatio() {
    return sys?.cpu_count ? Math.min(((sys.load_avg?.one || 0) / sys.cpu_count) * 100, 300) : 0;
  }

  function bootTimeText() {
    if (!sys?.boot_time) return '-';
    return new Date(Number(sys.boot_time) * 1000).toLocaleString('zh-CN', { hour12: false });
  }

  function successRate() {
    const total = Number(stats?.total_executions || 0);
    return total ? Math.round((Number(stats?.success_count || 0) / total) * 100) : 0;
  }

  function shortPath(value, max = 28) {
    const text = String(value || '-');
    if (text.length <= max) return text;
    const keep = Math.floor((max - 3) / 2);
    return text.slice(0, keep) + '...' + text.slice(-keep);
  }

  function procPath(p) {
    return p?.path || p?.cmd || p?.exe_path || p?.name || 'unknown';
  }

  function procStatus(p) {
    return p?.pid || p?.name ? '正常' : '未知';
  }

  function procStatusDetail(p) {
    return p?.status ? `进程存在，调度态: ${p.status}` : procStatus(p);
  }

  function networkKind(n) {
    if (!n?.name) return '未知';
    if (n.name === 'lo' || n.ip?.startsWith?.('127.')) return '回环';
    if (n.name.includes('docker') || n.name.includes('br-') || n.name.includes('veth') || n.name.includes('virbr')) return '虚拟';
    return '物理';
  }

  function loadLevelLabel() {
    const level = loadLevel(sys?.load_avg?.one, sys?.cpu_count);
    if (level === 'critical') return '高压';
    if (level === 'warning') return '偏高';
    return '正常';
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

  let sortedTopProcesses = $derived.by(() => {
    return [...(sys?.top_processes || [])].sort((a, b) => {
      const av = processValue(a, topProcessSortKey);
      const bv = processValue(b, topProcessSortKey);
      let cmp = typeof av === 'number' && typeof bv === 'number'
        ? av - bv
        : String(av).localeCompare(String(bv), 'zh-CN');
      if (topProcessSortDir === 'desc') cmp = -cmp;
      return cmp;
    }).slice(0, TOP_PROCESS_DISPLAY_LIMIT);
  });

  let resourceTiles = $derived.by(() => {
    const rx = latestRate('rx');
    const tx = latestRate('tx');
    const netPeak = Math.max(1, ...networkRatePoints('rx').map(p => p.value), ...networkRatePoints('tx').map(p => p.value));
    const netValue = Math.min(100, ((rx + tx) / Math.max(1, netPeak * 2)) * 100);
    return [
      {
        key: 'cpu',
        label: 'CPU',
        value: pct(sys?.cpu_usage),
        primary: `${(sys?.cpu_usage || 0).toFixed(1)}%`,
        detail: `${sys?.cpu_count || 0}核 · ${sys?.cpu_brand || '未知 CPU'}`,
      },
      {
        key: 'load',
        label: '负载',
        value: Math.min(100, loadRatio()),
        primary: `${(sys?.load_avg?.one || 0).toFixed(2)}`,
        detail: `1/5/15 分钟 ${sys?.load_avg?.one?.toFixed?.(2) || '0.00'} / ${sys?.load_avg?.five?.toFixed?.(2) || '0.00'} / ${sys?.load_avg?.fifteen?.toFixed?.(2) || '0.00'}`,
      },
      {
        key: 'mem',
        label: '内存',
        value: pct(sys?.memory_usage),
        primary: `${(sys?.memory_usage || 0).toFixed(1)}%`,
        detail: `${fmt(sys?.memory_used)} / ${fmt(sys?.memory_total)}`,
      },
      {
        key: 'swap',
        label: '交换',
        value: pct(sys?.swap_usage),
        primary: `${(sys?.swap_usage || 0).toFixed(1)}%`,
        detail: `${fmt(sys?.swap_used)} / ${fmt(sys?.swap_total)}`,
      },
      {
        key: 'disk',
        label: '磁盘',
        value: pct(sys?.disk_usage),
        primary: `${(sys?.disk_usage || 0).toFixed(1)}%`,
        detail: `${fmt(sys?.disk_used)} / ${fmt(sys?.disk_total)}`,
      },
      {
        key: 'net',
        label: '网络',
        value: netValue,
        primary: `${fmt(rx + tx)}/s`,
        detail: `RX ${fmt(rx)}/s · TX ${fmt(tx)}/s`,
      },
    ];
  });

  let categoryRows = $derived.by(() => Object.entries(stats?.categories || {})
    .map(([name, count]) => ({ name, count: Number(count || 0) }))
    .sort((a, b) => b.count - a.count)
    .slice(0, 12));

  let diskRows = $derived.by(() => [...(sys?.disks || [])]
    .sort((a, b) => Number(b.usage || 0) - Number(a.usage || 0))
    .slice(0, 10));

  let networkRows = $derived.by(() => [...(sys?.networks || [])]
    .sort((a, b) => {
      const ap = networkKind(a) === '物理' ? 0 : networkKind(a) === '回环' ? 1 : 2;
      const bp = networkKind(b) === '物理' ? 0 : networkKind(b) === '回环' ? 1 : 2;
      return ap - bp || String(a.name).localeCompare(String(b.name), 'zh-CN');
    })
    .slice(0, 10));

  let recentExecRows = $derived.by(() => [...(stats?.recent_execs || [])].slice(0, 12));

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
  <section class="hero-console">
    <div class="health-orbit">
      {#if sys}
        {@const h = healthScore()}
        <div class="orbit-ring" style="--score:{h.score};--health:{healthColor(h.level)}">
          <strong>{h.score}</strong>
          <span>{h.label}</span>
        </div>
      {:else}
        <div class="orbit-ring"><strong>--</strong><span>加载中</span></div>
      {/if}
    </div>
    <div class="hero-main">
      <div class="hero-title">
        <span class="status-pill" class:online={wsConnected}><i></i>{wsConnected ? '实时链路在线' : '接口轮询模式'}</span>
        <h1>{sys?.hostname || 'DM 控制台'}</h1>
        <p>{sys?.os || '-'} · {sys?.kernel || '-'} · {sys?.arch || '-'} · 启动 {bootTimeText()}</p>
      </div>
      <div class="hero-kpis">
        <div class="kpi-card"><span>维护脚本</span><strong>{num(stats?.total_scripts)}</strong><em>{num(categoryRows.length)} 类</em></div>
        <div class="kpi-card"><span>执行总量</span><strong>{num(stats?.total_executions)}</strong><em>成功率 {successRate()}%</em></div>
        <div class="kpi-card good"><span>成功</span><strong>{num(stats?.success_count)}</strong><em>最近 {recentExecRows.length} 条</em></div>
        <div class="kpi-card bad"><span>失败</span><strong>{num(stats?.failure_count)}</strong><em>{stats?.failure_count ? '需要关注' : '无异常记录'}</em></div>
        <div class="kpi-card"><span>进程</span><strong>{num(sys?.process_count)}</strong><em>{sys?.cpu_count || 0} 核 CPU</em></div>
        <div class="kpi-card load" data-level={loadLevel(sys?.load_avg?.one, sys?.cpu_count)}><span>负载</span><strong>{(sys?.load_avg?.one || 0).toFixed(2)}</strong><em>{loadLevelLabel()} · {loadRatio().toFixed(0)}%</em></div>
      </div>
    </div>
    <div class="hero-side">
      <div class="meta-line"><span>公网/主IP</span><strong>{getPublicIp()}</strong></div>
      <div class="meta-line"><span>运行时间</span><strong>{sys?.uptime ? upt(sys.uptime) : '-'}</strong></div>
      <div class="meta-line"><span>刷新</span><strong>{lastUpdateText}</strong></div>
        <div class="refresh-select">{#each refreshOptions as opt}<button type="button" class="refresh-opt" class:active={refreshInterval === opt.value} onclick={() => setRefreshInterval(opt.value)}>{opt.label}</button>{/each}</div>
    </div>
  </section>

  <section class="quick-run-section">
    <div class="qr-header">
      <h2>快速执行</h2>
      <div class="qr-status ready">输入编号、名称、类别或 ID，直接进入执行/检查结果页</div>
    </div>
    <div class="qr-controls">
      <div class="qr-select-wrap">
        <button type="button" class="qr-select-btn" onclick={() => { showScriptSelect = !showScriptSelect; if (showScriptSelect) setTimeout(() => searchInput?.focus(), 100); }}>
          <span>{selectedQuickItem ? `${selectedQuickItem.kindLabel} · ${selectedQuickItem.name || selectedQuickItem.id}` : '选择脚本或检查...'}</span>
          <span class="qr-select-arrow">▼</span>
        </button>
        {#if showScriptSelect}
          <div class="qr-dropdown">
            <input type="text" placeholder="搜索编号、名称、ID、类别..." bind:value={scriptSearch} bind:this={searchInput} class="qr-dropdown-search" oninput={() => dropdownIndex = 0} onkeydown={(e) => {
              if (e.key === 'ArrowDown') { e.preventDefault(); moveDropdownSelection(1); }
              else if (e.key === 'ArrowUp') { e.preventDefault(); moveDropdownSelection(-1); }
              else if (e.key === 'Enter') { e.preventDefault(); if (filteredQuickItems[dropdownIndex]) { quickRunItem = filteredQuickItems[dropdownIndex].value; showScriptSelect = false; } }
              else if (e.key === 'Escape') { showScriptSelect = false; }
            }} />
            <div class="qr-dropdown-list" bind:this={dropdownList}>
              {#each filteredQuickItems as s, i}
                <button type="button" class="qr-dropdown-item" class:item-active={i === dropdownIndex} onclick={() => { quickRunItem = s.value; showScriptSelect = false; }}>
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
      <button type="button" class="qr-btn qr-start" onclick={() => startQuickRun(quickRunItem)} disabled={!quickRunItem}>执行并查看结果</button>
    </div>
  </section>

  <section class="dense-grid">
    <div class="panel resource-panel">
      <div class="panel-head"><h3>资源雷达</h3><span>{sys?.cpu_brand || '-'}</span></div>
      <div class="resource-matrix">
        {#each resourceTiles as item}
          <div class="resource-tile {item.key}">
            <div class="tile-top"><span>{item.label}</span><strong style="color:{barColor(item.value)}">{item.primary}</strong></div>
            <div class="res-bar"><div class="res-fill" style="width:{item.value}%;background:{barColor(item.value)}"></div></div>
            <em>{item.detail}</em>
          </div>
        {/each}
      </div>
    </div>

    <div class="panel trend-card">
      <div class="trend-head">
        <h3>系统趋势 <span class="load-hint">最近 {selectedTrendLabel()} · {timeRangeLabel()}</span></h3>
        <div class="trend-values">
          <div class="trend-range">
            {#each trendOptions as opt}
              <button type="button" class:active={trendMinutes === opt.value} disabled={trendLoading && trendMinutes === opt.value} onclick={() => setTrendMinutes(opt.value)}>{opt.label}</button>
            {/each}
          </div>
          {#each trendSeries as item}
            <button
              type="button"
              class="trend-chip {item.cls}"
              class:muted={!trendVisible[item.key]}
              onclick={() => toggleTrendSeries(item.key)}
              aria-pressed={trendVisible[item.key]}>
              {item.label}
              {#if item.key === 'cpu'}{(sys?.cpu_usage || 0).toFixed(1)}%{/if}
              {#if item.key === 'mem'}{(sys?.memory_usage || 0).toFixed(1)}%{/if}
              {#if item.key === 'load'}{sys?.load_avg?.one?.toFixed(2) || '0.00'}{/if}
              {#if item.key === 'rx'}↓ {fmt(latestRate('rx'))}/s{/if}
              {#if item.key === 'tx'}↑ {fmt(latestRate('tx'))}/s{/if}
            </button>
          {/each}
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
          {#if trendVisible.cpu}<polyline class="trend-line cpu-line" points={seriesPath('cpu', 100)} />{/if}
          {#if trendVisible.mem}<polyline class="trend-line mem-line" points={seriesPath('mem', 100)} />{/if}
          {#if trendVisible.load}<polyline class="trend-line load-line" points={seriesPath('load', 300)} />{/if}
          {#if trendVisible.rx}<polyline class="trend-line rx-line" points={netPath('rx')} />{/if}
          {#if trendVisible.tx}<polyline class="trend-line tx-line" points={netPath('tx')} />{/if}
        </svg>
        {#if trendLoading}
          <div class="trend-loading"><span></span><em>切换 {selectedTrendLabel()} 数据</em></div>
        {/if}
        {#if trendHover?.chart === 'system'}
          <div class="trend-tooltip" style="left:{trendHover.x}px;top:{trendHover.y}px">
            <strong>{hoverTime(trendHover.point)}</strong>
            {#if trendVisible.cpu}<span>CPU {trendHover.point.cpu.toFixed(1)}%</span>{/if}
            {#if trendVisible.mem}<span>内存 {trendHover.point.mem.toFixed(1)}%</span>{/if}
            {#if trendVisible.load}<span>负载 {trendHover.point.load_one?.toFixed?.(2) || '-'}</span>{/if}
            {#if trendVisible.rx}<span>接收 {fmt(hoverRate('rx'))}/s</span>{/if}
            {#if trendVisible.tx}<span>发送 {fmt(hoverRate('tx'))}/s</span>{/if}
          </div>
        {/if}
        <div class="trend-axis">
          <span>{new Date(trendRangeStart()).toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit', hour12: false })}</span>
          <span>{new Date(trendRangeStart() + trendWindowMs() / 2).toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit', hour12: false })}</span>
          <span>{new Date(trendRangeEnd).toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit', hour12: false })}</span>
        </div>
      </div>
    </div>

    <div class="panel system-panel">
      <div class="panel-head"><h3>系统概要</h3><span>{sys?.hostname || '-'}</span></div>
      <div class="info-rows compact">
        <div class="info-row"><span>主IP</span><strong class="ip-value">{getPublicIp()}</strong></div>
        <div class="info-row"><span>系统</span><strong title={sys?.os || '-'}>{shortPath(sys?.os, 30)}</strong></div>
        <div class="info-row"><span>内核</span><strong>{sys?.kernel || '-'}</strong></div>
        <div class="info-row"><span>架构</span><strong>{sys?.arch || '-'} / {sys?.cpu_count || 0}核</strong></div>
        <div class="info-row"><span>运行</span><strong>{sys?.uptime ? upt(sys.uptime) : '-'}</strong></div>
        <div class="info-row"><span>网络总量</span><strong>{fmt(networkTotals().rx + networkTotals().tx)}</strong></div>
      </div>
    </div>

    <div class="panel proc-section">
      <div class="panel-head">
        <h3>TOP 进程</h3>
        <button type="button" class="panel-link" onclick={showAllProcessesPanel}>全部进程</button>
      </div>
      <div class="proc-table">
        <div class="proc-header">
          <span class="proc-col-rank">#</span>
          <button type="button" class="proc-sort proc-col-name" onclick={() => changeTopProcessSort('name')}>名称{sortMark(topProcessSortKey, topProcessSortDir, 'name')}</button>
          <button type="button" class="proc-sort proc-col-path" onclick={() => changeTopProcessSort('path')}>路径{sortMark(topProcessSortKey, topProcessSortDir, 'path')}</button>
          <button type="button" class="proc-sort proc-col-status" onclick={() => changeTopProcessSort('status')}>状态{sortMark(topProcessSortKey, topProcessSortDir, 'status')}</button>
          <button type="button" class="proc-sort proc-col-cpu" onclick={() => changeTopProcessSort('cpu_usage')}>CPU{sortMark(topProcessSortKey, topProcessSortDir, 'cpu_usage')}</button>
          <button type="button" class="proc-sort proc-col-mem" onclick={() => changeTopProcessSort('memory_bytes')}>内存{sortMark(topProcessSortKey, topProcessSortDir, 'memory_bytes')}</button>
        </div>
        <div class="proc-list">
          {#each sortedTopProcesses as p, i}
            <div class="proc-row" class:proc-warning={p.cpu_usage > 50} class:proc-critical={p.cpu_usage > 80}>
              <span class="proc-col-rank">{i + 1}</span>
              <span class="proc-col-name" title={p.name}>{p.name}</span>
              <span class="proc-col-path" title={procPath(p)}>{procPath(p)}</span>
              <span class="proc-col-status" title={procStatusDetail(p)}>{procStatus(p)}</span>
              <span class="proc-col-cpu" style="color:{barColor(p.cpu_usage)}">{p.cpu_usage.toFixed(1)}%</span>
              <span class="proc-col-mem">{fmt(p.memory_bytes)}</span>
            </div>
          {:else}
            <div class="empty-row">暂无进程数据</div>
          {/each}
        </div>
      </div>
    </div>

    <div class="panel io-panel">
      <div class="panel-head"><h3>磁盘 / 网卡</h3><span>{diskRows.length} 磁盘 · {networkRows.length} 网卡</span></div>
      <div class="split-lists">
        <div class="mini-list">
          {#each diskRows as d}
            <div class="mini-row" title={`${d.mount_point} ${d.fs_type || ''}`}>
              <span>{shortPath(d.mount_point || d.name, 18)}</span>
              <strong style="color:{barColor(d.usage)}">{Number(d.usage || 0).toFixed(1)}%</strong>
              <em>{fmt(d.used)} / {fmt(d.total)}</em>
            </div>
          {:else}
            <div class="empty-row">暂无磁盘数据</div>
          {/each}
        </div>
        <div class="mini-list">
          {#each networkRows as n}
            <div class="mini-row" title={`${n.name} ${n.ip || ''} ${n.mac || ''}`}>
              <span>{n.name}<small>{networkKind(n)}</small></span>
              <strong>{n.ip || '-'}</strong>
              <em>RX {fmt(n.received_bytes)} · TX {fmt(n.transmitted_bytes)}</em>
            </div>
          {:else}
            <div class="empty-row">暂无网卡数据</div>
          {/each}
        </div>
      </div>
    </div>
  </section>
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
            <button class="process-sort" onclick={() => changeProcessSort('path')}>路径 / 命令{sortMark(processSortKey, processSortDir, 'path')}</button>
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
                <span class="col-cmd" title={p.path || p.cmd || p.name}>{p.path || p.cmd || p.name}</span>
                <span class="col-cpu" style="color:{barColor(p.cpu_usage)}">{p.cpu_usage.toFixed(1)}%</span>
                <span class="col-mem">{fmt(p.memory_bytes)}</span>
                <span class="col-status" title={procStatusDetail(p)}>{procStatus(p)}</span>
              </div>
            {/each}
          </div>
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .dashboard {
    width: 100%;
    max-width: 100%;
    height: 100%;
    min-height: 0;
    margin: 0;
    overflow: hidden;
    display: grid;
    grid-template-rows: minmax(112px, .64fr) minmax(66px, auto) minmax(0, 4.9fr);
    gap: 8px;
    box-sizing: border-box;
    color: var(--text-primary);
  }
  .hero-console,
  .quick-run-section,
  .panel {
    position: relative;
    overflow: hidden;
    border: 1px solid rgba(34, 211, 238, .14);
    background:
      linear-gradient(135deg, rgba(9, 14, 28, .96), rgba(8, 13, 24, .92)),
      radial-gradient(circle at 80% 0%, rgba(34, 211, 238, .12), transparent 34%);
    box-shadow: 0 18px 46px rgba(2, 6, 23, .26), inset 0 0 36px rgba(34, 211, 238, .035);
  }
  .hero-console::before,
  .panel::before {
    content: '';
    position: absolute;
    inset: 0;
    background-image:
      linear-gradient(rgba(34, 211, 238, .04) 1px, transparent 1px),
      linear-gradient(90deg, rgba(34, 211, 238, .04) 1px, transparent 1px);
    background-size: 34px 34px;
    opacity: .36;
    pointer-events: none;
  }
  .hero-console { display: grid; grid-template-columns: 96px minmax(0, 1fr) 212px; gap: 10px; align-items: center; min-height: 0; height: 100%; margin-bottom: 0; padding: 10px; border-radius: 12px; }
  .health-orbit { position: relative; z-index: 1; display: grid; place-items: center; }
  .orbit-ring {
    --score: 0;
    --health: #22d3ee;
    width: 88px;
    aspect-ratio: 1;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background:
      radial-gradient(circle at center, rgba(2, 6, 23, .96) 55%, transparent 56%),
      conic-gradient(var(--health) calc(var(--score) * 1%), rgba(148, 163, 184, .16) 0);
    box-shadow: 0 0 34px color-mix(in srgb, var(--health) 28%, transparent), inset 0 0 22px rgba(255,255,255,.04);
  }
  .orbit-ring strong { margin-top: 10px; font-family: var(--theme-font-family-mono); font-size: 26px; line-height: 1; }
  .orbit-ring span { margin-top: -27px; color: var(--text-secondary); font-size: 10px; font-weight: 800; }
  .hero-main { position: relative; z-index: 1; min-width: 0; }
  .hero-title { display: flex; align-items: baseline; gap: 10px; flex-wrap: wrap; margin-bottom: 8px; }
  .hero-title h1 { margin: 0; min-width: 160px; font-size: 20px; line-height: 1.08; letter-spacing: 0; color: #f8fafc; }
  .hero-title p { flex: 1 1 420px; margin: 0; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--text-tertiary); font-size: 11px; }
  .status-pill { display: inline-flex; align-items: center; gap: 6px; height: 24px; padding: 0 9px; border-radius: 999px; border: 1px solid rgba(148, 163, 184, .18); color: var(--text-secondary); background: rgba(15, 23, 42, .62); font-size: 11px; font-weight: 800; }
  .status-pill i { width: 7px; height: 7px; border-radius: 50%; background: #64748b; }
  .status-pill.online { color: #67e8f9; border-color: rgba(34, 211, 238, .28); }
  .status-pill.online i { background: #22d3ee; box-shadow: 0 0 14px rgba(34,211,238,.75); animation: pulse 1.5s ease-in-out infinite; }
  .hero-kpis { display: grid; grid-template-columns: repeat(6, minmax(0, 1fr)); gap: 8px; }
  .kpi-card {
    min-width: 0;
    padding: 7px 8px;
    border: 1px solid rgba(148, 163, 184, .14);
    border-radius: 9px;
    background: rgba(15, 23, 42, .58);
    color: inherit;
    text-align: left;
  }
  .kpi-card span,
  .kpi-card em { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--text-tertiary); font-size: 10px; font-style: normal; }
  .kpi-card strong { display: block; margin: 3px 0 2px; font-family: var(--theme-font-family-mono); font-size: 18px; line-height: 1; color: #e2e8f0; }
  .kpi-card.good strong { color: #34d399; }
  .kpi-card.bad strong { color: #fb7185; }
  .kpi-card.load[data-level="warning"] strong { color: #fbbf24; }
  .kpi-card.load[data-level="critical"] strong { color: #fb7185; }
  .hero-side { position: relative; z-index: 1; display: flex; flex-direction: column; gap: 5px; min-width: 0; }
  .meta-line { display: grid; grid-template-columns: 68px minmax(0, 1fr); align-items: center; gap: 7px; min-height: 22px; padding: 0 8px; border-radius: 7px; background: rgba(2, 6, 23, .38); border: 1px solid rgba(148, 163, 184, .1); }
  .meta-line span { color: var(--text-tertiary); font-size: 10px; }
  .meta-line strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: #e2e8f0; font-family: var(--theme-font-family-mono); font-size: 11px; }
  .refresh-select { display: grid; grid-template-columns: repeat(4, 1fr); gap: 3px; padding: 3px; border-radius: 8px; background: rgba(2, 6, 23, .5); border: 1px solid rgba(148, 163, 184, .12); }
  .refresh-opt { min-height: 21px; border: none; border-radius: 6px; background: transparent; color: var(--text-tertiary); font-size: 10px; font-weight: 800; }
  .refresh-opt.active { background: rgba(34, 211, 238, .13); color: #67e8f9; box-shadow: inset 0 0 18px rgba(34,211,238,.08); }
  .quick-run-section { position: relative; z-index: 60; overflow: visible; min-height: 0; padding: 8px 10px; margin-bottom: 0; border-radius: 10px; }
  .qr-header { position: relative; z-index: 2; display: flex; align-items: center; justify-content: space-between; gap: 10px; margin-bottom: 6px; }
  .qr-header h2 { margin: 0; color: #e2e8f0; font-size: 13px; line-height: 1; }
  .qr-status { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--text-tertiary); font-size: 10px; }
  .qr-controls { position: relative; z-index: 70; display: grid; grid-template-columns: minmax(0, 1fr) 132px; gap: 8px; align-items: center; }
  .qr-select-wrap { position: relative; z-index: 80; min-width: 0; }
  .qr-select-btn { width: 100%; min-height: 32px; display: flex; justify-content: space-between; align-items: center; gap: 10px; padding: 0 11px; border: 1px solid rgba(34,211,238,.18); border-radius: 8px; background: rgba(2, 6, 23, .44); color: var(--text-primary); font-size: 12px; text-align: left; }
  .qr-select-btn span:first-child { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .qr-select-arrow { color: #67e8f9; font-size: 10px; }
  .qr-dropdown { position: absolute; top: calc(100% + 5px); left: 0; right: 0; z-index: 220; overflow: hidden; border: 1px solid rgba(34, 211, 238, .28); border-radius: 9px; background: rgba(4, 9, 21, .98); box-shadow: 0 22px 60px rgba(0,0,0,.42), 0 0 42px rgba(34,211,238,.12); }
  .qr-dropdown-search { width: 100%; box-sizing: border-box; padding: 10px 12px; border: none; border-bottom: 1px solid rgba(148,163,184,.14); outline: none; background: rgba(15, 23, 42, .9); color: var(--text-primary); font-size: 12px; }
  .qr-dropdown-list { max-height: 308px; overflow-y: auto; }
  .qr-dropdown-item { display: grid; grid-template-columns: 42px 42px 38px minmax(0, 1fr); align-items: center; gap: 8px; width: 100%; min-height: 48px; padding: 7px 10px; border: none; border-bottom: 1px solid rgba(148,163,184,.08); background: transparent; color: var(--text-primary); text-align: left; }
  .qr-dropdown-item:hover,
  .qr-dropdown-item.item-active { background: rgba(34,211,238,.09); }
  .qr-item-number,
  .qr-item-type,
  .qr-item-icon { display: inline-grid; place-items: center; height: 24px; border-radius: 7px; font-family: var(--theme-font-family-mono); font-size: 10px; font-weight: 900; }
  .qr-item-number { color: #67e8f9; border: 1px solid rgba(34,211,238,.26); background: rgba(34,211,238,.08); }
  .qr-item-type { color: #93c5fd; border: 1px solid rgba(96,165,250,.24); background: rgba(96,165,250,.1); }
  .qr-item-type.check-type { color: #34d399; border-color: rgba(52,211,153,.24); background: rgba(52,211,153,.1); }
  .qr-item-icon { color: #c4b5fd; border: 1px solid rgba(167,139,250,.22); background: rgba(167,139,250,.1); }
  .qr-item-info { min-width: 0; }
  .qr-item-name,
  .qr-item-id,
  .qr-item-desc { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .qr-item-name { color: #f8fafc; font-size: 12px; font-weight: 800; }
  .qr-item-id { color: var(--text-tertiary); font-family: var(--theme-font-family-mono); font-size: 10px; }
  .qr-item-desc { margin-top: 1px; color: var(--text-secondary); font-size: 10px; }
  .qr-empty { padding: 18px 12px; color: var(--text-tertiary); font-size: 12px; text-align: center; }
  .qr-btn { min-height: 32px; border: none; border-radius: 8px; font-size: 12px; font-weight: 900; }
  .qr-start { background: linear-gradient(135deg, #06b6d4, #2563eb 62%, #7c3aed); color: white; box-shadow: 0 12px 28px rgba(37,99,235,.24); }
  .qr-start:disabled { opacity: .48; }
  .dense-grid {
    position: relative;
    z-index: 1;
    display: grid;
    min-height: 0;
    height: 100%;
    grid-template-columns: minmax(218px, .72fr) minmax(0, 1.58fr) minmax(248px, .82fr);
    grid-template-areas:
      "system trend resource"
      "proc proc io";
    grid-template-rows: minmax(0, 1.05fr) minmax(0, 1fr);
    gap: 10px;
    align-items: stretch;
    overflow: hidden;
  }
  .panel { min-height: 0; border-radius: 10px; padding: 10px; }
  .panel > * { position: relative; z-index: 1; }
  .panel-head { display: flex; align-items: center; justify-content: space-between; gap: 10px; margin-bottom: 9px; }
  h3 { margin: 0; color: #cbd5e1; font-size: 11px; font-weight: 900; letter-spacing: .06em; text-transform: uppercase; }
  .panel-head span,
  .load-hint { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--text-tertiary); font-size: 10px; font-weight: 600; letter-spacing: 0; text-transform: none; }
  .panel-link { min-height: 24px; padding: 0 8px; border: 1px solid rgba(34,211,238,.2); border-radius: 7px; background: rgba(34,211,238,.08); color: #67e8f9; font-size: 10px; font-weight: 900; }
  .resource-panel { grid-area: resource; min-height: 0; display: flex; flex-direction: column; }
  .resource-matrix { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 8px; }
  .resource-matrix { flex: 1; min-height: 0; align-content: stretch; }
  .resource-tile { min-width: 0; min-height: 0; padding: 7px; border: 1px solid rgba(148,163,184,.12); border-radius: 9px; background: rgba(2, 6, 23, .38); }
  .tile-top { display: flex; justify-content: space-between; gap: 8px; margin-bottom: 5px; }
  .tile-top span { flex: 0 0 auto; color: var(--text-secondary); font-size: 11px; font-weight: 800; white-space: nowrap; }
  .tile-top strong { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; text-align: right; font-family: var(--theme-font-family-mono); font-size: 15px; }
  .resource-tile em { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; margin-top: 5px; color: var(--text-tertiary); font-size: 10px; font-style: normal; }
  .res-bar { height: 5px; overflow: hidden; border-radius: 999px; background: rgba(148,163,184,.14); }
  .res-fill { height: 100%; border-radius: inherit; transition: width .5s ease; box-shadow: 0 0 16px currentColor; }
  .trend-card { grid-area: trend; min-height: 0; display: flex; flex-direction: column; background: linear-gradient(180deg, rgba(7, 12, 25, .98), rgba(4, 9, 20, .97)); }
  .trend-head { display: grid; grid-template-columns: minmax(0, 1fr); gap: 7px; margin-bottom: 8px; }
  .trend-values { min-width: 0; display: flex; align-items: center; justify-content: flex-start; flex-wrap: wrap; gap: 5px; overflow: hidden; }
  .trend-range { display: inline-flex; gap: 2px; padding: 2px; border-radius: 8px; border: 1px solid rgba(148, 163, 184, .14); background: rgba(15, 23, 42, .62); }
  .trend-range button { min-height: 23px; padding: 0 7px; border: none; border-radius: 6px; background: transparent; color: var(--text-tertiary); font-size: 10px; font-weight: 900; }
  .trend-range button.active { color: #67e8f9; background: rgba(34, 211, 238, .12); }
  .trend-chip { min-width: 0; display: inline-flex; align-items: center; gap: 4px; max-width: 150px; padding: 4px 7px; border-radius: 999px; border: 1px solid rgba(148, 163, 184, .16); background: rgba(15, 23, 42, .55); color: var(--text-secondary); font-family: var(--theme-font-family-mono); font-size: 10px; font-weight: 900; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; transition: opacity .16s ease, filter .16s ease, background .16s ease; }
  .trend-chip.cpu { color: #67e8f9; border-color: rgba(34,211,238,.28); }
  .trend-chip.mem { color: #c4b5fd; border-color: rgba(167,139,250,.28); }
  .trend-chip.load { color: #fbbf24; border-color: rgba(245,158,11,.28); }
  .trend-chip.rx { color: #34d399; border-color: rgba(52,211,153,.28); }
  .trend-chip.tx { color: #60a5fa; border-color: rgba(96,165,250,.28); }
  .trend-chip.muted { opacity: .42; filter: grayscale(.7); background: rgba(15, 23, 42, .28); border-color: rgba(100, 116, 139, .18); }
  .trend-chart { position: relative; flex: 1; min-height: 0; padding: 8px; border: 1px solid rgba(148, 163, 184, .12); border-radius: 10px; background: linear-gradient(180deg, rgba(2, 6, 23, .56), rgba(15, 23, 42, .24)); box-shadow: inset 0 0 34px rgba(34, 211, 238, .05); }
  .trend-chart svg { display: block; width: 100%; height: 100%; overflow: visible; }
  .trend-loading { position: absolute; inset: 8px; display: grid; place-items: center; gap: 8px; border-radius: 8px; background: rgba(2, 6, 23, .58); backdrop-filter: blur(2px); color: #67e8f9; font-size: 11px; font-weight: 900; pointer-events: none; }
  .trend-loading span { width: 28px; height: 28px; border-radius: 50%; border: 2px solid rgba(34, 211, 238, .18); border-top-color: #22d3ee; box-shadow: 0 0 24px rgba(34,211,238,.18); animation: spin .8s linear infinite; }
  .trend-loading em { font-style: normal; font-family: var(--theme-font-family-mono); }
  .trend-tooltip { position: absolute; z-index: 6; min-width: 124px; pointer-events: none; transform: translate(-50%, calc(-100% - 12px)); padding: 9px 10px; border-radius: 9px; border: 1px solid rgba(34, 211, 238, .28); background: rgba(3, 7, 18, .94); color: #e2e8f0; box-shadow: 0 16px 36px rgba(0,0,0,.38), 0 0 28px rgba(34,211,238,.12); font-family: var(--theme-font-family-mono); }
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
  .system-panel { grid-area: system; min-height: 0; display: flex; flex-direction: column; }
  .info-rows { flex: 1; min-height: 0; display: flex; flex-direction: column; gap: 5px; }
  .info-row { display: grid; grid-template-columns: 52px minmax(0, 1fr); align-items: center; gap: 7px; min-height: 22px; padding: 0 7px; border-radius: 7px; background: rgba(2, 6, 23, .34); border: 1px solid rgba(148,163,184,.08); }
  .info-row span { color: var(--text-tertiary); font-size: 10px; }
  .info-row strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--text-primary); font-family: var(--theme-font-family-mono); font-size: 11px; font-weight: 800; }
  .ip-value { color: #67e8f9 !important; }
  .proc-section { grid-area: proc; min-height: 0; display: flex; flex-direction: column; }
  .proc-table { flex: 1; min-height: 0; display: flex; flex-direction: column; overflow-x: auto; overflow-y: hidden; }
  .proc-header,
  .proc-row { display: grid; grid-template-columns: 28px minmax(86px, .72fr) minmax(260px, 1.7fr) minmax(72px, .5fr) 58px 78px; align-items: center; gap: 7px; min-width: 650px; }
  .proc-header { flex: 0 0 auto; padding: 5px 0 6px; border-bottom: 1px solid rgba(148,163,184,.14); color: var(--text-tertiary); font-size: 10px; font-weight: 900; text-transform: uppercase; }
  .proc-sort { border: none; background: transparent; color: inherit; font: inherit; padding: 0; text-align: left; }
  .proc-list { flex: 1; min-height: 0; min-width: 650px; overflow-y: auto; overflow-x: hidden; padding-right: 3px; }
  .proc-row { min-height: 23px; padding: 3px 0; border-bottom: 1px solid rgba(148,163,184,.07); font-size: 11px; }
  .proc-row:hover { background: rgba(34,211,238,.06); }
  .proc-row.proc-warning { background: rgba(245,158,11,.06); }
  .proc-row.proc-critical { background: rgba(239,68,68,.07); }
  .proc-col-rank { color: var(--text-tertiary); font-family: var(--theme-font-family-mono); }
  .proc-col-name,
  .proc-col-path,
  .proc-col-status { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .proc-col-name { color: var(--text-primary); font-weight: 800; }
  .proc-col-path { color: var(--text-secondary); font-family: var(--theme-font-family-mono); font-size: 10px; }
  .proc-col-status { color: #67e8f9; font-size: 10px; font-weight: 900; }
  .proc-col-cpu,
  .proc-col-mem { font-family: var(--theme-font-family-mono); font-weight: 800; }
  .proc-col-mem { color: var(--text-secondary); }
  .io-panel { grid-area: io; min-height: 0; display: flex; flex-direction: column; }
  .split-lists { flex: 1; min-height: 0; display: grid; grid-template-rows: repeat(2, minmax(0, 1fr)); gap: 7px; }
  .mini-list { min-height: 0; height: 100%; max-height: none; overflow-y: auto; padding-right: 3px; }
  .mini-row { display: grid; align-items: center; gap: 7px; min-height: 24px; padding: 4px 6px; border-bottom: 1px solid rgba(148,163,184,.08); border-radius: 6px; background: rgba(2, 6, 23, .22); }
  .mini-row { grid-template-columns: minmax(62px, .72fr) minmax(68px, .6fr) minmax(108px, 1fr); }
  .mini-row span { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--text-primary); font-size: 11px; font-weight: 800; }
  .mini-row span small { margin-left: 5px; color: var(--text-tertiary); font-size: 9px; font-weight: 700; }
  .mini-row strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: #e2e8f0; font-family: var(--theme-font-family-mono); font-size: 11px; }
  .mini-row em { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--text-tertiary); font-size: 10px; font-style: normal; }
  .empty-row { padding: 18px 8px; color: var(--text-tertiary); font-size: 11px; text-align: center; }
  .modal-overlay { position: fixed; inset: 0; background: rgba(0,0,0,0.6); z-index: 100; display: flex; align-items: center; justify-content: center; }
  .modal { width: 500px; max-width: 90vw; max-height: 80vh; background: var(--bg-card); border: 1px solid var(--border-primary); border-radius: 14px; display: flex; flex-direction: column; overflow: hidden; }
  .modal-xl { width: 95vw; max-width: 1200px; height: 85vh; }
  .modal-header { display: flex; justify-content: space-between; align-items: center; padding: 14px 18px; border-bottom: 1px solid var(--border-primary); }
  .modal-header h3 { margin: 0; font-size: 15px; color: var(--text-primary); letter-spacing: 0; text-transform: none; }
  .modal-close { background: none; border: none; color: var(--text-tertiary); font-size: 18px; padding: 4px 8px; border-radius: 6px; }
  .modal-close:hover { background: var(--bg-hover); }
  .process-table { width: 100%; overflow-x: auto; overscroll-behavior-x: contain; }
  .process-header { display: grid; grid-template-columns: 40px 80px 140px minmax(360px, 1fr) 80px 100px 80px; gap: 8px; min-width: 900px; padding: 8px 0; border-bottom: 1px solid var(--border-primary); font-size: 11px; font-weight: 600; color: var(--text-tertiary); text-transform: uppercase; }
  .process-sort { border: none; background: transparent; color: inherit; font: inherit; text-transform: inherit; padding: 0; text-align: left; }
  .process-list { max-height: 60vh; overflow-y: auto; min-width: 900px; }
  .process-row { display: grid; grid-template-columns: 40px 80px 140px minmax(360px, 1fr) 80px 100px 80px; gap: 8px; padding: 6px 0; border-bottom: 1px solid var(--border-secondary); font-size: 12px; }
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
  .modal-refresh { display: flex; align-items: center; gap: 4px; padding: 4px 10px; background: var(--bg-secondary); border: 1px solid var(--border-primary); border-radius: 6px; color: var(--text-secondary); font-size: 12px; }
  .modal-refresh:hover { background: var(--bg-hover); color: var(--text-primary); }
  .modal-body { flex: 1; overflow-y: auto; padding: 12px; }
  .stat-list-head { display: grid; grid-template-columns: 1fr 92px 42px; gap: 10px; padding: 0 10px 6px; color: var(--text-tertiary); font-size: 11px; }
  .stat-list-head button { border: none; background: transparent; color: inherit; font: inherit; padding: 0; text-align: left; }
  .stat-list { display: flex; flex-direction: column; gap: 4px; }
  .stat-list-item { display: flex; align-items: center; gap: 10px; padding: 8px 10px; background: var(--bg-secondary); border-radius: 6px; }
  .stat-list-name { flex: 1; font-size: 13px; color: var(--text-primary); }
  .stat-list-time { font-size: 11px; color: var(--text-tertiary); }
  .stat-list-badge { width: 20px; height: 20px; border-radius: 50%; display: flex; align-items: center; justify-content: center; font-size: 10px; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @keyframes pulse { 0%, 100% { opacity: .75; transform: scale(.9); } 50% { opacity: 1; transform: scale(1.18); } }
  @media (min-width: 1440px) {
    .dashboard { grid-template-rows: minmax(118px, .66fr) minmax(66px, auto) minmax(0, 5fr); gap: 10px; }
    .hero-console { grid-template-columns: 104px minmax(0, 1fr) 232px; padding: 12px; }
    .orbit-ring { width: 96px; }
    .dense-grid { grid-template-columns: minmax(260px, .76fr) minmax(0, 1.48fr) minmax(300px, .9fr); grid-template-rows: minmax(0, 1.05fr) minmax(0, 1fr); gap: 10px; }
  }
  @media (min-width: 1800px) {
    .dashboard { grid-template-rows: minmax(128px, .68fr) minmax(68px, auto) minmax(0, 5.2fr); gap: 12px; }
    .hero-console { grid-template-columns: 112px minmax(0, 1fr) 256px; }
    .orbit-ring { width: 104px; }
    .dense-grid { grid-template-columns: minmax(300px, .8fr) minmax(0, 1.48fr) minmax(360px, .92fr); }
  }
  @media (max-width: 1420px) {
    .dashboard { grid-template-rows: minmax(106px, .58fr) minmax(64px, auto) minmax(0, 5fr); gap: 7px; }
    .hero-console { grid-template-columns: 88px minmax(0, 1fr) 198px; gap: 8px; padding: 9px; }
    .orbit-ring { width: 82px; }
    .hero-title { margin-bottom: 7px; }
    .hero-title h1 { font-size: 18px; min-width: 140px; }
    .hero-title p { flex-basis: 260px; }
    .status-pill { height: 22px; padding: 0 8px; }
    .hero-kpis { gap: 6px; }
    .kpi-card { padding: 6px 7px; }
    .kpi-card strong { font-size: 17px; }
    .meta-line { min-height: 20px; }
    .refresh-opt { min-height: 20px; }
    .dense-grid {
      grid-template-columns: minmax(204px, .68fr) minmax(0, 1.5fr) minmax(230px, .82fr);
      grid-template-areas:
        "system trend resource"
        "proc proc io";
      grid-template-rows: minmax(0, 1fr) minmax(0, .96fr);
      gap: 8px;
    }
    .panel { padding: 9px; }
    .panel-head { margin-bottom: 7px; }
    .resource-matrix { gap: 6px; }
    .resource-tile { padding: 6px; }
    .tile-top strong { font-size: 13px; }
    .info-rows { gap: 4px; }
    .info-row { min-height: 20px; }
    .proc-header,
    .proc-row { grid-template-columns: 26px minmax(76px, .7fr) minmax(190px, 1.45fr) minmax(62px, .5fr) 54px 70px; gap: 6px; min-width: 580px; }
    .proc-list { min-width: 580px; }
    .mini-row { grid-template-columns: minmax(56px, .72fr) minmax(58px, .58fr) minmax(88px, 1fr); gap: 6px; }
  }
  @media (max-width: 760px) {
    .dashboard { height: auto; min-height: 100%; overflow-y: auto; display: block; }
    .hero-console,
    .quick-run-section,
    .dense-grid { margin-bottom: 10px; }
    .hero-console,
    .dense-grid,
    .split-lists,
    .hero-side { grid-template-columns: 1fr; }
    .dense-grid {
      grid-template-areas:
        "trend"
        "system"
        "resource"
        "proc"
        "io";
    }
    .hero-kpis,
    .resource-matrix { grid-template-columns: repeat(2, minmax(0, 1fr)); }
    .qr-controls { grid-template-columns: 1fr; }
    .trend-card, .io-panel { grid-column: auto; }
  }
</style>
