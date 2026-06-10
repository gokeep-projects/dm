<script>
  import { onMount, onDestroy, tick } from 'svelte';
  import { Terminal } from '@xterm/xterm';
  import { FitAddon } from '@xterm/addon-fit';
  import '@xterm/xterm/css/xterm.css';

  let { id, autorun = false } = $props();
  let output = $state('');
  let running = $state(false);
  let error = $state(null);
  let exitCode = $state(null);
  let ws = $state(null);
  let info = $state(null);
  let termContainer = $state(null);
  let terminal = null;
  let fitAddon = null;
  let terminalWrap = $state(null);
  let isFullscreen = $state(false);
  let copyOk = $state(false);
  let copyTimer = null;
  let startTime = $state(0);
  let elapsedSec = $state(0);
  let durationMs = $state(0);
  let elapsedTimer = null;
  let lineCount = $state(0);
  let downloadOk = $state(false);
  let downloadTimer = null;
  let paramValues = $state({});
  let showParams = $state(false);
  let showSource = $state(false);
  let sourceContent = $state('');
  let sourceMeta = $state(null);
  let sourceLoading = $state(false);
  let sourceCopyOk = $state(false);
  let sourceCopyTimer = null;
  let commandCopyOk = $state(false);
  let commandCopyTimer = null;
  let copiedHistoryId = $state('');
  let reusedHistoryId = $state('');
  let replayingHistoryId = $state('');
  let showHistory = $state(false);
  let historyData = $state([]);
  let historyLoading = $state(false);
  let historySortKey = $state('timestamp');
  let historySortDir = $state('desc');
  let resultTableSort = $state({});
  let resultSectionOpen = $state({});
  let showDuplicate = $state(false);
  let duplicateId = $state('');
  let duplicateLoading = $state(false);
  let duplicateError = $state(null);
  let showStats = $state(false);
  let statsData = $state(null);
  let statsLoading = $state(false);
  let reconnectAttempts = $state(0);
  const MAX_RECONNECT = 5;
  let jsonData = $state(null);
  let showJsonView = $state(false);
  let lastProfileState = $derived(executionState(statsData?.last_execution || historyData[0]));

  function resultSectionKey(section, index) {
    return `${index}:${section?.title || 'section'}`;
  }

  function defaultSectionOpen(section) {
    return !/明细|详情/.test(String(section?.title || ''));
  }

  function isResultSectionOpen(section, index) {
    const key = resultSectionKey(section, index);
    return resultSectionOpen[key] ?? defaultSectionOpen(section);
  }

  function toggleResultSection(section, index) {
    const key = resultSectionKey(section, index);
    resultSectionOpen = { ...resultSectionOpen, [key]: !isResultSectionOpen(section, index) };
  }

  function formatDuration(ms) {
    const s = Math.floor(ms / 1000);
    if (s < 60) return s + 's';
    const m = Math.floor(s / 60);
    const rs = s % 60;
    if (m < 60) return m + 'm' + rs + 's';
    const h = Math.floor(m / 60);
    return h + 'h' + (m % 60) + 'm' + rs + 's';
  }

  function startTimer() {
    stopTimer();
    startTime = Date.now();
    elapsedSec = 0;
    elapsedTimer = setInterval(() => {
      if (startTime) elapsedSec = Math.floor((Date.now() - startTime) / 1000);
    }, 500);
  }

  function stopTimer() {
    if (elapsedTimer) { clearInterval(elapsedTimer); elapsedTimer = null; }
    if (startTime) durationMs = Date.now() - startTime;
  }

  function getTerminalText() {
    if (!terminal) return '';
    const text = terminal.buffer.active;
    const lines = [];
    for (let i = 0; i < text.length; i++) {
      const line = text.getLine(i);
      if (line) lines.push(line.translateToString(true));
    }
    return lines.join('\n');
  }

  async function loadInfo() {
    try { const r = await fetch('/api/scripts/' + id); if (r.ok) info = await r.json(); } catch (_) {}
    initParamValues();
  }

  async function openSource() {
    showSource = true;
    if (sourceContent) return;
    sourceLoading = true;
    try {
      const r = await fetch('/api/scripts/' + id + '/source');
      if (r.ok) {
        const d = await r.json();
        sourceContent = d.content || '';
        sourceMeta = { path: d.path, line_count: d.line_count, size_bytes: d.size_bytes };
      } else {
        sourceContent = '// 无法加载脚本源码';
        sourceMeta = null;
      }
    } catch (_) {
      sourceContent = '// 无法加载脚本源码';
      sourceMeta = null;
    }
    sourceLoading = false;
  }

  function closeSource() {
    showSource = false;
  }

  async function loadHistory() {
    if (showHistory) { showHistory = false; return; }
    showHistory = true;
    await refreshHistory(20);
  }

  async function refreshHistory(limit = 20) {
    historyLoading = true;
    try {
      const r = await fetch('/api/dashboard/history?script_id=' + encodeURIComponent(id) + '&limit=' + limit);
      if (r.ok) {
        const d = await r.json();
        historyData = d.records || [];
      }
    } catch (_) {}
    historyLoading = false;
  }

  function openDuplicate() {
    duplicateId = id + '-copy';
    duplicateError = null;
    showDuplicate = true;
  }

  async function doDuplicate() {
    if (!duplicateId.trim()) return;
    duplicateLoading = true;
    duplicateError = null;
    try {
      const r = await fetch('/api/scripts/' + encodeURIComponent(id) + '/duplicate', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ new_id: duplicateId.trim() }),
      });
      if (r.ok) {
        const d = await r.json();
        showDuplicate = false;
        location.hash = '#/script/' + d.new_id;
      } else if (r.status === 409) {
        duplicateError = '脚本 ID 已存在';
      } else {
        duplicateError = '复制失败';
      }
    } catch (_) { duplicateError = '网络错误'; }
    duplicateLoading = false;
  }

  async function loadStats() {
    if (showStats) { showStats = false; return; }
    showStats = true;
    await refreshStats();
  }

  async function refreshStats() {
    statsLoading = true;
    try {
      const r = await fetch('/api/scripts/' + encodeURIComponent(id) + '/stats');
      if (r.ok) statsData = await r.json();
    } catch (_) {}
    statsLoading = false;
  }

  async function refreshRunProfile() {
    await Promise.all([refreshStats(), refreshHistory(5)]);
  }

  function copySource() {
    if (!sourceContent) return;
    navigator.clipboard?.writeText(sourceContent).then(() => {
      sourceCopyOk = true;
      clearTimeout(sourceCopyTimer);
      sourceCopyTimer = setTimeout(() => sourceCopyOk = false, 2000);
    }).catch(() => {});
  }

  async function copyRunCommand() {
    const command = buildRunCommand();
    try {
      if (navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(command);
      } else {
        const textarea = document.createElement('textarea');
        textarea.value = command;
        textarea.setAttribute('readonly', '');
        textarea.style.position = 'fixed';
        textarea.style.opacity = '0';
        document.body.appendChild(textarea);
        textarea.select();
        document.execCommand('copy');
        textarea.remove();
      }
      commandCopyOk = true;
      clearTimeout(commandCopyTimer);
      commandCopyTimer = setTimeout(() => commandCopyOk = false, 1600);
    } catch (_) {}
  }

  function shellQuote(value) {
    const text = String(value ?? '');
    if (/^[A-Za-z0-9_./:=@%+,-]+$/.test(text)) return text;
    return "'" + text.replace(/'/g, "'\\''") + "'";
  }

  function normalizedHistoryParams(record) {
    const raw = record?.params || {};
    if (!raw || typeof raw !== 'object' || Array.isArray(raw)) return {};
    return Object.fromEntries(Object.entries(raw).map(([k, v]) => [k, String(v ?? '')]));
  }

  function buildRunCommand(record = null) {
    const params = record ? normalizedHistoryParams(record) : getParamPayload().params;
    const args = record ? (Array.isArray(record.args) ? record.args : []) : getParamPayload().args;
    const parts = ['dm', 'run', shellQuote(id)];
    for (const [key, value] of Object.entries(params || {})) {
      if (value !== '') parts.push('--param', shellQuote(`${key}=${value}`));
    }
    if (args?.length) {
      parts.push('--');
      parts.push(...args.map(shellQuote));
    }
    return parts.join(' ');
  }

  async function copyHistoryCommand(record) {
    const command = buildRunCommand(record);
    try {
      if (navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(command);
      } else {
        const textarea = document.createElement('textarea');
        textarea.value = command;
        textarea.setAttribute('readonly', '');
        textarea.style.position = 'fixed';
        textarea.style.opacity = '0';
        document.body.appendChild(textarea);
        textarea.select();
        document.execCommand('copy');
        textarea.remove();
      }
      copiedHistoryId = String(record.id);
      setTimeout(() => {
        if (copiedHistoryId === String(record.id)) copiedHistoryId = '';
      }, 1400);
    } catch (_) {}
  }

  function formatBytes(n) {
    if (!n) return '0 B';
    if (n < 1024) return n + ' B';
    if (n < 1048576) return (n / 1024).toFixed(1) + ' KB';
    return (n / 1048576).toFixed(2) + ' MB';
  }

  function formatDurationCompact(ms) {
    if (ms === null || ms === undefined) return '-';
    const n = Number(ms || 0);
    if (n < 1000) return Math.round(n) + 'ms';
    if (n < 60000) return (n / 1000).toFixed(n < 10000 ? 1 : 0) + 's';
    const m = Math.floor(n / 60000);
    const s = Math.round((n % 60000) / 1000);
    return `${m}m${s}s`;
  }

  function successRateText() {
    const total = statsData?.total_executions || 0;
    if (!total) return '-';
    return Math.round((statsData.success_count || 0) / total * 100) + '%';
  }

  function executionState(record) {
    if (!record) return { text: '未执行', tone: 'idle' };
    if (record.exit_code === 0) return { text: '成功', tone: 'ok' };
    if (record.exit_code === null || record.exit_code === undefined) return { text: '运行中', tone: 'running' };
    return { text: `失败 ${record.exit_code}`, tone: 'fail' };
  }

  function formatTime(ts) {
    if (!ts) return '-';
    try {
      const d = new Date(ts.replace(' ', 'T'));
      const diff = Math.floor((Date.now() - d.getTime()) / 1000);
      if (diff < 60) return diff + '秒前';
      if (diff < 3600) return Math.floor(diff/60) + '分钟前';
      if (diff < 86400) return Math.floor(diff/3600) + '小时前';
      return d.toLocaleDateString('zh-CN', { month: '2-digit', day: '2-digit' }) + ' ' + d.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit', hour12: false });
    } catch { return ts; }
  }

  function initParamValues() {
    const params = info?.metadata?.params || [];
    if (params.length === 0) { showParams = false; return; }
    const saved = loadParamPreset();
    const next = {};
    for (const p of params) {
      if (saved && saved[p.name] !== undefined) {
        next[p.name] = saved[p.name];
      } else {
        next[p.name] = p.default ?? (p.type === 'boolean' ? 'false' : '');
      }
    }
    paramValues = next;
    showParams = true;
  }

  function saveParamPreset() {
    try { localStorage.setItem('dm-params-' + id, JSON.stringify(paramValues)); } catch (_) {}
  }

  function loadParamPreset() {
    try { const raw = localStorage.getItem('dm-params-' + id); return raw ? JSON.parse(raw) : null; } catch (_) { return null; }
  }

  function loadPendingRunPayload() {
    try {
      const key = 'dm-run-payload-' + id;
      const raw = localStorage.getItem(key);
      if (!raw) return null;
      localStorage.removeItem(key);
      const payload = JSON.parse(raw);
      return {
        params: payload?.params && typeof payload.params === 'object' ? payload.params : {},
        args: Array.isArray(payload?.args) ? payload.args.map(String) : [],
      };
    } catch (_) {
      return null;
    }
  }

  function changeHistorySort(key) {
    if (historySortKey === key) historySortDir = historySortDir === 'asc' ? 'desc' : 'asc';
    else {
      historySortKey = key;
      historySortDir = key === 'timestamp' ? 'desc' : 'asc';
    }
  }

  function historySortMark(key) {
    if (historySortKey !== key) return '';
    return historySortDir === 'asc' ? ' ↑' : ' ↓';
  }

  function historyValue(row, key) {
    if (key === 'exit_code') return Number(row.exit_code ?? 9999);
    if (key === 'duration_ms') return Number(row.duration_ms || 0);
    return row.timestamp || '';
  }

  function changeResultTableSort(key, columnIndex) {
    const current = resultTableSort[key];
    const dir = current?.column === columnIndex && current?.dir === 'asc' ? 'desc' : 'asc';
    resultTableSort = { ...resultTableSort, [key]: { column: columnIndex, dir } };
  }

  function resultTableSortMark(key, columnIndex) {
    const current = resultTableSort[key];
    if (!current || current.column !== columnIndex) return '';
    return current.dir === 'asc' ? ' ↑' : ' ↓';
  }

  function compareCells(a, b) {
    const av = a ?? '';
    const bv = b ?? '';
    const an = Number(String(av).replace(/[%,$\s]/g, ''));
    const bn = Number(String(bv).replace(/[%,$\s]/g, ''));
    if (!Number.isNaN(an) && !Number.isNaN(bn) && String(av).trim() !== '' && String(bv).trim() !== '') {
      return an - bn;
    }
    return String(av).localeCompare(String(bv), 'zh-CN', { numeric: true, sensitivity: 'base' });
  }

  function sortedResultRows(item, key) {
    const rows = item?.rows || [];
    const sort = resultTableSort[key];
    if (!sort) return rows;
    return [...rows].sort((a, b) => {
      let cmp = compareCells(a?.[sort.column], b?.[sort.column]);
      if (sort.dir === 'desc') cmp = -cmp;
      return cmp;
    });
  }

  function sparklineBars(data) {
    const nums = (data || []).map(Number).filter(v => !Number.isNaN(v));
    if (nums.length === 0) return [];
    const min = Math.min(...nums);
    const max = Math.max(...nums);
    const range = Math.max(max - min, 1);
    return nums.map(value => ({
      value,
      height: Math.max(18, Math.round(((value - min) / range) * 46) + 18),
    }));
  }

  let sortedHistoryData = $derived.by(() => {
    return [...historyData].sort((a, b) => {
      const av = historyValue(a, historySortKey);
      const bv = historyValue(b, historySortKey);
      let cmp = typeof av === 'number' && typeof bv === 'number'
        ? av - bv
        : String(av).localeCompare(String(bv), 'zh-CN');
      if (historySortDir === 'desc') cmp = -cmp;
      return cmp;
    });
  });

  function clearParamPreset() {
    try { localStorage.removeItem('dm-params-' + id); } catch (_) {}
    initParamValues();
  }

  function reuseHistoryParams(record) {
    const params = normalizedHistoryParams(record);
    if (!Object.keys(params).length) return;
    const defined = new Set((info?.metadata?.params || []).map(p => p.name));
    const next = { ...paramValues };
    for (const [key, value] of Object.entries(params)) {
      if (!defined.size || defined.has(key)) next[key] = value;
    }
    paramValues = next;
    showParams = true;
    reusedHistoryId = String(record.id);
    setTimeout(() => {
      if (reusedHistoryId === String(record.id)) reusedHistoryId = '';
    }, 1400);
  }

  function getParamPayload() {
    const params = info?.metadata?.params || [];
    if (params.length === 0) return { params: {}, args: [] };
    const filtered = {};
    for (const p of params) {
      const v = (paramValues[p.name] ?? '').toString();
      if (v !== '' && !(p.type === 'boolean' && v === 'false' && !p.required)) {
        filtered[p.name] = v;
      }
    }
    return { params: filtered, args: [] };
  }

  function historyRunPayload(record) {
    return {
      params: normalizedHistoryParams(record),
      args: Array.isArray(record?.args) ? record.args : [],
    };
  }

  function initTerminal() {
    if (!termContainer || terminal) return;
    terminal = new Terminal({
      cursorBlink: true,
      fontSize: 13,
      fontFamily: '"JetBrains Mono", "Fira Code", "Cascadia Code", "Consolas", "Microsoft YaHei", monospace',
      theme: {
        background: '#0a0c10',
        foreground: '#c9d1d9',
        cursor: '#22d3ee',
        cursorAccent: '#0a0c10',
        selectionBackground: 'rgba(34, 211, 238, 0.3)',
        black: '#1f2937',
        red: '#ef4444',
        green: '#10b981',
        yellow: '#fbbf24',
        blue: '#3b82f6',
        magenta: '#d946ef',
        cyan: '#06b6d4',
        white: '#f1f5f9',
        brightBlack: '#6b7280',
        brightRed: '#f87171',
        brightGreen: '#34d399',
        brightYellow: '#fcd34d',
        brightBlue: '#60a5fa',
        brightMagenta: '#e879f9',
        brightCyan: '#22d3ee',
        brightWhite: '#ffffff',
      },
      scrollback: 10000,
      convertEol: false,
      disableStdin: true,
      allowProposedApi: true,
      wraparound: false,
    });
    fitAddon = new FitAddon();
    terminal.loadAddon(fitAddon);
    terminal.open(termContainer);
    fitAddon.fit();
    writePrompt();

    const ro = new ResizeObserver(() => {
      try { fitAddon.fit(); } catch (_) {}
    });
    ro.observe(termContainer);
    termContainer._ro = ro;
  }

  function writePrompt() {
    if (terminal) {
      terminal.writeln('\x1b[2m[READY] DM 终端已就绪，点击“执行”运行脚本\x1b[0m');
      terminal.writeln('');
    }
  }

  function clearTerminal() {
    if (showJsonView && jsonData) {
      jsonData = null;
      showJsonView = false;
      output = '';
      lineCount = 0;
      exitCode = null;
      if (terminal) {
        terminal.clear();
        terminal.reset();
        writePrompt();
      }
      return;
    }
    if (terminal) {
      terminal.clear();
      terminal.reset();
      writePrompt();
    }
    output = '';
  }

  function copyOutput() {
    const content = showJsonView && jsonData
      ? JSON.stringify(jsonData, null, 2)
      : getTerminalText();
    if (!content) return;
    if (navigator.clipboard) {
      navigator.clipboard.writeText(content).then(() => {
        copyOk = true;
        clearTimeout(copyTimer);
        copyTimer = setTimeout(() => copyOk = false, 2000);
      }).catch(() => {});
    }
  }

  function downloadLog() {
    const structured = showJsonView && jsonData;
    const content = structured ? JSON.stringify(jsonData, null, 2) : getTerminalText();
    if (!content) return;
    const ts = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19);
    const blob = new Blob([content], { type: structured ? 'application/json;charset=utf-8' : 'text/plain;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = id + '-' + ts + (structured ? '.json' : '.log');
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
    downloadOk = true;
    clearTimeout(downloadTimer);
    downloadTimer = setTimeout(() => downloadOk = false, 2000);
  }

  function toggleFullscreen() {
    if (!terminalWrap) return;
    if (!document.fullscreenElement) {
      terminalWrap.requestFullscreen?.();
    } else {
      document.exitFullscreen?.();
    }
  }

  function onFullscreenChange() {
    isFullscreen = !!document.fullscreenElement;
    setTimeout(() => {
      try { fitAddon?.fit(); } catch (_) {}
    }, 100);
  }

  function runScript(record = null, explicitPayload = null) {
    if (record?.currentTarget || record?.target) record = null;
    const replayId = record?.id !== undefined ? String(record.id) : '';
    if (replayId) replayingHistoryId = replayId;
    if (terminal) {
      terminal.clear();
      terminal.reset();
    }
    output = '';
    error = null;
    exitCode = null;
    jsonData = null;
    resultSectionOpen = {};
    showJsonView = false;
    lineCount = 0;
    durationMs = 0;
    running = true;
    writePrompt();
    if (record && terminal) {
      terminal.writeln(`\x1b[1;36m[REPLAY] 按历史记录 ${formatTime(record.timestamp)} 复现执行\x1b[0m`);
      terminal.writeln(`\x1b[2m${buildRunCommand(record)}\x1b[0m`);
      terminal.writeln('');
    }
    startTimer();
    connectWs(explicitPayload || (record ? historyRunPayload(record) : null));
  }

  function connectWs(payload = null) {
    const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
    ws = new WebSocket(`${proto}//${location.host}/ws/exec/${id}`);
    ws.onopen = () => {
      reconnectAttempts = 0;
      ws.send(JSON.stringify({ action: 'run', ...(payload || getParamPayload()) }));
    };
    ws.onmessage = (e) => {
      try {
        const d = JSON.parse(e.data);
        if (d.type === 'result') {
          exitCode = d.exit_code ?? null;
          running = false;
          stopTimer();
          if (d.elapsed_ms !== undefined && d.elapsed_ms !== null) {
            durationMs = d.elapsed_ms;
          }
          try {
            const parsed = JSON.parse(d.line);
            if (typeof parsed === 'object' && parsed !== null) {
              jsonData = parsed;
              showJsonView = true;
              if (typeof parsed.line_count === 'number') {
                lineCount = parsed.line_count;
              }
            }
          } catch (_) {
            jsonData = {
              name: '脚本执行结果',
              status: exitCode === 0 ? 'ok' : 'error',
              sections: [{
                title: '执行输出',
                items: [{ type: 'info', text: d.line || '' }]
              }]
            };
            showJsonView = true;
          }
          refreshRunProfile();
          replayingHistoryId = '';
          reconnectAttempts = 0;
          try { ws?.close(); } catch (_) {}
          return;
        }
        if (d.type === 'done') {
          exitCode = d.exit_code;
          running = false;
          stopTimer();
          reconnectAttempts = 0;
          ws.close();
          if (!showJsonView && terminal) {
            terminal.writeln('');
            const dur = durationMs > 0 ? ' [用时 ' + formatDuration(durationMs) + ']' : '';
            if (exitCode === 0) {
              terminal.writeln(`\x1b[1;32m[OK] 执行完成 (退出码: 0, ${lineCount} 行)${dur}\x1b[0m`);
            } else {
              terminal.writeln(`\x1b[1;31m[FAIL] 执行失败 (退出码: ${exitCode}, ${lineCount} 行)${dur}\x1b[0m`);
            }
          }
          refreshRunProfile();
          replayingHistoryId = '';
          return;
        }
        if (d.line) {
          output += d.line + '\n';
          lineCount++;
          if (terminal && !showJsonView) {
            terminal.writeln(d.line);
          }
        }
      } catch (_) {}
    };
    ws.onerror = () => { /* onclose will fire too */ };
    ws.onclose = (e) => {
      if (exitCode !== null) return;
      if (reconnectAttempts >= MAX_RECONNECT) {
        running = false;
        replayingHistoryId = '';
        stopTimer();
        if (terminal) terminal.writeln('\x1b[1;31m[FAIL] 连接已断开，重连失败（已达最大重试次数）\x1b[0m');
        return;
      }
      if (e && e.code === 1000) return;
      reconnectAttempts++;
      const delay = Math.min(1000 * Math.pow(2, reconnectAttempts - 1), 10000);
      if (terminal) terminal.writeln(`\x1b[1;33m[WARN] 连接已断开，${Math.round(delay/1000)}秒后重连 (${reconnectAttempts}/${MAX_RECONNECT})...\x1b[0m`);
      setTimeout(() => { if (running) connectWs(payload); }, delay);
    };
  }

  function stopScript() {
    if (ws) {
      ws.close();
      running = false;
      replayingHistoryId = '';
      stopTimer();
      if (terminal) {
        terminal.writeln('');
        const dur = durationMs > 0 ? ' [用时 ' + formatDuration(durationMs) + ']' : '';
        terminal.writeln(`\x1b[1;33m[STOP] 已手动停止${dur}\x1b[0m`);
      }
    }
  }

  function tryParseJsonOutput() {
    const lines = output.trim().split('\n');
    for (const line of lines) {
      try {
        const parsed = JSON.parse(line.trim());
        if (typeof parsed === 'object' && parsed !== null) {
          jsonData = parsed;
          showJsonView = true;
          return;
        }
      } catch (_) {}
    }
    jsonData = null;
    showJsonView = false;
  }

  function statusIcon(s) {
    if (s === 'ok') return 'OK';
    if (s === 'warn') return 'WARN';
    if (s === 'error') return 'FAIL';
    return 'INFO';
  }

  function resultStatus() {
    return jsonData?.status || (exitCode === 0 ? 'ok' : 'error');
  }

  function resultStatusText() {
    const status = resultStatus();
    if (status === 'ok') return '[OK] 正常';
    if (status === 'warn') return '[WARN] 警告';
    if (status === 'error') return '[FAIL] 异常';
    return '[INFO] 信息';
  }

  function resultSectionCount() {
    return Array.isArray(jsonData?.sections) ? jsonData.sections.length : 0;
  }

  function resultItemCount() {
    if (!Array.isArray(jsonData?.sections)) return 0;
    return jsonData.sections.reduce((sum, section) => sum + (Array.isArray(section.items) ? section.items.length : 0), 0);
  }

  function resultMetaText() {
    const sections = resultSectionCount();
    const items = resultItemCount();
    if (sections > 0 || items > 0) return `${sections} 模块 · ${items} 项`;
    if (lineCount > 0) return `${lineCount} 行`;
    return '';
  }

  function resultExitCode() {
    if (jsonData?.exit_code !== undefined && jsonData?.exit_code !== null) return jsonData.exit_code;
    return exitCode;
  }

  function resultExitText() {
    const code = resultExitCode();
    if (code === null || code === undefined) return '退出码 -';
    return `退出码 ${code}`;
  }

  function resultElapsedMs() {
    if (typeof jsonData?.elapsed_ms === 'number') return jsonData.elapsed_ms;
    if (typeof durationMs === 'number' && durationMs > 0) return durationMs;
    return null;
  }

  function statusColor(s) {
    if (s === 'ok') return '#34d399';
    if (s === 'warn') return '#fbbf24';
    if (s === 'error') return '#f87171';
    return '#94a3b8';
  }

  function barColor(s) {
    if (s === 'ok') return '#34d399';
    if (s === 'warn') return '#fbbf24';
    if (s === 'error') return '#f87171';
    return '#22d3ee';
  }

  function renderJsonValue(value, depth = 0) {
    if (value === null) return '<span class="json-null">null</span>';
    if (typeof value === 'boolean') return `<span class="json-bool">${value}</span>`;
    if (typeof value === 'number') return `<span class="json-number">${value}</span>`;
    if (typeof value === 'string') return `<span class="json-string">"${escapeHtml(value)}"</span>`;
    if (Array.isArray(value)) {
      if (value.length === 0) return '<span class="json-bracket">[]</span>';
      const items = value.map(v => renderJsonValue(v, depth + 1)).join(', ');
      return `<span class="json-bracket">[</span>${items}<span class="json-bracket">]</span>`;
    }
    if (typeof value === 'object') {
      const entries = Object.entries(value);
      if (entries.length === 0) return '<span class="json-bracket">{}</span>';
      return `<span class="json-bracket">{</span> ${entries.map(([k, v]) => `<span class="json-key">"${escapeHtml(k)}"</span>: ${renderJsonValue(v, depth + 1)}`).join(', ')} <span class="json-bracket">}</span>`;
    }
    return String(value);
  }

  function escapeHtml(str) {
    return str.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
  }

  $effect(() => {
    if (id && termContainer) {
      initTerminal();
      loadInfo();
      refreshRunProfile();
    }
  });

  onMount(() => {
    document.addEventListener('fullscreenchange', onFullscreenChange);
    document.addEventListener('keydown', onScriptKeyDown);
    if (termContainer) initTerminal();
    if (autorun) {
      const waitForReady = (tries = 0) => {
        if (terminal && info) {
          runScript(null, loadPendingRunPayload());
        } else if (tries < 60) {
          setTimeout(() => waitForReady(tries + 1), 50);
        }
      };
      waitForReady();
    }
  });

  onDestroy(() => {
    document.removeEventListener('fullscreenchange', onFullscreenChange);
    document.removeEventListener('keydown', onScriptKeyDown);
    stopTimer();
    if (copyTimer) clearTimeout(copyTimer);
    if (downloadTimer) clearTimeout(downloadTimer);
    if (commandCopyTimer) clearTimeout(commandCopyTimer);
    if (ws) ws.close();
    if (terminal) {
      try { terminal.dispose(); } catch (_) {}
      terminal = null;
    }
  });

  function onScriptKeyDown(e) {
    if (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA' || e.target.tagName === 'SELECT' || e.target.isContentEditable) return;
    if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') { e.preventDefault(); if (!running) runScript(); return; }
    if ((e.metaKey || e.ctrlKey) && e.key === '.') { e.preventDefault(); if (running) stopScript(); return; }
    if (e.key === 'Escape' && !e.metaKey && !e.ctrlKey) { e.preventDefault(); location.hash = '#/scripts'; return; }
  }
</script>

<div class="detail-page">
  <div class="detail-header">
    <a href="#/scripts" class="back-btn" aria-label="返回脚本列表">
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none">
        <path d="m15 18-6-6 6-6" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
      </svg>
    </a>
    <div class="header-info">
      <h1 class="detail-title">{info?.name || id}</h1>
      <p class="detail-desc">{info?.description || ''}</p>
    </div>
    <div class="header-actions">
      {#if info?.metadata?.version}
        <span class="version-badge">v{info.metadata.version}</span>
      {/if}
      <button class="action-btn source-btn" onclick={openSource} title="查看脚本源码" aria-label="查看脚本源码">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <polyline points="16 18 22 12 16 6" stroke-linecap="round" stroke-linejoin="round"/>
          <polyline points="8 6 2 12 8 18" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
        <span>源码</span>
      </button>
      <button class="action-btn history-btn" onclick={loadHistory} title="查看执行历史" aria-label="查看执行历史">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="12" cy="12" r="10"/>
          <polyline points="12 6 12 12 16 14" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
        <span>历史</span>
      </button>
      <button class="action-btn dup-btn" onclick={openDuplicate} title="复制此脚本" aria-label="复制脚本">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <rect x="9" y="9" width="13" height="13" rx="2" stroke-linecap="round" stroke-linejoin="round"/>
          <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
        <span>复制</span>
      </button>
      <button class="action-btn command-btn" onclick={copyRunCommand} title="复制运行命令" aria-label="复制运行命令">
        {#if commandCopyOk}
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="m5 12 5 5L20 7" stroke-linecap="round" stroke-linejoin="round"/>
          </svg>
          <span>已复制</span>
        {:else}
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M4 17l6-6-6-6" stroke-linecap="round" stroke-linejoin="round"/>
            <path d="M12 19h8" stroke-linecap="round"/>
          </svg>
          <span>命令</span>
        {/if}
      </button>
      <button class="action-btn refresh-btn" onclick={refreshRunProfile} title="刷新运行画像" aria-label="刷新运行画像">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M21 12a9 9 0 0 1-15.4 6.4L3 16" stroke-linecap="round" stroke-linejoin="round"/>
          <path d="M3 21v-5h5" stroke-linecap="round" stroke-linejoin="round"/>
          <path d="M3 12a9 9 0 0 1 15.4-6.4L21 8" stroke-linecap="round" stroke-linejoin="round"/>
          <path d="M21 3v5h-5" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
        <span>刷新</span>
      </button>
      <button class="action-btn stats-btn" onclick={loadStats} title="查看执行统计" aria-label="查看执行统计">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M18 20V10" stroke-linecap="round" stroke-linejoin="round"/>
          <path d="M12 20V4" stroke-linecap="round" stroke-linejoin="round"/>
          <path d="M6 20v-6" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
        <span>统计</span>
      </button>
      <button class="action-btn run-btn" onclick={runScript} disabled={running}>
        {#if running}
          <span class="btn-spinner"></span>
          <span>执行中</span>
        {:else}
          <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
            <path d="M8 5v14l11-7z"/>
          </svg>
          <span>执行</span>
        {/if}
      </button>
      {#if running}
        <button class="action-btn stop-btn" onclick={stopScript}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
            <rect x="6" y="6" width="12" height="12" rx="2"/>
          </svg>
          <span>停止</span>
        </button>
      {/if}
    </div>
  </div>

  {#if info}
    <div class="info-strip">
      <div class="info-item">
        <span class="info-label">分类</span>
        <span class="info-value">{info.category}</span>
      </div>
      <div class="info-item">
        <span class="info-label">标识</span>
        <span class="info-value">{info.id}</span>
      </div>
      <div class="info-item">
        <span class="info-label">文件</span>
        <span class="info-value">{(info.path || '').split('/').pop()}</span>
      </div>
    </div>
  {/if}

  <div class="run-profile">
    <div class="profile-card">
      <span class="profile-label">执行次数</span>
      <strong>{statsLoading && !statsData ? '...' : statsData?.total_executions ?? 0}</strong>
    </div>
    <div class="profile-card">
      <span class="profile-label">成功率</span>
      <strong class:profile-ok={successRateText() === '100%'}>{statsLoading && !statsData ? '...' : successRateText()}</strong>
    </div>
    <div class="profile-card">
      <span class="profile-label">最近结果</span>
      <strong class="profile-state {lastProfileState.tone}">{lastProfileState.text}</strong>
    </div>
    <div class="profile-card">
      <span class="profile-label">平均耗时</span>
      <strong>{statsLoading && !statsData ? '...' : formatDurationCompact(statsData?.avg_duration_ms)}</strong>
    </div>
    <button class="profile-card profile-wide profile-action" onclick={loadHistory} title="打开执行历史">
      <span class="profile-label">最近记录</span>
      {#if historyLoading && historyData.length === 0}
        <strong>加载中...</strong>
      {:else if historyData[0]}
        <strong>{formatTime(historyData[0].timestamp)} · {formatDurationCompact(historyData[0].duration_ms)} · {historyData[0].output_lines || 0} 行</strong>
      {:else}
        <strong>暂无执行记录</strong>
      {/if}
    </button>
  </div>

  {#if showParams && info?.metadata?.params?.length}
    <div class="params-form">
      <div class="params-header">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="12" cy="12" r="3"/>
          <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/>
        </svg>
        <span>参数设置</span>
        <span class="params-count">{info.metadata.params.length} 个</span>
        <button class="params-reset" onclick={initParamValues} type="button" title="重置为默认值">重置</button>
        <button class="params-save" onclick={saveParamPreset} type="button" title="保存当前参数为预设">保存</button>
        <button class="params-save" onclick={clearParamPreset} type="button" title="清除已保存的预设">清除</button>
      </div>
      <div class="params-grid">
        {#each info.metadata.params as p}
          <div class="param-row" class:param-required={p.required}>
            <label class="param-label" for="param-{p.name}">
              <code class="param-name">{p.name}</code>
              <span class="param-type">{p.type || 'string'}</span>
              {#if p.required}<span class="param-req">必填</span>{/if}
            </label>
            {#if p.description}
              <p class="param-desc">{p.description}</p>
            {/if}
            {#if p.type === 'boolean'}
              <select id="param-{p.name}" bind:value={paramValues[p.name]} class="param-input">
                <option value="true">true</option>
                <option value="false">false</option>
              </select>
            {:else if p.type === 'number'}
              <input id="param-{p.name}" type="number" bind:value={paramValues[p.name]} placeholder={p.default ?? ''} class="param-input" />
            {:else}
              <input id="param-{p.name}" type="text" bind:value={paramValues[p.name]} placeholder={p.default ?? ''} class="param-input" />
            {/if}
            {#if p.default}
              <span class="param-default">默认: <code>{p.default}</code></span>
            {/if}
          </div>
        {/each}
      </div>
      <div class="param-command-preview">
        <span>当前命令</span>
        <code>{buildRunCommand()}</code>
        <button type="button" onclick={copyRunCommand}>{commandCopyOk ? '已复制' : '复制'}</button>
      </div>
    </div>
  {/if}

  {#if showHistory}
    <div class="history-panel">
      <div class="history-panel-head">
        <h3>执行历史 — {id}</h3>
        <div class="history-panel-actions">
          <button class="history-sort-btn" onclick={() => changeHistorySort('timestamp')}>时间{historySortMark('timestamp')}</button>
          <button class="history-sort-btn" onclick={() => changeHistorySort('exit_code')}>结果{historySortMark('exit_code')}</button>
          <button class="history-sort-btn" onclick={() => changeHistorySort('duration_ms')}>耗时{historySortMark('duration_ms')}</button>
          <button class="history-panel-close" onclick={() => showHistory = false} aria-label="关闭">✕</button>
        </div>
      </div>
      {#if historyLoading}
        <div class="history-panel-loading">
          <div class="spinner-sm"></div>
          <span>加载中...</span>
        </div>
      {:else if historyData.length === 0}
        <div class="history-panel-empty">暂无执行记录</div>
      {:else}
        <div class="history-panel-list">
          {#each sortedHistoryData as r}
            <div class="history-panel-item">
              <div class="history-panel-item-head">
                <span class="history-panel-time" title={r.timestamp}>{formatTime(r.timestamp)}</span>
                <div class="history-panel-right">
                  <span class="history-panel-result" style="color:{r.exit_code === 0 ? '#34d399' : r.exit_code === null ? '#fbbf24' : '#f87171'}">
                    {r.exit_code === 0 ? '[OK] 成功' : r.exit_code === null ? '[RUN] 运行中' : '[FAIL] 失败 (' + r.exit_code + ')'}
                  </span>
                  <button class="history-mini-btn" onclick={() => copyHistoryCommand(r)} title="复制可复现运行命令">
                    {copiedHistoryId === String(r.id) ? '已复制' : '复制命令'}
                  </button>
                  <button class="history-mini-btn replay" onclick={() => runScript(r)} disabled={running} title="按这条历史记录的参数再次执行">
                    {replayingHistoryId === String(r.id) ? '执行中' : '按此执行'}
                  </button>
                  {#if Object.keys(normalizedHistoryParams(r)).length > 0}
                    <button class="history-mini-btn reuse" onclick={() => reuseHistoryParams(r)} title="把本次历史参数填回参数表单">
                      {reusedHistoryId === String(r.id) ? '已复用' : '复用参数'}
                    </button>
                  {/if}
                </div>
              </div>
              <div class="history-panel-item-meta">
                {#if r.duration_ms}
                  <span class="history-panel-dur">耗时 {r.duration_ms >= 1000 ? (r.duration_ms/1000).toFixed(1) + 's' : r.duration_ms + 'ms'}</span>
                {/if}
                {#if Object.keys(normalizedHistoryParams(r)).length > 0}
                  {#each Object.entries(normalizedHistoryParams(r)) as [k, v]}
                    <span class="history-panel-param">{k}={v}</span>
                  {/each}
                {/if}
                {#if r.args?.length}
                  {#each r.args as arg}
                    <span class="history-panel-param arg">arg:{arg}</span>
                  {/each}
                {/if}
              </div>
            </div>
          {/each}
        </div>
        <div class="history-panel-footer">共 {historyData.length} 条记录</div>
      {/if}
    </div>
  {/if}

  {#if showStats}
    <div class="stats-panel">
      <div class="stats-panel-head">
        <h3>执行统计 — {id}</h3>
        <button class="stats-panel-close" onclick={() => showStats = false} aria-label="关闭">✕</button>
      </div>
      {#if statsLoading}
        <div class="stats-panel-loading">
          <div class="spinner-sm"></div>
          <span>加载中...</span>
        </div>
      {:else if statsData}
        <div class="stats-grid">
          <div class="stat-cell">
            <div class="stat-num">{statsData.total_executions}</div>
            <div class="stat-label">总执行</div>
          </div>
          <div class="stat-cell">
            <div class="stat-num" style="color:#34d399">{statsData.success_count}</div>
            <div class="stat-label">成功</div>
          </div>
          <div class="stat-cell">
            <div class="stat-num" style="color:#f87171">{statsData.failure_count}</div>
            <div class="stat-label">失败</div>
          </div>
          <div class="stat-cell">
            <div class="stat-num">{statsData.total_executions > 0 ? Math.round(statsData.success_count / statsData.total_executions * 100) : 0}%</div>
            <div class="stat-label">成功率</div>
          </div>
          <div class="stat-cell">
            <div class="stat-num">{statsData.avg_duration_ms ? (statsData.avg_duration_ms >= 1000 ? (statsData.avg_duration_ms/1000).toFixed(1) + 's' : Math.round(statsData.avg_duration_ms) + 'ms') : '-'}</div>
            <div class="stat-label">平均耗时</div>
          </div>
          <div class="stat-cell">
            <div class="stat-num">{statsData.last_execution ? formatTime(statsData.last_execution.timestamp) : '-'}</div>
            <div class="stat-label">最后执行</div>
          </div>
        </div>
        {#if statsData.last_execution}
          <div class="stats-last">
            <span class="stats-last-label">最后结果:</span>
            <span class="stats-last-value" style="color:{statsData.last_execution.exit_code === 0 ? '#34d399' : '#f87171'}">
              {statsData.last_execution.exit_code === 0 ? '[OK] 成功' : '[FAIL] 失败 (' + statsData.last_execution.exit_code + ')'}
            </span>
          </div>
        {/if}
      {:else}
        <div class="stats-panel-empty">暂无执行数据</div>
      {/if}
    </div>
  {/if}

  {#if error}
    <div class="error-banner">
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none">
        <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="2"/>
        <path d="m15 9-6 6M9 9l6 6" stroke="currentColor" stroke-width="2" stroke-linecap="round"/>
      </svg>
      <span>{error}</span>
    </div>
  {/if}

  <div class="terminal" class:fullscreen={isFullscreen} class:structured={showJsonView && jsonData} bind:this={terminalWrap}>
    <div class="terminal-bar">
      <div class="terminal-dots">
        <span class="dot dot-red"></span>
        <span class="dot dot-yellow"></span>
        <span class="dot dot-green"></span>
      </div>
      <span class="terminal-title">{showJsonView && jsonData ? (jsonData.name || id) : id}</span>
      <span class="terminal-status" class:running class:success={exitCode === 0} class:fail={exitCode !== null && exitCode !== 0}>
        {running ? '[RUN] 运行中' : showJsonView && jsonData ? resultStatusText() : exitCode !== null ? (exitCode === 0 ? '[OK] 完成' : '[FAIL] 失败') : '[READY] 就绪'}
      </span>
      {#if running}
        <span class="terminal-timer">耗时 {formatDuration(elapsedSec * 1000)}</span>
      {:else if durationMs > 0}
        <span class="terminal-timer terminal-timer-static">耗时 {formatDuration(durationMs)}</span>
      {/if}
      {#if resultMetaText()}
        <span class="terminal-meta">{resultMetaText()}</span>
      {/if}
      <div class="terminal-actions">
        <button class="tool-btn" onclick={copyOutput} aria-label={showJsonView && jsonData ? '复制结果 JSON' : '复制输出'} title={showJsonView && jsonData ? '复制结果 JSON' : '复制输出'}>
          {#if copyOk}
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="m5 12 5 5L20 7" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
          {:else}
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <rect x="9" y="9" width="13" height="13" rx="2" stroke-linecap="round" stroke-linejoin="round"/>
              <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
          {/if}
        </button>
        <button class="tool-btn" onclick={downloadLog} aria-label={showJsonView && jsonData ? '下载结果 JSON' : '下载日志'} title={showJsonView && jsonData ? '下载结果 JSON' : '下载日志'}>
          {#if downloadOk}
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="m5 12 5 5L20 7" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
          {:else}
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" stroke-linecap="round" stroke-linejoin="round"/>
              <polyline points="7 10 12 15 17 10" stroke-linecap="round" stroke-linejoin="round"/>
              <line x1="12" y1="15" x2="12" y2="3" stroke-linecap="round" stroke-linejoin="round"/>
            </svg>
          {/if}
        </button>
        <button class="tool-btn" onclick={clearTerminal} aria-label={showJsonView && jsonData ? '清空结果' : '清屏'} title={showJsonView && jsonData ? '清空结果' : '清屏'}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M3 6h18" stroke-linecap="round"/>
            <path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" stroke-linecap="round"/>
            <path d="m19 6-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6" stroke-linecap="round"/>
          </svg>
        </button>
        <button class="tool-btn" onclick={toggleFullscreen} aria-label="全屏" title="全屏 (F11)">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            {#if isFullscreen}
              <path d="M8 3v3a2 2 0 0 1-2 2H3" stroke-linecap="round" stroke-linejoin="round"/>
              <path d="M21 8h-3a2 2 0 0 1-2-2V3" stroke-linecap="round" stroke-linejoin="round"/>
              <path d="M3 16h3a2 2 0 0 1 2 2v3" stroke-linecap="round" stroke-linejoin="round"/>
              <path d="M16 21v-3a2 2 0 0 1 2-2h3" stroke-linecap="round" stroke-linejoin="round"/>
            {:else}
              <path d="M3 8V5a2 2 0 0 1 2-2h3" stroke-linecap="round" stroke-linejoin="round"/>
              <path d="M21 8V5a2 2 0 0 0-2-2h-3" stroke-linecap="round" stroke-linejoin="round"/>
              <path d="M3 16v3a2 2 0 0 0 2 2h3" stroke-linecap="round" stroke-linejoin="round"/>
              <path d="M16 21h3a2 2 0 0 0 2-2v-3" stroke-linecap="round" stroke-linejoin="round"/>
            {/if}
          </svg>
        </button>
      </div>
    </div>
    {#if showJsonView && jsonData}
      <div class="result-view">
        <div class="result-header">
          <span class="result-title">执行结果</span>
          <span class="result-badge" style="color:{statusColor(resultStatus())};background:{statusColor(resultStatus())}15">
            {resultStatusText()}
          </span>
          <span class="result-chip">{resultExitText()}</span>
          {#if resultMetaText()}
            <span class="result-chip">{resultMetaText()}</span>
          {/if}
          {#if resultElapsedMs() !== null}
            <span class="result-chip">耗时 {formatDurationCompact(resultElapsedMs())}</span>
          {/if}
        </div>

        {#if jsonData.sections}
          <div class="result-sections">
            {#each jsonData.sections as section, sectionIndex}
              {@const sectionOpen = isResultSectionOpen(section, sectionIndex)}
              <div class="result-section">
                <button type="button" class="section-header" onclick={() => toggleResultSection(section, sectionIndex)} aria-expanded={sectionOpen}>
                  {#if section.icon}<span class="section-icon">{section.icon}</span>{/if}
                  <h3 class="section-title">{section.title}</h3>
                  <span class="section-toggle">{sectionOpen ? '收起' : '展开'}</span>
                </button>
                {#if section.description && !sectionOpen}
                  <p class="section-desc collapsed">{section.description}</p>
                {/if}
                {#if sectionOpen}
                  {#if section.description}
                    <p class="section-desc">{section.description}</p>
                  {/if}
                  <div class="section-items">
                  {#each section.items as item, itemIndex}
                    {@const itemKey = `result-${sectionIndex}-${itemIndex}`}
                    {#if item.type === 'finding'}
                      <div class="finding-card" class:finding-error={item.level === 'error'}>
                        <div class="finding-head">
                          <span class="finding-level">{statusIcon(item.level)}</span>
                          <div class="finding-main">
                            <div class="finding-title">{item.title}</div>
                            <div class="finding-summary">{item.summary}</div>
                          </div>
                        </div>
                        <div class="finding-meta">
                          <span>规则 {item.rule_id}</span>
                          <span>对象 {item.target || '-'}</span>
                          <span>分类 {item.category || '-'}</span>
                        </div>
                        <div class="finding-detail">
                          {#if item.evidence?.length}
                            <div class="finding-block">
                              <div class="finding-block-title">证据</div>
                              {#each item.evidence as line}
                                <div class="finding-line">{line}</div>
                              {/each}
                            </div>
                          {/if}
                          {#if item.suggestion}
                            <div class="finding-block">
                              <div class="finding-block-title">处理建议</div>
                              <div class="finding-line">{item.suggestion}</div>
                            </div>
                          {/if}
                          {#if item.commands?.length}
                            <div class="finding-block">
                              <div class="finding-block-title">建议命令</div>
                              {#each item.commands as cmd}
                                <code class="finding-command">{cmd}</code>
                              {/each}
                            </div>
                          {/if}
                        </div>
                      </div>
                    {:else if item.type === 'label'}
                      <div class="result-item">
                        <span class="item-key">{item.key}</span>
                        <span class="item-value" style="color:{statusColor(item.status)}">{item.value}</span>
                        {#if item.status}
                          <span class="item-status" style="color:{statusColor(item.status)}">{statusIcon(item.status)}</span>
                        {/if}
                      </div>
                    {:else if item.type === 'bar'}
                      <div class="result-item result-bar">
                        <span class="item-key">{item.key}</span>
                        <div class="bar-wrap">
                          <div class="bar-track">
                            <div class="bar-fill" style="width:{Math.min(item.value / item.max * 100, 100)}%;background:{barColor(item.status)}"></div>
                          </div>
                          <span class="bar-value" style="color:{barColor(item.status)}">{item.value?.toFixed(1)}{item.unit || ''}</span>
                        </div>
                      </div>
                    {:else if item.type === 'table'}
                      <div class="result-table">
                        <table>
                          <thead>
                            <tr>
                              {#each item.headers as h, columnIndex}
                                <th>
                                  <button class="table-sort-btn" onclick={() => changeResultTableSort(itemKey, columnIndex)}>
                                    {h}{resultTableSortMark(itemKey, columnIndex)}
                                  </button>
                                </th>
                              {/each}
                            </tr>
                          </thead>
                          <tbody>
                            {#each sortedResultRows(item, itemKey) as row}
                              <tr>
                                {#each row as cell}
                                  <td>{cell}</td>
                                {/each}
                              </tr>
                            {/each}
                          </tbody>
                        </table>
                      </div>
                    {:else if item.type === 'sparkline'}
                      <div class="result-item result-sparkline">
                        <span class="item-key">{item.key}</span>
                        <div class="sparkline-bars" aria-label="{item.key} 趋势">
                          {#each sparklineBars(item.data) as point}
                            <span class="sparkline-bar" style="height:{point.height}px;background:{barColor(item.status)}" title="{point.value}{item.unit || ''}"></span>
                          {/each}
                        </div>
                        <span class="sparkline-value" style="color:{statusColor(item.status)}">
                          {item.data?.length > 0 ? Number(item.data[item.data.length - 1]).toFixed(1) : '0'}{item.unit || ''}
                        </span>
                      </div>
                    {:else if item.type === 'info'}
                      <div class="result-msg msg-info"><span class="msg-tag">INFO</span><pre class="msg-pre">{item.text}</pre></div>
                    {:else if item.type === 'warning'}
                      <div class="result-msg msg-warn"><span class="msg-tag">WARN</span><span class="msg-text">{item.text}</span></div>
                    {:else if item.type === 'error'}
                      <div class="result-msg msg-error"><span class="msg-tag">FAIL</span><span class="msg-text">{item.text}</span></div>
                    {:else if item.type === 'success'}
                      <div class="result-msg msg-success"><span class="msg-tag">OK</span><span class="msg-text">{item.text}</span></div>
                    {:else if item.type === 'divider'}
                      <div class="result-divider"></div>
                    {/if}
                  {/each}
                  </div>
                {/if}
              </div>
            {/each}
          </div>
        {:else}
          <div class="result-raw">
            <pre class="raw-content">{JSON.stringify(jsonData, null, 2)}</pre>
          </div>
        {/if}
      </div>
    {:else}
      <div class="terminal-body" bind:this={termContainer}></div>
    {/if}
    {#if running}
      <div class="terminal-progress"></div>
    {/if}
  </div>
</div>

{#if showSource}
  <div class="source-overlay" onclick={closeSource} role="presentation">
    <div class="source-modal" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="dialog" aria-modal="true" aria-labelledby="src-title" tabindex="-1">
      <div class="source-head">
        <div class="source-head-info">
          <h2 id="src-title">脚本源码 — {id}</h2>
          {#if sourceMeta}
            <span class="source-meta">{sourceMeta.line_count} 行 · {formatBytes(sourceMeta.size_bytes)} · {sourceMeta.path}</span>
          {/if}
        </div>
        <div class="source-head-actions">
          <button class="source-action-btn" onclick={copySource} aria-label="复制源码" title="复制源码">
            {#if sourceCopyOk}
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="m5 12 5 5L20 7" stroke-linecap="round" stroke-linejoin="round"/></svg>
              <span>已复制</span>
            {:else}
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="9" y="9" width="13" height="13" rx="2" stroke-linecap="round" stroke-linejoin="round"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" stroke-linecap="round" stroke-linejoin="round"/></svg>
              <span>复制</span>
            {/if}
          </button>
          <button class="source-action-btn source-close" onclick={closeSource} aria-label="关闭" title="关闭 (Esc)">✕</button>
        </div>
      </div>
      <div class="source-body">
        {#if sourceLoading}
          <div class="source-loading">
            <div class="spinner"></div>
            <span>加载源码中...</span>
          </div>
        {:else}
          <pre class="source-code"><code>{sourceContent}</code></pre>
        {/if}
      </div>
    </div>
  </div>
{/if}

{#if showDuplicate}
  <div class="source-overlay" onclick={() => showDuplicate = false} role="presentation">
    <div class="source-modal dup-modal" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="dialog" aria-modal="true" tabindex="-1">
      <div class="source-head">
        <h2>复制脚本 — {id}</h2>
        <button class="source-action-btn source-close" onclick={() => showDuplicate = false} aria-label="关闭">✕</button>
      </div>
      <div style="padding: 16px;">
        <div style="margin-bottom: 12px;">
          <label for="duplicate-id" style="display:block;font-size:12px;color:#94a3b8;margin-bottom:6px;">新脚本 ID</label>
          <input
            id="duplicate-id"
            type="text"
            bind:value={duplicateId}
            class="dup-input"
            placeholder="输入新脚本 ID"
            aria-label="新脚本 ID"
            onkeydown={(e) => { if (e.key === 'Enter') doDuplicate(); if (e.key === 'Escape') showDuplicate = false; }} />
          {#if duplicateError}
            <p style="margin:6px 0 0;font-size:11px;color:#ef4444;">{duplicateError}</p>
          {/if}
        </div>
        <div style="display:flex;gap:8px;justify-content:flex-end;">
          <button class="source-action-btn" onclick={() => showDuplicate = false}>取消</button>
          <button class="source-action-btn" style="background:rgba(34,211,238,0.1);border-color:rgba(34,211,238,0.2);color:#22d3ee;" onclick={doDuplicate} disabled={duplicateLoading}>
            {#if duplicateLoading}
              <span class="spinner-sm"></span> 复制中...
            {:else}
              确认复制
            {/if}
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .detail-page {
    max-width: 1200px;
    margin: 0 auto;
  }

  .detail-header {
    display: flex;
    align-items: flex-start;
    gap: 16px;
    margin-bottom: 16px;
  }

  .back-btn {
    color: #4b5563;
    text-decoration: none;
    padding: 8px;
    border-radius: 10px;
    transition: all 0.2s;
    display: flex;
    align-items: center;
    justify-content: center;
    margin-top: 4px;
  }

  .back-btn:hover {
    color: #94a3b8;
    background: rgba(255, 255, 255, 0.05);
  }

  .header-info {
    flex: 1;
    min-width: 0;
  }

  .detail-title {
    font-size: 20px;
    font-weight: 700;
    color: #f1f5f9;
    letter-spacing: -0.3px;
  }

  .detail-desc {
    font-size: 12px;
    color: #4b5563;
    margin-top: 4px;
  }

  .header-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .version-badge {
    font-size: 11px;
    padding: 4px 10px;
    border-radius: 8px;
    background: rgba(34, 211, 238, 0.08);
    color: #22d3ee;
    border: 1px solid rgba(34, 211, 238, 0.15);
    font-weight: 500;
  }

  .action-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 16px;
    border-radius: 8px;
    border: none;
    cursor: pointer;
    font-size: 12px;
    font-weight: 600;
    transition: all 0.2s;
  }

  .run-btn {
    background: linear-gradient(135deg, #06b6d4, #0891b2);
    color: #fff;
    box-shadow: 0 4px 16px rgba(6, 182, 212, 0.3);
  }

  .run-btn:hover:not(:disabled) {
    transform: translateY(-1px);
    box-shadow: 0 6px 20px rgba(6, 182, 212, 0.4);
  }

  .run-btn:disabled {
    background: #1c1f2b;
    color: #4b5563;
    cursor: not-allowed;
    box-shadow: none;
  }

  .btn-spinner {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    border: 2px solid rgba(255, 255, 255, 0.2);
    border-top-color: #fff;
    animation: spin 0.8s linear infinite;
  }

  .stop-btn {
    background: rgba(239, 68, 68, 0.1);
    color: #ef4444;
    border: 1px solid rgba(239, 68, 68, 0.2);
  }

  .stop-btn:hover {
    background: rgba(239, 68, 68, 0.2);
  }

  .info-strip {
    display: flex;
    gap: 24px;
    padding: 12px 18px;
    background: rgba(15, 17, 23, 0.7);
    backdrop-filter: blur(16px);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 12px;
    margin-bottom: 16px;
  }

  .run-profile {
    display: grid;
    grid-template-columns: repeat(4, minmax(120px, 1fr)) minmax(220px, 1.35fr);
    gap: 8px;
    margin-bottom: 16px;
  }

  .profile-card {
    min-width: 0;
    display: grid;
    gap: 4px;
    padding: 10px 12px;
    border: 1px solid var(--border-primary);
    border-radius: 10px;
    background:
      linear-gradient(180deg, rgba(255,255,255,.025), transparent 65%),
      var(--bg-card);
  }

  .profile-action {
    width: 100%;
    text-align: left;
    cursor: pointer;
    transition: border-color .16s ease, background .16s ease, transform .16s ease;
  }

  .profile-action:hover,
  .profile-action:focus-visible {
    border-color: var(--border-focus);
    background:
      linear-gradient(180deg, rgba(34,211,238,.065), transparent 72%),
      var(--bg-card);
    transform: translateY(-1px);
    outline: none;
  }

  .profile-label {
    color: var(--text-tertiary);
    font-size: 10px;
    font-weight: 800;
    letter-spacing: .06em;
  }

  .profile-card strong {
    min-width: 0;
    color: var(--text-primary);
    font-family: var(--theme-font-family-mono);
    font-size: 15px;
    font-weight: 900;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .profile-card .profile-ok,
  .profile-state.ok {
    color: #34d399;
  }

  .profile-state.fail {
    color: #fb7185;
  }

  .profile-state.running {
    color: #fbbf24;
  }

  .profile-state.idle {
    color: var(--text-secondary);
  }

  .info-item {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .info-label {
    font-size: 11px;
    color: #4b5563;
    font-weight: 500;
  }

  .info-value {
    font-size: 12px;
    color: #94a3b8;
    font-family: var(--theme-font-family-mono);
  }

  .params-form {
    background: rgba(15, 17, 23, 0.7);
    backdrop-filter: blur(16px);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 12px;
    padding: 14px 18px;
    margin-bottom: 16px;
  }
  .params-header {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 12px;
    color: #22d3ee;
    font-size: 12px;
    font-weight: 600;
  }
  .params-count {
    font-size: 10px;
    color: #6b7280;
    background: rgba(255, 255, 255, 0.04);
    padding: 2px 8px;
    border-radius: 8px;
    font-weight: 500;
  }
  .params-reset {
    margin-left: auto;
    background: transparent;
    border: 1px solid rgba(255, 255, 255, 0.08);
    color: #94a3b8;
    font-size: 11px;
    padding: 3px 10px;
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.15s;
  }
  .params-reset:hover { background: rgba(34, 211, 238, 0.08); color: #22d3ee; border-color: rgba(34, 211, 238, 0.2); }
  .params-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: 10px;
  }
  .param-command-preview {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    align-items: center;
    gap: 8px;
    margin-top: 10px;
    padding: 8px 10px;
    border: 1px solid rgba(34, 211, 238, 0.14);
    border-radius: 9px;
    background: rgba(34, 211, 238, 0.045);
  }
  .param-command-preview span {
    color: #67e8f9;
    font-size: 10px;
    font-weight: 900;
    letter-spacing: .06em;
    white-space: nowrap;
  }
  .param-command-preview code {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: #dbeafe;
    font-family: var(--theme-font-family-mono);
    font-size: 11px;
  }
  .param-command-preview button {
    min-height: 24px;
    padding: 0 8px;
    border: 1px solid rgba(34, 211, 238, 0.2);
    border-radius: 7px;
    background: rgba(34, 211, 238, 0.08);
    color: #67e8f9;
    font-size: 10px;
    font-weight: 800;
    cursor: pointer;
  }
  .param-command-preview button:hover {
    border-color: rgba(34, 211, 238, 0.36);
    background: rgba(34, 211, 238, 0.14);
  }
  .param-row {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 10px 12px;
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.04);
    border-radius: 8px;
    transition: border-color 0.15s;
  }
  .param-row:hover { border-color: rgba(255, 255, 255, 0.08); }
  .param-row.param-required { border-color: rgba(239, 68, 68, 0.15); }
  .param-label {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }
  .param-name {
    font-family: var(--theme-font-family-mono);
    font-size: 12px;
    color: #f1f5f9;
    font-weight: 600;
  }
  .param-type {
    font-size: 10px;
    color: #6b7280;
    background: rgba(255, 255, 255, 0.04);
    padding: 1px 6px;
    border-radius: 4px;
    font-family: var(--theme-font-family-mono);
  }
  .param-req {
    font-size: 9px;
    color: #ef4444;
    background: rgba(239, 68, 68, 0.1);
    padding: 1px 5px;
    border-radius: 3px;
    font-weight: 600;
  }
  .param-desc {
    margin: 0;
    font-size: 11px;
    color: #6b7280;
    line-height: 1.5;
  }
  .param-input {
    width: 100%;
    padding: 6px 10px;
    background: rgba(0, 0, 0, 0.3);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 6px;
    color: #e2e8f0;
    font-size: 12px;
    font-family: var(--theme-font-family-mono);
    outline: none;
    transition: all 0.15s;
    box-sizing: border-box;
  }
  .param-input:focus { border-color: rgba(34, 211, 238, 0.3); box-shadow: 0 0 0 3px rgba(34, 211, 238, 0.08); }
  select.param-input { appearance: none; background-image: url("data:image/svg+xml;charset=utf-8,%3Csvg xmlns='http://www.w3.org/2000/svg' width='10' height='10' viewBox='0 0 24 24' fill='none' stroke='%2394a3b8' stroke-width='2'%3E%3Cpath d='m6 9 6 6 6-6'/%3E%3C/svg%3E"); background-repeat: no-repeat; background-position: right 8px center; padding-right: 24px; }
  select.param-input option { background: #0f1117; color: #e2e8f0; }
  .param-default {
    font-size: 10px;
    color: #6b7280;
  }
  .param-default code {
    font-family: var(--theme-font-family-mono);
    color: #94a3b8;
    background: rgba(255, 255, 255, 0.03);
    padding: 0 4px;
    border-radius: 3px;
  }

  .source-btn {
    background: rgba(255, 255, 255, 0.05);
    color: #94a3b8;
    border: 1px solid rgba(255, 255, 255, 0.06);
  }
  .source-btn:hover:not(:disabled) {
    background: rgba(34, 211, 238, 0.08);
    color: #22d3ee;
    border-color: rgba(34, 211, 238, 0.2);
    transform: none;
    box-shadow: none;
  }

  .history-btn {
    background: rgba(255, 255, 255, 0.05);
    color: #94a3b8;
    border: 1px solid rgba(255, 255, 255, 0.06);
  }
  .history-btn:hover:not(:disabled) {
    background: rgba(139, 92, 246, 0.08);
    color: #a78bfa;
    border-color: rgba(139, 92, 246, 0.2);
    transform: none;
    box-shadow: none;
  }

  .dup-btn {
    background: rgba(255, 255, 255, 0.05);
    color: #94a3b8;
    border: 1px solid rgba(255, 255, 255, 0.06);
  }
  .dup-btn:hover:not(:disabled) {
    background: rgba(52, 211, 153, 0.08);
    color: #34d399;
    border-color: rgba(52, 211, 153, 0.2);
    transform: none;
    box-shadow: none;
  }

  .command-btn {
    background: rgba(34, 211, 238, 0.06);
    color: #67e8f9;
    border: 1px solid rgba(34, 211, 238, 0.14);
  }

  .command-btn:hover:not(:disabled) {
    background: rgba(34, 211, 238, 0.11);
    color: #22d3ee;
    border-color: rgba(34, 211, 238, 0.28);
    transform: none;
    box-shadow: none;
  }

  .refresh-btn {
    background: rgba(96, 165, 250, 0.06);
    color: #93c5fd;
    border: 1px solid rgba(96, 165, 250, 0.14);
  }

  .refresh-btn:hover:not(:disabled) {
    background: rgba(96, 165, 250, 0.11);
    color: #bfdbfe;
    border-color: rgba(96, 165, 250, 0.28);
    transform: none;
    box-shadow: none;
  }

  .stats-btn {
    background: rgba(255, 255, 255, 0.05);
    color: #94a3b8;
    border: 1px solid rgba(255, 255, 255, 0.06);
  }
  .stats-btn:hover:not(:disabled) {
    background: rgba(251, 191, 36, 0.08);
    color: #fbbf24;
    border-color: rgba(251, 191, 36, 0.2);
    transform: none;
    box-shadow: none;
  }

  .dup-modal { max-width: 420px; }
  .dup-input {
    width: 100%; padding: 8px 12px;
    background: rgba(0, 0, 0, 0.3); border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 6px; color: #e2e8f0; font-size: 13px;
    font-family: var(--theme-font-family-mono); outline: none;
    transition: all 0.15s; box-sizing: border-box;
  }
  .dup-input:focus { border-color: rgba(34, 211, 238, 0.3); box-shadow: 0 0 0 3px rgba(34, 211, 238, 0.08); }

  .history-panel {
    background: var(--bg-card);
    border: 1px solid var(--border-primary);
    border-radius: 12px;
    margin-bottom: 16px;
    overflow: hidden;
    max-height: 400px;
    display: flex;
    flex-direction: column;
  }
  .history-panel-head {
    display: flex; align-items: center; justify-content: space-between;
    padding: 10px 14px;
    border-bottom: 1px solid var(--border-primary);
    background: var(--bg-secondary);
  }
  .history-panel-head h3 { margin: 0; font-size: 13px; font-weight: 600; color: var(--text-primary); }
  .history-panel-actions { display: flex; align-items: center; gap: 6px; }
  .history-sort-btn { min-height: 26px; padding: 0 8px; border: 1px solid var(--border-primary); border-radius: 6px; background: var(--bg-card); color: var(--text-secondary); font-size: 11px; cursor: pointer; }
  .history-sort-btn:hover { background: var(--bg-hover); color: var(--text-primary); }
  .history-panel-close { background: none; border: none; color: var(--text-tertiary); font-size: 14px; cursor: pointer; padding: 4px 8px; border-radius: 5px; transition: all 0.15s; }
  .history-panel-close:hover { background: var(--bg-hover); color: var(--text-primary); }
  .history-panel-loading { display: flex; align-items: center; justify-content: center; gap: 8px; padding: 24px; color: var(--text-secondary); font-size: 12px; }
  .spinner-sm { width: 14px; height: 14px; border-radius: 50%; border: 2px solid var(--accent-primary-light); border-top-color: var(--accent-primary); animation: spin 0.8s linear infinite; }
  .history-panel-empty { padding: 24px; text-align: center; color: var(--text-secondary); font-size: 12px; }
  .history-panel-list { overflow-y: auto; flex: 1; }
  .history-panel-item { padding: 8px 14px; border-bottom: 1px solid var(--border-secondary); transition: background 0.15s; }
  .history-panel-item:hover { background: var(--bg-hover); }
  .history-panel-item:last-child { border-bottom: none; }
  .history-panel-item-head { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
  .history-panel-time { font-family: var(--theme-font-family-mono); font-size: 11px; color: var(--text-secondary); }
  .history-panel-right { display: flex; align-items: center; justify-content: flex-end; gap: 6px; min-width: 0; }
  .history-panel-result { font-size: 11px; font-weight: 600; }
  .history-mini-btn { min-height: 22px; padding: 0 7px; border-radius: 6px; border: 1px solid var(--border-primary); background: var(--bg-card); color: var(--text-secondary); font-size: 10px; font-weight: 800; cursor: pointer; white-space: nowrap; }
  .history-mini-btn:hover { border-color: var(--border-focus); background: var(--accent-primary-light); color: var(--accent-primary); }
  .history-mini-btn:disabled { opacity: .48; cursor: not-allowed; }
  .history-mini-btn.replay { border-color: rgba(34,211,238,.2); color: #67e8f9; }
  .history-mini-btn.replay:hover:not(:disabled) { background: rgba(34,211,238,.1); border-color: rgba(34,211,238,.36); }
  .history-mini-btn.reuse { border-color: rgba(52,211,153,.2); color: #86efac; }
  .history-mini-btn.reuse:hover { background: rgba(52,211,153,.1); border-color: rgba(52,211,153,.36); }
  .history-panel-item-meta { display: flex; align-items: center; flex-wrap: wrap; gap: 6px; margin-top: 4px; }
  .history-panel-dur { font-family: var(--theme-font-family-mono); font-size: 10px; color: var(--text-secondary); }
  .history-panel-param { font-family: var(--theme-font-family-mono); font-size: 9px; color: var(--text-secondary); background: var(--bg-secondary); padding: 1px 5px; border-radius: 3px; }
  .history-panel-param.arg { color: #93c5fd; background: rgba(59,130,246,.1); }
  .history-panel-footer { padding: 6px 14px; border-top: 1px solid var(--border-primary); font-size: 10px; color: var(--text-tertiary); }

  .stats-panel {
    background: rgba(15, 17, 23, 0.7);
    backdrop-filter: blur(16px);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 12px;
    margin-bottom: 16px;
    overflow: hidden;
  }
  .stats-panel-head {
    display: flex; align-items: center; justify-content: space-between;
    padding: 10px 14px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    background: rgba(15, 17, 23, 0.5);
  }
  .stats-panel-head h3 { margin: 0; font-size: 13px; font-weight: 600; color: #c9d1d9; }
  .stats-panel-close { background: none; border: none; color: #6b7280; font-size: 14px; cursor: pointer; padding: 4px 8px; border-radius: 5px; transition: all 0.15s; }
  .stats-panel-close:hover { background: rgba(255, 255, 255, 0.06); color: #94a3b8; }
  .stats-panel-loading { display: flex; align-items: center; justify-content: center; gap: 8px; padding: 24px; color: #6b7280; font-size: 12px; }
  .stats-panel-empty { padding: 24px; text-align: center; color: #6b7280; font-size: 12px; }
  .stats-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 1px; background: rgba(255, 255, 255, 0.04); }
  .stat-cell { display: flex; flex-direction: column; align-items: center; justify-content: center; padding: 12px 8px; background: rgba(15, 17, 23, 0.7); }
  .stat-num { font-size: 18px; font-weight: 700; color: #f1f5f9; font-variant-numeric: tabular-nums; font-family: var(--theme-font-family-mono); }
  .stat-label { font-size: 10px; color: #6b7280; margin-top: 2px; }
  .stats-last { display: flex; align-items: center; gap: 6px; padding: 8px 14px; border-top: 1px solid rgba(255, 255, 255, 0.04); font-size: 11px; }
  .stats-last-label { color: #6b7280; }
  .stats-last-value { font-weight: 600; }

  .source-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.7);
    backdrop-filter: blur(10px);
    z-index: 105;
    display: flex;
    align-items: center;
    justify-content: center;
    animation: scFadeIn 0.18s ease-out;
  }
  @keyframes scFadeIn { from { opacity: 0; } to { opacity: 1; } }

  .source-modal {
    width: 880px;
    max-width: 94vw;
    max-height: 84vh;
    background: rgba(15, 17, 23, 0.97);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 14px;
    box-shadow: 0 24px 80px rgba(0, 0, 0, 0.6);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    animation: scSlideUp 0.2s cubic-bezier(0.4, 0, 0.2, 1);
  }
  @keyframes scSlideUp { from { opacity: 0; transform: translateY(10px) scale(0.98); } to { opacity: 1; transform: translateY(0) scale(1); } }

  .source-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 12px 16px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    background: rgba(15, 17, 23, 0.6);
  }
  .source-head-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    flex: 1;
  }
  .source-head-info h2 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    color: #f1f5f9;
    letter-spacing: -0.2px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .source-meta {
    font-size: 11px;
    color: #6b7280;
    font-family: var(--theme-font-family-mono);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .source-head-actions {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }
  .source-action-btn {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 5px 10px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.08);
    color: #94a3b8;
    font-size: 11px;
    font-weight: 500;
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.15s;
  }
  .source-action-btn:hover { background: rgba(34, 211, 238, 0.1); color: #22d3ee; border-color: rgba(34, 211, 238, 0.2); }
  .source-action-btn.source-close { padding: 5px 9px; font-size: 14px; line-height: 1; }

  .source-body {
    flex: 1;
    overflow: auto;
    background: #0a0c10;
    min-height: 0;
  }
  .source-code {
    margin: 0;
    padding: 14px 18px;
    font-family: var(--theme-font-family-mono);
    font-size: 12px;
    line-height: 1.6;
    color: #c9d1d9;
    white-space: pre;
    font-variant-numeric: tabular-nums;
  }
  .source-code code {
    font-family: inherit;
    color: inherit;
    background: transparent;
    padding: 0;
  }
  .source-body::-webkit-scrollbar { width: 8px; height: 8px; }
  .source-body::-webkit-scrollbar-track { background: rgba(255, 255, 255, 0.02); }
  .source-body::-webkit-scrollbar-thumb { background: linear-gradient(180deg, rgba(34, 211, 238, 0.4), rgba(139, 92, 246, 0.4)); border-radius: 4px; }
  .source-body::-webkit-scrollbar-thumb:hover { background: linear-gradient(180deg, rgba(34, 211, 238, 0.6), rgba(139, 92, 246, 0.6)); }
  .source-body::-webkit-scrollbar-corner { background: rgba(255, 255, 255, 0.02); }

  .source-loading {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 12px;
    padding: 60px 0;
    color: #6b7280;
    font-size: 13px;
  }
  .source-loading .spinner {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    border: 2px solid rgba(34, 211, 238, 0.15);
    border-top-color: #22d3ee;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }

  .error-banner {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 16px;
    background: rgba(239, 68, 68, 0.08);
    border: 1px solid rgba(239, 68, 68, 0.15);
    border-radius: 10px;
    color: #ef4444;
    font-size: 13px;
    margin-bottom: 16px;
  }

  .terminal {
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 14px;
    overflow: hidden;
    background: #0a0c10;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.5), inset 0 1px 0 rgba(255, 255, 255, 0.03);
    display: flex;
    flex-direction: column;
  }

  .terminal.structured {
    background: var(--bg-card);
    border-color: var(--border-primary);
    box-shadow: var(--shadow-sm);
    overflow: visible;
  }

  .terminal.fullscreen {
    border-radius: 0;
    border: none;
    background: #000;
  }

  .terminal-bar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 16px;
    background: rgba(15, 17, 23, 0.95);
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    flex-shrink: 0;
  }

  .terminal.structured .terminal-bar {
    background: var(--bg-card);
    border-bottom-color: var(--border-primary);
  }

  .terminal.structured .terminal-dots {
    display: none;
  }

  .terminal.structured .terminal-title {
    color: var(--text-secondary);
    font-weight: 600;
  }

  .terminal.structured .tool-btn {
    color: var(--text-secondary);
  }

  .terminal.structured .tool-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .terminal-dots {
    display: flex;
    gap: 7px;
  }

  .dot {
    width: 11px;
    height: 11px;
    border-radius: 50%;
    transition: all 0.2s;
  }

  .dot:hover {
    transform: scale(1.2);
  }

  .dot-red { background: #ef4444; box-shadow: 0 0 6px rgba(239, 68, 68, 0.4); }
  .dot-yellow { background: #f59e0b; box-shadow: 0 0 6px rgba(245, 158, 11, 0.4); }
  .dot-green { background: #10b981; box-shadow: 0 0 6px rgba(16, 185, 129, 0.4); }

  .terminal-title {
    flex: 1;
    font-size: 12px;
    color: #4b5563;
    font-family: var(--theme-font-family-mono);
  }

  .terminal-status {
    font-size: 11px;
    color: #4b5563;
    padding: 3px 10px;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.03);
    font-weight: 500;
    transition: all 0.3s;
    margin-right: 8px;
  }

  .terminal-status.running {
    color: #f59e0b;
    background: rgba(245, 158, 11, 0.1);
    animation: statusPulse 1.5s ease-in-out infinite;
  }

  .terminal-status.success {
    color: #10b981;
    background: rgba(16, 185, 129, 0.1);
  }

  .terminal-status.fail {
    color: #ef4444;
    background: rgba(239, 68, 68, 0.1);
  }

  .terminal-timer {
    font-size: 11px;
    color: #fbbf24;
    font-family: var(--theme-font-family-mono);
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    padding: 3px 8px;
    border-radius: 6px;
    background: rgba(245, 158, 11, 0.08);
    margin-right: 6px;
    letter-spacing: 0.3px;
  }

  .terminal-timer-static {
    color: #94a3b8;
    background: rgba(255, 255, 255, 0.03);
  }

  .terminal-meta {
    font-size: 10px;
    color: #6b7280;
    font-family: var(--theme-font-family-mono);
    margin-right: 8px;
    padding: 2px 6px;
    border-radius: 4px;
  }

  .terminal-actions {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .tool-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border-radius: 6px;
    border: none;
    background: transparent;
    color: #6b7280;
    cursor: pointer;
    transition: all 0.2s;
  }

  .tool-btn:hover {
    background: rgba(255, 255, 255, 0.06);
    color: #c9d1d9;
  }

  .tool-btn svg {
    stroke: currentColor;
  }

  .terminal-body {
    padding: 0;
    height: 60vh;
    min-height: 400px;
    background: #0a0c10;
    position: relative;
  }

  .terminal.fullscreen .terminal-body {
    height: 100vh;
    min-height: auto;
  }

  .terminal-body :global(.xterm) {
    height: 100%;
    padding: 12px 14px;
  }

  .terminal-body :global(.xterm-viewport) {
    background: transparent !important;
  }

  .terminal-body :global(.xterm-viewport::-webkit-scrollbar) {
    width: 8px;
    height: 8px;
  }

  .terminal-body :global(.xterm-viewport::-webkit-scrollbar-track) {
    background: rgba(255, 255, 255, 0.02);
  }

  .terminal-body :global(.xterm-viewport::-webkit-scrollbar-thumb) {
    background: linear-gradient(180deg, rgba(34, 211, 238, 0.4), rgba(139, 92, 246, 0.4));
    border-radius: 4px;
  }

  .terminal-body :global(.xterm-viewport::-webkit-scrollbar-thumb:hover) {
    background: linear-gradient(180deg, rgba(34, 211, 238, 0.6), rgba(139, 92, 246, 0.6));
  }

  .terminal-body :global(.xterm-viewport::-webkit-scrollbar-corner) {
    background: rgba(255, 255, 255, 0.02);
  }

  @keyframes statusPulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.6; }
  }

  .terminal-progress {
    height: 3px;
    background: linear-gradient(90deg, transparent, #06b6d4, #3b82f6, #8b5cf6, transparent);
    background-size: 200% 100%;
    animation: shimmer 2s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  @keyframes shimmer {
    0% { background-position: -200% 0; }
    100% { background-position: 200% 0; }
  }

  :global(.terminal.fullscreen .xterm-viewport::-webkit-scrollbar) {
    width: 10px;
  }

  @media (max-width: 768px) {
    .detail-header { flex-direction: column; gap: 10px; }
    .header-actions { width: 100%; justify-content: flex-start; flex-wrap: wrap; gap: 6px; }
    .action-btn { padding: 6px 10px; font-size: 11px; }
    .info-strip { flex-wrap: wrap; gap: 10px; }
    .run-profile { grid-template-columns: repeat(2, minmax(0, 1fr)); }
    .profile-wide { grid-column: 1 / -1; }
    .params-grid { grid-template-columns: 1fr; }
    .terminal-body { min-height: 300px; height: 50vh; }
    .terminal-bar { flex-wrap: wrap; gap: 6px; }
    .terminal-timer, .terminal-meta { font-size: 9px; }
  }

  .result-view {
    background: var(--bg-primary);
    border-top: 1px solid var(--border-primary);
    display: flex;
    flex-direction: column;
    min-height: 420px;
    height: auto;
    overflow: visible;
  }

  .result-header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px 20px;
    background: var(--bg-card);
    border-bottom: 1px solid var(--border-primary);
    position: sticky;
    top: 0;
    z-index: 1;
  }

  .result-title {
    font-size: 14px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .result-badge {
    font-size: 11px;
    font-weight: 600;
    padding: 3px 10px;
    border-radius: 6px;
    border: 1px solid;
  }

  .result-chip,
  .result-duration {
    font-size: 11px;
    color: var(--text-secondary);
    font-family: var(--theme-font-family-mono);
    padding: 3px 8px;
    border-radius: 6px;
    background: var(--bg-secondary);
    border: 1px solid var(--border-secondary);
    white-space: nowrap;
  }

  .result-sections {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 16px 20px;
  }

  .result-section {
    background: var(--bg-card);
    border: 1px solid var(--border-primary);
    border-radius: 10px;
    padding: 16px;
    overflow: hidden;
  }

  .section-header {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 12px;
    padding-bottom: 10px;
    border: 0;
    border-bottom: 1px solid var(--border-primary);
    background: transparent;
    color: inherit;
    text-align: left;
  }

  .section-header[aria-expanded="false"] {
    margin-bottom: 0;
  }

  .section-icon {
    font-size: 16px;
  }

  .section-title {
    flex: 1;
    font-size: 14px;
    font-weight: 600;
    color: var(--text-primary);
    margin: 0;
  }

  .section-toggle {
    flex: 0 0 auto;
    padding: 3px 8px;
    border-radius: 999px;
    border: 1px solid rgba(34, 211, 238, .18);
    background: rgba(34, 211, 238, .07);
    color: #67e8f9;
    font-size: 11px;
    font-weight: 800;
  }

  .section-desc {
    font-size: 12px;
    color: var(--text-secondary);
    margin: 0 0 12px;
  }

  .section-desc.collapsed {
    margin: 10px 0 0;
  }

  .section-items {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .result-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 8px;
    border-radius: 6px;
    transition: background 0.15s;
    min-width: 0;
  }

  .result-item:hover {
    background: var(--bg-hover);
  }

  .item-key {
    font-size: 12px;
    color: var(--text-secondary);
    min-width: 80px;
  }

  .item-value {
    font-size: 12px;
    font-family: var(--theme-font-family-mono);
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
    word-break: break-word;
    line-height: 1.6;
  }

  .item-status {
    font-size: 14px;
    font-weight: 700;
  }

  .result-bar {
    flex-direction: column;
    align-items: stretch;
    gap: 4px;
  }

  .bar-wrap {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .bar-track {
    flex: 1;
    height: 6px;
    background: var(--bg-tertiary);
    border-radius: 3px;
    overflow: hidden;
  }

  .bar-fill {
    height: 100%;
    border-radius: 3px;
    transition: width 0.6s cubic-bezier(0.4, 0, 0.2, 1);
  }

  .bar-value {
    font-size: 12px;
    font-family: var(--theme-font-family-mono);
    font-weight: 600;
    min-width: 60px;
    text-align: right;
  }

  .result-table {
    overflow-x: auto;
  }

  .result-table table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
  }

  .result-table th {
    text-align: left;
    padding: 0;
    color: var(--text-secondary);
    font-weight: 600;
    border-bottom: 1px solid var(--border-primary);
    background: var(--bg-secondary);
  }

  .table-sort-btn {
    width: 100%;
    min-height: 32px;
    padding: 6px 10px;
    border: 0;
    background: transparent;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
    white-space: nowrap;
  }

  .table-sort-btn:hover {
    color: var(--text-primary);
    background: var(--bg-hover);
  }

  .result-table td {
    padding: 6px 10px;
    border-bottom: 1px solid var(--border-secondary);
    color: var(--text-primary);
    font-family: var(--theme-font-family-mono);
  }

  .result-table tr:hover td {
    background: var(--bg-hover);
  }

  .result-msg {
    padding: 8px 12px;
    border-radius: 8px;
    font-size: 12px;
    display: flex;
    align-items: flex-start;
    gap: 8px;
  }

  .msg-tag {
    flex: 0 0 auto;
    min-width: 38px;
    padding: 2px 6px;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.08);
    font-family: var(--theme-font-family-mono);
    font-size: 10px;
    font-weight: 700;
    text-align: center;
  }

  .msg-text {
    min-width: 0;
    overflow-wrap: anywhere;
    line-height: 1.55;
  }

  .msg-info {
    background: rgba(34, 211, 238, 0.06);
    color: #22d3ee;
  }
  .msg-pre {
    white-space: pre-wrap;
    word-break: break-all;
    font-family: var(--theme-font-family-mono);
    font-size: 12px;
    line-height: 1.5;
    margin: 0;
    max-height: 400px;
    overflow-y: auto;
    min-width: 0;
    flex: 1;
  }

  .result-sparkline {
    display: grid;
    grid-template-columns: minmax(90px, 160px) minmax(120px, 1fr) minmax(70px, auto);
    align-items: end;
  }

  .sparkline-bars {
    display: flex;
    align-items: end;
    gap: 3px;
    min-height: 72px;
    padding: 8px 10px;
    background: var(--bg-secondary);
    border: 1px solid var(--border-secondary);
    border-radius: 8px;
    overflow-x: auto;
  }

  .sparkline-bar {
    width: 7px;
    min-width: 7px;
    border-radius: 4px 4px 0 0;
    opacity: 0.78;
  }

  .sparkline-value {
    font-family: var(--theme-font-family-mono);
    font-size: 13px;
    font-weight: 700;
    text-align: right;
  }

  .msg-warn {
    background: rgba(251, 191, 36, 0.08);
    color: #fbbf24;
  }

  .msg-error {
    background: rgba(239, 68, 68, 0.08);
    color: #f87171;
  }

  .msg-success {
    background: rgba(52, 211, 153, 0.08);
    color: #34d399;
  }

  .result-divider {
    height: 1px;
    background: rgba(255, 255, 255, 0.06);
    margin: 4px 0;
  }

  .finding-card {
    padding: 12px;
    border-radius: 10px;
    border: 1px solid rgba(251, 191, 36, 0.24);
    background: rgba(251, 191, 36, 0.06);
  }

  .finding-card.finding-error {
    border-color: rgba(248, 113, 113, 0.26);
    background: rgba(248, 113, 113, 0.06);
  }

  .finding-head {
    display: flex;
    align-items: flex-start;
    gap: 10px;
  }

  .finding-level {
    flex: 0 0 auto;
    min-width: 42px;
    padding: 3px 7px;
    border-radius: 7px;
    background: rgba(255, 255, 255, 0.1);
    color: #fbbf24;
    font-family: var(--theme-font-family-mono);
    font-size: 10px;
    font-weight: 800;
    text-align: center;
  }

  .finding-error .finding-level {
    color: #f87171;
  }

  .finding-main {
    min-width: 0;
    flex: 1;
  }

  .finding-title {
    color: var(--text-primary);
    font-size: 13px;
    font-weight: 700;
    overflow-wrap: anywhere;
  }

  .finding-summary {
    margin-top: 3px;
    color: var(--text-secondary);
    font-size: 12px;
    line-height: 1.55;
    overflow-wrap: anywhere;
  }

  .finding-meta {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 9px;
  }

  .finding-meta span {
    padding: 2px 7px;
    border-radius: 999px;
    background: var(--bg-secondary);
    border: 1px solid var(--border-secondary);
    color: var(--text-secondary);
    font-size: 10px;
    font-family: var(--theme-font-family-mono);
  }

  .finding-detail {
    display: grid;
    gap: 8px;
    margin-top: 10px;
  }

  .finding-block {
    padding: 9px;
    border-radius: 8px;
    background: var(--bg-card);
    border: 1px solid var(--border-primary);
  }

  .finding-block-title {
    margin-bottom: 6px;
    color: var(--text-primary);
    font-size: 11px;
    font-weight: 700;
  }

  .finding-line {
    color: var(--text-secondary);
    font-size: 12px;
    line-height: 1.55;
    overflow-wrap: anywhere;
  }

  .finding-command {
    display: block;
    margin-top: 4px;
    padding: 6px 8px;
    border-radius: 6px;
    background: var(--bg-secondary);
    color: var(--accent-primary);
    font-family: var(--theme-font-family-mono);
    font-size: 11px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .result-raw {
    padding: 16px 20px;
  }

  .raw-content {
    font-family: var(--theme-font-family-mono);
    font-size: 12px;
    line-height: 1.6;
    color: #c9d1d9;
    white-space: pre-wrap;
    word-break: break-all;
    background: rgba(0, 0, 0, 0.3);
    padding: 14px;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.04);
  }

  .detail-page {
    max-width: 1480px;
    margin: 0 auto;
  }

  .detail-header,
  .info-strip,
  .run-profile .profile-card,
  .params-form,
  .terminal,
  .source-modal,
  .history-panel,
  .stats-panel,
  .result-view {
    background: var(--bg-card);
    border: 1px solid var(--border-primary);
    border-radius: 10px;
    box-shadow: none;
  }

  .detail-header {
    padding: 14px;
    gap: 12px;
    align-items: flex-start;
  }

  .back-btn,
  .action-btn,
  .tool-btn,
  .source-action-btn,
  .params-reset,
  .params-save {
    border-radius: 8px;
    border-color: var(--border-primary);
    background: var(--bg-card);
    color: var(--text-secondary);
    box-shadow: none;
  }

  .back-btn:hover,
  .action-btn:hover,
  .tool-btn:hover,
  .source-action-btn:hover,
  .params-reset:hover,
  .params-save:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
    transform: none;
    box-shadow: none;
  }

  .run-btn {
    background: var(--accent-primary);
    border-color: var(--accent-primary);
    color: #fff;
  }

  .run-btn:hover {
    background: var(--accent-primary-hover);
    color: #fff;
  }

  .stop-btn {
    background: rgba(220, 38, 38, 0.1);
    border-color: rgba(220, 38, 38, 0.22);
    color: var(--accent-danger);
  }

  .detail-title,
  .result-title,
  .section-title {
    color: var(--text-primary);
    letter-spacing: 0;
  }

  .detail-desc,
  .info-label,
  .section-desc,
  .item-key {
    color: var(--text-secondary);
  }

  .version-badge,
  .info-value,
  .result-duration {
    background: var(--bg-secondary);
    border: 1px solid var(--border-secondary);
    color: var(--text-primary);
    border-radius: 8px;
  }

  .params-header,
  .terminal-bar,
  .result-header,
  .section-header {
    background: transparent;
    border-bottom-color: var(--border-primary);
    color: var(--text-primary);
  }

  .params-count,
  .terminal-status,
  .result-badge {
    border-radius: 8px;
    font-family: var(--theme-font-family-mono);
    font-size: 11px;
  }

  .param-input,
  .dup-input {
    background: var(--bg-input);
    border-color: var(--border-primary);
    color: var(--text-primary);
    border-radius: 8px;
  }

  .param-input:focus,
  .dup-input:focus {
    border-color: var(--border-focus);
    box-shadow: 0 0 0 3px var(--accent-primary-light);
  }

  .terminal {
    overflow: hidden;
  }

  .terminal-bar {
    min-height: 42px;
    padding: 9px 12px;
  }

  .terminal-title,
  .terminal-meta {
    color: var(--text-secondary);
  }

  .terminal-body {
    background: #0b0f14;
  }

  .result-section {
    background: var(--bg-secondary);
    border-color: var(--border-primary);
    border-radius: 10px;
    backdrop-filter: none;
  }

  .result-item:hover,
  .result-table tr:hover td {
    background: var(--bg-hover);
  }

  .bar-track {
    background: var(--bg-tertiary);
  }

  .result-table th {
    color: var(--text-secondary);
    border-bottom-color: var(--border-primary);
  }

  .result-table td {
    color: var(--text-primary);
    border-bottom-color: var(--border-secondary);
  }

  .raw-content {
    background: var(--bg-secondary);
    border-color: var(--border-primary);
    color: var(--text-primary);
  }

  .section-icon {
    font-family: var(--theme-font-family-mono);
    font-size: 11px;
    color: var(--text-tertiary);
  }
</style>
