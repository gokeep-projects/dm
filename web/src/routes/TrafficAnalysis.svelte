<script>
  import { onMount, onDestroy } from 'svelte';

  let interfaces = $state([]);
  let loading = $state(true);
  let captureSupported = $state(false);
  let platform = $state('linux');
  let selectedInterface = $state('');
  let protocol = $state('all');
  let ip = $state('');
  let port = $state('');
  let domain = $state('');
  let path = $state('');
  let headerQuery = $state('');
  let status = $state('idle');
  let statusText = $state('未监听');
  let records = $state([]);
  let logs = $state([]);
  let selectedRecord = $state(null);
  let showAdvancedFilters = $state(false);
  let showTrafficStats = $state(false);
  let showInterfacePicker = $state(false);
  let importInput = $state(null);
  let formattedPanels = $state(new Set());
  let ws = null;
  let recordBuffer = [];
  let flushTimer = null;
  let lastFlushAt = 0;

  const protocols = [
    { value: 'all', label: '全部' },
    { value: 'http', label: 'HTTP' },
    { value: 'https', label: 'HTTPS' },
    { value: 'tcp', label: 'TCP' },
    { value: 'udp', label: 'UDP' },
  ];

  let filteredRecords = $derived.by(() => {
    const qIp = ip.trim();
    const qPort = port.trim();
    const qDomain = domain.trim().toLowerCase();
    const qPath = path.trim().toLowerCase();
    const qHeader = headerQuery.trim().toLowerCase();
    return records.filter((r) => {
      if (protocol !== 'all' && displayProtocol(r).toLowerCase() !== protocol) return false;
      if (qIp && r.src_ip !== qIp && r.dst_ip !== qIp) return false;
      if (qPort && String(r.src_port) !== qPort && String(r.dst_port) !== qPort) return false;
      const haystack = r._search || '';
      if (qDomain && !haystack.includes(qDomain)) return false;
      if (qPath && !haystack.includes(qPath)) return false;
      if (qHeader && !haystack.includes(qHeader)) return false;
      return true;
    });
  });

  let visibleRecords = $derived(filteredRecords.slice(0, 100));

  let stats = $derived.by(() => {
    const total = records.length;
    const filtered = filteredRecords.length;
    const http = records.filter(r => r.type === 'http_exchange').length;
    const https = records.filter(r => displayProtocol(r) === 'HTTPS').length;
    const errors = records.filter(r => Number(r.status || 0) >= 400).length;
    const tcp = records.filter(r => displayProtocol(r) === 'TCP').length;
    const udp = records.filter(r => displayProtocol(r) === 'UDP').length;
    const bytes = records.reduce((sum, r) => sum + recordBytes(r), 0);
    return { total, filtered, http, https, tcp, udp, errors, bytes };
  });

  let selectedInterfaceStats = $derived.by(() => interfaces.find(item => item.name === selectedInterface) || null);

  async function loadInterfaces() {
    loading = true;
    try {
      const res = await fetch('/api/traffic/interfaces?ts=' + Date.now(), { cache: 'no-store' });
      if (res.ok) {
        const data = await res.json();
        interfaces = data.interfaces || [];
        captureSupported = Boolean(data.capture_supported);
        platform = data.platform || 'linux';
        if (!selectedInterface && interfaces.length) selectedInterface = interfaces[0].name;
      }
    } catch (e) {
      pushLog('error', '加载网卡失败: ' + (e.message || e));
    } finally {
      loading = false;
    }
  }

  function selectInterface(name) {
    if (status === 'running' || status === 'connecting') return;
    selectedInterface = name;
    showInterfacePicker = false;
  }

  function interfaceKindClass(item) {
    if (item?.kind === 'loopback' || item?.name === 'lo' || item?.ip === '127.0.0.1' || item?.ip === '::1') return 'loopback';
    return item?.kind || (item?.is_public ? 'public' : item?.is_physical ? 'physical' : item?.is_virtual ? 'virtual' : 'other');
  }

  function interfaceKindLabel(item) {
    if (interfaceKindClass(item) === 'loopback') return '本地回环';
    return item?.kind_label || (item?.is_public ? '公网' : item?.is_physical ? '物理' : item?.is_virtual ? '虚拟' : '其它');
  }

  function startCapture() {
    if (status === 'running' || status === 'connecting') return;
    if (!selectedInterface) return pushLog('error', '请选择网卡');
    records = [];
    selectedRecord = null;
    recordBuffer = [];
    formattedPanels = new Set();
    const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
    ws = new WebSocket(`${proto}//${location.host}/ws/traffic`);
    status = 'connecting';
    statusText = '正在建立 WebSocket...';
    ws.onopen = () => {
      ws.send(JSON.stringify({
        action: 'start',
        interface: selectedInterface,
        protocol: protocol === 'http' || protocol === 'https' ? 'tcp' : protocol,
        ip: ip.trim(),
        port: protocol === 'https' && !port.trim() ? '443' : port.trim(),
        domain: domain.trim(),
        path: path.trim(),
        limit: 5000,
      }));
      statusText = '已连接，等待 Linux raw socket 启动';
    };
    ws.onmessage = (event) => {
      let msg;
      try { msg = JSON.parse(event.data); } catch (_) { return; }
      if (msg.type === 'started') {
        status = 'running';
        statusText = `监听中 · ${msg.interface} · ${msg.filter || 'tcp/udp'}`;
        pushLog('ok', '监听已启动');
      } else if (msg.type === 'record' || msg.type === 'packet') {
        enqueueRecord(msg.record || normalizePacket(msg.packet));
      } else if (msg.type === 'error') {
        status = 'error';
        statusText = msg.message || '监听异常';
        pushLog('error', statusText);
      } else if (msg.type === 'log') {
        pushLog('info', msg.message || '抓包事件');
      } else if (msg.type === 'stopped') {
        status = 'idle';
        statusText = msg.message || '监听已停止';
        pushLog('info', statusText);
      }
    };
    ws.onerror = () => {
      status = 'error';
      statusText = 'WebSocket 连接异常';
      pushLog('error', statusText);
    };
    ws.onclose = () => {
      if (status === 'running' || status === 'connecting') {
        status = 'idle';
        statusText = '连接已关闭';
      }
      ws = null;
    };
  }

  function stopCapture() {
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({ action: 'stop' }));
      statusText = '正在停止监听...';
      setTimeout(() => { try { ws?.close(); } catch (_) {} }, 250);
    } else if (ws) {
      try { ws.close(); } catch (_) {}
    }
    status = 'idle';
    statusText = '监听已停止';
    flushRecords(true);
  }

  function clearRecords() {
    records = [];
    logs = [];
    recordBuffer = [];
    selectedRecord = null;
    formattedPanels = new Set();
  }

  function pushLog(type, message) {
    logs = [{ type, message, time: new Date().toLocaleTimeString('zh-CN') }, ...logs].slice(0, 6);
  }

  function enrichRecord(record) {
    const searchText = [
      record.host, record.path, record.method, getRequestMethod(record), getRequestPath(record), getHost(record), record.status, record.status_text, record.summary, record.decoded,
      headersText(record.request?.headers), headersText(record.response?.headers),
      record.request?.body_preview, record.response?.body_preview,
    ].filter(Boolean).join(' ').toLowerCase();
    return { ...record, _search: searchText };
  }

  function enqueueRecord(record) {
    if (!record) return;
    recordBuffer.unshift(enrichRecord(record));
    const now = performance.now();
    if (recordBuffer.length >= 200 || now - lastFlushAt > 1000) {
      flushRecords();
      return;
    }
    if (!flushTimer) flushTimer = setTimeout(() => flushRecords(), 1000);
  }

  function flushRecords(force = false) {
    if (flushTimer) {
      clearTimeout(flushTimer);
      flushTimer = null;
    }
    if (!recordBuffer.length && !force) return;
    const next = recordBuffer.splice(0, recordBuffer.length);
    if (next.length) {
      const merged = new Map(records.map(item => [recordIdentity(item), item]));
      for (const item of next.reverse()) merged.set(recordIdentity(item), item);
      records = [...merged.values()].sort((a, b) => Number(b.seq || 0) - Number(a.seq || 0)).slice(0, 5000);
    }
    lastFlushAt = performance.now();
  }

  function recordIdentity(record) {
    return record.flow_id || record.seq || `${record.src_ip}:${record.src_port}-${record.dst_ip}:${record.dst_port}-${record.timestamp}`;
  }

  function normalizePacket(packet) {
    const isTls = packet?.http?.kind === 'tls';
    const httpsCandidate = String(packet?.src_port || '') === '443' || String(packet?.dst_port || '') === '443' || protocol === 'https';
    return {
      ...packet,
      type: packet?.http?.method ? 'http_exchange' : 'raw_flow',
      protocol: packet?.http?.method ? (httpsCandidate ? 'HTTPS' : 'HTTP') : isTls ? 'HTTPS' : packet?.protocol,
      transport_protocol: packet?.protocol,
      https_plaintext: Boolean(packet?.http?.method && httpsCandidate),
      method: packet?.http?.method || '',
      host: packet?.http?.host || packet?.http?.headers?.sni || '',
      path: packet?.http?.path || '',
      request: { headers: packet?.http?.headers || {}, body_preview: packet?.raw || '', meta: packet },
      response: {},
    };
  }

  function headersText(headers) {
    return Object.entries(headers || {}).map(([k, v]) => `${k}: ${v}`).join('\n');
  }

  function headerEntries(headers) {
    return Object.entries(headers || {});
  }

  function displayProtocol(record) {
    if (record?.protocol) return String(record.protocol);
    if (record?.type === 'http_exchange' || record?.type === 'http_response') return 'HTTP';
    return record?.transport_protocol || '-';
  }

  function getRequestMethod(record) {
    return record?.method || record?.request?.method || '';
  }

  function getRequestPath(record) {
    return record?.path || record?.request?.path || '';
  }

  function getHost(record) {
    return record?.host || record?.request?.host || record?.request?.headers?.host || '';
  }

  function requestLine(record) {
    if (displayProtocol(record) === 'HTTPS') return getRequestMethod(record) || 'TLS';
    if (displayProtocol(record) !== 'HTTP') return record.type === 'raw_flow' ? '流量' : '-';
    return [getRequestMethod(record), getRequestPath(record)].filter(Boolean).join(' ') || '-';
  }

  function responseLine(record) {
    if (record.status) return `${record.status} ${record.status_text || ''}`.trim();
    if (displayProtocol(record) === 'UDP') return '数据报';
    if (displayProtocol(record) === 'TCP') return '连接/载荷';
    return '-';
  }

  function endpoint(r, side) {
    return side === 'src' ? `${r.src_ip || '-'}:${r.src_port || '-'}` : `${r.dst_ip || '-'}:${r.dst_port || '-'}`;
  }

  function statusClass(record) {
    const code = Number(record.status || 0);
    if (code >= 500) return 'bad';
    if (code >= 400) return 'warn';
    if (code >= 200 && code < 400) return 'ok';
    return '';
  }

  function formatBytes(value) {
    const n = Number(value || 0);
    if (n > 1024 * 1024 * 1024) return (n / 1024 / 1024 / 1024).toFixed(1) + ' GB';
    if (n > 1024 * 1024) return (n / 1024 / 1024).toFixed(1) + ' MB';
    if (n > 1024) return (n / 1024).toFixed(1) + ' KB';
    return n + ' B';
  }

  function metaEntries(record) {
    const meta = {
      序号: record.seq,
      协议类型: displayProtocol(record),
      传输协议: record.transport_protocol || (displayProtocol(record) === 'HTTP' ? 'TCP' : displayProtocol(record)),
      记录类型: record.type || '-',
      请求时间: record.request_time || record.timestamp || '-',
      响应时间: record.response_time || '-',
      源地址: endpoint(record, 'src'),
      目的地址: endpoint(record, 'dst'),
      Host: getHost(record) || '-',
      路径: getRequestPath(record) || '-',
      方法: getRequestMethod(record) || '-',
      状态: responseLine(record),
      请求字节: record.request?.payload_bytes ?? '-',
      响应字节: record.response?.payload_bytes ?? '-',
      流量大小: formatBytes(recordBytes(record)),
    };
    return Object.entries(meta);
  }

  function recordBytes(record) {
    return Number(record?.total_bytes || 0)
      || Number(record?.request?.payload_bytes || 0) + Number(record?.response?.payload_bytes || 0)
      || Number(record?.length || 0);
  }

  function decodedBody(record, side) {
    const part = side === 'request' ? record.request : record.response;
    return String(part?.body_preview || part?.raw_preview || part?.raw || '').trim();
  }

  function contentType(record, side) {
    const part = side === 'request' ? record.request : record.response;
    const headers = part?.headers || {};
    const key = Object.keys(headers).find(k => k.toLowerCase() === 'content-type');
    return key ? String(headers[key] || '').toLowerCase() : '';
  }

  function decodedHeaders(record, side) {
    const part = side === 'request' ? record.request : record.response;
    return headersText(part?.headers);
  }

  function panelText(record, side, pretty = true) {
    const headers = decodedHeaders(record, side);
    const body = decodedBody(record, side);
    const chunks = [];
    if (headers) chunks.push(`Headers\n${headers}`);
    if (body) chunks.push(`Body\n${pretty ? formatDecoded(body) : body}`);
    return chunks.join('\n\n') || '无可解码内容';
  }

  function rawVisibleText(record) {
    return String(
      record?.request?.body_preview
      || record?.request?.raw_preview
      || record?.response?.body_preview
      || record?.response?.raw_preview
      || record?.decoded
      || record?.summary
      || ''
    ).trim();
  }

  function formatDecoded(value) {
    const text = String(value || '').trim();
    if (!text) return '';
    return formatJsonSmart(text) || formatXmlSmart(text) || text;
  }

  function smartBodyText(record, side) {
    const body = decodedBody(record, side);
    if (!body) return side === 'request' ? '无请求体' : '无响应体';
    const type = contentType(record, side);
    if (isPanelFormatted(record, side) || /json|xml|html|text\/xml|application\/.*\+xml/.test(type)) {
      return formatDecoded(body);
    }
    return formatDecoded(body) || body;
  }

  function smartBodyKind(record, side) {
    const type = contentType(record, side);
    const body = decodedBody(record, side);
    if (/json/.test(type) || formatJsonSmart(body)) return 'json';
    if (/xml|application\/.*\+xml/.test(type) || looksLikeXml(body)) return 'xml';
    return 'text';
  }

  function formatJsonSmart(text) {
    const candidates = jsonCandidates(text);
    for (const candidate of candidates) {
      const repaired = repairJson(candidate);
      try {
        return JSON.stringify(JSON.parse(repaired), null, 2);
      } catch (_) {}
    }
    return '';
  }

  function jsonCandidates(text) {
    const trimmed = String(text || '').trim();
    const candidates = [trimmed];
    for (const [start, end] of [['{', '}'], ['[', ']']]) {
      const first = trimmed.indexOf(start);
      const last = trimmed.lastIndexOf(end);
      if (first >= 0 && last > first) candidates.push(trimmed.slice(first, last + 1));
    }
    return [...new Set(candidates)].filter(Boolean);
  }

  function repairJson(text) {
    let next = String(text || '').trim();
    next = next.replace(/,\s*([}\]])/g, '$1');
    next = next.replace(/([{,]\s*)([A-Za-z_$][\w$-]*)(\s*:)/g, '$1"$2"$3');
    const openBraces = (next.match(/{/g) || []).length - (next.match(/}/g) || []).length;
    const openArrays = (next.match(/\[/g) || []).length - (next.match(/]/g) || []).length;
    if (openArrays > 0) next += ']'.repeat(openArrays);
    if (openBraces > 0) next += '}'.repeat(openBraces);
    return next;
  }

  function looksLikeXml(text) {
    return /^\s*<[\w:.-]+[\s>]/.test(String(text || ''));
  }

  function formatXmlSmart(text) {
    const source = String(text || '').trim();
    if (!looksLikeXml(source)) return '';
    const formatted = prettyXmlLoose(source);
    return formatted || source;
  }

  function prettyXmlLoose(text) {
    const compact = text
      .replace(/>\s+</g, '><')
      .replace(/</g, '\n<')
      .replace(/>/g, '>\n')
      .split('\n')
      .map(line => line.trim())
      .filter(Boolean);
    let indent = 0;
    return compact.map(line => {
      if (/^<\//.test(line)) indent = Math.max(0, indent - 1);
      const out = '  '.repeat(indent) + line;
      if (/^<[^!?/][^>]*[^/]>\s*$/.test(line) && !/^<[^>]+>[^<]+<\/[^>]+>$/.test(line)) indent += 1;
      return out;
    }).join('\n');
  }

  function isPanelFormatted(record, side) {
    return formattedPanels.has(`${recordIdentity(record)}:${side}`);
  }

  function markFormatted(record, side) {
    const next = new Set(formattedPanels);
    next.add(`${recordIdentity(record)}:${side}`);
    formattedPanels = next;
    pushLog('ok', `${side === 'request' ? '请求' : '响应'}内容已格式化`);
  }

  function metaHighlights(record) {
    return [
      { label: '协议', value: displayProtocol(record), tone: displayProtocol(record).toLowerCase() },
      { label: '方法', value: getRequestMethod(record) || '-', tone: 'method' },
      { label: '明文', value: record?.https_plaintext ? '已解析' : (displayProtocol(record) === 'HTTPS' ? 'TLS密文' : '原始明文'), tone: record?.https_plaintext ? 'ok' : 'neutral' },
      { label: '状态', value: responseLine(record), tone: statusClass(record) || 'neutral' },
      { label: '大小', value: formatBytes(recordBytes(record)), tone: 'size' },
      { label: 'Host', value: getHost(record) || '-', tone: 'host' },
    ];
  }

  async function copyPanel(record, side) {
    const text = panelText(record, side, isPanelFormatted(record, side));
    try {
      await navigator.clipboard.writeText(text);
      pushLog('ok', `${side === 'request' ? '请求' : '响应'}内容已复制`);
    } catch (_) {
      pushLog('error', '复制失败，浏览器未授权剪贴板');
    }
  }

  function exportRecords() {
    const payload = {
      format: 'dm-traffic-capture-v1',
      exported_at: new Date().toISOString(),
      interface: selectedInterface,
      filters: { protocol, ip, port, domain, path, headerQuery },
      stats,
      records,
    };
    const blob = new Blob([JSON.stringify(payload, null, 2)], { type: 'application/json;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `dm-traffic-${selectedInterface || 'capture'}-${Date.now()}.json`;
    document.body.appendChild(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
  }

  function exportPcapRecords() {
    const exportFrames = pcapExportFrames(records);
    const bytes = buildPcap(exportFrames);
    const blob = new Blob([bytes], { type: 'application/vnd.tcpdump.pcap' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `dm-traffic-${selectedInterface || 'capture'}-${Date.now()}.pcap`;
    document.body.appendChild(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
    pushLog('ok', `已导出原始 PCAP ${exportFrames.raw} 帧，回退重构 ${exportFrames.synthetic} 帧`);
  }

  function pcapExportFrames(items) {
    const frames = [];
    let raw = 0;
    let synthetic = 0;
    items.forEach((record, index) => {
      const rawFrames = recordPcapFrames(record);
      if (rawFrames.length) {
        raw += rawFrames.length;
        frames.push(...rawFrames);
        return;
      }
      const fallback = syntheticFrame(record, index);
      if (fallback) {
        synthetic += 1;
        frames.push({ bytes: fallback, originalLength: fallback.length });
      }
    });
    return { frames, raw, synthetic };
  }

  function recordPcapFrames(record) {
    const entries = Array.isArray(record?.pcap_frames) ? record.pcap_frames : [];
    const metaEntries = [
      record?.request?.meta,
      record?.response?.meta,
    ].filter(Boolean).map(meta => ({
      frame_base64: meta.pcap_frame_base64,
      original_length: meta.pcap_original_length,
      link_type: meta.pcap_link_type,
    }));
    return [...entries, ...metaEntries]
      .map(entry => {
        const bytes = base64ToBytes(entry?.frame_base64);
        return bytes ? { bytes, originalLength: Number(entry?.original_length || bytes.length), linkType: Number(entry?.link_type || 1) } : null;
      })
      .filter(Boolean);
  }

  function base64ToBytes(value) {
    if (!value) return null;
    try {
      const binary = atob(String(value));
      const out = new Uint8Array(binary.length);
      for (let i = 0; i < binary.length; i += 1) out[i] = binary.charCodeAt(i);
      return out;
    } catch (_) {
      return null;
    }
  }

  function buildPcap(exportFrames) {
    const frames = exportFrames.frames;
    const total = 24 + frames.reduce((sum, frame) => sum + 16 + frame.bytes.length, 0);
    const out = new Uint8Array(total);
    const view = new DataView(out.buffer);
    let off = 0;
    view.setUint32(off, 0xa1b2c3d4, true); off += 4;
    view.setUint16(off, 2, true); off += 2;
    view.setUint16(off, 4, true); off += 2;
    view.setInt32(off, 0, true); off += 4;
    view.setUint32(off, 0, true); off += 4;
    view.setUint32(off, 65535, true); off += 4;
    view.setUint32(off, 1, true); off += 4;
    const nowSec = Math.floor(Date.now() / 1000);
    frames.forEach((frame, i) => {
      view.setUint32(off, nowSec + i, true); off += 4;
      view.setUint32(off, 0, true); off += 4;
      view.setUint32(off, frame.bytes.length, true); off += 4;
      view.setUint32(off, frame.originalLength || frame.bytes.length, true); off += 4;
      out.set(frame.bytes, off); off += frame.bytes.length;
    });
    return out;
  }

  function syntheticFrame(record, index) {
    const src = parseIp(record.src_ip) || [10, 0, 0, 1];
    const dst = parseIp(record.dst_ip) || [10, 0, 0, 2];
    const srcPort = clampPort(record.src_port, 40000 + (index % 10000));
    const dstPort = clampPort(record.dst_port, Number(port) || 80);
    const payloadText = [
      decodedBody(record, 'request'),
      decodedBody(record, 'response'),
      rawVisibleText(record),
      record.summary || '',
    ].find(v => String(v || '').trim()) || `${displayProtocol(record)} ${endpoint(record, 'src')} -> ${endpoint(record, 'dst')}`;
    const payload = new TextEncoder().encode(payloadText);
    const isUdp = displayProtocol(record).toUpperCase() === 'UDP';
    const transportLen = isUdp ? 8 : 20;
    const frame = new Uint8Array(14 + 20 + transportLen + payload.length);
    let off = 0;
    frame.set([0x02,0x00,0x00,0x00,0x00,0x02, 0x02,0x00,0x00,0x00,0x00,0x01, 0x08,0x00], off); off += 14;
    frame[off++] = 0x45; frame[off++] = 0;
    write16(frame, off, 20 + transportLen + payload.length); off += 2;
    write16(frame, off, index & 0xffff); off += 2;
    write16(frame, off, 0x4000); off += 2;
    frame[off++] = 64; frame[off++] = isUdp ? 17 : 6;
    write16(frame, off, 0); off += 2;
    frame.set(src, off); off += 4;
    frame.set(dst, off); off += 4;
    write16(frame, off, srcPort); off += 2;
    write16(frame, off, dstPort); off += 2;
    if (isUdp) {
      write16(frame, off, 8 + payload.length); off += 2;
      write16(frame, off, 0); off += 2;
    } else {
      write32(frame, off, index + 1); off += 4;
      write32(frame, off, 0); off += 4;
      frame[off++] = 0x50; frame[off++] = 0x18;
      write16(frame, off, 65535); off += 2;
      write16(frame, off, 0); off += 2;
      write16(frame, off, 0); off += 2;
    }
    frame.set(payload, off);
    return frame;
  }

  function parseIp(value) {
    const parts = String(value || '').split('.').map(v => Number(v));
    return parts.length === 4 && parts.every(v => Number.isInteger(v) && v >= 0 && v <= 255) ? parts : null;
  }

  function clampPort(value, fallback) {
    const n = Number(value);
    return Number.isInteger(n) && n > 0 && n <= 65535 ? n : fallback;
  }

  function write16(buf, offset, value) {
    buf[offset] = (value >> 8) & 0xff;
    buf[offset + 1] = value & 0xff;
  }

  function write32(buf, offset, value) {
    buf[offset] = (value >>> 24) & 0xff;
    buf[offset + 1] = (value >>> 16) & 0xff;
    buf[offset + 2] = (value >>> 8) & 0xff;
    buf[offset + 3] = value & 0xff;
  }

  async function importCaptureFile(event) {
    const file = event.currentTarget.files?.[0];
    if (!file) return;
    try {
      const buffer = await file.arrayBuffer();
      const imported = file.name.toLowerCase().endsWith('.json')
        ? importJsonCapture(new TextDecoder().decode(buffer))
        : importPcapCapture(buffer);
      records = [...imported.map(enrichRecord), ...records].slice(0, 5000);
      pushLog('ok', `已导入 ${imported.length} 条记录: ${file.name}`);
    } catch (e) {
      pushLog('error', '导入失败: ' + (e.message || e));
    } finally {
      event.currentTarget.value = '';
    }
  }

  function importJsonCapture(text) {
    const data = JSON.parse(text);
    if (Array.isArray(data)) return data;
    if (Array.isArray(data.records)) return data.records;
    throw new Error('JSON 抓包文件必须包含 records 数组');
  }

  function importPcapCapture(buffer) {
    const view = new DataView(buffer);
    if (view.byteLength < 24) throw new Error('PCAP 文件过小');
    const magic = view.getUint32(0, false);
    const little = magic === 0xd4c3b2a1 || magic === 0x4d3cb2a1;
    const normal = magic === 0xa1b2c3d4 || magic === 0xa1b23c4d || little;
    if (!normal) throw new Error('当前仅支持标准 PCAP，暂不支持 PCAPNG');
    const get32 = (offset) => view.getUint32(offset, little);
    let offset = 24;
    let seq = 0;
    const out = [];
    while (offset + 16 <= view.byteLength && out.length < 5000) {
      const tsSec = get32(offset);
      const tsUsec = get32(offset + 4);
      const inclLen = get32(offset + 8);
      offset += 16;
      if (offset + inclLen > view.byteLength) break;
      const frame = new Uint8Array(buffer, offset, inclLen);
      const record = decodePcapFrame(frame, ++seq, new Date(tsSec * 1000 + Math.floor(tsUsec / 1000)).toLocaleTimeString('zh-CN'));
      if (record) out.push(record);
      offset += inclLen;
    }
    return out;
  }

  function decodePcapFrame(frame, seq, timestamp) {
    let ipOffset = 14;
    if (frame.length < ipOffset + 20 || frame[ipOffset] >> 4 !== 4) return null;
    const ihl = (frame[ipOffset] & 0x0f) * 4;
    const protoNum = frame[ipOffset + 9];
    const srcIp = `${frame[ipOffset + 12]}.${frame[ipOffset + 13]}.${frame[ipOffset + 14]}.${frame[ipOffset + 15]}`;
    const dstIp = `${frame[ipOffset + 16]}.${frame[ipOffset + 17]}.${frame[ipOffset + 18]}.${frame[ipOffset + 19]}`;
    const transport = ipOffset + ihl;
    if (protoNum !== 6 && protoNum !== 17) return null;
    const srcPort = (frame[transport] << 8) | frame[transport + 1];
    const dstPort = (frame[transport + 2] << 8) | frame[transport + 3];
    const headerLen = protoNum === 6 ? ((frame[transport + 12] >> 4) * 4) : 8;
    const payload = frame.slice(transport + headerLen);
    const raw = payloadPreviewFromBytes(payload);
    const http = decodeHttpText(raw);
    const protocolName = protoNum === 6 ? 'TCP' : 'UDP';
    const httpsCandidate = srcPort === 443 || dstPort === 443 || protocol === 'https';
    const displayName = http.kind === 'request' || http.kind === 'response'
      ? (httpsCandidate ? 'HTTPS' : 'HTTP')
      : http.kind === 'tls' ? 'HTTPS' : protocolName;
    const base = {
      seq,
      timestamp,
      request_time: timestamp,
      response_time: '',
      protocol: displayName,
      transport_protocol: protocolName,
      src_ip: srcIp,
      src_port: String(srcPort),
      dst_ip: dstIp,
      dst_port: String(dstPort),
      total_bytes: payload.length,
      request: { headers: http.kind === 'tls' ? http.headers : {}, body_preview: raw, payload_bytes: payload.length, raw_preview: raw },
      response: {},
      summary: `${protocolName} ${srcIp}:${srcPort} -> ${dstIp}:${dstPort}`,
      decoded: raw,
    };
    if (http.kind === 'request') {
      return {
        ...base,
        type: 'http_exchange',
        method: http.method,
        path: http.path,
        host: http.host,
        https_plaintext: httpsCandidate,
        decoded: httpsCandidate ? `HTTPS plaintext payload decoded from raw TCP stream\n${raw}` : raw,
        request: { ...base.request, ...http },
      };
    }
    if (http.kind === 'response') {
      return {
        ...base,
        type: 'http_response',
        status: http.status,
        status_text: http.status_text,
        https_plaintext: httpsCandidate,
        decoded: httpsCandidate ? `HTTPS plaintext response decoded from raw TCP stream\n${raw}` : raw,
        request: {},
        response: { ...http, payload_bytes: payload.length, body_preview: http.body_preview },
      };
    }
    return { ...base, type: 'raw_flow', path: '', method: http.kind === 'tls' ? 'TLS' : '', host: http.kind === 'tls' ? http.host : '', status: '', status_text: '' };
  }

  function payloadPreviewFromBytes(bytes) {
    if (!bytes?.length) return '<empty payload>';
    const tls = tlsSummary(bytes);
    if (tls) return tls;
    const slice = bytes.slice(0, 4096);
    const utf8 = decodeText(slice, 'utf-8', true) || decodeText(slice, 'gb18030', true);
    if (utf8 && readableRatio(utf8) >= 0.75) return appendTruncation(cleanText(utf8), bytes.length, slice.length);
    const loose = decodeText(slice, 'utf-8', false);
    if (loose && readableRatio(loose) >= 0.85 && (loose.match(/\uFFFD/g) || []).length <= 2) {
      return appendTruncation(cleanText(loose), bytes.length, slice.length);
    }
    const printable = printablePayloadText(slice, bytes.length);
    if (printable) return appendTruncation(printable, bytes.length, slice.length);
    return `<binary payload: ${bytes.length} bytes, not safe text>\nHEX ${hexPreview(slice, 96)}\nASCII ${asciiPreview(slice, 96)}`;
  }

  function printablePayloadText(bytes, originalLength = bytes?.length || 0) {
    if (!bytes?.length) return '';
    let out = '';
    let printable = 0;
    let lastSpace = false;
    for (const byte of bytes) {
      if (byte === 10 || byte === 13 || byte === 9 || (byte >= 32 && byte <= 126)) {
        out += String.fromCharCode(byte);
        printable += 1;
        lastSpace = byte === 32;
      } else if (!lastSpace) {
        out += ' ';
        lastSpace = true;
      }
    }
    const cleaned = out.split(/\r?\n/).map((line) => line.trim()).filter(Boolean).join('\n');
    if (printable < 8 || cleaned.length < 8) return '';
    const ratio = printable / Math.max(1, bytes.length);
    if (ratio < 0.18 && cleaned.split(/\s+/).filter(Boolean).length < 2) return '';
    return cleanText(cleaned);
  }

  function decodeText(bytes, encoding, fatal) {
    try {
      return new TextDecoder(encoding, { fatal }).decode(bytes);
    } catch (_) {
      return '';
    }
  }

  function cleanText(text) {
    return String(text || '').replace(/[\u0000-\u0008\u000B\u000C\u000E-\u001F]/g, ' ').trim();
  }

  function readableRatio(text) {
    const chars = [...String(text || '')];
    if (!chars.length) return 0;
    const readable = chars.filter(c => c === '\n' || c === '\r' || c === '\t' || (c >= ' ' && c !== '\uFFFD')).length;
    return readable / chars.length;
  }

  function appendTruncation(text, originalLength, shownLength) {
    return originalLength > shownLength ? `${text}\n<truncated ${originalLength - shownLength} bytes>` : text;
  }

  function hexPreview(bytes, limit) {
    return [...bytes.slice(0, limit)].map(b => b.toString(16).padStart(2, '0')).join(' ');
  }

  function asciiPreview(bytes, limit) {
    return [...bytes.slice(0, limit)].map(b => (b >= 32 && b <= 126) || b === 10 || b === 13 || b === 9 ? String.fromCharCode(b) : '.').join('');
  }

  function tlsSummary(bytes) {
    if (!isTlsRecord(bytes)) return '';
    const sni = tlsClientHelloSni(bytes);
    const lines = [
      sni
        ? `HTTPS/TLS encrypted payload. SNI: ${sni}.`
        : 'HTTPS/TLS encrypted payload.',
      '当前包是 TLS 密文，网络原包内没有 HTTP 明文请求方法/正文。',
      '要还原 HTTP 原始明文，必须提供 TLS session keys，或让流量经过受信任解密代理后再抓取。',
      `TLS 原始字节 HEX ${hexPreview(bytes, 128)}`,
      `TLS 原始字节 ASCII ${asciiPreview(bytes, 128)}`,
    ];
    return lines.join('\n');
  }

  function isTlsRecord(bytes) {
    return bytes.length >= 5 && [0x14, 0x15, 0x16, 0x17].includes(bytes[0]) && bytes[1] === 0x03 && bytes[2] <= 0x04;
  }

  function tlsClientHelloSni(bytes) {
    if (bytes.length < 5 || bytes[0] !== 0x16 || bytes[5] !== 0x01) return '';
    let offset = 9;
    if (bytes.length < offset + 34) return '';
    offset += 34;
    const sessionLen = bytes[offset] || 0;
    offset += 1 + sessionLen;
    if (bytes.length < offset + 2) return '';
    const cipherLen = (bytes[offset] << 8) | bytes[offset + 1];
    offset += 2 + cipherLen;
    if (bytes.length < offset + 1) return '';
    const compLen = bytes[offset] || 0;
    offset += 1 + compLen;
    if (bytes.length < offset + 2) return '';
    const extLen = (bytes[offset] << 8) | bytes[offset + 1];
    offset += 2;
    const end = Math.min(bytes.length, offset + extLen);
    while (offset + 4 <= end) {
      const type = (bytes[offset] << 8) | bytes[offset + 1];
      const len = (bytes[offset + 2] << 8) | bytes[offset + 3];
      offset += 4;
      if (offset + len > end) return '';
      if (type === 0) return parseSniExtension(bytes.slice(offset, offset + len));
      offset += len;
    }
    return '';
  }

  function parseSniExtension(ext) {
    if (ext.length < 5) return '';
    const listLen = (ext[0] << 8) | ext[1];
    let offset = 2;
    const end = Math.min(ext.length, offset + listLen);
    while (offset + 3 <= end) {
      const nameType = ext[offset];
      const nameLen = (ext[offset + 1] << 8) | ext[offset + 2];
      offset += 3;
      if (offset + nameLen > end) return '';
      if (nameType === 0) return decodeText(ext.slice(offset, offset + nameLen), 'utf-8', false);
      offset += nameLen;
    }
    return '';
  }

  function decodeHttpText(text) {
    const normalized = String(text || '').replace(/\r\n/g, '\n');
    if (normalized.startsWith('HTTPS/TLS encrypted payload')) {
      const sni = normalized.match(/SNI:\s*([^\s.]+(?:\.[^\s.]+)+)/)?.[1] || '';
      return { kind: 'tls', method: 'TLS', path: '', host: sni, headers: { tls: 'encrypted', sni }, body_preview: normalized };
    }
    const [head, body = ''] = normalized.split(/\n\n/);
    const lines = (head || '').split('\n');
    const first = (lines.shift() || '').trim();
    const headers = {};
    for (const line of lines) {
      const idx = line.indexOf(':');
      if (idx > 0) headers[line.slice(0, idx).trim().toLowerCase()] = line.slice(idx + 1).trim();
    }
    const methods = ['GET', 'POST', 'PUT', 'DELETE', 'PATCH', 'HEAD', 'OPTIONS', 'CONNECT', 'TRACE'];
    for (const method of methods) {
      if (first.startsWith(method + ' ')) {
        const bits = first.split(/\s+/);
        return { kind: 'request', method, path: bits[1] || '', version: bits[2] || '', host: headers.host || '', headers, body_preview: body.trim() };
      }
    }
    if (first.startsWith('HTTP/')) {
      const bits = first.split(/\s+/, 3);
      return { kind: 'response', version: bits[0] || '', status: Number(bits[1] || 0), status_text: bits[2] || '', headers, body_preview: body.trim() };
    }
    return { kind: 'unknown' };
  }

  onMount(loadInterfaces);
  onDestroy(() => {
    if (flushTimer) clearTimeout(flushTimer);
    try { ws?.close(); } catch (_) {}
  });
</script>

<div class="traffic-page compact">
  <section class="traffic-toolbar">
    <div class="toolbar-title">
      <h2>流量分析</h2>
      <span class="status-pill" class:run={status === 'running'} class:error={status === 'error'}>{statusText}</span>
    </div>

    <div class="filters primary-filters">
      <div class="iface-picker-wrap">
        <span class="filter-label">网卡</span>
        <button
          class="iface-trigger"
          class:open={showInterfacePicker}
          onclick={() => status !== 'running' && status !== 'connecting' && (showInterfacePicker = !showInterfacePicker)}
          disabled={status === 'running' || status === 'connecting'}
          type="button"
        >
          {#if selectedInterfaceStats}
            <span class="iface-kind {interfaceKindClass(selectedInterfaceStats)}">{interfaceKindLabel(selectedInterfaceStats)}</span>
            <strong>{selectedInterfaceStats.name}</strong>
            <em>{selectedInterfaceStats.ip || '无 IP'}</em>
          {:else}
            <strong>{loading ? '扫描网卡...' : '请选择网卡'}</strong>
          {/if}
        </button>
        {#if showInterfacePicker && status !== 'running' && status !== 'connecting'}
          <div class="iface-menu">
            {#each interfaces as item}
              <button class="iface-option" class:active={item.name === selectedInterface} onclick={() => selectInterface(item.name)} type="button">
                <span class="iface-kind {interfaceKindClass(item)}">{interfaceKindLabel(item)}</span>
                <strong>{item.name}</strong>
                <em>{item.ip || '无 IP'}</em>
                <small>{formatBytes(item.received_bytes)} ↓ / {formatBytes(item.transmitted_bytes)} ↑</small>
              </button>
            {/each}
          </div>
        {/if}
      </div>
      <label><span>协议</span><select bind:value={protocol}>{#each protocols as p}<option value={p.value}>{p.label}</option>{/each}</select></label>
      <label><span>端口</span><input bind:value={port} placeholder="80/443" inputmode="numeric" /></label>
      <button class="filter-fold" onclick={() => showAdvancedFilters = !showAdvancedFilters}>
        筛选条件 {showAdvancedFilters ? '收起' : '展开'}
      </button>
    </div>
    {#if showAdvancedFilters}
      <div class="filters advanced-filters">
        <label><span>IP</span><input bind:value={ip} placeholder="源/目的 IP" /></label>
        <label><span>域名</span><input bind:value={domain} placeholder="Host / SNI" /></label>
        <label><span>路径</span><input bind:value={path} placeholder="/api" /></label>
        <label><span>头/正文</span><input bind:value={headerQuery} placeholder="header/body" /></label>
      </div>
    {/if}

    <div class="actions">
      <button class="primary" onclick={startCapture} disabled={!captureSupported || status === 'running' || status === 'connecting'}>监听</button>
      <button class="danger" onclick={stopCapture} disabled={status !== 'running' && status !== 'connecting'}>停止</button>
      <button class="secondary" onclick={clearRecords}>清空</button>
      <button class="secondary" onclick={() => importInput?.click()}>导入抓包</button>
      <button class="secondary" onclick={exportRecords} disabled={records.length === 0}>导出抓包</button>
      <button class="secondary" onclick={exportPcapRecords} disabled={records.length === 0}>导出 PCAP</button>
      <button class="secondary" onclick={() => showTrafficStats = !showTrafficStats}>统计 {showTrafficStats ? '收起' : '展开'}</button>
      <input bind:this={importInput} class="hidden-file" type="file" accept=".json,.pcap,.cap,application/json,application/vnd.tcpdump.pcap" onchange={importCaptureFile} />
      <div class="metrics">
        <span>保留 {stats.total}</span>
        <span>匹配 {stats.filtered}</span>
        <span>HTTP {stats.http}</span>
        <span>HTTPS {stats.https}</span>
        <span>TCP {stats.tcp}</span>
        <span>UDP {stats.udp}</span>
        <span>流量 {formatBytes(stats.bytes)}</span>
        <span>错误 {stats.errors}</span>
        <span>显示 {visibleRecords.length}/100</span>
      </div>
    </div>

    {#if !captureSupported}
      <div class="unsupported">当前平台 {platform} 不支持抓包。本模块仅支持 Linux raw socket。</div>
    {/if}
    {#if protocol === 'https'}
      <div class="unsupported https-note">
        HTTPS 原始网络包通常是 TLS 密文；只有提供 TLS session keys，或流量已经过受信任解密代理转为明文时，才能显示 HTTP 请求方法、路径和正文。普通 TLS 密文会展示 SNI 与原始字节 HEX/ASCII。
      </div>
    {/if}
    {#if showTrafficStats}
      <div class="traffic-stats-panel">
        <div class="stat-section">
          <h3>事件</h3>
          {#if logs.length === 0}
            <p class="muted">等待事件...</p>
          {:else}
            {#each logs as log}
              <div class="log-line {log.type}"><span>{log.time}</span>{log.message}</div>
            {/each}
          {/if}
        </div>
        <div class="stat-section">
          <h3>当前网卡</h3>
          <div class="iface-grid single">
            {#if selectedInterfaceStats}
              <div class="iface-row selected">
                <strong>{selectedInterfaceStats.name}</strong>
                <span>{formatBytes(selectedInterfaceStats.received_bytes)} ↓ / {formatBytes(selectedInterfaceStats.transmitted_bytes)} ↑</span>
              </div>
            {:else}
              <p class="muted">未选择网卡</p>
            {/if}
          </div>
          <div class="protocol-grid">
            <span>HTTP <strong>{stats.http}</strong></span>
            <span>HTTPS <strong>{stats.https}</strong></span>
            <span>TCP <strong>{stats.tcp}</strong></span>
            <span>UDP <strong>{stats.udp}</strong></span>
          </div>
        </div>
      </div>
    {/if}
  </section>

  <section class="traffic-body">
    <div class="traffic-table-wrap table-scroll">
      <table class="traffic-table">
        <thead>
          <tr>
            <th class="seq">#</th>
            <th>时间</th>
            <th>协议</th>
            <th>源</th>
            <th>目的</th>
            <th>请求</th>
            <th>响应</th>
            <th>Host</th>
            <th>路径</th>
            <th class="op">详情</th>
          </tr>
        </thead>
        <tbody>
          {#if visibleRecords.length === 0}
            <tr><td colspan="10" class="empty-cell">暂无匹配记录。点击“监听”后会按请求-响应聚合展示，最多显示前 100 条。</td></tr>
          {:else}
            {#each visibleRecords as record (record.seq)}
              <tr>
                <td class="seq">{record.seq}</td>
                <td class="time">{record.request_time || record.timestamp || '-'}</td>
                <td><span class="proto {displayProtocol(record).toLowerCase()}">{displayProtocol(record)}</span></td>
                <td class="mono">{endpoint(record, 'src')}</td>
                <td class="mono">{endpoint(record, 'dst')}</td>
                <td class="request-cell"><span>{requestLine(record)}</span></td>
                <td><span class="http-status {statusClass(record)}">{responseLine(record)}</span></td>
                <td class="host">{getHost(record) || '-'}</td>
                <td class="path-cell"><span>{getRequestPath(record) || '-'}</span></td>
                <td class="op"><button class="detail-btn" onclick={() => selectedRecord = record}>详情</button></td>
              </tr>
            {/each}
          {/if}
        </tbody>
      </table>
    </div>
  </section>
</div>

{#if selectedRecord}
  <div class="traffic-modal-overlay" onclick={() => selectedRecord = null} role="presentation">
    <div class="traffic-modal" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="dialog" aria-modal="true" tabindex="-1">
      <div class="traffic-modal-head">
        <div>
          <div class="modal-kicker-row">
            <span class="modal-kicker">FLOW #{selectedRecord.seq}</span>
            <span class="modal-badge proto-badge {displayProtocol(selectedRecord).toLowerCase()}">{displayProtocol(selectedRecord)}</span>
            <span class="modal-badge method-badge">{getRequestMethod(selectedRecord) || '-'}</span>
          </div>
          <h3>{displayProtocol(selectedRecord)} · {requestLine(selectedRecord)}</h3>
        </div>
        <button class="modal-close" onclick={() => selectedRecord = null}>✕</button>
      </div>
      <div class="detail-scroll modal-detail-scroll">
        <div class="detail-grid">
          <div class="detail-card meta-card">
            <h4>元数据</h4>
            <div class="meta-highlights">
              {#each metaHighlights(selectedRecord) as item}
                <div class="meta-chip {item.tone}">
                  <span>{item.label}</span>
                  <strong>{item.value}</strong>
                </div>
              {/each}
            </div>
            <div class="flow-line">
              <div>
                <span>源</span>
                <strong>{endpoint(selectedRecord, 'src')}</strong>
              </div>
              <i></i>
              <div>
                <span>目的</span>
                <strong>{endpoint(selectedRecord, 'dst')}</strong>
              </div>
            </div>
            <dl>
              {#each metaEntries(selectedRecord) as [key, value]}
                <dt>{key}</dt><dd>{value}</dd>
              {/each}
            </dl>
            {#if rawVisibleText(selectedRecord)}
              <div class="section-label">明文摘要</div>
              <pre>{rawVisibleText(selectedRecord)}</pre>
            {/if}
          </div>
          <div class="detail-card">
            <div class="detail-card-head">
              <h4>请求信息</h4>
              <div class="detail-tools">
                <button class="tool-btn" onclick={() => markFormatted(selectedRecord, 'request')}>美化格式化</button>
                <button class="tool-btn" onclick={() => copyPanel(selectedRecord, 'request')}>复制</button>
              </div>
            </div>
            <div class="request-method-grid" aria-label="请求基础信息">
              <div>
                <span>请求方法</span>
                <strong>{getRequestMethod(selectedRecord) || '-'}</strong>
              </div>
              <div>
                <span>请求路径</span>
                <strong>{getRequestPath(selectedRecord) || '-'}</strong>
              </div>
              <div>
                <span>Host / SNI</span>
                <strong>{getHost(selectedRecord) || '-'}</strong>
              </div>
              <div>
                <span>传输协议</span>
                <strong>{selectedRecord.transport_protocol || displayProtocol(selectedRecord)}</strong>
              </div>
            </div>
            <div class="request-summary">
              <span>{getRequestMethod(selectedRecord) || '-'}</span>
              <strong>{getRequestPath(selectedRecord) || '-'}</strong>
              <em>{getHost(selectedRecord) || '-'}</em>
            </div>
            <div class="section-label">请求头</div>
            {#if headerEntries(selectedRecord.request?.headers).length}
              <div class="header-box">
                {#each headerEntries(selectedRecord.request.headers) as [k, v]}
                  <div class="header-line"><strong>{k}</strong><span>{v}</span></div>
                {/each}
              </div>
            {:else}<p class="muted">无可解码请求头</p>{/if}
            <div class="section-label">请求体</div>
            <pre class="body-block {smartBodyKind(selectedRecord, 'request')}">{smartBodyText(selectedRecord, 'request')}</pre>
          </div>
          <div class="detail-card">
            <div class="detail-card-head">
              <h4>响应信息</h4>
              <div class="detail-tools">
                <button class="tool-btn" onclick={() => markFormatted(selectedRecord, 'response')}>美化格式化</button>
                <button class="tool-btn" onclick={() => copyPanel(selectedRecord, 'response')}>复制</button>
              </div>
            </div>
            <div class="request-summary response">
              <span>{selectedRecord.response?.status || selectedRecord.status || '-'}</span>
              <strong>{selectedRecord.response?.status_text || selectedRecord.status_text || responseLine(selectedRecord)}</strong>
              <em>{formatBytes(selectedRecord.response?.payload_bytes || 0)}</em>
            </div>
            <div class="section-label">响应头</div>
            {#if headerEntries(selectedRecord.response?.headers).length}
              <div class="header-box">
                {#each headerEntries(selectedRecord.response.headers) as [k, v]}
                  <div class="header-line"><strong>{k}</strong><span>{v}</span></div>
                {/each}
              </div>
            {:else}<p class="muted">无可解码响应头</p>{/if}
            <div class="section-label">响应体</div>
            <pre class="body-block {smartBodyKind(selectedRecord, 'response')}">{smartBodyText(selectedRecord, 'response')}</pre>
          </div>
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .traffic-page { display: grid; gap: 8px; width: 100%; min-width: 0; }
  .traffic-toolbar { padding: 10px; border: 1px solid rgba(45,212,191,.18); border-radius: 12px; background: linear-gradient(135deg, rgba(8,13,24,.96), rgba(10,26,28,.92)); box-shadow: 0 12px 30px rgba(0,0,0,.16); }
  .toolbar-title { display: flex; align-items: center; justify-content: space-between; gap: 12px; margin-bottom: 8px; }
  h2 { margin: 0; color: #f8fafc; font-size: 20px; letter-spacing: -0.03em; }
  .status-pill { position: relative; padding: 5px 9px 5px 22px; border-radius: 999px; background: rgba(100,116,139,.18); color: #cbd5e1; font-size: 11px; font-weight: 900; }
  .status-pill::before { content: ""; position: absolute; left: 9px; top: 50%; width: 7px; height: 7px; border-radius: 50%; transform: translateY(-50%); background: #64748b; }
  .status-pill.run { color: #86efac; background: rgba(34,197,94,.14); box-shadow: 0 0 0 1px rgba(34,197,94,.18), 0 0 26px rgba(34,197,94,.16); animation: listenGlow 1.1s ease-in-out infinite; }
  .status-pill.run::before { background: #22c55e; box-shadow: 0 0 0 0 rgba(34,197,94,.55); animation: listenDot 1s ease-out infinite; }
  .status-pill.error { color: #fecaca; background: rgba(239,68,68,.16); }
  @keyframes listenGlow { 0%,100% { filter: brightness(1); } 50% { filter: brightness(1.35); } }
  @keyframes listenDot { 0% { box-shadow: 0 0 0 0 rgba(34,197,94,.5); } 100% { box-shadow: 0 0 0 12px rgba(34,197,94,0); } }
  .filters { display: grid; gap: 7px; }
  .primary-filters { grid-template-columns: 300px 116px 150px auto; align-items: end; }
  .advanced-filters { grid-template-columns: repeat(4, minmax(140px, 1fr)); margin-top: 8px; padding: 9px; border: 1px solid rgba(45,212,191,.12); border-radius: 10px; background: rgba(2,6,23,.28); }
  label { display: grid; gap: 4px; color: #93c5fd; font-size: 11px; font-weight: 800; }
  .filter-label { color: #93c5fd; font-size: 11px; font-weight: 800; }
  input, select { min-height: 30px; width: 100%; box-sizing: border-box; border: 1px solid rgba(45,212,191,.22); border-radius: 8px; background: rgba(2,6,23,.7); color: #f8fafc; padding: 0 8px; outline: none; font-size: 12px; }
  input:focus, select:focus { border-color: #fbbf24; box-shadow: 0 0 0 2px rgba(251,191,36,.12); }
  .iface-picker-wrap { position: relative; display: grid; gap: 4px; min-width: 0; }
  .iface-trigger {
    width: 100%;
    min-height: 34px;
    display: grid;
    grid-template-columns: 44px minmax(70px, .8fr) minmax(0, 1fr);
    gap: 7px;
    align-items: center;
    border: 1px solid rgba(45,212,191,.26);
    border-radius: 9px;
    background: linear-gradient(135deg, rgba(2,6,23,.86), rgba(8,47,73,.46));
    color: #f8fafc;
    text-align: left;
    box-shadow: inset 0 0 18px rgba(45,212,191,.04);
  }
  .iface-trigger.open { border-color: rgba(251,191,36,.44); box-shadow: 0 0 0 2px rgba(251,191,36,.10); }
  .iface-trigger strong, .iface-option strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-family: var(--theme-font-family-mono); font-size: 12px; }
  .iface-trigger em, .iface-option em { overflow: hidden; color: #94a3b8; font-family: var(--theme-font-family-mono); font-size: 11px; font-style: normal; text-overflow: ellipsis; white-space: nowrap; }
  .iface-kind { display: inline-grid; place-items: center; min-height: 20px; border-radius: 6px; border: 1px solid rgba(148,163,184,.16); background: rgba(15,23,42,.72); color: #cbd5e1; font-size: 10px; font-weight: 900; }
  .iface-kind.public { color: #fde68a; border-color: rgba(251,191,36,.38); background: rgba(251,191,36,.10); box-shadow: 0 0 18px rgba(251,191,36,.08); }
  .iface-kind.physical { color: #99f6e4; border-color: rgba(45,212,191,.34); background: rgba(20,184,166,.12); }
  .iface-kind.loopback { color: #f0abfc; border-color: rgba(217,70,239,.30); background: rgba(134,25,143,.14); }
  .iface-kind.virtual { color: #c4b5fd; border-color: rgba(167,139,250,.28); background: rgba(109,40,217,.12); }
  .iface-kind.other { color: #bfdbfe; border-color: rgba(96,165,250,.24); background: rgba(37,99,235,.10); }
  .iface-menu {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    right: 0;
    z-index: 50;
    max-height: 310px;
    overflow: auto;
    padding: 6px;
    border: 1px solid rgba(94,234,212,.22);
    border-radius: 11px;
    background: linear-gradient(145deg, rgba(2,6,23,.98), rgba(8,24,34,.97));
    box-shadow: 0 20px 54px rgba(0,0,0,.46), inset 0 0 22px rgba(45,212,191,.04);
  }
  .iface-option {
    width: 100%;
    display: grid;
    grid-template-columns: 44px minmax(70px, .8fr) minmax(0, 1fr);
    gap: 7px;
    align-items: center;
    min-height: 42px;
    margin: 0 0 5px;
    border: 1px solid rgba(148,163,184,.10);
    border-radius: 8px;
    background: rgba(15,23,42,.46);
    color: #e2e8f0;
    text-align: left;
  }
  .iface-option:last-child { margin-bottom: 0; }
  .iface-option.active { border-color: rgba(45,212,191,.46); background: rgba(20,184,166,.14); }
  .iface-option small { grid-column: 2 / -1; color: #64748b; font-family: var(--theme-font-family-mono); font-size: 10px; }
  .actions { display: flex; align-items: center; flex-wrap: wrap; gap: 7px; margin-top: 8px; }
  button { min-height: 29px; border-radius: 8px; padding: 0 10px; font-size: 12px; font-weight: 900; cursor: pointer; }
  button:disabled { opacity: .45; cursor: not-allowed; }
  .primary { border: 0; color: #032018; background: linear-gradient(135deg, #5eead4, #facc15); }
  .danger { border: 1px solid rgba(248,113,113,.36); background: rgba(127,29,29,.32); color: #fecaca; }
  .secondary { border: 1px solid rgba(94,234,212,.18); background: rgba(20,184,166,.08); color: #99f6e4; }
  .filter-fold { min-height: 30px; border: 1px solid rgba(251,191,36,.22); background: rgba(251,191,36,.08); color: #fde68a; }
  .hidden-file { display: none; }
  .metrics { display: flex; flex-wrap: wrap; gap: 6px; margin-left: auto; }
  .metrics span { padding: 4px 7px; border-radius: 8px; background: rgba(15,23,42,.68); color: #cbd5e1; font-size: 11px; font-weight: 800; }
  .unsupported { margin-top: 8px; padding: 8px 10px; border-radius: 10px; background: rgba(251,191,36,.12); color: #fde68a; font-size: 12px; }
  .https-note { border: 1px solid rgba(251,191,36,.18); line-height: 1.45; }
  .traffic-stats-panel { display: grid; grid-template-columns: 260px minmax(0, 1fr); gap: 9px; margin-top: 9px; padding: 9px; border-radius: 12px; border: 1px solid rgba(94,234,212,.16); background: radial-gradient(circle at top right, rgba(45,212,191,.16), transparent 38%), rgba(2,6,23,.46); }
  .stat-section { min-width: 0; padding: 9px; border-radius: 10px; background: rgba(15,23,42,.64); border: 1px solid rgba(148,163,184,.1); }
  .stat-section h3 { margin: 0 0 7px; color: #f8fafc; font-size: 12px; }
  .iface-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 6px; }
  .iface-grid.single { grid-template-columns: minmax(180px, 280px); }
  .protocol-grid { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 6px; margin-top: 8px; }
  .protocol-grid span { padding: 8px; border-radius: 9px; background: rgba(2,6,23,.44); border: 1px solid rgba(94,234,212,.11); color: #94a3b8; font-size: 11px; }
  .protocol-grid strong { display: block; margin-top: 3px; color: #f8fafc; font-family: var(--theme-font-family-mono); font-size: 13px; }
  .traffic-body { display: grid; grid-template-columns: minmax(0, 1fr); gap: 8px; align-items: start; min-height: 0; }
  .traffic-table-wrap { min-width: 0; max-height: calc(100vh - 188px); overflow: auto; border: 1px solid rgba(148,163,184,.14); border-radius: 12px; background: rgba(2,6,23,.68); }
  .traffic-table { width: 100%; min-width: 1120px; border-collapse: separate; border-spacing: 0; color: #dbeafe; font-size: 11px; }
  th { position: sticky; top: 0; z-index: 2; height: 28px; padding: 0 7px; background: #07111f; color: #93c5fd; text-align: left; border-bottom: 1px solid rgba(148,163,184,.18); white-space: nowrap; }
  td { padding: 5px 7px; border-bottom: 1px solid rgba(148,163,184,.09); vertical-align: middle; }
  .seq { width: 48px; color: #99f6e4; font-family: var(--theme-font-family-mono); font-size: 10px; font-weight: 800; letter-spacing: 0; }
  .time { white-space: nowrap; color: #94a3b8; font-family: var(--theme-font-family-mono); }
  .mono { white-space: nowrap; color: #e0f2fe; font-family: var(--theme-font-family-mono); }
  .proto, .method, .http-status { display: inline-flex; align-items: center; min-height: 18px; padding: 0 6px; border-radius: 999px; background: rgba(45,212,191,.14); color: #99f6e4; font-family: var(--theme-font-family-mono); font-size: 10px; font-weight: 800; letter-spacing: 0; white-space: nowrap; }
  .proto.http { background: rgba(251,191,36,.14); color: #fde68a; }
  .proto.https { background: rgba(167,139,250,.16); color: #ddd6fe; }
  .proto.tcp { background: rgba(59,130,246,.14); color: #bfdbfe; }
  .proto.udp { background: rgba(45,212,191,.14); color: #99f6e4; }
  .http-status.ok { background: rgba(34,197,94,.14); color: #bbf7d0; }
  .http-status.warn { background: rgba(251,191,36,.14); color: #fde68a; }
  .http-status.bad { background: rgba(239,68,68,.16); color: #fecaca; }
  .host, .request-cell span, .path-cell span { display: block; max-width: 190px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .request-cell span { max-width: 210px; color: #fef3c7; }
  .path-cell span { max-width: 260px; color: #dbeafe; }
  .op { position: sticky; right: 0; z-index: 1; width: 76px; min-width: 76px; background: #07111f; text-align: center; box-shadow: -8px 0 16px rgba(2,6,23,.55); }
  .detail-btn { min-height: 23px; padding: 0 9px; border: 1px solid rgba(94,234,212,.2); background: rgba(20,184,166,.1); color: #99f6e4; white-space: nowrap; }
  .empty-cell { height: 150px; text-align: center; color: #64748b; }
  .detail-scroll { max-width: 100%; overflow-x: auto; padding-bottom: 2px; }
  .detail-grid { display: grid; grid-template-columns: 280px minmax(360px, 1fr) minmax(360px, 1fr); gap: 8px; min-width: 1000px; font-size: 11px; }
  .detail-card { min-width: 0; padding: 9px; border: 1px solid rgba(148,163,184,.12); border-radius: 10px; background: rgba(15,23,42,.76); color: #dbeafe; font-size: 11px; line-height: 1.45; }
  .meta-card { background: radial-gradient(circle at top left, rgba(94,234,212,.13), transparent 46%), rgba(15,23,42,.78); }
  .meta-highlights { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 7px; margin: 8px 0; }
  .meta-chip { min-width: 0; padding: 7px 8px; border-radius: 9px; border: 1px solid rgba(148,163,184,.12); background: rgba(2,6,23,.42); box-shadow: inset 0 0 20px rgba(94,234,212,.03); }
  .meta-chip span { display: block; color: #64748b; font-size: 10px; font-weight: 800; margin-bottom: 4px; }
  .meta-chip strong { display: block; color: #e0f2fe; font-family: var(--theme-font-family-mono); font-size: 11px; font-weight: 800; overflow-wrap: anywhere; }
  .meta-chip.http strong, .meta-chip.ok strong { color: #86efac; }
  .meta-chip.warn strong { color: #fde68a; }
  .meta-chip.bad strong { color: #fecaca; }
  .meta-chip.size strong { color: #67e8f9; }
  .meta-chip.method strong { color: #fef3c7; }
  .flow-line { display: grid; grid-template-columns: minmax(0,1fr) 28px minmax(0,1fr); align-items: center; gap: 6px; margin-bottom: 10px; padding: 8px; border: 1px solid rgba(94,234,212,.13); border-radius: 10px; background: rgba(20,184,166,.06); }
  .flow-line div { min-width: 0; }
  .flow-line span { display: block; color: #64748b; font-size: 10px; font-weight: 900; margin-bottom: 3px; }
  .flow-line strong { display: block; color: #dbeafe; font-family: var(--theme-font-family-mono); font-size: 11px; overflow-wrap: anywhere; }
  .flow-line i { height: 2px; border-radius: 999px; background: linear-gradient(90deg, #5eead4, #facc15); position: relative; }
  .flow-line i::after { content: ""; position: absolute; right: -1px; top: 50%; width: 6px; height: 6px; border-top: 2px solid #facc15; border-right: 2px solid #facc15; transform: translateY(-50%) rotate(45deg); }
  .detail-card-head { display: flex; align-items: center; justify-content: space-between; gap: 8px; margin-bottom: 7px; }
  .detail-card h4 { margin: 0; color: #f8fafc; font-size: 12px; font-weight: 800; }
  .detail-tools { display: flex; gap: 6px; }
  .tool-btn { min-height: 23px; padding: 0 7px; border: 1px solid rgba(251,191,36,.18); background: rgba(251,191,36,.08); color: #fde68a; font-size: 11px; }
  .section-label { margin: 8px 0 5px; color: #5eead4; font-size: 11px; font-weight: 900; }
  .request-method-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 7px; margin-bottom: 8px; }
  .request-method-grid div { min-width: 0; padding: 7px 8px; border-radius: 9px; border: 1px solid rgba(148,163,184,.12); background: rgba(2,6,23,.42); }
  .request-method-grid span { display: block; margin-bottom: 3px; color: #64748b; font-size: 10px; font-weight: 800; }
  .request-method-grid strong { display: block; color: #e0f2fe; font-family: var(--theme-font-family-mono); font-size: 11px; font-weight: 800; overflow-wrap: anywhere; }
  .request-summary { display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 6px; align-items: center; padding: 8px; border-radius: 9px; border: 1px solid rgba(251,191,36,.14); background: rgba(251,191,36,.07); }
  .request-summary span { padding: 2px 7px; border-radius: 999px; background: rgba(251,191,36,.16); color: #fde68a; font-family: var(--theme-font-family-mono); font-weight: 800; font-size: 10px; }
  .request-summary strong { color: #f8fafc; overflow-wrap: anywhere; font-family: var(--theme-font-family-mono); font-size: 11px; font-weight: 800; }
  .request-summary em { grid-column: 1 / -1; color: #94a3b8; font-style: normal; overflow-wrap: anywhere; font-size: 11px; }
  .request-summary.response { border-color: rgba(94,234,212,.14); background: rgba(20,184,166,.07); }
  .request-summary.response span { background: rgba(20,184,166,.16); color: #99f6e4; }
  dl { display: grid; grid-template-columns: 74px minmax(0, 1fr); gap: 6px 8px; margin: 0; }
  dt { color: #64748b; font-size: 10px; font-weight: 800; } dd { margin: 0; color: #dbeafe; font-size: 11px; overflow-wrap: anywhere; }
  .header-box { max-height: 132px; overflow: auto; border: 1px solid rgba(148,163,184,.08); border-radius: 8px; padding: 2px 7px; background: rgba(2,6,23,.38); }
  .header-line { display: grid; grid-template-columns: 126px minmax(0, 1fr); gap: 7px; padding: 4px 0; border-bottom: 1px solid rgba(148,163,184,.08); }
  .header-line strong { color: #93c5fd; font-size: 11px; font-weight: 800; overflow-wrap: anywhere; }
  .header-line span { color: #e2e8f0; font-size: 11px; overflow-wrap: anywhere; }
  pre { max-height: 190px; overflow: auto; margin: 0; padding: 8px; border-radius: 8px; background: #020617; color: #d1fae5; font-size: 11px; line-height: 1.45; white-space: pre-wrap; overflow-wrap: anywhere; word-break: break-word; }
  .body-block { min-height: 92px; max-height: 230px; border: 1px solid rgba(148,163,184,.14); box-shadow: inset 0 0 24px rgba(2,6,23,.55); }
  .body-block.json { color: #bfdbfe; border-color: rgba(96,165,250,.22); background: linear-gradient(180deg, rgba(15,23,42,.96), #020617); }
  .body-block.xml { color: #fde68a; border-color: rgba(251,191,36,.2); background: linear-gradient(180deg, rgba(30,24,10,.55), #020617); }
  .body-block.text { color: #d1fae5; }
  .muted { margin: 0; color: #64748b; }
  .log-line { padding: 6px 0; border-bottom: 1px solid rgba(148,163,184,.08); color: #cbd5e1; font-size: 12px; line-height: 1.35; }
  .log-line span { display: inline-block; margin-right: 6px; color: #64748b; font-family: var(--theme-font-family-mono); }
  .log-line.error { color: #fecaca; } .log-line.ok { color: #bbf7d0; }
  .iface-row { display: grid; gap: 3px; padding: 7px 0; border-bottom: 1px solid rgba(148,163,184,.08); }
  .iface-row strong { color: #cbd5e1; } .iface-row.selected strong { color: #5eead4; }
  .iface-row span { color: #94a3b8; font-size: 11px; }
  .traffic-modal-overlay { position: fixed; inset: 0; z-index: 120; display: flex; align-items: center; justify-content: center; padding: 18px; background: radial-gradient(circle at 20% 0%, rgba(45,212,191,.20), transparent 34%), rgba(0,0,0,.66); backdrop-filter: blur(8px); }
  .traffic-modal { width: min(1220px, calc(100vw - 36px)); max-height: min(86vh, 860px); display: flex; flex-direction: column; overflow: hidden; border-radius: 16px; border: 1px solid rgba(94,234,212,.24); background: linear-gradient(145deg, rgba(2,6,23,.98), rgba(8,24,34,.96)); box-shadow: 0 26px 80px rgba(0,0,0,.45), inset 0 0 36px rgba(45,212,191,.05); font-family: var(--theme-font-family-base); font-size: 12px; }
  .traffic-modal-head { display: flex; align-items: center; justify-content: space-between; gap: 14px; padding: 14px 16px; border-bottom: 1px solid rgba(94,234,212,.16); background: linear-gradient(90deg, rgba(20,184,166,.12), rgba(251,191,36,.06)); }
  .traffic-modal-head h3 { margin: 4px 0 0; color: #f8fafc; font-size: 14px; font-weight: 800; overflow-wrap: anywhere; }
  .modal-kicker-row { display: flex; align-items: center; flex-wrap: wrap; gap: 6px; }
  .modal-kicker { color: #5eead4; font-family: var(--theme-font-family-mono); font-size: 10px; font-weight: 800; letter-spacing: 0; }
  .modal-badge { display: inline-grid; place-items: center; min-height: 18px; padding: 0 7px; border-radius: 999px; border: 1px solid rgba(148,163,184,.16); background: rgba(15,23,42,.64); color: #cbd5e1; font-family: var(--theme-font-family-mono); font-size: 10px; font-weight: 800; }
  .proto-badge.http { color: #fde68a; border-color: rgba(251,191,36,.24); background: rgba(251,191,36,.1); }
  .proto-badge.https { color: #ddd6fe; border-color: rgba(167,139,250,.24); background: rgba(167,139,250,.1); }
  .proto-badge.tcp { color: #bfdbfe; border-color: rgba(96,165,250,.24); background: rgba(96,165,250,.1); }
  .proto-badge.udp { color: #99f6e4; border-color: rgba(45,212,191,.24); background: rgba(45,212,191,.1); }
  .method-badge { color: #fef3c7; border-color: rgba(251,191,36,.22); background: rgba(251,191,36,.08); }
  .modal-close { min-width: 32px; border: 1px solid rgba(148,163,184,.18); background: rgba(15,23,42,.7); color: #cbd5e1; }
  .modal-close:hover { border-color: rgba(248,113,113,.4); color: #fecaca; }
  .modal-detail-scroll { overflow: auto; padding: 10px; }
  @media (max-width: 1180px) {
    .primary-filters, .advanced-filters, .traffic-stats-panel { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  }
  @media (max-width: 760px) {
    .primary-filters, .advanced-filters, .traffic-stats-panel { grid-template-columns: 1fr; }
    .metrics { margin-left: 0; }
    .detail-grid { grid-template-columns: 1fr; }
  }
</style>
