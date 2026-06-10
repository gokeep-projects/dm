<script>
  import { onDestroy, onMount, tick } from 'svelte';

  let processes = $state([]);
  let loading = $state(false);
  let analyzing = $state(false);
  let query = $state('');
  let selectedPid = $state(null);
  let analysis = $state(null);
  let error = $state('');
  let includeHistogram = $state(true);
  let activeState = $state('ALL');
  let selectedThread = $state(null);
  let activeRaw = $state('threads');
  let activeView = $state('telemetry-overview');
  let progress = $state(0);
  let autoRefresh = $state(false);
  let trackingUpdating = $state(false);
  let trackingRound = $state(0);
  let trackingHistory = $state([]);
  let lastAnalysisAt = $state('');
  let liveLogs = $state([]);
  let showProcessMenu = $state(false);
  let treeExpanded = $state({});
  let treeExpandAll = $state(false);
  let tableSearch = $state('');
  let chartHover = $state(null);
  let selectedFinding = $state(null);
  let hprofExporting = $state(false);
  let fleetScan = $state(null);
  let fleetScanning = $state(false);
  let fleetProgress = $state(0);
  let sortState = $state({
    profile: { key: 'estimateMs', dir: 'desc' },
    objects: { key: 'bytes', dir: 'desc' },
    recorded: { key: 'bytes', dir: 'desc' },
    callTree: { key: 'estimateMs', dir: 'desc' },
    threads: { key: 'depth', dir: 'desc' },
    complexity: { key: 'depth', dir: 'desc' },
    db: { key: 'estimateMs', dir: 'desc' },
  });
  let processInput;
  let progressTimer = null;
  let autoTimer = null;
  let metricTimer = null;
  let processPoll = null;
  let logSeq = 0;
  let pageLeaving = false;
  let activeControllers = new Set();
  let activeCancelKeys = new Set();
  const realtimeMetricMs = 1600;
  const DEFAULT_SAMPLES = 4;
  const DEFAULT_INTERVAL_MS = 700;
  const TRACE_SAMPLES = 2;
  const TRACE_INTERVAL_MS = 260;

  const stateLabels = ['ALL', 'RUNNABLE', 'BLOCKED', 'WAITING', 'TIMED_WAITING'];
  const menuGroups = [
    {
      title: '遥测',
      icon: 'telemetry',
      items: [
        ['telemetry-overview', '概览', 'overview'],
        ['telemetry-memory', '内存', 'memory'],
        ['telemetry-threads', '线程', 'threads'],
        ['telemetry-cpu', 'CPU 负载', 'cpu'],
      ],
    },
    {
      title: '实时内存',
      icon: 'memory',
      items: [
        ['memory-objects', '所有对象', 'objects'],
        ['memory-recorded', '记录的对象', 'recorded'],
        ['memory-allocation-tree', '分配调用树', 'tree'],
        ['memory-allocation-hot', '分配热点', 'hot'],
        ['memory-class-tracker', '类跟踪器', 'class'],
      ],
    },
    {
      title: 'CPU 视图',
      icon: 'cpu',
      items: [
        ['cpu-call-tree', '调用树', 'tree'],
        ['cpu-hotspots', '热点', 'hot'],
        ['cpu-anomaly', '异常检测值', 'anomaly'],
        ['cpu-complexity', '复杂度分析', 'complexity'],
      ],
    },
    {
      title: '线程',
      icon: 'threads',
      items: [
        ['thread-history', '线程历史', 'history'],
        ['thread-monitor', '线程监视器', 'monitor'],
      ],
    },
    {
      title: '监视器与锁',
      icon: 'lock',
      items: [
        ['lock-state', '当前锁状态图', 'lock'],
        ['lock-monitor', '当前监视器', 'monitor'],
      ],
    },
    {
      title: '数据库',
      icon: 'database',
      items: [
        ['db-jdbc', 'JDBC', 'database'],
        ['db-es', 'ES', 'search'],
        ['db-redis', 'REDIS', 'redis'],
      ],
    },
    {
      title: '异常综合分析',
      icon: 'anomaly',
      items: [
        ['exception-summary', '综合结论', 'anomaly'],
      ],
    },
  ];
  const progressHints = [
    ['discover', '定位 Java 进程与服务特征'],
    ['attach', '建立 HotSpot 附加通道'],
    ['thread', '采集线程转储'],
    ['heap', '读取堆概要与类直方图'],
    ['sampling', '采样堆栈活跃度'],
    ['parse', '生成方法剖析、诊断结论与线程模型'],
  ];

  let selectedProcess = $derived.by(() => processes.find((p) => p.pid === selectedPid) || processes[0] || null);
  let processRows = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return processes.filter((p) => {
      if (!q) return true;
      return [
        p.pid,
        p.service_name,
        p.display_name,
        p.main,
        p.jar,
        p.cwd,
        p.exe,
        p.cmd,
        ...(p.ports || []),
      ].join(' ').toLowerCase().includes(q);
    });
  });
  let activeViewTitle = $derived.by(() => {
    for (const group of menuGroups) {
      const found = group.items.find(([id]) => id === activeView);
      if (found) return `${group.title} / ${found[1]}`;
    }
    return '遥测 / 概览';
  });
  let displayedLogs = $derived.by(() => liveLogs.length ? liveLogs : (analysis?.runtime_logs || []).map(normalizeRuntimeLog));
  let latestLog = $derived.by(() => displayedLogs[displayedLogs.length - 1] || null);
  let filteredThreads = $derived.by(() => {
    const list = analysis?.threads || [];
    if (activeState === 'ALL') return list;
    return list.filter((thread) => thread.state === activeState);
  });
  let maxHotWeight = $derived.by(() => Math.max(1, ...(analysis?.hot_frames || []).map((item) => item.weight || 0)));
  let profileRows = $derived.by(() => (analysis?.hot_frames || []).slice(0, 42).map((frame) => {
    const count = Number(frame.count || 0);
    const sampleCount = Math.max(1, analysis?.samples?.length || DEFAULT_SAMPLES);
    const estimateMs = Math.max(DEFAULT_INTERVAL_MS, count * DEFAULT_INTERVAL_MS);
    const method = String(frame.method || '');
    return {
      ...frame,
      percent: Math.min(100, Math.round((count * 100) / sampleCount)),
      estimateMs,
      score: Math.round((Number(frame.weight || 0) * 100) / maxHotWeight),
      depth: method.split('.').length,
      owner: method.split('.').slice(0, -1).join('.') || method,
      leaf: method.split('.').pop() || method,
    };
  }).sort((a, b) => b.estimateMs - a.estimateMs || Number(b.weight || 0) - Number(a.weight || 0) || Number(b.count || 0) - Number(a.count || 0)));
  let callTreeRoots = $derived.by(() => buildCallTree(profileRows));
  let callTreeRows = $derived.by(() => filterRows(flattenCallTree(sortTreeNodes(callTreeRoots, 'callTree'), treeExpanded, treeExpandAll), ['name', 'path', 'category', 'leafMethod']));
  let callTreeTotal = $derived.by(() => countTreeNodes(callTreeRoots));
  let chartPoints = $derived.by(() => {
    if (trackingHistory.length) return trackingHistory;
    if (!selectedProcess) return [];
    return [{
      id: 'current',
      time: '--:--:--',
      cpu: Number(selectedProcess.cpu_ticks || 0),
      memory: Number(selectedProcess.memory_bytes || 0) / 1024 / 1024,
      threads: Number(selectedProcess.threads || 0),
      runnable: Number(analysis?.summary?.RUNNABLE || 0),
      blocked: Number(analysis?.summary?.BLOCKED || 0),
      waiting: Number(analysis?.summary?.WAITING || 0),
      objects: objectTotals().instances,
      bytes: objectTotals().bytes / 1024 / 1024,
    }];
  });
  let threadPoints = $derived.by(() => {
    const samplesList = analysis?.samples || [];
    if (autoRefresh && trackingHistory.length > samplesList.length) {
      return trackingHistory.map((point) => ({
        id: point.id,
        time: point.time,
        runnable: Number(point.runnable || 0),
        blocked: Number(point.blocked || 0),
        waiting: Number(point.waiting || 0),
        timed: 0,
        threads: Number(point.threads || 0),
      }));
    }
    if (samplesList.length) {
      return samplesList.map((point, index) => ({
        id: `sample-${index}`,
        time: point.timestamp || `${index + 1}`,
        runnable: Number(point.runnable || 0),
        blocked: Number(point.blocked || 0),
        waiting: Number(point.waiting || 0),
        timed: Number(point.timed_waiting || 0),
        threads: Number(point.runnable || 0) + Number(point.blocked || 0) + Number(point.waiting || 0) + Number(point.timed_waiting || 0),
      }));
    }
    return chartPoints.map((point) => ({
      ...point,
      timed: 0,
      threads: Number(point.threads || 0),
    }));
  });
  let objectRows = $derived.by(() => parseClassHistogram(analysis?.class_histogram || ''));
  let recordedObjectRows = $derived.by(() => objectRows.filter((row) => isRecordedObject(row.className)).slice(0, 24));
  let displayProfileRows = $derived.by(() => sortRows(filterRows(profileRows, ['method', 'category', 'owner', 'leaf']), 'profile'));
  let displayObjectRows = $derived.by(() => sortRows(filterRows(objectRows, ['className']), 'objects'));
  let displayRecordedObjectRows = $derived.by(() => sortRows(filterRows(recordedObjectRows, ['className']), 'recorded'));
  let telemetryCards = $derived.by(() => {
    const totals = objectTotals();
    return [
      { label: '进程内存', value: fmtBytes(selectedProcess?.memory_bytes), hint: '宿主机 RSS 视角', tone: 'info' },
      { label: '记录对象', value: formatNumber(totals.instances), hint: `${fmtBytes(totals.bytes)} 已解析`, tone: 'ok' },
      { label: '记录吞吐量', value: `${formatNumber(Math.round(totals.instances / Math.max(1, analysis?.samples?.length || 1)))}/次`, hint: '按当前采样估算', tone: 'info' },
      { label: 'GC 活动', value: heapGcSignal(), hint: heapCards.length ? '堆概要已返回' : '等待堆概要', tone: heapCards.length ? 'ok' : 'idle' },
      { label: '类数量', value: formatNumber(objectRows.length), hint: '类直方图条目', tone: 'info' },
      { label: '线程总数', value: analysis?.threads?.length ?? selectedProcess?.threads ?? '-', hint: `${stateCount('RUNNABLE')} 运行 / ${stateCount('BLOCKED')} 阻塞`, tone: stateCount('BLOCKED') ? 'warn' : 'ok' },
      { label: 'CPU 计数', value: latestMetric('cpu'), hint: '来自进程时间片', tone: 'info' },
      { label: '诊断结论', value: analysis ? exceptionRows.length : '-', hint: analysis ? '规则与综合分析实时刷新' : '等待分析', tone: exceptionRows.length ? 'warn' : 'ok' },
    ];
  });
  let heapCards = $derived.by(() => heapHighlights(analysis?.heap_info || ''));
  let selectedThreadFrames = $derived.by(() => selectedThread?.frames || []);
  let lockThreads = $derived.by(() => (analysis?.threads || []).filter((thread) => thread.state === 'BLOCKED' || /lock|monitor|park|wait/i.test(`${thread.top_frame} ${thread.raw_header}`)));
  let threadComplexityRows = $derived.by(() => (analysis?.threads || []).map((thread) => ({
    ...thread,
    depth: thread.frames?.length || 0,
    risk: complexityRisk(thread),
  })).sort((a, b) => b.depth - a.depth).slice(0, 24));
  let displayThreads = $derived.by(() => sortRows(filterRows(filteredThreads.map((thread) => ({
    ...thread,
    depth: thread.frames?.length || 0,
  })), ['name', 'state', 'top_frame', 'raw_header']), 'threads'));
  let displayComplexityRows = $derived.by(() => sortRows(filterRows(threadComplexityRows, ['name', 'state', 'top_frame', 'risk']), 'complexity'));
  let dbRows = $derived.by(() => ({
    jdbc: profileRows.filter((row) => /jdbc|datasource|mybatis|hibernate|mysql|postgres|oracle|sql|hikari/i.test(row.method)),
    es: profileRows.filter((row) => /elasticsearch|opensearch|resthighlevel|elastic/i.test(row.method)),
    redis: profileRows.filter((row) => /redis|jedis|lettuce|redisson/i.test(row.method)),
  }));
  let displayDbRows = $derived.by(() => {
    const rows = activeView === 'db-jdbc' ? dbRows.jdbc : activeView === 'db-es' ? dbRows.es : dbRows.redis;
    return sortRows(filterRows(rows, ['method', 'category', 'owner', 'leaf']), 'db');
  });
  let exceptionRows = $derived.by(() => {
    const findings = (analysis?.findings || []).map((finding, index) => ({
      id: `finding-${index}`,
      level: finding.level || 'info',
      title: finding.title,
      detail: finding.detail,
      suggestion: finding.suggestion,
      evidence: [
        analysis?.summary?.['热点方法'] ? `热点方法: ${analysis.summary['热点方法']}` : '',
        analysis?.summary?.['线程数'] ? `线程数: ${analysis.summary['线程数']}` : '',
      ].filter(Boolean),
    }));
    const synthetic = [];
    const totals = objectTotals();
    const topObject = objectRows[0];
    if (stateCount('BLOCKED') > 0) {
      synthetic.push({ id: 'blocked', level: 'warn', title: '锁等待风险', detail: `${stateCount('BLOCKED')} 个线程处于阻塞状态。`, suggestion: '进入“监视器与锁”查看阻塞线程和顶部调用。', evidence: lockThreads.slice(0, 6).map((t) => `${t.name} / ${t.top_frame || '-'}`) });
    }
    if (profileRows[0]) {
      synthetic.push({ id: 'hot', level: 'warn', title: '热点方法线索', detail: `${profileRows[0].method} 当前权重 ${profileRows[0].weight}，估算耗时 ${profileMs(profileRows[0].estimateMs)}。`, suggestion: '连续跟踪后如果仍居首位，优先排查该调用链的外部依赖、锁和循环。', evidence: profileRows.slice(0, 8).map((row) => `${row.method} / 命中 ${row.count} / 权重 ${row.weight}`) });
    }
    if (topObject && topObject.bytes > 0) {
      const ratio = Math.round((topObject.bytes * 1000) / Math.max(1, totals.bytes)) / 10;
      synthetic.push({ id: 'heap-top', level: ratio >= 30 ? 'warn' : 'info', title: '堆对象占用线索', detail: `${topObject.className} 占用 ${fmtBytes(topObject.bytes)}，约 ${ratio}% 类直方图字节。`, suggestion: ratio >= 30 ? '该类占比偏高，建议结合业务缓存、集合增长、批处理结果集和反序列化对象排查。' : '对象分布暂未出现单类明显压倒性占用，建议持续跟踪趋势。', evidence: objectRows.slice(0, 10).map((row) => `${row.className} / ${formatNumber(row.instances)} / ${fmtBytes(row.bytes)}`) });
    }
    if ((dbRows.jdbc.length + dbRows.es.length + dbRows.redis.length) > 0) {
      synthetic.push({ id: 'db-hot', level: 'warn', title: '外部存储调用热点', detail: `采样中识别到 JDBC ${dbRows.jdbc.length}、ES ${dbRows.es.length}、Redis ${dbRows.redis.length} 个相关热点。`, suggestion: '查看数据库视图，优先确认慢 SQL、连接池等待、网络抖动、索引和批量请求大小。', evidence: [...dbRows.jdbc, ...dbRows.es, ...dbRows.redis].slice(0, 8).map((row) => `${row.method} / ${profileMs(row.estimateMs)} / 权重 ${row.weight}`) });
    }
    if (analysis && !analysis.attach_ok) {
      synthetic.push({ id: 'attach-failed', level: 'error', title: 'Attach 通道不可用', detail: analysis.attach_error || 'JVM Attach 未成功。', suggestion: '确认运行用户、JVM Attach 开关、容器 /tmp 可见性，以及目标 JVM 是否为 HotSpot。', evidence: [selectedProcess?.attach_path || '未发现 attach socket', selectedProcess?.cmd || ''] });
    }
    return dedupeFindings([...findings, ...synthetic]).slice(0, 30);
  });
  let fleetFindingRows = $derived.by(() => {
    const rows = [];
    for (const report of fleetScan?.reports || []) {
      for (const finding of report.findings || []) {
        rows.push({
          id: `fleet-${report.process?.pid}-${finding.title}-${finding.level}`,
          pid: report.process?.pid,
          service: report.process?.service_name || report.process?.display_name || `PID ${report.process?.pid || '-'}`,
          level: finding.level || 'info',
          title: finding.title,
          detail: finding.detail,
          suggestion: finding.suggestion,
          evidence: [
            `进程: ${report.process?.service_name || '-'} / PID ${report.process?.pid || '-'}`,
            report.hot_method ? `热点方法: ${report.hot_method}` : '',
            report.summary?.['线程数'] ? `线程数: ${report.summary['线程数']}` : '',
            report.process?.cmd || '',
          ].filter(Boolean),
        });
      }
      if (!(report.findings || []).length && report.error) {
        rows.push({
          id: `fleet-${report.process?.pid}-error`,
          pid: report.process?.pid,
          service: report.process?.service_name || `PID ${report.process?.pid || '-'}`,
          level: 'error',
          title: '规则扫描失败',
          detail: report.error,
          suggestion: '确认进程仍在运行、权限一致、Attach 未被禁用，并重新扫描。',
          evidence: [report.process?.cmd || ''],
        });
      }
    }
    return dedupeFindings(rows).sort((a, b) => severityRank(b.level) - severityRank(a.level) || String(a.service).localeCompare(String(b.service), 'zh-CN'));
  });

  onMount(() => {
    pageLeaving = false;
    loadProcesses();
    processPoll = setInterval(() => {
      if (!analyzing) loadProcesses(false, true);
    }, 5000);
    window.addEventListener('click', closeProcessMenuOnOutside);
    window.addEventListener('pagehide', stopJavaPageTasks);
    window.addEventListener('beforeunload', stopJavaPageTasks);
    document.addEventListener('visibilitychange', stopWhenHidden);
  });

  onDestroy(() => {
    stopJavaPageTasks();
    if (processPoll) clearInterval(processPoll);
    window.removeEventListener('click', closeProcessMenuOnOutside);
    window.removeEventListener('pagehide', stopJavaPageTasks);
    window.removeEventListener('beforeunload', stopJavaPageTasks);
    document.removeEventListener('visibilitychange', stopWhenHidden);
  });

  function stopWhenHidden() {
    if (document.visibilityState === 'hidden') stopJavaPageTasks();
  }

  function resetAnalysisData() {
    stopProgress();
    chartHover = null;
    selectedFinding = null;
    selectedThread = null;
    analysis = null;
    fleetScan = null;
    fleetProgress = 0;
    trackingHistory = [];
    trackingRound = 0;
    trackingUpdating = false;
    tableSearch = '';
    activeState = 'ALL';
    activeRaw = 'threads';
    treeExpanded = {};
    treeExpandAll = false;
    liveLogs = [];
    logSeq = 0;
    progress = 0;
    lastAnalysisAt = '';
    error = '';
  }

  function resetProfiler() {
    stopAutoRefresh();
    resetAnalysisData();
    addLog('info', 'reset', '已复位当前 Java 分析工作台，历史采样和诊断结果已清空。', 0);
  }

  function stopJavaPageTasks() {
    pageLeaving = true;
    stopProgress();
    stopAutoRefresh();
    stopMetricStream();
    trackingUpdating = false;
    analyzing = false;
    fleetScanning = false;
    for (const controller of activeControllers) {
      try { controller.abort(); } catch {}
    }
    activeControllers.clear();
    for (const key of activeCancelKeys) {
      cancelServerTask(key);
    }
    activeCancelKeys.clear();
  }

  function cancelServerTask(key) {
    if (!key) return;
    const url = `/api/java/cancel/${encodeURIComponent(key)}`;
    try {
      if (navigator.sendBeacon) {
        navigator.sendBeacon(url, new Blob([], { type: 'text/plain' }));
        return;
      }
    } catch {}
    fetch(url, { method: 'POST', keepalive: true }).catch(() => {});
  }

  function newCancelKey(prefix) {
    return `${prefix}-${Date.now()}-${Math.random().toString(16).slice(2)}`;
  }

  async function trackedFetch(url, options = {}, cancelKey = '') {
    const controller = new AbortController();
    activeControllers.add(controller);
    if (cancelKey) activeCancelKeys.add(cancelKey);
    try {
      return await fetch(url, { ...options, signal: controller.signal });
    } finally {
      activeControllers.delete(controller);
      if (cancelKey) activeCancelKeys.delete(cancelKey);
    }
  }

  function closeProcessMenuOnOutside(event) {
    if (!event.target.closest?.('.process-combobox')) showProcessMenu = false;
  }

  function fmtBytes(value) {
    const n = Number(value || 0);
    if (!n) return '-';
    const units = ['B', 'KB', 'MB', 'GB', 'TB'];
    let size = n;
    let idx = 0;
    while (size >= 1024 && idx < units.length - 1) {
      size /= 1024;
      idx += 1;
    }
    return `${size.toFixed(idx < 2 ? 0 : 1)} ${units[idx]}`;
  }

  function formatNumber(value) {
    const n = Number(value || 0);
    return n ? n.toLocaleString('zh-CN') : '0';
  }

  function serviceTitle(p) {
    return p?.service_name || p?.display_name || p?.main || p?.jar || p?.name || `PID ${p?.pid || '-'}`;
  }

  function shortText(value, size = 70) {
    if (!value) return '-';
    const text = String(value);
    return text.length <= size ? text : '...' + text.slice(-(size - 3));
  }

  function methodShort(method) {
    if (!method) return '-';
    const parts = String(method).split('.');
    if (parts.length <= 6) return method;
    return `${parts.slice(0, 2).join('.')}.…${parts.slice(-4).join('.')}`;
  }

  function sortMark(scope, key) {
    const state = sortState[scope] || {};
    if (state.key !== key) return '↕';
    return state.dir === 'asc' ? '↑' : '↓';
  }

  function setSort(scope, key) {
    const current = sortState[scope] || { key, dir: 'desc' };
    const dir = current.key === key && current.dir === 'desc' ? 'asc' : 'desc';
    sortState = { ...sortState, [scope]: { key, dir } };
  }

  function sortValue(row, key) {
    const value = row?.[key];
    if (value === undefined || value === null) return '';
    return value;
  }

  function compareRows(scope, a, b) {
    const state = sortState[scope] || { key: 'estimateMs', dir: 'desc' };
    const av = sortValue(a, state.key);
    const bv = sortValue(b, state.key);
    const an = Number(av);
    const bn = Number(bv);
    let result;
    if (!Number.isNaN(an) && !Number.isNaN(bn) && String(av).trim?.() !== '' && String(bv).trim?.() !== '') {
      result = an - bn;
    } else {
      result = String(av).localeCompare(String(bv), 'zh-CN', { numeric: true, sensitivity: 'base' });
    }
    return state.dir === 'asc' ? result : -result;
  }

  function sortRows(rows, scope) {
    return [...(rows || [])].sort((a, b) => compareRows(scope, a, b));
  }

  function filterRows(rows, fields) {
    const q = tableSearch.trim().toLowerCase();
    if (!q) return rows || [];
    return (rows || []).filter((row) => fields.some((field) => String(row?.[field] ?? '').toLowerCase().includes(q)));
  }

  function sortTreeNodes(nodes, scope) {
    return sortRows((nodes || []).map((node) => ({
      ...node,
      children: sortTreeNodes(node.children, scope),
    })), scope);
  }

  function dedupeFindings(rows) {
    const seen = new Map();
    for (const row of rows || []) {
      const key = findingKey(row);
      if (seen.has(key)) {
        const existing = seen.get(key);
        existing.evidence = [...new Set([...(existing.evidence || []), ...(row.evidence || [])])].slice(0, 12);
        if (severityRank(row.level) > severityRank(existing.level)) existing.level = row.level;
        continue;
      }
      seen.set(key, { ...row });
    }
    return [...seen.values()];
  }

  function findingKey(row) {
    const title = normalizeFindingText(row?.title);
    const detail = normalizeFindingText(row?.detail)
      .replace(/\d+(\.\d+)?\s*(个|次|层|gb|mb|kb|bytes?|线程|方法|权重)?/gi, '#')
      .slice(0, 96);
    return `${row?.level || 'info'}:${title}:${detail}`;
  }

  function normalizeFindingText(value) {
    return String(value || '')
      .toLowerCase()
      .replace(/\s+/g, ' ')
      .replace(/[，。；：、,.；:]/g, ' ')
      .trim();
  }

  function openFinding(item) {
    selectedFinding = item;
  }

  function closeFinding() {
    selectedFinding = null;
  }

  function stateCount(name) {
    return Number(analysis?.summary?.[name] || 0);
  }

  function stateLabel(state) {
    const map = {
      ALL: '全部',
      RUNNABLE: '运行中',
      BLOCKED: '阻塞',
      WAITING: '等待',
      TIMED_WAITING: '限时等待',
      NEW: '新建',
      TERMINATED: '已终止',
      UNKNOWN: '未知',
    };
    return map[state] || state || '-';
  }

  function categoryLabel(category) {
    const map = {
      application: '业务方法',
      framework: '框架调用',
      io: '输入输出',
      lock: '锁竞争',
      gc: '回收相关',
      jdbc: '数据库调用',
      network: '网络调用',
      reflection: '反射调用',
      thread: '线程调度',
      unknown: '未知',
    };
    const key = String(category || '').toLowerCase();
    return map[key] || category || '-';
  }

  function levelLabel(level) {
    const map = {
      info: '信息',
      warn: '警告',
      error: '错误',
      ok: '正常',
    };
    return map[String(level || '').toLowerCase()] || level || '信息';
  }

  function severityRank(level) {
    const map = { error: 3, warn: 2, info: 1, ok: 0 };
    return map[String(level || '').toLowerCase()] ?? 1;
  }

  function profileMs(value) {
    const n = Number(value || 0);
    if (n >= 1000) return `${(n / 1000).toFixed(1)}s`;
    return `${Math.round(n)}ms`;
  }

  function treeParts(method) {
    const raw = String(method || '').split('.').filter(Boolean);
    if (raw.length <= 1) return raw.length ? raw : ['未知调用'];
    if (raw.length <= 5) return raw;
    return [...raw.slice(0, 3), raw.slice(3, -1).join('.'), raw[raw.length - 1]].filter(Boolean);
  }

  function buildCallTree(rows) {
    const root = { children: [], childMap: new Map() };
    for (const row of rows || []) {
      const parts = treeParts(row.method);
      let current = root;
      let key = '';
      parts.forEach((part, index) => {
        key = key ? `${key}.${part}` : part;
        let node = current.childMap.get(part);
        if (!node) {
          node = {
            key,
            name: part,
            path: key,
            level: index,
            count: 0,
            weight: 0,
            estimateMs: 0,
            percent: 0,
            score: 0,
            category: row.category,
            leafMethod: '',
            children: [],
            childMap: new Map(),
          };
          current.childMap.set(part, node);
          current.children.push(node);
        }
        node.count += Number(row.count || 0);
        node.weight += Number(row.weight || 0);
        node.estimateMs += Number(row.estimateMs || 0);
        node.percent = Math.max(node.percent, Number(row.percent || 0));
        node.score = Math.max(node.score, Number(row.score || 0));
        if (index === parts.length - 1) {
          node.leafMethod = row.method;
          node.category = row.category;
        }
        current = node;
      });
    }

    function seal(nodes) {
      return nodes
        .map((node) => ({
          ...node,
          children: seal(node.children),
          childMap: undefined,
        }))
        .sort((a, b) => b.estimateMs - a.estimateMs || b.weight - a.weight || b.count - a.count || a.name.localeCompare(b.name, 'zh-CN'));
    }

    return seal(root.children);
  }

  function flattenCallTree(nodes, expanded, expandAll) {
    const out = [];
    function visit(list) {
      for (const node of list || []) {
        const hasChildren = Boolean(node.children?.length);
        const open = expandAll || Boolean(expanded[node.key]);
        out.push({ ...node, hasChildren, open });
        if (hasChildren && open) visit(node.children);
      }
    }
    visit(nodes);
    return out;
  }

  function countTreeNodes(nodes) {
    return (nodes || []).reduce((sum, node) => sum + 1 + countTreeNodes(node.children), 0);
  }

  function toggleTreeNode(node) {
    if (!node?.hasChildren) return;
    treeExpanded = { ...treeExpanded, [node.key]: !treeExpanded[node.key] };
  }

  function expandCallTree() {
    treeExpandAll = true;
  }

  function collapseCallTree() {
    treeExpandAll = false;
    treeExpanded = {};
  }

  function latestMetric(metric) {
    const point = chartPoints[chartPoints.length - 1];
    if (!point) return '-';
    if (metric === 'memory' || metric === 'bytes') return `${Math.round(point[metric] || 0)} MB`;
    return String(Math.round(point[metric] || 0));
  }

  function metricLabel(metric) {
    const map = {
      memory: '内存',
      bytes: '对象字节',
      threads: '线程',
      runnable: '运行线程',
      blocked: '阻塞线程',
      waiting: '等待线程',
      timed: '限时等待',
      cpu: 'CPU 计数',
      objects: '对象数',
      findings: '诊断',
    };
    return map[metric] || metric;
  }

  function metricValue(point, metric) {
    const value = Number(point?.[metric] || 0);
    if (metric === 'memory' || metric === 'bytes') return `${value.toFixed(1)} MB`;
    if (metric === 'objects') return formatNumber(value);
    return formatNumber(Math.round(value));
  }

  function showChartTooltip(chart, points, metrics, event) {
    const list = points || [];
    if (!list.length) {
      chartHover = null;
      return;
    }
    const rect = event.currentTarget.getBoundingClientRect();
    const x = Math.max(0, Math.min(rect.width, event.clientX - rect.left));
    const y = Math.max(0, Math.min(rect.height, event.clientY - rect.top));
    const index = list.length === 1 ? 0 : Math.round((x / rect.width) * (list.length - 1));
    chartHover = {
      chart,
      x,
      y,
      point: list[Math.max(0, Math.min(list.length - 1, index))],
      metrics,
    };
  }

  function hideChartTooltip() {
    chartHover = null;
  }

  function polyline(points, metric, height = 118, width = 520) {
    const list = points || [];
    if (!list.length) return '';
    const values = list.map((p) => Number(p[metric] || 0));
    const min = Math.min(...values);
    const max = Math.max(...values, min + 1);
    return values.map((value, index) => {
      const x = list.length === 1 ? width - 8 : 8 + (index * (width - 16)) / (list.length - 1);
      const y = height - 8 - ((value - min) * (height - 18)) / (max - min);
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    }).join(' ');
  }

  function normalizeRuntimeLog(log) {
    return {
      id: log.id || ++logSeq,
      timestamp: String(log.timestamp || '').split(' ').pop() || new Date().toLocaleTimeString('zh-CN', { hour12: false }),
      level: log.level || 'info',
      stage: translateStage(log.stage),
      message: translateLog(log.message || ''),
      percent: Math.max(0, Math.min(100, Number(log.percent || 0))),
    };
  }

  function translateStage(stage) {
    const map = {
      discover: '发现',
      attach: '附加',
      thread: '线程',
      heap: '堆',
      histogram: '直方图',
      sampling: '采样',
      parse: '解析',
      done: '完成',
      failed: '失败',
      trace: '跟踪',
      rules: '规则',
      export: '导出',
      start: '启动',
      select: '选择',
    };
    return String(stage || '运行时').split('/').map((part) => map[part] || part).join('/');
  }

  function translateLog(message) {
    const text = String(message || '');
    const replacements = [
      ['Resolving Java process', '正在定位 Java 进程'],
      ['Locked target', '已锁定目标'],
      ['Connecting through HotSpot Attach protocol', '正在通过 HotSpot Attach 协议连接 JVM'],
      ['JVM attach succeeded; first thread dump collected', 'JVM 附加成功，已获取首轮线程转储'],
      ['Reading GC.heap_info heap summary', '正在读取 GC.heap_info 堆概要'],
      ['Heap summary collected', '堆概要读取完成'],
      ['Reading GC.class_histogram', '正在读取 GC.class_histogram 类直方图'],
      ['Class histogram collected', '类直方图读取完成'],
      ['Running sample', '执行第'],
      ['thread sample', '次线程采样'],
      ['Aggregating hot stacks and diagnostics', '正在聚合热点堆栈与诊断结论'],
      ['Analysis completed', '分析完成'],
      ['Continuous trace stopped', '连续跟踪已停止'],
      ['Continuous trace enabled; metrics and stacks update asynchronously', '连续跟踪已开启，指标与堆栈将异步更新'],
      ['Background trace round', '后台跟踪第'],
      ['Starting analysis task', '启动分析任务'],
      ['Analysis response rendered', '分析结果已返回并完成渲染'],
      ['Waiting for runtime analysis', '等待执行运行时分析'],
      ['Selected PID', '已选择 PID'],
    ];
    return replacements.reduce((acc, [from, to]) => acc.replaceAll(from, to), text) || '运行事件';
  }

  function addLog(level, stage, message, percent = progress) {
    const now = new Date();
    liveLogs = [
      ...liveLogs,
      {
        id: ++logSeq,
        timestamp: now.toLocaleTimeString('zh-CN', { hour12: false }),
        level,
        stage: translateStage(stage),
        message: translateLog(message),
        percent: Math.max(0, Math.min(100, Math.round(percent))),
      },
    ].slice(-160);
  }

  function appendServerLogs(result, roundLabel = '') {
    const logs = result?.runtime_logs || [];
    if (!logs.length) return;
    const mapped = logs.map((log) => {
      const normalized = normalizeRuntimeLog(log);
      return {
        ...normalized,
        id: ++logSeq,
        stage: roundLabel ? `${roundLabel}/${normalized.stage}` : normalized.stage,
      };
    });
    liveLogs = [...liveLogs, ...mapped].slice(-220);
  }

  function makeMetricPoint(proc = selectedProcess, result = analysis) {
    const summary = result?.summary || {};
    const totals = result && result !== analysis
      ? parseClassHistogram(result?.class_histogram || '').reduce((acc, row) => {
        acc.instances += row.instances;
        acc.bytes += row.bytes;
        return acc;
      }, { instances: 0, bytes: 0 })
      : objectTotals();
    return {
      id: `${Date.now()}-${trackingRound}`,
      time: new Date().toLocaleTimeString('zh-CN', { hour12: false }),
      threads: Number(summary['线程数'] || result?.threads?.length || proc?.threads || 0),
      runnable: Number(summary.RUNNABLE || 0),
      blocked: Number(summary.BLOCKED || 0),
      waiting: Number(summary.WAITING || 0),
      cpu: Number(proc?.cpu_ticks || 0),
      memory: Number(proc?.memory_bytes || 0) / 1024 / 1024,
      objects: totals.instances,
      bytes: totals.bytes / 1024 / 1024,
      hot: result?.hot_frames?.[0]?.method || '-',
      findings: result?.findings?.length || 0,
    };
  }

  function appendMetricPoint(proc = selectedProcess, result = analysis) {
    if (!proc) return;
    const point = makeMetricPoint(proc, result);
    trackingHistory = [...trackingHistory, point].slice(-48);
    lastAnalysisAt = point.time;
  }

  function upsertProcess(proc) {
    if (!proc?.pid) return;
    let matched = false;
    processes = processes.map((item) => {
      if (item.pid !== proc.pid) return item;
      matched = true;
      return { ...item, ...proc };
    });
    if (!matched) processes = [proc, ...processes];
  }

  function rememberTracePoint(result) {
    if (result?.process) upsertProcess(result.process);
    appendMetricPoint(result?.process || selectedProcess, result);
  }

  function safeTimestamp() {
    return new Date().toISOString().replace(/[:.]/g, '-');
  }

  function viewIconPath(name) {
    const icons = {
      telemetry: 'M4 12h4l2-7 4 14 3-9 2 5h3M5 4h4m10 0h-4M5 20h4m10 0h-4',
      overview: 'M4 5h7v7H4V5Zm9 0h7v4h-7V5ZM4 14h7v5H4v-5Zm9-3h7v8h-7v-8Z',
      memory: 'M7 7h10v10H7V7Zm3-4v4m4-4v4M10 17v4m4-4v4M3 10h4m10 0h4M3 14h4m10 0h4',
      threads: 'M7 8a3 3 0 1 0 0-6 3 3 0 0 0 0 6Zm10 0a3 3 0 1 0 0-6 3 3 0 0 0 0 6ZM4 22v-3a4 4 0 0 1 4-4h1m11 7v-3a4 4 0 0 0-4-4h-1M12 13a3 3 0 1 0 0-6 3 3 0 0 0 0 6Zm-5 9v-2a5 5 0 0 1 10 0v2',
      cpu: 'M8 3v3m4-3v3m4-3v3M8 18v3m4-3v3m4-3v3M3 8h3m-3 4h3m-3 4h3m12-8h3m-3 4h3m-3 4h3M7 7h10v10H7V7Zm3 6 2-5 2 8',
      objects: 'M6 6h6v6H6V6Zm6 6h6v6h-6v-6ZM6 15h4v4H6v-4Zm9-9h3v3h-3V6Z',
      recorded: 'M5 4h14v16H5V4Zm4 4h6M9 12h6M9 16h4',
      tree: 'M12 4v5M7 14v6m10-6v6M12 9H7v5m5-5h5v5M5 20h4m6 0h4',
      hot: 'M13 3 6 13h5l-1 8 8-12h-5l0-6Z',
      class: 'M4 6l8-4 8 4v12l-8 4-8-4V6Zm4 2 4 2 4-2M8 14l4 2 4-2M12 10v6',
      anomaly: 'M12 3 22 20H2L12 3Zm0 6v5m0 3h.01',
      complexity: 'M4 18c4-12 12-12 16 0M7 18c2-6 8-6 10 0M12 18v-5',
      history: 'M3 12a9 9 0 1 0 3-6.7M3 4v6h6M12 7v5l4 2',
      monitor: 'M4 5h16v10H4V5Zm5 15h6m-3-5v5M8 9h2m3 0h3',
      lock: 'M7 11V8a5 5 0 0 1 10 0v3M6 11h12v10H6V11Zm6 4v3',
      database: 'M5 6c0-2 14-2 14 0v12c0 2-14 2-14 0V6Zm0 0c0 2 14 2 14 0M5 12c0 2 14 2 14 0',
      search: 'M10.5 18a7.5 7.5 0 1 1 5.3-2.2L21 21M8 10h5M8 13h4',
      redis: 'M4 8 12 4l8 4-8 4-8-4Zm0 4 8 4 8-4M4 16l8 4 8-4',
    };
    return icons[name] || icons.overview;
  }

  function escapeHtml(value) {
    return String(value ?? '')
      .replaceAll('&', '&amp;')
      .replaceAll('<', '&lt;')
      .replaceAll('>', '&gt;')
      .replaceAll('"', '&quot;')
      .replaceAll("'", '&#39;');
  }

  async function exportHprofSnapshot() {
    const pid = selectedPid || analysis?.process?.pid;
    if (!pid || hprofExporting) return;
    hprofExporting = true;
    const cancelKey = newCancelKey('java-hprof');
    addLog('info', 'export', '正在生成 JVM HPROF 堆快照，目标进程可能会短暂停顿', progress);
    try {
      const res = await trackedFetch('/api/java/hprof', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ pid, live: true, cancel_key: cancelKey }),
      }, cancelKey);
      if (pageLeaving) return;
      if (!res.ok) throw new Error(await res.text() || 'HPROF 快照导出失败');
      const blob = await res.blob();
      const disposition = res.headers.get('content-disposition') || '';
      const filename = disposition.match(/filename="([^"]+)"/)?.[1] || `dm-java-heap-${pid}-${safeTimestamp()}.hprof`;
      const url = URL.createObjectURL(blob);
      const link = document.createElement('a');
      link.href = url;
      link.download = filename;
      document.body.appendChild(link);
      link.click();
      link.remove();
      URL.revokeObjectURL(url);
      addLog('ok', 'export', `HPROF 快照已导出: ${filename}`, progress);
    } catch (e) {
      if (e?.name === 'AbortError') return;
      error = e.message || String(e);
      addLog('error', 'export', error, progress);
    } finally {
      hprofExporting = false;
    }
  }

  function exportRawData() {
    if (!analysis) return;
    const payload = {
      exported_at: new Date().toISOString(),
      pid: analysis.process?.pid,
      service_name: analysis.process?.service_name,
      thread_dump: analysis.thread_dump || '',
      heap_info: analysis.heap_info || '',
      class_histogram: analysis.class_histogram || '',
      runtime_logs: analysis.runtime_logs || [],
      samples: analysis.samples || [],
    };
    const blob = new Blob([JSON.stringify(payload, null, 2)], { type: 'application/json;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = url;
    link.download = `dm-java-raw-${selectedPid || analysis?.process?.pid || 'unknown'}-${safeTimestamp()}.json`;
    document.body.appendChild(link);
    link.click();
    link.remove();
    URL.revokeObjectURL(url);
    addLog('ok', 'export', '原始运行时数据已导出', progress);
  }

  async function exportPdfReport() {
    if (!analysis && !selectedProcess) return;
    addLog('info', 'export', '正在生成 PDF 分析报告', progress);
    const [{ jsPDF }, html2canvasModule] = await Promise.all([
      import('jspdf'),
      import('html2canvas'),
    ]);
    const html2canvas = html2canvasModule.default || html2canvasModule;
    const report = document.createElement('div');
    const totals = objectTotals();
    report.className = 'pdf-report';
    report.innerHTML = `
      <h1>DM Java 运行时分析报告</h1>
      <p class="meta">导出时间: ${new Date().toLocaleString('zh-CN', { hour12: false })}</p>
      <section><h2>目标进程</h2><p>${escapeHtml(serviceTitle(selectedProcess || analysis?.process))} / PID ${selectedPid || analysis?.process?.pid || '-'}</p><p>${escapeHtml(selectedProcess?.cmd || analysis?.process?.cmd || '-')}</p></section>
      <section><h2>关键指标</h2><div class="pdf-grid">
        <span>线程 ${analysis?.threads?.length ?? '-'}</span><span>阻塞 ${stateCount('BLOCKED')}</span><span>热点 ${profileRows.length}</span><span>对象 ${formatNumber(totals.instances)}</span><span>堆字节 ${fmtBytes(totals.bytes)}</span><span>诊断 ${exceptionRows.length}</span>
      </div></section>
      <section><h2>综合结论</h2>${exceptionRows.slice(0, 12).map((item) => `<article><b>${escapeHtml(levelLabel(item.level))}</b><strong>${escapeHtml(item.title)}</strong><p>${escapeHtml(item.detail)}</p><em>${escapeHtml(item.suggestion)}</em></article>`).join('') || '<p>暂无诊断结论</p>'}</section>
      <section><h2>CPU 热点</h2>${profileRows.slice(0, 16).map((row, index) => `<p>${index + 1}. ${escapeHtml(row.method)} / ${profileMs(row.estimateMs)} / 权重 ${row.weight}</p>`).join('') || '<p>暂无热点</p>'}</section>
      <section><h2>对象占用</h2>${objectRows.slice(0, 16).map((row, index) => `<p>${index + 1}. ${escapeHtml(row.className)} / ${formatNumber(row.instances)} / ${fmtBytes(row.bytes)}</p>`).join('') || '<p>暂无对象数据</p>'}</section>
      <section><h2>线程状态</h2>${(analysis?.threads || []).slice(0, 20).map((thread) => `<p>${escapeHtml(thread.state)} / ${escapeHtml(thread.name)} / ${escapeHtml(thread.top_frame || '-')}</p>`).join('') || '<p>暂无线程数据</p>'}</section>
    `;
    document.body.appendChild(report);
    try {
      const canvas = await html2canvas(report, { backgroundColor: '#08111f', scale: 2, useCORS: true });
      const pdf = new jsPDF('p', 'mm', 'a4');
      const pageWidth = pdf.internal.pageSize.getWidth();
      const pageHeight = pdf.internal.pageSize.getHeight();
      const imgWidth = pageWidth;
      const imgHeight = (canvas.height * imgWidth) / canvas.width;
      let heightLeft = imgHeight;
      let position = 0;
      const imgData = canvas.toDataURL('image/png');
      pdf.addImage(imgData, 'PNG', 0, position, imgWidth, imgHeight);
      heightLeft -= pageHeight;
      while (heightLeft > 0) {
        position = heightLeft - imgHeight;
        pdf.addPage();
        pdf.addImage(imgData, 'PNG', 0, position, imgWidth, imgHeight);
        heightLeft -= pageHeight;
      }
      pdf.save(`dm-java-report-${selectedPid || analysis?.process?.pid || 'unknown'}-${safeTimestamp()}.pdf`);
      addLog('ok', 'export', 'PDF 分析报告已生成', progress);
    } finally {
      report.remove();
    }
  }

  function heapHighlights(text) {
    if (!text) return [];
    const lines = String(text).split('\n').map(line => line.trim()).filter(Boolean);
    const interesting = lines.filter(line =>
      /heap|metaspace|class space|used|committed|reserved|region|young|old/i.test(line)
    ).slice(0, 10);
    return interesting.map((line, index) => {
      const [label, ...rest] = line.split(':');
      return {
        id: `${index}-${label}`,
        label: rest.length ? label.trim() : `堆片段 ${index + 1}`,
        value: rest.length ? rest.join(':').trim() : line,
      };
    });
  }

  function parseClassHistogram(text) {
    if (!text) return [];
    const rows = [];
    for (const line of String(text).split('\n')) {
      const match = line.match(/^\s*\d+:\s+(\d+)\s+(\d+)\s+(.+?)\s*$/);
      if (!match) continue;
      const instances = Number(match[1] || 0);
      const bytes = Number(match[2] || 0);
      const className = String(match[3] || '').replace(/\s+\(.+\)$/, '');
      rows.push({ className, instances, bytes });
    }
    const totalBytes = rows.reduce((sum, row) => sum + row.bytes, 0) || 1;
    return rows
      .map((row) => ({ ...row, percent: Math.round((row.bytes * 1000) / totalBytes) / 10 }))
      .sort((a, b) => b.bytes - a.bytes)
      .slice(0, 120);
  }

  function objectTotals() {
    return objectRows.reduce((acc, row) => {
      acc.instances += Number(row.instances || 0);
      acc.bytes += Number(row.bytes || 0);
      return acc;
    }, { instances: 0, bytes: 0 });
  }

  function isRecordedObject(row) {
    const name = String(row?.className || '');
    if (!name) return false;
    return !/^\[|^java\.|^jdk\.|^sun\.|^com\.sun\.|^org\.springframework|^io\.netty|^ch\.qos/i.test(name);
  }

  function heapGcSignal() {
    const text = String(analysis?.heap_info || '').toLowerCase();
    if (!text) return '待采集';
    if (text.includes('g1')) return 'G1 活跃';
    if (text.includes('zgc')) return 'ZGC 活跃';
    if (text.includes('shenandoah')) return 'Shenandoah';
    if (text.includes('young') || text.includes('old')) return '分代活动';
    return '已采集';
  }

  function complexityRisk(thread) {
    const depth = thread.frames?.length || 0;
    if (thread.state === 'BLOCKED') return '锁风险';
    if (depth >= 70) return '深栈';
    if (depth >= 35) return '偏深';
    return '正常';
  }

  function threadKey(thread) {
    if (!thread) return '';
    return `${thread.name || ''}:${thread.nid || ''}:${thread.java_tid || ''}`;
  }

  function isSelectedThread(thread) {
    return Boolean(selectedThread && thread && threadKey(selectedThread) === threadKey(thread));
  }

  function restoreSelectedThread(nextAnalysis, previousThread) {
    const list = nextAnalysis?.threads || [];
    if (!list.length) return null;
    if (previousThread) {
      const key = threadKey(previousThread);
      const matched = list.find((thread) => threadKey(thread) === key || thread.name === previousThread.name);
      if (matched) return matched;
    }
    return list[0];
  }

  async function loadProcesses(selectFirst = true, silent = false) {
    if (pageLeaving) return;
    loading = !silent;
    if (!silent) error = '';
    try {
      const res = await trackedFetch('/api/java/processes?ts=' + Date.now(), { cache: 'no-store' });
      if (!res.ok) throw new Error('Java 进程扫描失败');
      const data = await res.json();
      if (pageLeaving) return;
      processes = data.processes || [];
      if (selectFirst && (!selectedPid || !processes.some((p) => p.pid === selectedPid))) {
        selectedPid = processes[0]?.pid || null;
        if (processes[0]) query = serviceTitle(processes[0]);
      }
    } catch (e) {
      if (e?.name === 'AbortError') return;
      if (!silent) error = e.message || String(e);
    } finally {
      loading = false;
    }
  }

  function selectProcess(process) {
    pageLeaving = false;
    selectedPid = process?.pid || null;
    query = process ? serviceTitle(process) : '';
    showProcessMenu = false;
    analysis = null;
    selectedThread = null;
    activeState = 'ALL';
    activeRaw = 'threads';
    trackingHistory = [];
    trackingRound = 0;
    collapseCallTree();
    error = '';
    if (autoRefresh) {
      stopMetricStream();
      startMetricStream();
      scheduleAutoTrace(1000);
    }
    if (process) addLog('info', 'select', `已选择 PID ${process.pid} / ${serviceTitle(process)}`, 0);
  }

  function openProcessMenu(event = null) {
    event?.stopPropagation?.();
    showProcessMenu = true;
    if (!processes.length && !loading) loadProcesses(true);
    tick().then(() => processInput?.focus());
  }

  function startProgress({ preserveLogs = false, background = false } = {}) {
    stopProgress();
    progress = background ? Math.max(8, Math.min(78, progress || 8)) : 2;
    if (!preserveLogs) {
      liveLogs = [];
      logSeq = 0;
    }
    addLog('info', background ? 'trace' : 'start', background ? `后台跟踪第 ${trackingRound + 1} 轮开始` : `启动分析: ${serviceTitle(selectedProcess)}`, progress);
    let hintIdx = 0;
    progressTimer = setInterval(() => {
      const target = Math.min(92, (background ? 42 : 24) + hintIdx * 10);
      progress = Math.min(94, progress + Math.max(0.6, (target - progress) * 0.12));
      if (hintIdx < progressHints.length && progress >= 8 + hintIdx * 13) {
        const [stage, message] = progressHints[hintIdx];
        addLog('info', stage, message, progress);
        hintIdx += 1;
      }
    }, 260);
  }

  function stopProgress(done = false) {
    if (progressTimer) {
      clearInterval(progressTimer);
      progressTimer = null;
    }
    if (done) {
      progress = 100;
      addLog('ok', 'done', '分析结果已返回并完成渲染', 100);
    }
  }

  async function runAnalyze(options = {}) {
    pageLeaving = false;
    const background = options?.background === true;
    const pid = selectedPid || selectedProcess?.pid;
    if (!pid) return;
    if (background && trackingUpdating) return;
    if (!background && analyzing) return;
    if (!background) analyzing = true;
    trackingUpdating = background;
    const previousThread = selectedThread;
    if (!background) {
      resetAnalysisData();
    }
    error = '';
    if (background) {
      startProgress({ preserveLogs: true, background: true });
      addLog('info', 'trace', `后台深度采样第 ${trackingRound + 1} 轮执行中，运行时数据继续更新`, progress);
    } else {
      startProgress({ preserveLogs: false, background: false });
    }
    const cancelKey = newCancelKey(background ? 'java-trace' : 'java-analyze');
    try {
      const res = await trackedFetch('/api/java/analyze', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({
          pid,
          samples: background ? TRACE_SAMPLES : DEFAULT_SAMPLES,
          interval_ms: background ? TRACE_INTERVAL_MS : DEFAULT_INTERVAL_MS,
          include_histogram: background ? true : includeHistogram,
          cancel_key: cancelKey,
        }),
      }, cancelKey);
      if (pageLeaving) return;
      if (!res.ok) throw new Error(await res.text() || 'Java 运行时分析失败');
      const nextAnalysis = await res.json();
      if (pageLeaving) return;
      analysis = nextAnalysis;
      selectedThread = restoreSelectedThread(nextAnalysis, previousThread);
      activeRaw = analysis.thread_dump ? 'threads' : 'heap';
      if (background) trackingRound += 1;
      appendServerLogs(nextAnalysis, background ? `轮次 ${trackingRound || 1}` : '');
      rememberTracePoint(nextAnalysis);
      if (background) {
        addLog('ok', 'done', '后台深度采样已完成，诊断数据已局部更新', progress);
        stopProgress(true);
      } else {
        stopProgress(true);
      }
    } catch (e) {
      if (e?.name === 'AbortError') return;
      error = e.message || String(e);
      addLog('error', 'failed', error, progress);
      stopProgress();
    } finally {
      trackingUpdating = false;
      if (!background) analyzing = false;
    }
  }

  async function runFleetScan() {
    pageLeaving = false;
    if (fleetScanning) return;
    fleetScanning = true;
    fleetProgress = 4;
    fleetScan = null;
    error = '';
    activeView = 'exception-summary';
    addLog('info', 'rules', '启动 Java 全进程堆栈规则引擎扫描', 4);
    const cancelKey = newCancelKey('java-fleet');
    const timer = setInterval(() => {
      fleetProgress = Math.min(94, fleetProgress + Math.max(1, (96 - fleetProgress) * 0.08));
      progress = Math.max(progress, fleetProgress);
    }, 260);
    try {
      const res = await trackedFetch('/api/java/scan', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({
          samples: TRACE_SAMPLES,
          interval_ms: TRACE_INTERVAL_MS,
          include_histogram: true,
          max_processes: 128,
          cancel_key: cancelKey,
        }),
      }, cancelKey);
      if (pageLeaving) return;
      if (!res.ok) throw new Error(await res.text() || 'Java 全进程规则扫描失败');
      const data = await res.json();
      if (pageLeaving) return;
      fleetScan = data;
      fleetProgress = Number(data.progress || 100);
      progress = Math.max(progress, fleetProgress);
      appendServerLogs(data, '全进程规则');
      addLog('ok', 'done', `全进程规则扫描完成: ${data.analyzed || 0}/${data.total || 0} 个 Java 进程，${data.warnings || 0} 条警告，${data.errors || 0} 条错误`, 100);
    } catch (e) {
      if (e?.name === 'AbortError') return;
      error = e.message || String(e);
      addLog('error', 'failed', error, progress);
    } finally {
      clearInterval(timer);
      fleetScanning = false;
    }
  }

  function toggleAutoRefresh() {
    pageLeaving = false;
    autoRefresh = !autoRefresh;
    if (!autoRefresh) {
      stopAutoRefresh();
      addLog('info', 'trace', '实时跟踪已停止', progress);
      return;
    }
    resetAnalysisData();
    addLog('info', 'trace', '实时跟踪已开启，旧分析数据已清空，运行时数据将异步持续更新', progress);
    startMetricStream();
    scheduleAutoTrace(0);
  }

  function scheduleAutoTrace(delay) {
    if (autoTimer) clearTimeout(autoTimer);
    autoTimer = setTimeout(async () => {
      if (!autoRefresh || !selectedPid || pageLeaving) return;
      await runAnalyze({ background: true });
      if (autoRefresh && !pageLeaving) scheduleAutoTrace(Math.max(1800, TRACE_SAMPLES * TRACE_INTERVAL_MS + 650));
    }, delay);
  }

  function stopAutoRefresh() {
    autoRefresh = false;
    if (autoTimer) {
      clearTimeout(autoTimer);
      autoTimer = null;
    }
    stopMetricStream();
  }

  function startMetricStream() {
    stopMetricStream();
    appendMetricPoint();
    metricTimer = setInterval(async () => {
      if (!autoRefresh || !selectedPid || pageLeaving) return;
      try {
        const res = await trackedFetch('/api/java/processes?ts=' + Date.now(), { cache: 'no-store' });
        if (!res.ok) return;
        const data = await res.json();
        if (pageLeaving) return;
        const nextProcesses = data.processes || [];
        processes = nextProcesses;
        const proc = nextProcesses.find((p) => p.pid === selectedPid) || selectedProcess;
        if (analysis && proc?.pid === analysis.process?.pid) {
          analysis = { ...analysis, process: { ...analysis.process, ...proc } };
        }
        appendMetricPoint(proc, analysis);
      } catch {
        // 指标流失败时不打断当前分析视图，下一轮继续尝试。
      }
    }, realtimeMetricMs);
  }

  function stopMetricStream() {
    if (metricTimer) {
      clearInterval(metricTimer);
      metricTimer = null;
    }
  }
</script>

<div class="java-profiler">
  <section class="command-bar">
    <div class="brand-strip">
      <span class="brand-orbit" class:active={analyzing || autoRefresh}></span>
      <div>
        <p>运行时观测台</p>
        <h2>Java 堆栈实时分析</h2>
      </div>
    </div>

    <div class="process-combobox">
      <label for="java-process-filter">目标 Java 进程</label>
      <input
        id="java-process-filter"
        bind:this={processInput}
        bind:value={query}
        onfocus={openProcessMenu}
        onpointerdown={openProcessMenu}
        onclick={openProcessMenu}
        placeholder="输入 PID、服务名、路径、端口过滤，点击展开进程列表"
        autocomplete="off"
        spellcheck="false" />
      {#if showProcessMenu}
        <div class="process-menu" role="listbox">
          <div class="process-menu-head">
            <span>{loading ? '正在扫描...' : `${processRows.length}/${processes.length} 个 Java 进程`}</span>
            <button type="button" onclick={(event) => { event.stopPropagation(); loadProcesses(true); }}>重新扫描</button>
          </div>
          {#each processRows as p}
            <button type="button" class="process-option" class:active={p.pid === selectedPid} onclick={(event) => { event.stopPropagation(); selectProcess(p); }}>
              <span class="pid">PID {p.pid}</span>
              <strong>{serviceTitle(p)}</strong>
              <em>{shortText(p.cwd || p.exe || p.cmd, 92)}</em>
              <small>{p.ports?.length ? `监听端口 ${p.ports.join(', ')}` : '无监听端口'} · {p.threads || 0} 线程 · {fmtBytes(p.memory_bytes)}</small>
            </button>
          {:else}
            <div class="process-empty">没有匹配的 Java 进程</div>
          {/each}
        </div>
      {/if}
    </div>

    <div class="control-grid">
      <label class="switch"><input type="checkbox" bind:checked={includeHistogram} /><span>类直方图</span></label>
      <button class="trace-action" class:active={autoRefresh} onclick={toggleAutoRefresh} disabled={!selectedProcess}>
        {autoRefresh ? '停止跟踪' : '实时跟踪'}
      </button>
      <button class="primary-action" onclick={() => runAnalyze()} disabled={!selectedProcess || analyzing}>
        {analyzing ? `分析中 ${Math.round(progress)}%` : '快速分析'}
      </button>
      <button class="export-action reset-action" onclick={resetProfiler} disabled={analyzing || trackingUpdating || fleetScanning}>复位</button>
      <button class="rule-action" onclick={runFleetScan} disabled={fleetScanning || !processes.length}>
        {fleetScanning ? `规则扫描 ${Math.round(fleetProgress)}%` : '全进程规则扫描'}
      </button>
      <button class="export-action" onclick={exportHprofSnapshot} disabled={(!selectedProcess && !analysis) || hprofExporting}>
        {hprofExporting ? '导出HPROF中' : '导出HPROF'}
      </button>
      <button class="export-action" onclick={exportRawData} disabled={!analysis}>导出原始数据</button>
      <button class="export-action" onclick={exportPdfReport} disabled={!analysis}>导出PDF</button>
    </div>
  </section>

  <section class="runtime-strip">
    <div class="progress-line"><i style={`width:${Math.round(progress)}%`}></i></div>
    <div class="runtime-event {latestLog?.level || 'info'}">
      {#if analyzing || trackingUpdating || fleetScanning}<span class="spinner"></span>{/if}
      <b>{latestLog?.stage || '空闲'}</b>
      <em>{Math.round(analysis?.progress ?? progress)}%</em>
      <p>{latestLog?.message || '选择目标 Java 进程后开始实时分析。'}</p>
      <time>{latestLog?.timestamp || lastAnalysisAt || '--:--:--'}</time>
    </div>
  </section>

  {#if error}
    <div class="error-banner"><strong>错误</strong><span>{error}</span></div>
  {/if}

  <section class="profiler-shell">
    <aside class="left-nav">
      {#each menuGroups as group}
        <div class="nav-group">
          <h3>
            <span class="group-icon">
              <svg viewBox="0 0 24 24" aria-hidden="true"><path d={viewIconPath(group.icon)} /></svg>
            </span>
            {group.title}
          </h3>
          {#each group.items as item}
            <button class:active={activeView === item[0]} onclick={() => activeView = item[0]}>
              <span class="menu-icon">
                <svg viewBox="0 0 24 24" aria-hidden="true"><path d={viewIconPath(item[2])} /></svg>
              </span>
              <strong>{item[1]}</strong>
            </button>
          {/each}
        </div>
      {/each}
    </aside>

    <main class="workbench">
      <section class="target-ribbon">
        <article class="target-main">
          <span>当前目标</span>
          <strong>{selectedProcess ? serviceTitle(selectedProcess) : '未选择 Java 进程'}</strong>
          <em title={selectedProcess?.cmd || ''}>{selectedProcess ? shortText(selectedProcess.cmd, 140) : '点击顶部过滤框选择运行中的 Java 进程。'}</em>
        </article>
        <article><span>进程号</span><strong>{selectedProcess?.pid || '-'}</strong><em>{selectedProcess?.user || '-'}</em></article>
        <article><span>监听端口</span><strong>{selectedProcess?.ports?.length ? selectedProcess.ports.join(', ') : '-'}</strong><em>{selectedProcess?.uptime || '-'}</em></article>
        <article><span>附加状态</span><strong>{analysis ? (analysis.attach_ok ? '正常' : '失败') : selectedProcess?.attach_available ? '就绪' : '待确认'}</strong><em>{shortText(selectedProcess?.attach_path, 36)}</em></article>
      </section>

      <div class="view-head">
        <div>
          <span>当前视图</span>
          <h3>{activeViewTitle}</h3>
        </div>
        <div class="view-status">
          <span>{trackingRound ? `跟踪 ${trackingRound} 轮` : '单次快照'}</span>
          <strong>{lastAnalysisAt || '未分析'}</strong>
        </div>
      </div>

      <div class="table-toolbar">
        <label for="java-table-search">数据搜索</label>
        <input id="java-table-search" bind:value={tableSearch} placeholder="搜索方法、线程、类、状态、异常结论" spellcheck="false" />
        <span>列头可点击排序，默认按关键数值降序</span>
      </div>

      {#if activeView === 'telemetry-overview'}
        <section class="telemetry-grid">
          {#each telemetryCards as card}
            <article class="metric-card {card.tone}">
              <span>{card.label}</span>
              <strong>{card.value}</strong>
              <em>{card.hint}</em>
            </article>
          {/each}
        </section>
        <section class="two-columns">
          <div class="panel chart-panel wide">
            <div class="panel-title"><h3>内存、线程、CPU 趋势</h3><span>{chartPoints.length} 个采样点</span></div>
            <div class="chart-legend">
              <span class="memory">内存 {latestMetric('memory')}</span>
              <span class="threads">线程 {latestMetric('threads')}</span>
              <span class="cpu">CPU {latestMetric('cpu')}</span>
            </div>
            <div class="chart-stage">
              <svg role="img" aria-label="内存线程CPU趋势图" viewBox="0 0 520 118" preserveAspectRatio="none" onmousemove={(event) => showChartTooltip('overview', chartPoints, ['memory', 'threads', 'cpu'], event)} onmouseleave={hideChartTooltip}>
                <path class="grid" d="M8 28 H512 M8 58 H512 M8 88 H512" />
                <polyline class="line memory" points={polyline(chartPoints, 'memory')} />
                <polyline class="line threads" points={polyline(chartPoints, 'threads')} />
                <polyline class="line cpu" points={polyline(chartPoints, 'cpu')} />
              </svg>
              {#if chartHover?.chart === 'overview'}
                <div class="chart-tooltip" style={`left:${chartHover.x}px;top:${chartHover.y}px`}>
                  <strong>{chartHover.point.time}</strong>
                  {#each chartHover.metrics as metric}
                    <span>{metricLabel(metric)} <b>{metricValue(chartHover.point, metric)}</b></span>
                  {/each}
                </div>
              {/if}
            </div>
          </div>
          <div class="panel">
            <div class="panel-title"><h3>诊断结论</h3><span>{exceptionRows.length} 条</span></div>
            <div class="finding-list compact">
              {#each exceptionRows as item}
                <button type="button" class={`finding-item ${item.level || 'info'}`} onclick={() => openFinding(item)}>
                  <b>{levelLabel(item.level)}</b>
                  <div><strong>{item.title}</strong><p>{item.detail}</p><em>{item.suggestion}</em></div>
                </button>
              {:else}
                <div class="empty-note">执行分析后展示综合诊断。</div>
              {/each}
            </div>
          </div>
        </section>
      {:else if activeView === 'telemetry-memory'}
        <section class="single-column">
          <div class="panel chart-panel tall">
            <div class="panel-title"><h3>内存随时间变化</h3><span>进程内存与类直方图字节数</span></div>
            <div class="chart-legend">
              <span class="memory">进程内存 {latestMetric('memory')}</span>
              <span class="objects">对象字节 {latestMetric('bytes')}</span>
            </div>
            <div class="chart-stage">
              <svg role="img" aria-label="内存趋势图" viewBox="0 0 520 118" preserveAspectRatio="none" onmousemove={(event) => showChartTooltip('memory', chartPoints, ['memory', 'bytes', 'objects'], event)} onmouseleave={hideChartTooltip}>
                <path class="grid" d="M8 28 H512 M8 58 H512 M8 88 H512" />
                <polyline class="line memory" points={polyline(chartPoints, 'memory')} />
                <polyline class="line objects" points={polyline(chartPoints, 'bytes')} />
              </svg>
              {#if chartHover?.chart === 'memory'}
                <div class="chart-tooltip" style={`left:${chartHover.x}px;top:${chartHover.y}px`}>
                  <strong>{chartHover.point.time}</strong>
                  {#each chartHover.metrics as metric}
                    <span>{metricLabel(metric)} <b>{metricValue(chartHover.point, metric)}</b></span>
                  {/each}
                </div>
              {/if}
            </div>
          </div>
          <div class="panel">
            <div class="panel-title"><h3>堆概要</h3><span>{heapCards.length} 项</span></div>
            <div class="heap-list">
              {#each heapCards as item}
                <div><span>{item.label}</span><strong>{item.value}</strong></div>
              {:else}
                <div class="empty-note">执行分析后展示堆概要。</div>
              {/each}
            </div>
          </div>
        </section>
      {:else if activeView === 'telemetry-threads' || activeView === 'thread-history'}
        <section class="single-column">
          <div class="panel chart-panel tall">
            <div class="panel-title"><h3>线程状态历史</h3><span>{threadPoints.length} 个采样点</span></div>
            <div class="chart-legend">
              <span class="runnable">运行 {stateCount('RUNNABLE')}</span>
              <span class="blocked">阻塞 {stateCount('BLOCKED')}</span>
              <span class="waiting">等待 {stateCount('WAITING') + stateCount('TIMED_WAITING')}</span>
            </div>
            <div class="chart-stage">
              <svg role="img" aria-label="线程状态历史图" viewBox="0 0 520 118" preserveAspectRatio="none" onmousemove={(event) => showChartTooltip('threads', threadPoints, ['runnable', 'blocked', 'waiting', 'timed', 'threads'], event)} onmouseleave={hideChartTooltip}>
                <path class="grid" d="M8 28 H512 M8 58 H512 M8 88 H512" />
                <polyline class="line runnable" points={polyline(threadPoints, 'runnable')} />
                <polyline class="line blocked" points={polyline(threadPoints, 'blocked')} />
                <polyline class="line waiting" points={polyline(threadPoints, 'waiting')} />
              </svg>
              {#if chartHover?.chart === 'threads'}
                <div class="chart-tooltip" style={`left:${chartHover.x}px;top:${chartHover.y}px`}>
                  <strong>{chartHover.point.time}</strong>
                  {#each chartHover.metrics as metric}
                    <span>{metricLabel(metric)} <b>{metricValue(chartHover.point, metric)}</b></span>
                  {/each}
                </div>
              {/if}
            </div>
          </div>
          <div class="panel">
            <div class="panel-title"><h3>线程状态矩阵</h3><span>{filteredThreads.length}/{analysis?.threads?.length || 0}</span></div>
            <div class="state-tabs">
              {#each stateLabels as state}
                <button class:active={activeState === state} onclick={() => activeState = state}>{stateLabel(state)} {state === 'ALL' ? analysis?.threads?.length || 0 : stateCount(state)}</button>
              {/each}
            </div>
            <div class="thread-list">
              <div class="thread-head"><button onclick={() => setSort('threads', 'state')}>状态 {sortMark('threads', 'state')}</button><button onclick={() => setSort('threads', 'name')}>线程 {sortMark('threads', 'name')}</button><button onclick={() => setSort('threads', 'depth')}>栈深 {sortMark('threads', 'depth')}</button><button onclick={() => setSort('threads', 'top_frame')}>顶部调用 {sortMark('threads', 'top_frame')}</button></div>
              {#each displayThreads as thread}
                <button class="thread-row" class:active={isSelectedThread(thread)} onclick={() => selectedThread = thread}>
                  <span class={`state ${String(thread.state || '').toLowerCase()}`}>{stateLabel(thread.state)}</span>
                  <strong>{thread.name}</strong>
                  <b>{thread.depth}</b>
                  <em>{thread.top_frame || '-'}</em>
                </button>
              {:else}
                <div class="empty-note">执行分析后展示线程状态。</div>
              {/each}
            </div>
          </div>
        </section>
      {:else if activeView === 'telemetry-cpu'}
        <section class="single-column">
          <div class="panel chart-panel tall">
            <div class="panel-title"><h3>CPU 负载趋势</h3><span>进程时间片变化</span></div>
            <div class="chart-stage">
              <svg role="img" aria-label="CPU负载趋势图" viewBox="0 0 520 118" preserveAspectRatio="none" onmousemove={(event) => showChartTooltip('cpu', chartPoints, ['cpu', 'runnable', 'threads'], event)} onmouseleave={hideChartTooltip}>
                <path class="grid" d="M8 28 H512 M8 58 H512 M8 88 H512" />
                <polyline class="line cpu" points={polyline(chartPoints, 'cpu')} />
                <polyline class="line runnable" points={polyline(chartPoints, 'runnable')} />
              </svg>
              {#if chartHover?.chart === 'cpu'}
                <div class="chart-tooltip" style={`left:${chartHover.x}px;top:${chartHover.y}px`}>
                  <strong>{chartHover.point.time}</strong>
                  {#each chartHover.metrics as metric}
                    <span>{metricLabel(metric)} <b>{metricValue(chartHover.point, metric)}</b></span>
                  {/each}
                </div>
              {/if}
            </div>
          </div>
          <div class="panel profile-panel">
            <div class="panel-title"><h3>CPU 热点方法</h3><span>按采样权重排序</span></div>
            <div class="profile-table">
              <div class="profile-head"><button onclick={() => setSort('profile', 'method')}>方法 {sortMark('profile', 'method')}</button><button onclick={() => setSort('profile', 'category')}>类型 {sortMark('profile', 'category')}</button><button onclick={() => setSort('profile', 'count')}>命中 {sortMark('profile', 'count')}</button><button onclick={() => setSort('profile', 'percent')}>占比 {sortMark('profile', 'percent')}</button><button onclick={() => setSort('profile', 'estimateMs')}>估算耗时 {sortMark('profile', 'estimateMs')}</button><button onclick={() => setSort('profile', 'weight')}>权重 {sortMark('profile', 'weight')}</button></div>
              {#each displayProfileRows as frame}
                <div class="profile-row" title={frame.method}>
                  <strong>{methodShort(frame.method)}</strong>
                  <span>{categoryLabel(frame.category)}</span>
                  <span>{frame.count}</span>
                  <span>{frame.percent}%</span>
                  <span>{profileMs(frame.estimateMs)}</span>
                  <div class="weight"><i style={`width:${frame.score}%`}></i><em>{frame.weight}</em></div>
                </div>
              {:else}
                <div class="empty-note">执行分析后展示热点方法。</div>
              {/each}
            </div>
          </div>
        </section>
      {:else if activeView === 'memory-objects' || activeView === 'memory-recorded' || activeView === 'memory-class-tracker'}
        <section class="panel">
          <div class="panel-title">
            <h3>{activeView === 'memory-recorded' ? '记录的对象' : activeView === 'memory-class-tracker' ? '类跟踪器' : '所有对象'}</h3>
            <span>{(activeView === 'memory-recorded' ? recordedObjectRows : objectRows).length} 条</span>
          </div>
          <div class="object-table">
            <div class="object-head"><button onclick={() => setSort(activeView === 'memory-recorded' ? 'recorded' : 'objects', 'className')}>类名 {sortMark(activeView === 'memory-recorded' ? 'recorded' : 'objects', 'className')}</button><button onclick={() => setSort(activeView === 'memory-recorded' ? 'recorded' : 'objects', 'instances')}>实例数 {sortMark(activeView === 'memory-recorded' ? 'recorded' : 'objects', 'instances')}</button><button onclick={() => setSort(activeView === 'memory-recorded' ? 'recorded' : 'objects', 'bytes')}>字节 {sortMark(activeView === 'memory-recorded' ? 'recorded' : 'objects', 'bytes')}</button><button onclick={() => setSort(activeView === 'memory-recorded' ? 'recorded' : 'objects', 'percent')}>占比 {sortMark(activeView === 'memory-recorded' ? 'recorded' : 'objects', 'percent')}</button><span>趋势</span></div>
            {#each activeView === 'memory-recorded' ? displayRecordedObjectRows : displayObjectRows as row}
              <div class="object-row" title={row.className}>
                <strong>{row.className}</strong>
                <span>{formatNumber(row.instances)}</span>
                <span>{fmtBytes(row.bytes)}</span>
                <span>{row.percent}%</span>
                <div class="weight"><i style={`width:${Math.max(3, row.percent)}%`}></i><em>{row.percent}%</em></div>
              </div>
            {:else}
              <div class="empty-note">开启类直方图并执行分析后展示对象分布。</div>
            {/each}
          </div>
        </section>
      {:else if activeView === 'memory-allocation-tree' || activeView === 'cpu-call-tree'}
        <section class="panel call-tree-panel">
          <div class="panel-title call-tree-title">
            <div>
              <h3>{activeView === 'cpu-call-tree' ? 'CPU 调用树' : '分配调用树'}</h3>
              <span>按估算耗时、权重和命中次数动态排序，实时跟踪时自动更新</span>
            </div>
            <div class="tree-actions">
              <button type="button" onclick={expandCallTree} disabled={!callTreeRows.length}>展开到底</button>
              <button type="button" onclick={collapseCallTree} disabled={!callTreeRows.length}>全部收起</button>
              <em>{callTreeRows.length}/{callTreeTotal} 节点</em>
            </div>
          </div>
          <div class="call-tree-table">
            <div class="call-tree-head">
              <button onclick={() => setSort('callTree', 'name')}>调用节点 {sortMark('callTree', 'name')}</button>
              <button onclick={() => setSort('callTree', 'category')}>类型 {sortMark('callTree', 'category')}</button>
              <button onclick={() => setSort('callTree', 'count')}>命中 {sortMark('callTree', 'count')}</button>
              <button onclick={() => setSort('callTree', 'percent')}>占比 {sortMark('callTree', 'percent')}</button>
              <button onclick={() => setSort('callTree', 'estimateMs')}>估算耗时 {sortMark('callTree', 'estimateMs')}</button>
              <button onclick={() => setSort('callTree', 'weight')}>权重 {sortMark('callTree', 'weight')}</button>
            </div>
            {#each callTreeRows as node}
              <button type="button" class="call-tree-row" class:leaf={!node.hasChildren} class:open={node.open} style={`--level:${Math.min(8, node.level)}`} title={node.leafMethod || node.path} onclick={() => toggleTreeNode(node)}>
                <span class="tree-node-main">
                  <i>{node.hasChildren ? (node.open ? '−' : '+') : '·'}</i>
                  <strong>{node.name}</strong>
                  <small>{shortText(node.path, 120)}</small>
                </span>
                <span>{categoryLabel(node.category)}</span>
                <span>{node.count}</span>
                <span>{node.percent}%</span>
                <b>{profileMs(node.estimateMs)}</b>
                <div class="weight"><i style={`width:${Math.max(3, node.score)}%`}></i><em>{node.weight}</em></div>
              </button>
            {:else}
              <div class="empty-note">执行分析后展示调用树。</div>
            {/each}
          </div>
        </section>
      {:else if activeView === 'memory-allocation-hot' || activeView === 'cpu-hotspots'}
        <section class="panel profile-panel">
          <div class="panel-title"><h3>{activeView === 'cpu-hotspots' ? 'CPU 热点' : '分配热点'}</h3><span>命中率、估算耗时、权重</span></div>
          <div class="profile-table">
            <div class="profile-head"><button onclick={() => setSort('profile', 'method')}>方法 {sortMark('profile', 'method')}</button><button onclick={() => setSort('profile', 'category')}>类型 {sortMark('profile', 'category')}</button><button onclick={() => setSort('profile', 'count')}>命中 {sortMark('profile', 'count')}</button><button onclick={() => setSort('profile', 'percent')}>占比 {sortMark('profile', 'percent')}</button><button onclick={() => setSort('profile', 'estimateMs')}>估算耗时 {sortMark('profile', 'estimateMs')}</button><button onclick={() => setSort('profile', 'weight')}>权重 {sortMark('profile', 'weight')}</button></div>
            {#each displayProfileRows as frame}
              <div class="profile-row" title={frame.method}>
                <strong>{methodShort(frame.method)}</strong>
                <span>{categoryLabel(frame.category)}</span>
                <span>{frame.count}</span>
                <span>{frame.percent}%</span>
                <span>{profileMs(frame.estimateMs)}</span>
                <div class="weight"><i style={`width:${frame.score}%`}></i><em>{frame.weight}</em></div>
              </div>
            {:else}
              <div class="empty-note">执行分析后展示热点列表。</div>
            {/each}
          </div>
        </section>
      {:else if activeView === 'cpu-anomaly' || activeView === 'exception-summary'}
        <section class="panel">
          <div class="panel-title"><h3>{activeView === 'exception-summary' ? '异常综合分析' : '异常检测值'}</h3><span>{exceptionRows.length} 条</span></div>
          {#if activeView === 'exception-summary'}
            <div class="fleet-scan-strip">
              <div>
                <span>全进程规则引擎</span>
                <strong>{fleetScan ? `${fleetScan.analyzed || 0}/${fleetScan.total || 0} 个进程` : fleetScanning ? '扫描中' : '未扫描'}</strong>
                <em>{fleetScan ? `${fleetScan.warnings || 0} 条警告 / ${fleetScan.errors || 0} 条错误` : '扫描所有运行中的 Java 进程，识别锁、CPU、OOM、堆、外部依赖等风险。'}</em>
              </div>
              <button type="button" onclick={runFleetScan} disabled={fleetScanning || !processes.length}>{fleetScanning ? `${Math.round(fleetProgress)}%` : '立即扫描'}</button>
            </div>
            {#if fleetFindingRows.length}
              <div class="fleet-finding-table">
                <div class="fleet-finding-head"><span>级别</span><span>进程</span><span>异常项目</span><span>处理建议</span></div>
                {#each fleetFindingRows as item}
                  <button type="button" class={`fleet-finding-row ${item.level || 'info'}`} onclick={() => openFinding(item)}>
                    <span>{levelLabel(item.level)}</span>
                    <strong>PID {item.pid} · {item.service}</strong>
                    <em>{item.title} · {item.detail}</em>
                    <small>{item.suggestion}</small>
                  </button>
                {/each}
              </div>
            {/if}
          {/if}
          <div class="finding-list">
            {#each exceptionRows as item}
              <button type="button" class={`finding-item ${item.level || 'info'}`} onclick={() => openFinding(item)}>
                <b>{levelLabel(item.level)}</b>
                <div><strong>{item.title}</strong><p>{item.detail}</p><em>{item.suggestion}</em></div>
              </button>
            {:else}
              <div class="empty-note">执行分析后展示异常检测值。</div>
            {/each}
          </div>
        </section>
      {:else if activeView === 'cpu-complexity'}
        <section class="panel">
          <div class="panel-title"><h3>复杂度分析</h3><span>按线程栈深度和状态评估</span></div>
          <div class="complexity-list">
            <div class="complexity-head"><button onclick={() => setSort('complexity', 'state')}>状态 {sortMark('complexity', 'state')}</button><button onclick={() => setSort('complexity', 'name')}>线程 {sortMark('complexity', 'name')}</button><button onclick={() => setSort('complexity', 'depth')}>栈深 {sortMark('complexity', 'depth')}</button><button onclick={() => setSort('complexity', 'risk')}>风险 {sortMark('complexity', 'risk')}</button><button onclick={() => setSort('complexity', 'top_frame')}>顶部调用 {sortMark('complexity', 'top_frame')}</button></div>
            {#each displayComplexityRows as thread}
              <button class="complexity-row" class:active={isSelectedThread(thread)} onclick={() => selectedThread = thread}>
                <span class={`state ${String(thread.state || '').toLowerCase()}`}>{stateLabel(thread.state)}</span>
                <strong>{thread.name}</strong>
                <em>栈深 {thread.depth}</em>
                <b>{thread.risk}</b>
                <small>{thread.top_frame || '-'}</small>
              </button>
            {:else}
              <div class="empty-note">执行分析后展示复杂度分析。</div>
            {/each}
          </div>
        </section>
      {:else if activeView === 'thread-monitor'}
        <section class="two-columns">
          <div class="panel">
            <div class="panel-title"><h3>线程监视器</h3><span>{filteredThreads.length} 个线程</span></div>
            <div class="thread-list">
              <div class="thread-head"><button onclick={() => setSort('threads', 'state')}>状态 {sortMark('threads', 'state')}</button><button onclick={() => setSort('threads', 'name')}>线程 {sortMark('threads', 'name')}</button><button onclick={() => setSort('threads', 'depth')}>栈深 {sortMark('threads', 'depth')}</button><button onclick={() => setSort('threads', 'top_frame')}>顶部调用 {sortMark('threads', 'top_frame')}</button></div>
              {#each displayThreads as thread}
                <button class="thread-row" class:active={isSelectedThread(thread)} onclick={() => selectedThread = thread}>
                  <span class={`state ${String(thread.state || '').toLowerCase()}`}>{stateLabel(thread.state)}</span>
                  <strong>{thread.name}</strong>
                  <b>{thread.depth}</b>
                  <em>{thread.top_frame || '-'}</em>
                </button>
              {:else}
                <div class="empty-note">执行分析后展示线程监视器。</div>
              {/each}
            </div>
          </div>
          <div class="panel stack-panel">
            <div class="panel-title"><h3>选中线程堆栈</h3><span>{selectedThread?.name || '未选择'}</span></div>
            <div class="stack-list">
              {#if selectedThread}
                {#each selectedThreadFrames as frame, index}
                  <div class="frame">
                    <span>{String(index + 1).padStart(2, '0')}</span>
                    <strong>{frame.class_method}</strong>
                    <em>{frame.file_line || '-'}</em>
                  </div>
                {/each}
              {:else}
                <div class="empty-note">选择线程后查看完整堆栈帧。</div>
              {/if}
            </div>
          </div>
        </section>
      {:else if activeView === 'lock-state' || activeView === 'lock-monitor'}
        <section class="two-columns">
          <div class="panel lock-map">
            <div class="panel-title"><h3>{activeView === 'lock-state' ? '当前锁状态图' : '当前监视器'}</h3><span>{lockThreads.length} 个相关线程</span></div>
            <div class="lock-nodes">
              <div class="lock-core">
                <strong>{stateCount('BLOCKED')}</strong>
                <span>阻塞线程</span>
              </div>
              {#each lockThreads.slice(0, 16) as thread}
                <button class="lock-node" class:active={isSelectedThread(thread)} onclick={() => selectedThread = thread}>
                  <b>{stateLabel(thread.state)}</b>
                  <strong>{shortText(thread.name, 24)}</strong>
                  <em>{shortText(thread.top_frame, 54)}</em>
                </button>
              {:else}
                <div class="empty-note">当前采样未发现明显锁等待。</div>
              {/each}
            </div>
          </div>
          <div class="panel stack-panel">
            <div class="panel-title"><h3>锁相关堆栈</h3><span>{selectedThread?.name || '未选择'}</span></div>
            <div class="stack-list">
              {#if selectedThread}
                {#each selectedThreadFrames as frame, index}
                  <div class="frame">
                    <span>{String(index + 1).padStart(2, '0')}</span>
                    <strong>{frame.class_method}</strong>
                    <em>{frame.file_line || '-'}</em>
                  </div>
                {/each}
              {:else}
                <div class="empty-note">选择锁相关线程后查看堆栈。</div>
              {/if}
            </div>
          </div>
        </section>
      {:else if activeView === 'db-jdbc' || activeView === 'db-es' || activeView === 'db-redis'}
        <section class="panel profile-panel">
          <div class="panel-title">
            <h3>{activeView === 'db-jdbc' ? 'JDBC 调用分析' : activeView === 'db-es' ? 'ES 调用分析' : 'REDIS 调用分析'}</h3>
            <span>{activeView === 'db-jdbc' ? dbRows.jdbc.length : activeView === 'db-es' ? dbRows.es.length : dbRows.redis.length} 个热点</span>
          </div>
          <div class="profile-table">
            <div class="profile-head"><button onclick={() => setSort('db', 'method')}>调用方法 {sortMark('db', 'method')}</button><button onclick={() => setSort('db', 'category')}>类型 {sortMark('db', 'category')}</button><button onclick={() => setSort('db', 'count')}>命中 {sortMark('db', 'count')}</button><button onclick={() => setSort('db', 'percent')}>占比 {sortMark('db', 'percent')}</button><button onclick={() => setSort('db', 'estimateMs')}>估算耗时 {sortMark('db', 'estimateMs')}</button><button onclick={() => setSort('db', 'weight')}>权重 {sortMark('db', 'weight')}</button></div>
            {#each displayDbRows as frame}
              <div class="profile-row" title={frame.method}>
                <strong>{methodShort(frame.method)}</strong>
                <span>{categoryLabel(frame.category)}</span>
                <span>{frame.count}</span>
                <span>{frame.percent}%</span>
                <span>{profileMs(frame.estimateMs)}</span>
                <div class="weight"><i style={`width:${frame.score}%`}></i><em>{frame.weight}</em></div>
              </div>
            {:else}
              <div class="empty-note">本次采样未发现对应数据库调用热点。</div>
            {/each}
          </div>
        </section>
      {/if}

      {#if analysis}
        <details class="panel raw-panel">
          <summary>
            <span>原始运行时数据</span>
            <em>默认折叠，展开后查看线程转储、堆概要和类直方图</em>
          </summary>
          <div class="raw-tabs">
            <button class:active={activeRaw === 'threads'} onclick={() => activeRaw = 'threads'}>线程转储</button>
            <button class:active={activeRaw === 'heap'} onclick={() => activeRaw = 'heap'}>堆概要</button>
            <button class:active={activeRaw === 'histogram'} onclick={() => activeRaw = 'histogram'}>类直方图</button>
          </div>
          <pre>{activeRaw === 'heap' ? analysis.heap_info || '暂无数据' : activeRaw === 'histogram' ? analysis.class_histogram || '暂无数据' : analysis.thread_dump || '暂无数据'}</pre>
        </details>
      {/if}
    </main>
  </section>

  {#if selectedFinding}
    <div class="finding-modal-layer" role="presentation">
      <div class="finding-modal" role="dialog" aria-modal="true" aria-label="异常详情">
        <header>
          <div>
            <span>{levelLabel(selectedFinding.level)}</span>
            <h3>{selectedFinding.title}</h3>
          </div>
          <button type="button" aria-label="关闭异常详情" onclick={closeFinding}>×</button>
        </header>
        <div class="finding-modal-body">
          <article>
            <b>异常说明</b>
            <p>{selectedFinding.detail || '暂无详细说明。'}</p>
          </article>
          <article>
            <b>处理建议</b>
            <p>{selectedFinding.suggestion || '暂无处理建议。'}</p>
          </article>
          <article>
            <b>证据链路</b>
            <div class="evidence-list">
              {#each selectedFinding.evidence || [] as item, index}
                <code>{String(index + 1).padStart(2, '0')} {item}</code>
              {:else}
                <code>暂无更多证据，建议先执行快速分析或开启实时跟踪。</code>
              {/each}
            </div>
          </article>
          <article>
            <b>目标上下文</b>
            <p>PID {selectedPid || analysis?.process?.pid || '-'} · {serviceTitle(selectedProcess || analysis?.process)} · {activeViewTitle}</p>
          </article>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .java-profiler {
    width: 100%;
    height: calc(100vh - 74px);
    min-height: 700px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    overflow: hidden;
    color: #dbeafe;
    background:
      linear-gradient(rgba(34, 211, 238, .024) 1px, transparent 1px),
      linear-gradient(90deg, rgba(52, 211, 153, .018) 1px, transparent 1px);
    background-size: 24px 24px;
  }

  .java-profiler button { cursor: default; }
  .java-profiler input { cursor: text; }

  .command-bar,
  .runtime-strip,
  .target-ribbon,
  .panel,
  .view-head,
  .error-banner {
    border: 1px solid rgba(34, 211, 238, .16);
    border-radius: 10px;
    background: linear-gradient(135deg, rgba(2, 6, 23, .90), rgba(8, 47, 73, .24));
    box-shadow: 0 14px 34px rgba(2, 8, 23, .24);
  }

  .command-bar {
    display: grid;
    grid-template-columns: 190px minmax(360px, 1fr) max-content;
    gap: 10px;
    align-items: end;
    padding: 9px 10px;
    position: relative;
    z-index: 60;
    overflow: visible;
  }

  .brand-strip {
    display: flex;
    align-items: center;
    gap: 9px;
    min-width: 0;
  }

  .brand-orbit {
    position: relative;
    width: 36px;
    height: 36px;
    display: inline-grid;
    place-items: center;
    border-radius: 50%;
    border: 1px solid rgba(34, 211, 238, .44);
    box-shadow: inset 0 0 18px rgba(34, 211, 238, .10), 0 0 22px rgba(34, 211, 238, .10);
  }

  .brand-orbit::before,
  .brand-orbit::after {
    content: "";
    position: absolute;
    inset: 5px;
    border-radius: 50%;
    border: 1px solid transparent;
    border-top-color: #22d3ee;
    border-right-color: rgba(250, 204, 21, .86);
    animation: spin 1.1s linear infinite;
  }

  .brand-orbit::after {
    inset: 12px;
    animation-duration: .72s;
    animation-direction: reverse;
    border-top-color: #34d399;
  }

  .brand-orbit.active::before,
  .brand-orbit.active::after {
    animation-duration: .44s;
  }

  .brand-strip p,
  .process-combobox label,
  .control-grid label span,
  .target-ribbon span,
  .view-head span,
  .metric-card span {
    margin: 0;
    color: #93c5fd;
    font-size: 10px;
    font-weight: 900;
  }

  .brand-strip h2 {
    margin: 1px 0 0;
    overflow: hidden;
    color: #f8fafc;
    font-size: 15px;
    line-height: 1.15;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .process-combobox {
    position: relative;
    display: grid;
    gap: 4px;
    min-width: 0;
  }

  .process-combobox input,
  .control-grid input {
    height: 31px;
    box-sizing: border-box;
    border: 1px solid rgba(34, 211, 238, .22);
    border-radius: 8px;
    background: rgba(2, 6, 23, .58);
    color: #f8fafc;
    outline: none;
    padding: 0 10px;
    font-size: 12px;
  }

  .process-combobox input:focus,
  .control-grid input:focus {
    border-color: rgba(250, 204, 21, .44);
    box-shadow: 0 0 0 3px rgba(250, 204, 21, .09);
  }

  .process-menu {
    position: absolute;
    left: 0;
    right: 0;
    top: calc(100% + 6px);
    z-index: 80;
    max-height: 420px;
    overflow: auto;
    padding: 7px;
    border: 1px solid rgba(34, 211, 238, .26);
    border-radius: 12px;
    background: linear-gradient(145deg, rgba(2, 6, 23, .98), rgba(8, 24, 34, .98));
    box-shadow: 0 24px 72px rgba(0, 0, 0, .52), inset 0 0 26px rgba(34, 211, 238, .04);
  }

  .process-menu-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 2px 2px 7px;
    color: #94a3b8;
    font-size: 11px;
  }

  .process-menu-head button,
  .primary-action,
  .trace-action,
  .state-tabs button,
  .raw-tabs button {
    border: 1px solid rgba(34, 211, 238, .22);
    border-radius: 7px;
    background: rgba(15, 23, 42, .48);
    color: #cbd5e1;
    font-size: 10px;
    font-weight: 900;
  }

  .process-option {
    width: 100%;
    min-height: 58px;
    display: grid;
    grid-template-columns: 74px minmax(140px, .8fr) minmax(0, 1.2fr);
    gap: 8px;
    align-items: center;
    margin-bottom: 5px;
    border: 1px solid rgba(148, 163, 184, .10);
    border-radius: 9px;
    background: rgba(15, 23, 42, .48);
    color: #dbeafe;
    text-align: left;
  }

  .process-option:hover,
  .process-option.active {
    border-color: rgba(34, 211, 238, .44);
    background: linear-gradient(135deg, rgba(8, 145, 178, .22), rgba(15, 118, 110, .12));
  }

  .process-option .pid {
    display: inline-grid;
    place-items: center;
    min-height: 24px;
    border-radius: 7px;
    background: rgba(34, 211, 238, .10);
    color: #67e8f9;
    font-family: var(--theme-font-family-mono);
    font-size: 10px;
    font-weight: 900;
  }

  .process-option strong,
  .process-option em,
  .process-option small {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .process-option strong {
    color: #f8fafc;
    font-size: 12px;
  }

  .process-option em {
    color: #94a3b8;
    font-family: var(--theme-font-family-mono);
    font-size: 10px;
    font-style: normal;
  }

  .process-option small {
    grid-column: 2 / -1;
    color: #a7f3d0;
    font-family: var(--theme-font-family-mono);
    font-size: 10px;
  }

  .process-empty,
  .empty-note {
    display: grid;
    place-items: center;
    min-height: 96px;
    color: #64748b;
    font-size: 12px;
    text-align: center;
  }

  .control-grid {
    display: flex;
    flex-wrap: nowrap;
    gap: 7px;
    align-items: end;
    min-width: max-content;
  }

  .control-grid label {
    display: grid;
    gap: 4px;
  }

  .switch {
    height: 31px;
    display: inline-flex !important;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 0 8px;
    border: 1px solid rgba(34, 211, 238, .18);
    border-radius: 8px;
    background: rgba(2, 6, 23, .36);
    white-space: nowrap;
  }

  .switch input {
    width: 14px;
    height: 14px;
    padding: 0;
    accent-color: #22d3ee;
  }

  .primary-action,
  .trace-action,
  .rule-action,
  .export-action {
    height: 31px;
    padding: 0 12px;
    white-space: nowrap;
  }

  .primary-action {
    border: 0;
    color: #ecfeff;
    background: linear-gradient(135deg, #0891b2, #0f766e);
    box-shadow: 0 0 22px rgba(34, 211, 238, .18);
  }

  .trace-action.active,
  .rule-action.active,
  .state-tabs button.active,
  .raw-tabs button.active {
    border-color: rgba(250, 204, 21, .55);
    color: #fde68a;
    background: rgba(250, 204, 21, .08);
  }

  .primary-action:disabled,
  .trace-action:disabled,
  .rule-action:disabled,
  .export-action:disabled {
    opacity: .46;
  }

  .rule-action {
    border: 1px solid rgba(250, 204, 21, .30);
    border-radius: 7px;
    color: #fde68a;
    background: rgba(250, 204, 21, .08);
    font-size: 10px;
    font-weight: 900;
  }

  .export-action {
    border: 1px solid rgba(34, 211, 238, .22);
    border-radius: 7px;
    background: rgba(15, 23, 42, .48);
    color: #bae6fd;
    font-size: 10px;
    font-weight: 900;
  }

  .table-toolbar {
    display: grid;
    grid-template-columns: 74px minmax(220px, 420px) minmax(0, 1fr);
    gap: 8px;
    align-items: center;
    padding: 7px 10px;
    border: 1px solid rgba(34, 211, 238, .14);
    border-radius: 10px;
    background: rgba(2, 6, 23, .36);
  }

  .table-toolbar label,
  .table-toolbar span {
    color: #93c5fd;
    font-size: 10px;
    font-weight: 900;
  }

  .table-toolbar input {
    height: 28px;
    border: 1px solid rgba(34, 211, 238, .20);
    border-radius: 7px;
    background: rgba(2, 6, 23, .58);
    color: #f8fafc;
    outline: none;
    padding: 0 10px;
    font-size: 11px;
  }

  .runtime-strip {
    display: grid;
    grid-template-rows: 4px 26px;
    overflow: hidden;
  }

  .progress-line {
    height: 4px;
    overflow: hidden;
    background: rgba(15, 23, 42, .72);
  }

  .progress-line i {
    display: block;
    height: 100%;
    border-radius: 999px;
    background: linear-gradient(90deg, #22d3ee, #34d399, #facc15);
    box-shadow: 0 0 20px rgba(34, 211, 238, .42);
    transition: width .22s ease;
  }

  .runtime-event {
    display: grid;
    grid-template-columns: auto auto 44px minmax(0, 1fr) 74px;
    gap: 8px;
    align-items: center;
    padding: 0 10px;
    color: #cbd5e1;
    font-family: var(--theme-font-family-mono);
    font-size: 10px;
  }

  .runtime-event b {
    color: #67e8f9;
  }

  .runtime-event em {
    color: #fde68a;
    font-style: normal;
    text-align: right;
  }

  .runtime-event p {
    margin: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .runtime-event time {
    color: #64748b;
    text-align: right;
  }

  .runtime-event.ok p { color: #bbf7d0; }
  .runtime-event.warn p { color: #fde68a; }
  .runtime-event.error p { color: #fecaca; }

  .spinner {
    width: 11px;
    height: 11px;
    border-radius: 50%;
    border: 2px solid rgba(34, 211, 238, .18);
    border-top-color: #22d3ee;
    border-right-color: #facc15;
    animation: spin .56s linear infinite;
  }

  .error-banner {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 7px 10px;
    border-color: rgba(239, 68, 68, .28);
    background: rgba(127, 29, 29, .22);
    color: #fecaca;
    font-size: 12px;
  }

  .profiler-shell {
    min-height: 0;
    flex: 1;
    display: grid;
    grid-template-columns: 230px minmax(0, 1fr);
    gap: 8px;
    overflow: hidden;
  }

  .left-nav {
    min-height: 0;
    overflow: auto;
    padding: 8px;
    border: 1px solid rgba(34, 211, 238, .16);
    border-radius: 10px;
    background: linear-gradient(180deg, rgba(2, 6, 23, .90), rgba(3, 18, 28, .78));
  }

  .nav-group {
    padding: 7px 0;
    border-bottom: 1px solid rgba(148, 163, 184, .08);
  }

  .nav-group h3 {
    margin: 0 0 7px;
    padding: 0 2px 7px;
    display: flex;
    align-items: center;
    gap: 7px;
    color: #67e8f9;
    font-size: 12px;
    letter-spacing: 0;
    border-bottom: 1px solid rgba(103, 232, 249, .20);
    box-shadow: 0 1px 0 rgba(15, 23, 42, .85);
  }

  .group-icon {
    width: 19px;
    height: 19px;
    display: inline-grid;
    place-items: center;
    border-radius: 6px;
    background: rgba(34, 211, 238, .10);
    border: 1px solid rgba(34, 211, 238, .18);
    color: #99f6e4;
  }

  .group-icon svg {
    width: 13px;
    height: 13px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.9;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .nav-group button {
    width: 100%;
    min-height: 30px;
    display: grid;
    grid-template-columns: 28px minmax(0, 1fr);
    gap: 7px;
    align-items: center;
    margin: 3px 0;
    padding-left: 12px;
    border: 1px solid transparent;
    border-radius: 7px;
    background: transparent;
    color: #94a3b8;
    text-align: left;
  }

  .nav-group button .menu-icon {
    width: 24px;
    height: 24px;
    display: inline-grid;
    place-items: center;
    border-radius: 7px;
    color: #38bdf8;
    background: rgba(15, 23, 42, .70);
    border: 1px solid rgba(148, 163, 184, .12);
    box-shadow: inset 0 0 12px rgba(34, 211, 238, .04);
  }

  .nav-group button .menu-icon svg {
    width: 15px;
    height: 15px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.9;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .nav-group button strong {
    overflow: hidden;
    font-size: 12px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .nav-group button:hover,
  .nav-group button.active {
    border-color: rgba(34, 211, 238, .28);
    color: #f8fafc;
    background: linear-gradient(90deg, rgba(34, 211, 238, .13), rgba(52, 211, 153, .06));
  }

  .nav-group button:hover .menu-icon,
  .nav-group button.active .menu-icon {
    color: #a7f3d0;
    border-color: rgba(45, 212, 191, .30);
    background: rgba(20, 184, 166, .14);
    filter: drop-shadow(0 0 10px rgba(45, 212, 191, .18));
  }

  .workbench {
    min-height: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding-right: 2px;
  }

  .target-ribbon {
    display: grid;
    grid-template-columns: minmax(260px, 1fr) 120px minmax(150px, .36fr) minmax(150px, .36fr);
    gap: 6px;
    padding: 8px;
  }

  .target-ribbon article,
  .metric-card,
  .heap-list div {
    min-width: 0;
    padding: 8px;
    border: 1px solid rgba(148, 163, 184, .10);
    border-radius: 8px;
    background: rgba(2, 6, 23, .30);
  }

  .target-ribbon strong,
  .metric-card strong,
  .heap-list strong {
    display: block;
    margin-top: 3px;
    overflow: hidden;
    color: #f8fafc;
    font-family: var(--theme-font-family-mono);
    font-size: 13px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .target-ribbon em,
  .metric-card em {
    display: block;
    margin-top: 2px;
    overflow: hidden;
    color: #94a3b8;
    font-family: var(--theme-font-family-mono);
    font-size: 10px;
    font-style: normal;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .view-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 9px 10px;
  }

  .view-head h3 {
    margin: 2px 0 0;
    color: #f8fafc;
    font-size: 14px;
  }

  .view-status {
    text-align: right;
  }

  .view-status strong {
    display: block;
    color: #fde68a;
    font-family: var(--theme-font-family-mono);
    font-size: 12px;
  }

  .telemetry-grid {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 8px;
  }

  .metric-card {
    min-height: 68px;
  }

  .metric-card.ok strong { color: #86efac; }
  .metric-card.warn strong { color: #fde68a; }
  .metric-card.info strong { color: #67e8f9; }

  .two-columns {
    display: grid;
    grid-template-columns: minmax(520px, 1.18fr) minmax(360px, .82fr);
    gap: 8px;
    min-height: 0;
  }

  .single-column {
    display: grid;
    gap: 8px;
  }

  .panel {
    min-width: 0;
    overflow: hidden;
  }

  .panel-title {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 8px 10px;
    border-bottom: 1px solid rgba(148, 163, 184, .10);
  }

  .panel-title h3 {
    margin: 0;
    color: #f8fafc;
    font-size: 12px;
    letter-spacing: 0;
  }

  .panel-title span {
    color: #64748b;
    font-size: 10px;
  }

  .chart-panel {
    display: grid;
    grid-template-rows: auto 30px minmax(120px, 1fr);
  }

  .chart-panel.tall {
    min-height: 220px;
  }

  .chart-legend {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
    padding: 6px 10px 0;
    font-family: var(--theme-font-family-mono);
    font-size: 10px;
    font-weight: 900;
  }

  .chart-legend span {
    min-height: 20px;
    display: inline-flex;
    align-items: center;
    padding: 0 7px;
    border-radius: 999px;
    border: 1px solid rgba(148, 163, 184, .12);
    background: rgba(2, 6, 23, .28);
  }

  .chart-stage {
    position: relative;
    min-height: 144px;
  }

  .chart-panel svg {
    width: calc(100% - 18px);
    height: 128px;
    margin: 6px 9px 10px;
    border-radius: 8px;
    background:
      radial-gradient(circle at top right, rgba(34, 211, 238, .12), transparent 42%),
      rgba(2, 6, 23, .26);
  }

  .chart-tooltip {
    position: absolute;
    z-index: 6;
    min-width: 132px;
    transform: translate(10px, -50%);
    padding: 7px 8px;
    border: 1px solid rgba(34, 211, 238, .42);
    border-radius: 8px;
    background: rgba(2, 6, 23, .94);
    box-shadow: 0 16px 40px rgba(0, 0, 0, .38), 0 0 22px rgba(34, 211, 238, .12);
    color: #dbeafe;
    font-family: var(--theme-font-family-mono);
    pointer-events: none;
  }

  .chart-tooltip strong,
  .chart-tooltip span {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    white-space: nowrap;
  }

  .chart-tooltip strong {
    margin-bottom: 4px;
    color: #67e8f9;
    font-size: 10px;
  }

  .chart-tooltip span {
    color: #cbd5e1;
    font-size: 10px;
  }

  .chart-tooltip b {
    color: #fde68a;
  }

  .grid {
    fill: none;
    stroke: rgba(148, 163, 184, .14);
    stroke-width: .8;
  }

  .line {
    fill: none;
    stroke-width: 2.2;
    stroke-linecap: round;
    stroke-linejoin: round;
    filter: drop-shadow(0 0 5px rgba(34, 211, 238, .25));
  }

  .line.cpu,
  .chart-legend .cpu { stroke: #22d3ee; color: #67e8f9; }
  .line.memory,
  .chart-legend .memory { stroke: #34d399; color: #86efac; }
  .line.threads,
  .chart-legend .threads { stroke: #facc15; color: #fde68a; }
  .line.objects,
  .chart-legend .objects { stroke: #a78bfa; color: #ddd6fe; }
  .line.runnable,
  .chart-legend .runnable { stroke: #38bdf8; color: #bae6fd; }
  .line.blocked,
  .chart-legend .blocked { stroke: #fb7185; color: #fecdd3; }
  .line.waiting,
  .chart-legend .waiting { stroke: #c084fc; color: #e9d5ff; }

  .heap-list {
    display: grid;
    gap: 6px;
    max-height: 280px;
    overflow: auto;
    padding: 8px;
  }

  .heap-list span {
    display: block;
    color: #64748b;
    font-size: 10px;
    font-weight: 900;
  }

  .heap-list strong {
    white-space: normal;
    overflow-wrap: anywhere;
  }

  .profile-table,
  .object-table,
  .call-tree-table,
  .finding-list,
  .thread-list,
  .stack-list,
  .complexity-list {
    max-height: 520px;
    overflow: auto;
    padding: 8px;
  }

  .profile-head,
  .profile-row {
    display: grid;
    grid-template-columns: minmax(300px, 1fr) 90px 54px 54px 76px minmax(112px, .36fr);
    gap: 8px;
    align-items: center;
  }

  .object-head,
  .object-row {
    display: grid;
    grid-template-columns: minmax(360px, 1fr) 100px 100px 64px minmax(120px, .3fr);
    gap: 8px;
    align-items: center;
  }

  .profile-head,
  .object-head {
    position: sticky;
    top: 0;
    z-index: 2;
    min-height: 28px;
    color: #94a3b8;
    background: #07111f;
    font-size: 10px;
    font-weight: 900;
  }

  .profile-head button,
  .object-head button,
  .call-tree-head button,
  .thread-head button,
  .complexity-head button {
    min-width: 0;
    border: 0;
    background: transparent;
    color: inherit;
    font: inherit;
    font-size: 10px;
    font-weight: 900;
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .profile-row,
  .object-row {
    min-height: 32px;
    border-top: 1px solid rgba(148, 163, 184, .08);
    color: #cbd5e1;
    font-family: var(--theme-font-family-mono);
    font-size: 11px;
  }

  .profile-row strong,
  .object-row strong {
    overflow: hidden;
    color: #f8fafc;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .weight {
    position: relative;
    height: 16px;
    overflow: hidden;
    border-radius: 999px;
    background: rgba(15, 23, 42, .58);
  }

  .weight i {
    display: block;
    height: 100%;
    background: linear-gradient(90deg, #0891b2, #facc15);
  }

  .weight em {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    color: #fff;
    font-size: 10px;
    font-style: normal;
    font-weight: 900;
  }

  .finding-list {
    display: grid;
    gap: 7px;
  }

  .finding-list.compact {
    max-height: 244px;
  }

  .finding-list .finding-item {
    width: 100%;
    display: grid;
    grid-template-columns: 48px minmax(0, 1fr);
    gap: 8px;
    padding: 8px;
    border: 1px solid rgba(148, 163, 184, .10);
    border-radius: 8px;
    background: rgba(2, 6, 23, .24);
    color: inherit;
    text-align: left;
    outline: none;
  }

  .finding-list .finding-item:hover,
  .finding-list .finding-item:focus {
    border-color: rgba(34, 211, 238, .42);
    background: rgba(8, 145, 178, .11);
  }

  .finding-list .finding-item.warn { border-color: rgba(245, 158, 11, .30); }
  .finding-list .finding-item.error { border-color: rgba(239, 68, 68, .34); }

  .fleet-scan-strip {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 10px;
    align-items: center;
    margin: 8px 8px 0;
    padding: 8px;
    border: 1px solid rgba(250, 204, 21, .20);
    border-radius: 8px;
    background: linear-gradient(135deg, rgba(250, 204, 21, .08), rgba(34, 211, 238, .05));
  }

  .fleet-scan-strip span,
  .fleet-scan-strip em {
    display: block;
    color: #94a3b8;
    font-size: 10px;
    font-style: normal;
  }

  .fleet-scan-strip strong {
    display: block;
    margin: 2px 0;
    color: #f8fafc;
    font-size: 12px;
  }

  .fleet-scan-strip button {
    min-height: 28px;
    border: 1px solid rgba(250, 204, 21, .34);
    border-radius: 7px;
    background: rgba(250, 204, 21, .08);
    color: #fde68a;
    font-size: 10px;
    font-weight: 900;
  }

  .fleet-finding-table {
    margin: 8px 8px 0;
    max-height: 260px;
    overflow: auto;
    border: 1px solid rgba(148, 163, 184, .10);
    border-radius: 8px;
    background: rgba(2, 6, 23, .20);
  }

  .fleet-finding-head,
  .fleet-finding-row {
    display: grid;
    grid-template-columns: 52px 180px minmax(260px, 1fr) minmax(220px, .8fr);
    gap: 8px;
    align-items: center;
  }

  .fleet-finding-head {
    position: sticky;
    top: 0;
    z-index: 2;
    min-height: 28px;
    padding: 0 8px;
    background: #07111f;
    color: #94a3b8;
    font-size: 10px;
    font-weight: 900;
  }

  .fleet-finding-row {
    width: 100%;
    min-height: 34px;
    padding: 0 8px;
    border: 0;
    border-top: 1px solid rgba(148, 163, 184, .08);
    background: transparent;
    color: #cbd5e1;
    text-align: left;
  }

  .fleet-finding-row:hover {
    background: rgba(34, 211, 238, .08);
  }

  .fleet-finding-row.warn span { color: #fde68a; }
  .fleet-finding-row.error span { color: #fecaca; }

  .fleet-finding-row strong,
  .fleet-finding-row em,
  .fleet-finding-row small {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 10px;
    font-style: normal;
  }

  .fleet-finding-row strong {
    color: #f8fafc;
    font-family: var(--theme-font-family-mono);
  }

  .fleet-finding-row small {
    color: #94a3b8;
  }

  .finding-list b {
    display: inline-grid;
    place-items: center;
    height: 24px;
    border-radius: 6px;
    background: rgba(15, 23, 42, .62);
    color: #67e8f9;
    font-size: 9px;
  }

  .finding-list strong {
    display: block;
    margin-bottom: 4px;
    color: #f8fafc;
    font-size: 12px;
  }

  .finding-list p,
  .finding-list em {
    display: block;
    margin: 0;
    color: #cbd5e1;
    font-size: 11px;
    line-height: 1.45;
  }

  .finding-list em {
    margin-top: 4px;
    color: #94a3b8;
    font-style: normal;
  }

  .state-tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    padding: 8px 8px 0;
  }

  .state-tabs button,
  .raw-tabs button {
    min-height: 23px;
    padding: 0 8px;
  }

  .thread-row {
    width: 100%;
    display: grid;
    grid-template-columns: 96px minmax(0, .62fr) 48px minmax(0, 1fr);
    gap: 8px;
    align-items: center;
    min-height: 34px;
    border: 0;
    border-bottom: 1px solid rgba(148, 163, 184, .08);
    background: transparent;
    color: #dbeafe;
    text-align: left;
  }

  .thread-head {
    position: sticky;
    top: 0;
    z-index: 2;
    display: grid;
    grid-template-columns: 96px minmax(0, .62fr) 48px minmax(0, 1fr);
    gap: 8px;
    align-items: center;
    min-height: 28px;
    color: #94a3b8;
    background: #07111f;
    font-size: 10px;
    font-weight: 900;
  }

  .thread-row.active,
  .complexity-row.active {
    background: rgba(34, 211, 238, .08);
  }

  .thread-row strong,
  .thread-row b,
  .thread-row em {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .thread-row strong {
    color: #f8fafc;
    font-size: 11px;
  }

  .thread-row em {
    color: #94a3b8;
    font-family: var(--theme-font-family-mono);
    font-size: 10px;
    font-style: normal;
  }

  .thread-row b {
    color: #fde68a;
    font-family: var(--theme-font-family-mono);
    font-size: 10px;
  }

  .state {
    display: inline-grid;
    place-items: center;
    min-height: 21px;
    border-radius: 6px;
    background: rgba(148, 163, 184, .12);
    color: #cbd5e1;
    font-family: var(--theme-font-family-mono);
    font-size: 9px;
    font-weight: 900;
  }

  .state.runnable { color: #67e8f9; background: rgba(34, 211, 238, .12); }
  .state.blocked { color: #fecaca; background: rgba(239, 68, 68, .12); }
  .state.waiting,
  .state.timed_waiting { color: #ddd6fe; background: rgba(167, 139, 250, .12); }

  .call-tree-panel {
    min-height: 460px;
  }

  .call-tree-title {
    align-items: center;
  }

  .call-tree-title > div:first-child {
    min-width: 0;
  }

  .call-tree-title h3 {
    margin-bottom: 3px;
  }

  .tree-actions {
    display: flex;
    align-items: center;
    gap: 6px;
    white-space: nowrap;
  }

  .tree-actions button {
    min-height: 24px;
    border: 1px solid rgba(34, 211, 238, .22);
    border-radius: 7px;
    background: rgba(15, 23, 42, .52);
    color: #cbd5e1;
    font-size: 10px;
    font-weight: 900;
  }

  .tree-actions button:disabled {
    opacity: .45;
  }

  .tree-actions em {
    color: #67e8f9;
    font-family: var(--theme-font-family-mono);
    font-size: 10px;
    font-style: normal;
  }

  .call-tree-head,
  .call-tree-row {
    display: grid;
    grid-template-columns: minmax(340px, 1fr) 82px 56px 56px 78px minmax(120px, .26fr);
    gap: 6px;
    align-items: center;
  }

  .call-tree-head {
    position: sticky;
    top: 0;
    z-index: 2;
    min-height: 26px;
    color: #94a3b8;
    background: #07111f;
    font-size: 10px;
    font-weight: 900;
  }

  .call-tree-row {
    width: 100%;
    min-height: 28px;
    border: 0;
    border-bottom: 1px solid rgba(148, 163, 184, .08);
    background: linear-gradient(90deg, rgba(15, 23, 42, .30), transparent);
    color: #cbd5e1;
    font-family: var(--theme-font-family-mono);
    font-size: 10px;
    text-align: left;
  }

  .call-tree-row:hover,
  .call-tree-row.open {
    background: linear-gradient(90deg, rgba(34, 211, 238, .10), rgba(15, 23, 42, .16));
  }

  .call-tree-row.leaf {
    background: transparent;
  }

  .tree-node-main {
    min-width: 0;
    display: grid;
    grid-template-columns: 18px minmax(0, .44fr) minmax(0, .56fr);
    gap: 5px;
    align-items: center;
    padding-left: calc(var(--level) * 12px);
  }

  .tree-node-main i {
    width: 16px;
    height: 16px;
    display: inline-grid;
    place-items: center;
    border-radius: 5px;
    background: rgba(34, 211, 238, .10);
    color: #67e8f9;
    font-style: normal;
    font-weight: 900;
  }

  .call-tree-row.leaf .tree-node-main i {
    background: transparent;
    color: #64748b;
  }

  .tree-node-main strong,
  .tree-node-main small,
  .call-tree-row span,
  .call-tree-row b {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tree-node-main strong {
    color: #f8fafc;
  }

  .tree-node-main small {
    color: #94a3b8;
    font-size: 9px;
  }

  .call-tree-row b {
    color: #fde68a;
    font-size: 10px;
  }

  .complexity-row {
    width: 100%;
    display: grid;
    grid-template-columns: 100px minmax(150px, .45fr) 72px 72px minmax(0, 1fr);
    gap: 8px;
    align-items: center;
    min-height: 34px;
    border: 0;
    border-bottom: 1px solid rgba(148, 163, 184, .08);
    background: transparent;
    color: #dbeafe;
    text-align: left;
  }

  .complexity-row strong,
  .complexity-row small {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .complexity-row b {
    color: #fde68a;
    font-size: 11px;
  }

  .complexity-row em,
  .complexity-row small {
    color: #94a3b8;
    font-family: var(--theme-font-family-mono);
    font-size: 10px;
    font-style: normal;
  }

  .lock-map {
    min-height: 360px;
  }

  .lock-nodes {
    min-height: 312px;
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 8px;
    align-items: stretch;
    padding: 12px;
  }

  .lock-core,
  .lock-node {
    min-width: 0;
    display: grid;
    align-content: center;
    gap: 4px;
    border: 1px solid rgba(34, 211, 238, .18);
    border-radius: 10px;
    background: rgba(2, 6, 23, .34);
    color: #dbeafe;
    text-align: center;
  }

  .lock-core {
    grid-row: span 2;
    min-height: 128px;
    background:
      radial-gradient(circle at center, rgba(239, 68, 68, .16), transparent 58%),
      rgba(2, 6, 23, .36);
  }

  .lock-core strong {
    color: #fecaca;
    font-size: 34px;
    line-height: 1;
  }

  .lock-node {
    min-height: 82px;
    padding: 8px;
  }

  .lock-node.active {
    border-color: rgba(250, 204, 21, .45);
  }

  .lock-node b {
    color: #fde68a;
    font-size: 10px;
  }

  .lock-node strong,
  .lock-node em {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .lock-node em {
    color: #94a3b8;
    font-family: var(--theme-font-family-mono);
    font-size: 10px;
    font-style: normal;
  }

  .frame {
    display: grid;
    grid-template-columns: 34px minmax(0, 1fr) 110px;
    gap: 8px;
    align-items: start;
    padding: 7px 0;
    border-bottom: 1px solid rgba(148, 163, 184, .08);
    font-family: var(--theme-font-family-mono);
  }

  .frame span {
    color: #67e8f9;
    font-size: 10px;
    font-weight: 900;
  }

  .frame strong {
    overflow-wrap: anywhere;
    color: #e0f2fe;
    font-size: 11px;
  }

  .frame em {
    overflow: hidden;
    color: #94a3b8;
    font-size: 10px;
    font-style: normal;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .raw-tabs {
    display: flex;
    gap: 5px;
    padding: 8px 10px 0;
  }

  .raw-panel summary {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 9px 10px;
    color: #f8fafc;
    font-size: 12px;
    font-weight: 900;
    list-style: none;
  }

  .raw-panel summary::-webkit-details-marker {
    display: none;
  }

  .raw-panel summary::after {
    content: "展开";
    min-width: 42px;
    color: #67e8f9;
    font-size: 10px;
    text-align: right;
  }

  .raw-panel[open] summary::after {
    content: "收起";
  }

  .raw-panel summary em {
    min-width: 0;
    overflow: hidden;
    color: #94a3b8;
    font-family: var(--theme-font-family-mono);
    font-size: 10px;
    font-style: normal;
    font-weight: 500;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .raw-panel pre {
    max-height: 220px;
    overflow: auto;
    margin: 0;
    padding: 10px;
    background: rgba(2, 6, 23, .42);
    color: #cbd5e1;
    font-family: var(--theme-font-family-mono);
    font-size: 10px;
    line-height: 1.55;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .finding-modal-layer {
    position: fixed;
    inset: 0;
    z-index: 220;
    display: grid;
    place-items: center;
    padding: 24px;
    background: rgba(2, 6, 23, .72);
    backdrop-filter: blur(10px);
  }

  .finding-modal {
    width: min(860px, calc(100vw - 48px));
    max-height: min(720px, calc(100vh - 48px));
    overflow: hidden;
    border: 1px solid rgba(34, 211, 238, .34);
    border-radius: 12px;
    background:
      linear-gradient(rgba(34, 211, 238, .035) 1px, transparent 1px),
      linear-gradient(90deg, rgba(52, 211, 153, .026) 1px, transparent 1px),
      #06101d;
    background-size: 22px 22px;
    box-shadow: 0 28px 90px rgba(0, 0, 0, .54), inset 0 0 36px rgba(34, 211, 238, .05);
  }

  .finding-modal header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 12px 14px;
    border-bottom: 1px solid rgba(148, 163, 184, .12);
  }

  .finding-modal header span {
    color: #67e8f9;
    font-size: 10px;
    font-weight: 900;
  }

  .finding-modal header h3 {
    margin: 2px 0 0;
    color: #f8fafc;
    font-size: 16px;
  }

  .finding-modal header button {
    width: 30px;
    height: 30px;
    border: 1px solid rgba(34, 211, 238, .28);
    border-radius: 8px;
    background: rgba(15, 23, 42, .58);
    color: #f8fafc;
    font-size: 18px;
    line-height: 1;
  }

  .finding-modal-body {
    max-height: calc(min(720px, 100vh - 48px) - 58px);
    overflow: auto;
    display: grid;
    gap: 8px;
    padding: 10px;
  }

  .finding-modal-body article {
    padding: 10px;
    border: 1px solid rgba(148, 163, 184, .12);
    border-radius: 8px;
    background: rgba(2, 6, 23, .34);
  }

  .finding-modal-body b {
    display: block;
    margin-bottom: 6px;
    color: #fde68a;
    font-size: 11px;
  }

  .finding-modal-body p {
    margin: 0;
    color: #dbeafe;
    font-size: 12px;
    line-height: 1.65;
  }

  .evidence-list {
    display: grid;
    gap: 5px;
  }

  .evidence-list code {
    display: block;
    overflow-wrap: anywhere;
    padding: 6px 8px;
    border-radius: 6px;
    background: rgba(15, 23, 42, .62);
    color: #bfdbfe;
    font-family: var(--theme-font-family-mono);
    font-size: 10px;
  }

  :global(.pdf-report) {
    position: fixed;
    left: -99999px;
    top: 0;
    width: 794px;
    box-sizing: border-box;
    padding: 34px;
    background: #08111f;
    color: #e5f3ff;
    font-family: "Noto Sans CJK SC", "Microsoft YaHei", Arial, sans-serif;
  }

  :global(.pdf-report h1) {
    margin: 0 0 8px;
    color: #67e8f9;
    font-size: 28px;
  }

  :global(.pdf-report h2) {
    margin: 18px 0 8px;
    color: #fde68a;
    font-size: 16px;
  }

  :global(.pdf-report p),
  :global(.pdf-report article) {
    color: #dbeafe;
    font-size: 12px;
    line-height: 1.62;
  }

  :global(.pdf-report article) {
    margin: 6px 0;
    padding: 8px;
    border: 1px solid rgba(103, 232, 249, .25);
    border-radius: 8px;
    background: rgba(15, 23, 42, .65);
  }

  :global(.pdf-grid) {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
  }

  :global(.pdf-grid span) {
    padding: 8px;
    border: 1px solid rgba(103, 232, 249, .24);
    border-radius: 8px;
    background: rgba(2, 6, 23, .45);
  }

  @keyframes spin { to { transform: rotate(360deg); } }

  @media (max-width: 1280px) {
    .java-profiler {
      height: auto;
      overflow: visible;
    }

    .target-ribbon,
    .two-columns,
    .telemetry-grid {
      grid-template-columns: 1fr;
    }

    .profiler-shell {
      grid-template-columns: 1fr;
      overflow: visible;
    }

    .left-nav {
      max-height: 340px;
    }

    .command-bar {
      grid-template-columns: 170px minmax(320px, 1fr) max-content;
    }
  }
</style>
