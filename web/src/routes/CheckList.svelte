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
  let displayedHealthPercent = $state(0);
  let healthPollTimer = null;
  let healthFinishTimer = null;
  let healthPercentAnim = null;
  let healthStartedAt = 0;
  let exporting = $state(false);
  let exportingConfigs = $state(false);
  let importing = $state(false);
  let importingConfigs = $state(false);
  let exportError = $state(null);
  let importInput = $state(null);
  let configImportInput = $state(null);
  let expandedChecks = $state(new Set());
  let viewMode = $state('grid');
  let sortKey = $state('name');
  let sortDir = $state('asc');
  let showFilters = $state(false);
  let showHealthLogs = $state(false);
  let showHealthModal = $state(false);
  let expandedReportItem = $state('');
  let exportingPdf = $state(false);
  let pdfExportStatus = $state('');
  let healthResultFilter = $state('all');

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
    showHealthLogs = true;
    healthCheckRunning = true;
    healthCheckResult = null;
    healthCheckError = null;
    exportError = null;
    expandedReportItem = '';
    healthResultFilter = 'all';
    healthStartedAt = Date.now();
    if (healthFinishTimer) clearTimeout(healthFinishTimer);
    healthFinishTimer = null;
    setHealthProgress({
      percent: 0,
      current_step: '准备启动体检任务',
      logs: ['正在创建后台体检任务...'],
      completed: 0,
      total: 0,
      warnings: 0,
      errors: 0,
      status: 'running',
    });
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
          setHealthProgress({
            ...d,
            status: 'running',
            percent: Math.min(d.percent || 100, 96),
            current_step: '正在生成体检报告与同步告警',
            logs: [...(d.logs || []), '正在整理结构化检查结果、告警命中和导出数据...'],
          });
          if (!healthFinishTimer) {
            healthFinishTimer = setTimeout(() => {
              healthFinishTimer = null;
              if (healthPollTimer) clearInterval(healthPollTimer);
              healthPollTimer = null;
              setHealthProgress({ ...d, percent: 100 });
              healthCheckRunning = false;
              showHealthLogs = true;
              if (d.result) healthCheckResult = d.result;
              window.dispatchEvent(new CustomEvent('dm-alerts-refresh'));
            }, minVisibleMs - elapsed);
          }
          return;
        }
        setHealthProgress(d);
        if (d.status === 'done' || d.status === 'error') {
          if (healthPollTimer) clearInterval(healthPollTimer);
          healthPollTimer = null;
          healthCheckRunning = false;
          showHealthLogs = true;
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

  function smoothPercentValue(value) {
    const n = Number(value || 0);
    if (!Number.isFinite(n)) return 0;
    return Math.max(0, Math.min(100, n));
  }

  function setHealthProgress(next) {
    healthProgress = next;
    animateHealthPercent(smoothPercentValue(next?.percent));
  }

  function animateHealthPercent(target) {
    if (healthPercentAnim) cancelAnimationFrame(healthPercentAnim);
    const from = smoothPercentValue(displayedHealthPercent);
    const to = smoothPercentValue(target);
    const start = performance.now();
    const duration = Math.max(260, Math.min(720, Math.abs(to - from) * 18));
    const ease = (t) => 1 - Math.pow(1 - t, 3);
    const tick = (now) => {
      const ratio = Math.min(1, (now - start) / duration);
      displayedHealthPercent = Math.round((from + (to - from) * ease(ratio)) * 10) / 10;
      if (ratio < 1) {
        healthPercentAnim = requestAnimationFrame(tick);
      } else {
        displayedHealthPercent = to;
        healthPercentAnim = null;
      }
    };
    healthPercentAnim = requestAnimationFrame(tick);
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
      window.dispatchEvent(new CustomEvent('dm-alerts-refresh'));
    } catch (e) {
      exportError = e.message;
    }
    exporting = false;
  }

  async function exportConnectionConfigs() {
    exportingConfigs = true;
    exportError = null;
    try {
      const r = await fetch('/api/check-configs/export', { cache: 'no-store' });
      if (!r.ok) throw new Error('连接配置导出失败: ' + r.status);
      const data = await r.json();
      const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json;charset=utf-8' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      const ts = new Date().toISOString().replace(/[:.]/g, '-');
      a.href = url;
      a.download = `dm-check-connection-configs-${ts}.json`;
      document.body.appendChild(a);
      a.click();
      a.remove();
      URL.revokeObjectURL(url);
    } catch (e) {
      exportError = e.message;
    }
    exportingConfigs = false;
  }

  function importAllChecks() {
    importInput?.click();
  }

  function importConnectionConfigs() {
    configImportInput?.click();
  }

  function closeHealthPanels() {
    showHealthModal = false;
    showHealthLogs = false;
    healthCheckResult = null;
    healthCheckError = null;
    exportError = null;
    expandedReportItem = '';
    healthResultFilter = 'all';
  }

  function closeHealthModal() {
    showHealthModal = false;
  }

  function handleHealthModalKeydown(event) {
    if (event.key === 'Escape') {
      event.stopPropagation();
    }
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

  async function importConnectionConfigFile(event) {
    const file = event.currentTarget.files?.[0];
    if (!file) return;
    importingConfigs = true;
    exportError = null;
    try {
      const text = await file.text();
      const data = JSON.parse(text);
      const r = await fetch('/api/check-configs/import', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(data),
      });
      const d = await r.json();
      if (!r.ok || d.status === 'error') throw new Error(d.message || '连接配置导入失败');
      exportError = `${d.message || '连接配置已导入'}${d.config_file ? '：' + d.config_file : ''}`;
    } catch (e) {
      exportError = '连接配置导入失败: ' + (e.message || e);
    } finally {
      importingConfigs = false;
      event.currentTarget.value = '';
    }
  }

  function toggleCheck(id) {
    const next = new Set(expandedChecks);
    if (next.has(id)) next.delete(id); else next.add(id);
    expandedChecks = next;
  }

  function reportItemOpen(key) {
    return expandedReportItem === key;
  }

  function toggleReportItem(key) {
    expandedReportItem = expandedReportItem === key ? '' : key;
  }

  function checkWarnings(check) {
    return Number(check.warning_count ?? check.warnings ?? 0);
  }

  function checkErrors(check) {
    return Number(check.error_count ?? check.errors ?? 0);
  }

  function checkSectionCount(check) {
    return Number(check.section_count ?? (check.sections || []).length ?? 0);
  }

  function checkSummaryText(check) {
    const warnings = checkWarnings(check);
    const errors = checkErrors(check);
    if (errors || warnings) return `${warnings} 警告 / ${errors} 错误`;
    return `${checkSectionCount(check)} 个分组`;
  }

  function filteredHealthChecks() {
    const checks = healthCheckResult?.checks || [];
    if (healthResultFilter === 'warn') return checks.filter(check => checkWarnings(check) > 0);
    if (healthResultFilter === 'error') return checks.filter(check => checkErrors(check) > 0);
    return checks;
  }

  function filteredHealthAlerts() {
    const alerts = healthCheckResult?.alerts || [];
    let result = alerts;
    if (healthResultFilter === 'warn') result = result.filter(alert => (alert.level || 'warn') !== 'error');
    if (healthResultFilter === 'error') result = result.filter(alert => alert.level === 'error');
    const seen = new Set();
    return result.filter((alert, index) => {
      const key = [
        alert.rule_id || alert.id || alert.type || 'alert',
        alert.service_name || alert.target || alert.pid || alert.log_path || 'system',
        alert.summary || alert.message || alert.title || index,
      ].join('|').toLowerCase();
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
  }

  function setHealthResultFilter(value) {
    healthResultFilter = healthResultFilter === value ? 'all' : value;
    expandedReportItem = '';
  }

  function itemSeverity(item) {
    if (item.type === 'finding') return item.level === 'error' ? 'error' : 'warn';
    if (item.type === 'error') return 'error';
    if (item.type === 'warning') return 'warn';
    if (item.type === 'label' || item.type === 'bar' || item.type === 'table') return null;
    if (item.status === 'error') return 'error';
    if (item.status === 'warn') return 'warn';
    return null;
  }

  function statusText(s) {
    if (s === 'ok') return '正常';
    if (s === 'warn') return '警告';
    if (s === 'error') return '异常';
    return '信息';
  }

  function itemTitle(item) {
    if (item.type === 'finding') return `${item.title || '规则命中'}: ${item.summary || ''}`;
    if (item.type === 'label') return `${item.key}: ${item.value}`;
    if (item.type === 'bar') return `${item.key}: ${Number(item.value || 0).toFixed(1)}${item.unit || ''}`;
    if (item.type === 'warning' || item.type === 'error' || item.type === 'info' || item.type === 'success') return item.text || '';
    if (item.type === 'table') return `${item.headers?.join(' / ') || '表格'} (${item.rows?.length || 0} 行)`;
    return item.key || item.title || item.type || '检查项';
  }

  function itemDetails(item) {
    if (item.details) return item.details;
    if (item.type === 'finding') {
      const evidence = (item.evidence || []).map(v => `- ${v}`).join('\n');
      const commands = (item.commands || []).map(v => `$ ${v}`).join('\n');
      return [
        item.rule_id ? `规则: ${item.rule_id}` : '',
        `对象: ${item.target || '-'}`,
        `分类: ${item.category || '-'}`,
        `概要: ${item.summary || '-'}`,
        evidence ? `证据:\n${evidence}` : '',
        item.suggestion ? `处理建议: ${item.suggestion}` : '',
        commands ? `建议命令:\n${commands}` : '',
      ].filter(Boolean).join('\n');
    }
    if (item.type === 'bar') return `当前值: ${Number(item.value || 0).toFixed(2)}${item.unit || ''}\n阈值上限: ${item.max ?? '-'}${item.unit || ''}\n状态: ${statusText(item.status)}`;
    if (item.type === 'label') return `检查项: ${item.key}\n当前值: ${item.value}\n状态: ${statusText(item.status)}`;
    return item.text || '';
  }

  function sectionIssues(section) {
    return (section.items || [])
      .map((item, index) => ({ item, index, severity: itemSeverity(item), title: itemTitle(item), details: itemDetails(item) }))
      .filter(issue => issue.severity);
  }

  function sectionNormalItems(section) {
    return (section.items || []).filter(item => !itemSeverity(item));
  }

  function reportValueClass(status, value = '') {
    const text = String(value ?? '').toLowerCase();
    if (status === 'error' || /异常|失败|failed|fatal|error|critical/.test(text)) return 'value-error';
    if (status === 'warn' || /警告|告警|warning|warn|timeout|超过|过高|过低/.test(text)) return 'value-warn';
    if (status === 'ok' || /正常|运行中|success|ok|healthy/.test(text)) return 'value-ok';
    if (/^-?\d+(\.\d+)?(%|mb|gb|kb|ms|s)?$/i.test(String(value).trim())) return 'value-number';
    return 'value-info';
  }

  function reportCellClass(value) {
    const text = String(value ?? '').trim();
    const lower = text.toLowerCase();
    if (/异常|失败|failed|fatal|error|critical|down|stopped/.test(lower)) return 'cell-bad';
    if (/警告|告警|warning|warn|timeout|超过|过高|过低/.test(lower)) return 'cell-warn';
    if (/正常|运行中|success|ok|healthy|active/.test(lower)) return 'cell-good';
    if (/^-?\d+(\.\d+)?(%|mb|gb|kb|ms|s)?$/i.test(text)) return 'cell-number';
    return '';
  }

  function barPercent(item) {
    const value = Number(item.value || 0);
    const max = Number(item.max || 0);
    if (!Number.isFinite(value) || !Number.isFinite(max) || max <= 0) return 0;
    return Math.max(0, Math.min(100, (value / max) * 100));
  }

  function alertKey(alert, index) {
    return alert.id || alert.rule_id || `${alert.level || 'warn'}-${alert.title || alert.summary || index}`;
  }

  function alertEvidence(alert) {
    return Array.isArray(alert.evidence) ? alert.evidence : [];
  }

  function alertSuggestions(alert) {
    if (Array.isArray(alert.suggestions)) return alert.suggestions;
    return alert.handling ? [alert.handling] : [];
  }

  async function exportHealthReportPdf() {
    if (!healthCheckResult || exportingPdf) return;
    exportingPdf = true;
    pdfExportStatus = '正在准备报告...';
    exportError = null;
    let exportRoot = null;
    let exportOverlay = null;
    try {
      const [{ default: html2canvas }, { jsPDF }] = await Promise.all([
        import('html2canvas'),
        import('jspdf'),
      ]);
      pdfExportStatus = '正在生成导出报告...';
      exportOverlay = buildPdfExportOverlay();
      exportRoot = document.createElement('div');
      exportRoot.className = 'pdf-export-root';
      const exportReport = buildHealthReportExportNode(healthCheckResult);
      exportRoot.appendChild(exportReport);
      document.body.appendChild(exportRoot);
      document.body.appendChild(exportOverlay);
      await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
      const pdf = new jsPDF('p', 'mm', 'a4');
      const pageWidth = pdf.internal.pageSize.getWidth();
      const pageHeight = pdf.internal.pageSize.getHeight();
      const margin = 8;
      const contentWidth = pageWidth - margin * 2;
      const contentHeight = pageHeight - margin * 2;
      let hasPage = false;
      const blocks = [
        exportReport.querySelector('.pdf-report-cover'),
        ...exportReport.querySelectorAll('.pdf-report-block'),
      ].filter(Boolean);
      if (!blocks.length) blocks.push(exportReport);
      for (let i = 0; i < blocks.length; i += 1) {
        pdfExportStatus = `正在渲染 PDF ${i + 1}/${blocks.length}`;
        updatePdfExportOverlay(exportOverlay, pdfExportStatus);
        await new Promise(resolve => setTimeout(resolve, 0));
        const canvas = await html2canvas(blocks[i], {
          scale: 1.15,
          backgroundColor: '#ffffff',
          useCORS: true,
          logging: false,
          windowWidth: exportReport.scrollWidth,
          windowHeight: blocks[i].scrollHeight,
        });
        hasPage = addCanvasToPdf(pdf, canvas, {
          hasPage,
          margin,
          contentWidth,
          contentHeight,
        });
      }
      pdfExportStatus = '正在保存 PDF...';
      updatePdfExportOverlay(exportOverlay, pdfExportStatus);
      const ts = new Date().toISOString().replace(/[:.]/g, '-');
      pdf.save(`dm-health-report-${ts}.pdf`);
    } catch (e) {
      exportError = 'PDF 导出失败: ' + (e.message || e);
    } finally {
      exportRoot?.remove();
      exportOverlay?.remove();
      exportingPdf = false;
      pdfExportStatus = '';
    }
  }

  function exportHealthRawData() {
    if (!healthCheckResult) return;
    const payload = {
      schema: 'dm-health-raw/v1',
      exported_at: new Date().toISOString(),
      source: 'current_health_check',
      progress: healthProgress,
      result: healthCheckResult,
      checks: healthCheckResult.checks || [],
      alerts: healthCheckResult.alerts || [],
      summary: {
        overall_status: healthCheckResult.overall_status,
        total_checks: healthCheckResult.total_checks,
        total_warnings: healthCheckResult.total_warnings,
        total_errors: healthCheckResult.total_errors,
      },
    };
    const blob = new Blob([JSON.stringify(payload, null, 2)], { type: 'application/json;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    const ts = new Date().toISOString().replace(/[:.]/g, '-');
    a.href = url;
    a.download = `dm-health-raw-${ts}.json`;
    document.body.appendChild(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
  }

  function buildPdfExportOverlay() {
    const overlay = document.createElement('div');
    overlay.className = 'pdf-export-screen';
    const card = document.createElement('div');
    card.className = 'pdf-export-screen-card';
    const spinner = document.createElement('div');
    spinner.className = 'pdf-export-screen-spinner';
    const text = document.createElement('div');
    text.className = 'pdf-export-screen-text';
    text.textContent = pdfExportStatus || '正在导出 PDF...';
    card.append(spinner, text);
    overlay.appendChild(card);
    return overlay;
  }

  function updatePdfExportOverlay(overlay, text) {
    const target = overlay?.querySelector('.pdf-export-screen-text');
    if (target) target.textContent = text;
  }

  function appendText(parent, tag, className, text) {
    const el = document.createElement(tag);
    if (className) el.className = className;
    el.textContent = text ?? '';
    parent.appendChild(el);
    return el;
  }

  function buildHealthReportExportNode(report) {
    const root = document.createElement('div');
    root.className = 'pdf-export-report';
    const cover = document.createElement('section');
    cover.className = 'pdf-report-cover';
    appendText(cover, 'h1', '', 'DM 系统体检报告');
    appendText(cover, 'p', 'pdf-report-subtitle', `生成时间: ${report.timestamp || new Date().toLocaleString('zh-CN')}`);
    const summary = document.createElement('div');
    summary.className = 'pdf-summary-grid';
    [
      ['总体状态', statusText(report.overall_status)],
      ['检查项', report.total_checks ?? 0],
      ['警告', report.total_warnings ?? 0],
      ['错误', report.total_errors ?? 0],
    ].forEach(([label, value]) => {
      const item = document.createElement('div');
      appendText(item, 'span', '', label);
      appendText(item, 'strong', '', String(value));
      summary.appendChild(item);
    });
    cover.appendChild(summary);
    root.appendChild(cover);

    (report.checks || []).forEach((check, index) => {
      const block = document.createElement('section');
      block.className = 'pdf-report-block';
      appendText(block, 'h2', '', `${index + 1}. ${check.name || check.id || '检查项'}`);
      appendText(block, 'p', 'pdf-report-meta', `状态: ${statusText(check.status)} | ID: ${check.id || '-'} | 分类: ${check.category || '-'} | 耗时: ${check.duration_ms || 0}ms`);
      if (check.description) appendText(block, 'p', 'pdf-report-desc', check.description);
      (check.sections || []).forEach(section => appendPdfSection(block, section));
      root.appendChild(block);
    });

    if ((report.alerts || []).length) {
      const block = document.createElement('section');
      block.className = 'pdf-report-block';
      appendText(block, 'h2', '', `规则命中告警 (${report.alerts.length})`);
      (report.alerts || []).forEach((alert, index) => {
        const item = document.createElement('div');
        item.className = 'pdf-alert-item';
        appendText(item, 'h3', '', `${index + 1}. ${alert.title || alert.summary || alert.message || '告警'}`);
        appendText(item, 'p', '', `级别: ${alert.level || 'warn'} | 对象: ${alert.service_name || alert.target || alert.pid || alert.log_path || '系统'}`);
        if (alert.summary || alert.message) appendText(item, 'p', '', alert.summary || alert.message);
        alertEvidence(alert).forEach(line => appendText(item, 'pre', '', line));
        alertSuggestions(alert).forEach(line => appendText(item, 'p', 'pdf-suggestion', `建议: ${line}`));
        block.appendChild(item);
      });
      root.appendChild(block);
    }
    return root;
  }

  function appendPdfSection(parent, section) {
    const wrap = document.createElement('div');
    wrap.className = 'pdf-section';
    appendText(wrap, 'h3', '', section.title || '分组');
    if (section.description) appendText(wrap, 'p', 'pdf-report-desc', section.description);
    (section.items || []).forEach(item => appendPdfItem(wrap, item));
    parent.appendChild(wrap);
  }

  function appendPdfItem(parent, item) {
    if (item.type === 'table') {
      appendPdfTable(parent, item.headers || [], item.rows || []);
      return;
    }
    const box = document.createElement('div');
    box.className = `pdf-item pdf-item-${itemSeverity(item) || 'info'}`;
    appendText(box, 'strong', '', itemTitle(item));
    const details = itemDetails(item);
    if (details) appendText(box, 'pre', '', details);
    parent.appendChild(box);
  }

  function appendPdfTable(parent, headers, rows) {
    const wrap = document.createElement('div');
    wrap.className = 'pdf-table-wrap';
    const table = document.createElement('table');
    const thead = document.createElement('thead');
    const headRow = document.createElement('tr');
    headers.forEach(header => appendText(headRow, 'th', '', header));
    thead.appendChild(headRow);
    table.appendChild(thead);
    const tbody = document.createElement('tbody');
    rows.forEach(row => {
      const tr = document.createElement('tr');
      row.forEach(cell => appendText(tr, 'td', '', cell));
      tbody.appendChild(tr);
    });
    table.appendChild(tbody);
    wrap.appendChild(table);
    parent.appendChild(wrap);
  }

  function addCanvasToPdf(pdf, canvas, options) {
    const { margin, contentWidth, contentHeight } = options;
    const pxPerMm = canvas.width / contentWidth;
    const pageCanvasHeight = Math.floor(contentHeight * pxPerMm);
    let sourceY = 0;
    let hasPage = options.hasPage;
    while (sourceY < canvas.height) {
      const sliceHeight = Math.min(pageCanvasHeight, canvas.height - sourceY);
      const pageCanvas = document.createElement('canvas');
      pageCanvas.width = canvas.width;
      pageCanvas.height = sliceHeight;
      const ctx = pageCanvas.getContext('2d');
      if (!ctx) throw new Error('无法创建 PDF 页面画布');
      ctx.fillStyle = '#ffffff';
      ctx.fillRect(0, 0, pageCanvas.width, pageCanvas.height);
      ctx.drawImage(canvas, 0, sourceY, canvas.width, sliceHeight, 0, 0, canvas.width, sliceHeight);
      if (hasPage) pdf.addPage();
      pdf.addImage(pageCanvas.toDataURL('image/jpeg', 0.86), 'JPEG', margin, margin, contentWidth, sliceHeight / pxPerMm);
      sourceY += sliceHeight;
      hasPage = true;
    }
    return hasPage;
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
    if (healthPercentAnim) cancelAnimationFrame(healthPercentAnim);
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
        {exporting ? '导出中' : '导出检查项'}
      </button>
      <button class="tool-btn config-export-mini" onclick={exportConnectionConfigs} disabled={exportingConfigs || healthCheckRunning}>
        {exportingConfigs ? '导出中' : '导出连接配置'}
      </button>
      <button class="tool-btn config-import-mini" onclick={importConnectionConfigs} disabled={importingConfigs || healthCheckRunning}>
        {importingConfigs ? '导入中' : '导入连接配置'}
      </button>
      <button class="tool-btn import-mini" onclick={importAllChecks} disabled={importing || healthCheckRunning}>
        {importing ? '导入中' : '导入报告'}
      </button>
      <input bind:this={importInput} class="hidden-file" type="file" accept="application/json,.json" onchange={importCheckFile} />
      <input bind:this={configImportInput} class="hidden-file" type="file" accept="application/json,.json" onchange={importConnectionConfigFile} />
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
              <div class="btn-spinner"><i></i></div>
              <span>{displayedHealthPercent.toFixed(displayedHealthPercent % 1 ? 1 : 0)}%</span>
        {:else}
          <span class="btn-icon">▶</span>
          <span>一键体检</span>
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
  <div class="health-modal-overlay" onkeydown={handleHealthModalKeydown} role="presentation" tabindex="-1">
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
          {#if healthCheckResult}
            <button class="modal-log-toggle pdf-action" onclick={exportHealthReportPdf} disabled={exportingPdf}>
              {exportingPdf ? '导出中' : '导出 PDF'}
            </button>
            <button class="modal-log-toggle raw-action" onclick={exportHealthRawData}>
              导出原始数据
            </button>
          {/if}
          {#if pdfExportStatus}
            <span class="pdf-export-status">{pdfExportStatus}</span>
          {/if}
          {#if healthCheckRunning}
            <span class="modal-running">{displayedHealthPercent.toFixed(displayedHealthPercent % 1 ? 1 : 0)}%</span>
          {/if}
          <button class="modal-icon-btn" onclick={closeHealthModal} aria-label="关闭体检窗口">✕</button>
        </div>
      </div>

      <div class="health-modal-body">
        {#if healthCheckRunning}
          <div class="health-loading modal-loading">
            <div class="scanner-grid"></div>
            <div class="health-progress-head">
              <div class="progress-orb" class:complete={displayedHealthPercent >= 100} style="--p:{displayedHealthPercent}">
                <div class="progress-orb-inner">
                  <span>{displayedHealthPercent.toFixed(displayedHealthPercent % 1 ? 1 : 0)}%</span>
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
                  <div class="loading-progress" style="width:{displayedHealthPercent}%"></div>
                </div>
              </div>
            </div>
            <div class="step-rail" aria-label="体检步骤">
              {#each ['启动', '采集', '规则命中', '告警同步', '报告'] as step, i}
                <div class="step-dot" class:active={displayedHealthPercent >= i * 24}>
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
              </div>
            </div>
            {#if (healthProgress?.logs || []).length}
              {#each (healthProgress?.logs || []) as line}
                <div class="log-line">{line}</div>
              {/each}
            {:else if healthCheckResult}
              <div class="log-line">{healthCheckResult.timestamp || ''} 体检完成：{healthCheckResult.total_checks || 0} 项，警告 {healthCheckResult.total_warnings || 0}，错误 {healthCheckResult.total_errors || 0}</div>
            {:else}
              <div class="log-line">暂无体检日志，点击一键体检后会实时记录步骤。</div>
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
                <button class="stat-item stat-button" class:active={healthResultFilter === 'all'} onclick={() => setHealthResultFilter('all')}>
                  <span class="stat-value">{healthCheckResult.total_checks}</span>
                  <span class="stat-label">检查项</span>
                </button>
                <button class="stat-item stat-warn stat-button" class:active={healthResultFilter === 'warn'} onclick={() => setHealthResultFilter('warn')}>
                  <span class="stat-value">{healthCheckResult.total_warnings}</span>
                  <span class="stat-label">警告</span>
                </button>
                <button class="stat-item stat-error stat-button" class:active={healthResultFilter === 'error'} onclick={() => setHealthResultFilter('error')}>
                  <span class="stat-value">{healthCheckResult.total_errors}</span>
                  <span class="stat-label">错误</span>
                </button>
              </div>
            </div>

            <div class="result-checks">
              <div class="report-section-title">
                检查结果明细
                {#if healthResultFilter !== 'all'}<span class="active-filter">当前过滤：{healthResultFilter === 'warn' ? '警告检查项' : '错误检查项'}</span>{/if}
              </div>
              {#each filteredHealthChecks() as check, checkIndex}
                {@const key = `check-${check.id || 'item'}-${checkIndex}`}
                <div class="result-check-item" class:check-open={reportItemOpen(key)} class:check-ok={check.status === 'ok'} class:check-warn={check.status === 'warn'} class:check-error={check.status === 'error'}>
                  <button class="check-main" onclick={() => toggleReportItem(key)} aria-expanded={reportItemOpen(key)}>
                    <span class="check-status-dot" style="background:{statusColor(check.status)}"></span>
                    <span class="check-name">
                      <strong>{check.name}</strong>
                      <em>{check.id}</em>
                    </span>
                    <span class="check-findings">{checkSummaryText(check)}</span>
                    <span class="check-duration">{check.duration_ms || 0}ms</span>
                    <span class="check-arrow">{reportItemOpen(key) ? '↑' : '↓'}</span>
                  </button>
                  {#if reportItemOpen(key)}
                    <div class="report-detail check-report-detail">
                      <div class="report-meta-grid">
                        <div><span>状态</span><strong style="color:{statusColor(check.status)}">{statusText(check.status)}</strong></div>
                        <div><span>分类</span><strong>{check.category || '-'}</strong></div>
                        <div><span>时间</span><strong>{check.timestamp || healthCheckResult.timestamp || '-'}</strong></div>
                        <div><span>分组</span><strong>{checkSectionCount(check)}</strong></div>
                      </div>
                      {#if check.description}
                        <p class="report-desc">{check.description}</p>
                      {/if}
                      {#each (check.sections || []) as section}
                        {@const issues = sectionIssues(section)}
                        <div class="report-subsection">
                          <div class="report-subsection-head">
                            {#if section.icon}<span>{section.icon}</span>{/if}
                            <strong>{section.title}</strong>
                            {#if issues.length}<em>{issues.length} 条异常/告警</em>{/if}
                          </div>
                          {#if section.description}
                            <p class="report-desc">{section.description}</p>
                          {/if}
                          {#if issues.length}
                            <div class="report-issues">
                              {#each issues as issue}
                                <div class="report-issue" class:issue-row-error={issue.severity === 'error'}>
                                  <span class="issue-level">{issue.severity === 'error' ? 'FAIL' : 'WARN'}</span>
                                  <div>
                                    <div class="issue-title">{issue.title}</div>
                                    <pre class="issue-details">{issue.details}</pre>
                                  </div>
                                </div>
                              {/each}
                            </div>
                          {/if}
                          <div class="report-items">
                            {#each sectionNormalItems(section) as item}
                              {#if item.type === 'table'}
                                <div class="report-table-wrap table-scroll">
                                  <table class="report-table">
                                    <thead>
                                      <tr>
                                        {#each (item.headers || []) as h}
                                          <th>{h}</th>
                                        {/each}
                                      </tr>
                                    </thead>
                                    <tbody>
                                      {#each (item.rows || []) as row}
                                        <tr>
                                          {#each row as cell}
                                            <td class={reportCellClass(cell)} title={cell}>{cell}</td>
                                          {/each}
                                        </tr>
                                      {/each}
                                    </tbody>
                                  </table>
                                </div>
                              {:else if item.type === 'label'}
                                <div class="report-kv {reportValueClass(item.status, item.value)}">
                                  <span>{item.key}</span>
                                  <strong title={item.value}>{item.value}</strong>
                                  {#if item.status}<em>{statusText(item.status)}</em>{/if}
                                </div>
                              {:else if item.type === 'bar'}
                                <div class="report-metric {reportValueClass(item.status, item.value)}">
                                  <div class="metric-head">
                                    <span>{item.key}</span>
                                    <strong>{Number(item.value || 0).toFixed(1)}{item.unit || ''}</strong>
                                    <em>阈值 {item.max ?? '-'}{item.unit || ''}</em>
                                  </div>
                                  <div class="metric-track">
                                    <div class="metric-fill" style="width:{barPercent(item)}%"></div>
                                  </div>
                                </div>
                              {:else if item.type === 'divider'}
                                <div class="report-divider"></div>
                              {:else}
                                <div class="report-line" class:line-warn={itemSeverity(item) === 'warn'} class:line-error={itemSeverity(item) === 'error'}>
                                  <span>{itemSeverity(item) === 'error' ? 'FAIL' : itemSeverity(item) === 'warn' ? 'WARN' : item.status ? statusText(item.status) : item.type}</span>
                                  <strong>{itemTitle(item)}</strong>
                                  {#if itemDetails(item)}
                                    <pre>{itemDetails(item)}</pre>
                                  {/if}
                                </div>
                              {/if}
                            {/each}
                          </div>
                        </div>
                      {/each}
                    </div>
                  {/if}
                </div>
              {:else}
                <div class="empty-filtered-result">当前过滤条件下没有检查结果。</div>
              {/each}
            </div>
            {#if (healthCheckResult.alerts || []).length}
              <div class="result-alerts">
                <div class="result-alerts-title">
                  规则命中告警 {filteredHealthAlerts().length} 条
                  {#if healthResultFilter !== 'all'}<span class="active-filter">总计 {(healthCheckResult.alerts || []).length} 条</span>{/if}
                </div>
                {#each filteredHealthAlerts() as alert, alertIndex}
                  {@const key = `alert-${alertKey(alert, alertIndex)}-${alertIndex}`}
                  <div class="result-alert-item" class:alert-open={reportItemOpen(key)}>
                    <button class="alert-main" onclick={() => toggleReportItem(key)} aria-expanded={reportItemOpen(key)}>
                      <span class="alert-level" class:error={alert.level === 'error'}>{alert.level || 'warn'}</span>
                      <span class="alert-title">{alert.title || alert.summary || alert.message}</span>
                      <span class="alert-target">{alert.service_name || alert.target || alert.pid || alert.log_path || '系统'}</span>
                      <span class="alert-handling">{alert.handling || (alert.suggestions || [])[0] || '查看处理建议'}</span>
                      <span class="check-arrow">{reportItemOpen(key) ? '↑' : '↓'}</span>
                    </button>
                    {#if reportItemOpen(key)}
                      <div class="report-detail alert-report-detail">
                        <div class="report-meta-grid">
                          <div><span>级别</span><strong>{alert.level || 'warn'}</strong></div>
                          <div><span>类型</span><strong>{alert.type || alert.category || '-'}</strong></div>
                          <div><span>对象</span><strong>{alert.service_name || alert.target || alert.pid || alert.log_path || '系统'}</strong></div>
                          <div><span>时间</span><strong>{alert.last_seen || alert.timestamp || healthCheckResult.timestamp || '-'}</strong></div>
                        </div>
                        {#if alert.summary || alert.message}
                          <div class="report-line">
                            <span>摘要</span>
                            <strong>{alert.summary || alert.message}</strong>
                          </div>
                        {/if}
                        {#if alert.rule_id}
                          <div class="report-line">
                            <span>规则</span>
                            <strong>{alert.rule_id}</strong>
                          </div>
                        {/if}
                        {#if alertEvidence(alert).length}
                          <div class="report-line">
                            <span>证据</span>
                            {#each alertEvidence(alert) as line}
                              <pre>{line}</pre>
                            {/each}
                          </div>
                        {/if}
                        {#if alertSuggestions(alert).length}
                          <div class="report-line">
                            <span>建议</span>
                            {#each alertSuggestions(alert) as line}
                              <pre>{line}</pre>
                            {/each}
                          </div>
                        {/if}
                        {#if (alert.commands || []).length}
                          <div class="report-line">
                            <span>命令</span>
                            {#each alert.commands as cmd}
                              <pre>$ {cmd}</pre>
                            {/each}
                          </div>
                        {/if}
                      </div>
                    {/if}
                  </div>
                {:else}
                  <div class="empty-filtered-result">当前过滤条件下没有规则命中告警。</div>
                {/each}
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
  .config-export-mini { color: #fde68a; border-color: rgba(251,191,36,.24); }
  .config-import-mini { color: #fbcfe8; border-color: rgba(244,114,182,.24); }
  .import-mini { color: #a7f3d0; border-color: rgba(52,211,153,.2); }
  .hidden-file { display: none; }
  .health-check-btn:hover:not(:disabled) { transform: translateY(-2px); box-shadow: 0 8px 20px rgba(0,0,0,0.2); }
  .health-check-btn:disabled { opacity: 0.7; cursor: not-allowed; }
  .health-close-all { margin-left: 10px; padding: 10px 14px; border-radius: 10px; border: 1px solid rgba(255,255,255,.28); background: rgba(15, 23, 42, .18); color: white; font-size: 12px; font-weight: 800; cursor: pointer; }
  .health-close-all:hover { background: rgba(15, 23, 42, .3); }
  .btn-icon { font-size: 16px; }
  .btn-spinner { position: relative; width: 20px; height: 20px; border-radius: 50%; border: 2px solid rgba(37, 99, 235, 0.18); border-top-color: #2563eb; border-right-color: #10b981; animation: spin 0.58s linear infinite; box-shadow: 0 0 14px rgba(37,99,235,.18); }
  .btn-spinner::before { content: ''; position: absolute; inset: -5px; border-radius: 50%; border: 1px solid rgba(37,99,235,.2); border-left-color: #f59e0b; animation: spinReverse 1.2s linear infinite; }
  .btn-spinner i { position: absolute; left: 50%; top: 50%; width: 2px; height: 9px; background: linear-gradient(#2563eb, transparent); transform-origin: 50% 0; animation: scanNeedle .72s linear infinite; }
  .health-check-btn:has(.btn-spinner) .btn-spinner { animation-play-state: running; }
  @keyframes pulse { 0%, 100% { transform: scale(0.8); opacity: 0.5; } 50% { transform: scale(1.2); opacity: 1; } }
  @keyframes spinReverse { to { transform: rotate(-360deg); } }
  @keyframes scanNeedle { to { transform: rotate(360deg); } }

  .health-loading { position: relative; overflow: hidden; background: linear-gradient(180deg, rgba(7, 13, 29, 0.98), rgba(2, 7, 18, 0.98)); border: 1px solid rgba(34, 211, 238, 0.22); border-radius: 12px; padding: 18px; margin-top: 12px; box-shadow: 0 18px 48px rgba(15, 23, 42, 0.24), inset 0 0 42px rgba(34, 211, 238, 0.045); }
  .health-loading::before { content: ''; position: absolute; inset: 0; background: linear-gradient(115deg, transparent 0%, rgba(34,211,238,0.11) 32%, rgba(251,191,36,0.08) 44%, transparent 58%); transform: translateX(-120%); animation: scanSweep 2.1s cubic-bezier(.55,0,.28,1) infinite; pointer-events: none; }
  .health-loading::after { content: ''; position: absolute; right: 18px; top: 18px; bottom: 18px; width: 2px; border-radius: 999px; background: linear-gradient(180deg, transparent, rgba(94,234,212,.8), rgba(251,191,36,.55), transparent); box-shadow: 0 0 24px rgba(45,212,191,.22); animation: verticalProbe 1.35s ease-in-out infinite alternate; pointer-events: none; }
  .scanner-grid { position: absolute; inset: 0; opacity: .2; background-image: linear-gradient(rgba(34,211,238,.15) 1px, transparent 1px), linear-gradient(90deg, rgba(34,211,238,.10) 1px, transparent 1px); background-size: 24px 24px; animation: gridDrift 4.4s linear infinite; pointer-events: none; }
  .health-progress-head { display: flex; align-items: center; gap: 18px; }
  .progress-orb { --p: 0; width: 86px; height: 86px; border-radius: 50%; flex-shrink: 0; display: grid; place-items: center; background: conic-gradient(from 210deg, #22d3ee calc(var(--p) * 1%), rgba(148, 163, 184, 0.16) 0); position: relative; box-shadow: 0 0 26px rgba(34,211,238,.14); }
  .progress-orb::before { content: ''; position: absolute; inset: 5px; border-radius: 50%; border: 1px dashed rgba(251,191,36,.34); animation: spinReverse 2.7s linear infinite; }
  .progress-orb::after { content: ''; position: absolute; inset: -6px; border-radius: 50%; border: 1px solid rgba(34, 211, 238, 0.16); border-top-color: rgba(34, 211, 238, 0.72); }
  .progress-orb.complete::after { animation: none; border-color: rgba(52, 211, 153, 0.38); }
  .progress-orb:not(.complete) { animation: orbSpin 1.6s linear infinite; }
  .progress-orb:not(.complete)::after { animation: spin 1.1s linear infinite; }
  .progress-orb-inner { width: 66px; height: 66px; border-radius: 50%; display: grid; place-items: center; background: linear-gradient(180deg, rgba(15,23,42,.96), rgba(2,6,23,.98)); color: #e0f2fe; font-family: var(--theme-font-family-mono); font-size: 17px; font-weight: 900; box-shadow: inset 0 0 18px rgba(34,211,238,.08); }
  .progress-meta { flex: 1; min-width: 0; }
  .progress-title { color: var(--text-primary); font-size: 15px; font-weight: 700; margin-bottom: 6px; }
  .progress-sub { color: var(--text-secondary); font-size: 12px; margin-bottom: 10px; }
  .loading-bar { height: 8px; background: rgba(15,23,42,.82); border: 1px solid rgba(148,163,184,.12); border-radius: 999px; overflow: hidden; box-shadow: inset 0 0 14px rgba(2,6,23,.55); }
  .loading-progress { position: relative; height: 100%; background: linear-gradient(90deg, #22d3ee, #10b981, #f59e0b); background-size: 200% 100%; animation: shimmer 1.4s linear infinite; border-radius: 999px; transition: width .58s cubic-bezier(.22,.9,.28,1); box-shadow: 0 0 18px rgba(34,211,238,.22); }
  .loading-progress::after { content: ''; position: absolute; right: 0; top: -4px; bottom: -4px; width: 18px; background: radial-gradient(circle, rgba(255,255,255,.85), transparent 62%); filter: blur(1px); }
  .loading-logs { margin-top: 14px; max-height: 178px; overflow: auto; padding: 10px; border-radius: 9px; background: #0b1020; border: 1px solid rgba(148, 163, 184, 0.16); }
  .log-line { color: #cbd5e1; font-family: var(--theme-font-family-mono); font-size: 11px; line-height: 1.65; white-space: pre-wrap; overflow-wrap: anywhere; }
  .health-log-panel { margin: 12px 0 0; padding: 12px; max-height: 300px; overflow: auto; border-radius: 10px; border: 1px solid rgba(34,211,238,.16); background: #0b1020; box-shadow: inset 0 0 24px rgba(34,211,238,.04); }
  .health-log-head { display: flex; justify-content: space-between; align-items: center; gap: 10px; margin-bottom: 8px; }
  .health-log-title { color: #67e8f9; font-size: 12px; font-weight: 800; }
  .loading-actions { position: relative; z-index: 1; display: flex; justify-content: flex-end; margin-top: 12px; }
  .show-log-btn, .modal-log-toggle { min-height: 30px; padding: 0 11px; border-radius: 8px; border: 1px solid rgba(34,211,238,.24); background: rgba(34,211,238,.08); color: #67e8f9; font-size: 12px; font-weight: 800; cursor: pointer; }
  .show-log-btn:hover, .modal-log-toggle:hover { background: rgba(34,211,238,.14); border-color: rgba(34,211,238,.38); }
  .modal-log-toggle:disabled { opacity: .55; cursor: not-allowed; }
  .pdf-action { border-color: rgba(52,211,153,.28); background: rgba(52,211,153,.09); color: #86efac; }
  .raw-action { border-color: rgba(251,191,36,.28); background: rgba(251,191,36,.09); color: #fde68a; }
  .pdf-export-status { max-width: 180px; color: #86efac; font-size: 11px; font-family: var(--theme-font-family-mono); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
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
  @keyframes verticalProbe { from { transform: translateY(-8px); opacity: .5; } to { transform: translateY(8px); opacity: 1; } }

  .health-result-panel { background: var(--bg-card); border: 1px solid var(--border-primary); border-radius: 16px; margin-top: 16px; overflow: hidden; }
  .result-header { display: flex; align-items: center; justify-content: space-between; padding: 20px 24px; border-bottom: 1px solid var(--border-primary); }
  .result-status { display: flex; align-items: center; gap: 10px; font-size: 20px; font-weight: 700; }
  .result-icon { font-size: 24px; }
  .result-stats { display: flex; gap: 20px; }
  .stat-item { text-align: center; }
  .stat-button { min-width: 76px; padding: 6px 10px; border: 1px solid transparent; border-radius: 10px; background: transparent; cursor: pointer; }
  .stat-button:hover, .stat-button.active { border-color: rgba(34,211,238,.28); background: rgba(34,211,238,.08); }
  .stat-value { display: block; font-size: 24px; font-weight: 700; color: var(--text-primary); font-family: var(--theme-font-family-mono); }
  .stat-label { font-size: 12px; color: var(--text-secondary); }
  .stat-warn .stat-value { color: #f59e0b; }
  .stat-error .stat-value { color: #ef4444; }
  .result-close { background: none; border: none; color: var(--text-tertiary); font-size: 20px; cursor: pointer; padding: 4px 8px; border-radius: 6px; }
  .result-close:hover { background: var(--bg-hover); }

  .result-checks { padding: 12px; }
  .report-section-title { display: flex; align-items: center; gap: 8px; margin: 2px 0 10px; color: var(--text-primary); font-size: 13px; font-weight: 900; }
  .active-filter { padding: 2px 7px; border-radius: 999px; background: rgba(251,191,36,.1); color: #fbbf24; font-size: 10px; font-family: var(--theme-font-family-mono); }
  .empty-filtered-result { padding: 22px 12px; border-radius: 10px; border: 1px dashed var(--border-primary); color: var(--text-secondary); text-align: center; font-size: 12px; }
  .result-check-item { display: block; width: 100%; background: linear-gradient(135deg, rgba(15,23,42,.78), rgba(2,6,23,.62)); border: 1px solid rgba(148,163,184,.16); border-radius: 12px; margin-bottom: 10px; overflow: hidden; text-decoration: none; color: inherit; transition: border-color 0.15s, background 0.15s, box-shadow .15s; }
  .result-check-item:hover, .result-check-item.check-open { border-color: rgba(34,211,238,.46); box-shadow: 0 12px 30px rgba(2,6,23,.18), inset 0 0 28px rgba(34,211,238,.04); }
  .result-check-item.check-ok { border-left: 3px solid #10b981; }
  .result-check-item.check-warn { border-left: 3px solid #f59e0b; }
  .result-check-item.check-error { border-left: 3px solid #ef4444; }
  .check-main { display: flex; align-items: center; gap: 10px; padding: 12px 14px; width: 100%; border: 0; background: transparent; color: inherit; text-align: left; cursor: pointer; }
  .check-status-dot { width: 10px; height: 10px; border-radius: 50%; flex-shrink: 0; }
  .check-name { flex: 1; display: grid; gap: 2px; font-size: 14px; font-weight: 600; color: var(--text-primary); }
  .check-name em { color: var(--text-tertiary); font-style: normal; font-family: var(--theme-font-family-mono); font-size: 10px; font-weight: 500; }
  .check-findings { flex-shrink: 0; padding: 3px 7px; border-radius: 999px; color: #f59e0b; background: rgba(245, 158, 11, 0.08); border: 1px solid rgba(245, 158, 11, 0.16); font-size: 11px; font-family: var(--theme-font-family-mono); }
  .check-duration { font-size: 12px; color: var(--text-tertiary); font-family: var(--theme-font-family-mono); }
  .check-arrow { color: var(--text-tertiary); font-size: 14px; transition: all 0.2s; }
  .result-check-item:hover .check-arrow { color: var(--accent-primary); }
  .result-alerts { padding: 0 12px 12px; }
  .result-alerts-title { margin: 4px 0 8px; color: var(--text-primary); font-size: 13px; font-weight: 800; }
  .result-alert-item { display: block; margin-bottom: 6px; border: 1px solid var(--border-primary); border-radius: 9px; background: var(--bg-secondary); color: inherit; text-decoration: none; transition: border-color .16s ease, background .16s ease; overflow: hidden; }
  .result-alert-item:hover, .result-alert-item.alert-open { border-color: rgba(245, 158, 11, .45); }
  .alert-main { display: grid; grid-template-columns: 70px minmax(160px, 1fr) minmax(90px, 160px) minmax(160px, 1.2fr) 24px; gap: 10px; align-items: center; width: 100%; padding: 10px 12px; border: 0; background: transparent; color: inherit; text-align: left; cursor: pointer; }
  .alert-level { width: fit-content; min-width: 46px; padding: 3px 7px; border-radius: 7px; background: rgba(245, 158, 11, .12); color: #fbbf24; font-family: var(--theme-font-family-mono); font-size: 10px; font-weight: 900; text-align: center; text-transform: uppercase; }
  .alert-level.error { background: rgba(239, 68, 68, .13); color: #f87171; }
  .alert-title { min-width: 0; color: var(--text-primary); font-size: 12px; font-weight: 700; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .alert-target, .alert-handling { min-width: 0; color: var(--text-secondary); font-size: 11px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .more-alerts { display: inline-flex; margin-top: 6px; color: var(--accent-primary); font-size: 12px; font-weight: 700; text-decoration: none; }
  .report-detail { margin: 0 12px 12px 36px; padding: 14px; border-radius: 12px; border: 1px solid rgba(34,211,238,.16); background: radial-gradient(circle at top right, rgba(34,211,238,.08), transparent 34%), rgba(2, 6, 23, .34); }
  .alert-report-detail { margin-left: 12px; }
  .report-meta-grid { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 8px; margin-bottom: 10px; }
  .report-meta-grid div { min-width: 0; padding: 8px; border-radius: 7px; background: var(--bg-secondary); border: 1px solid var(--border-secondary); }
  .report-meta-grid span { display: block; margin-bottom: 3px; color: var(--text-tertiary); font-size: 10px; }
  .report-meta-grid strong { display: block; color: var(--text-primary); font-size: 11px; overflow-wrap: anywhere; }
  .report-desc { margin: 0 0 10px; color: var(--text-secondary); font-size: 12px; line-height: 1.55; }
  .report-subsection { margin-top: 10px; padding: 12px; border-radius: 11px; border: 1px solid rgba(148,163,184,.16); background: rgba(15, 23, 42, .58); }
  .report-subsection-head { display: flex; align-items: center; gap: 8px; margin-bottom: 8px; color: var(--text-primary); font-size: 12px; }
  .report-subsection-head em { margin-left: auto; color: #fbbf24; font-style: normal; font-size: 10px; font-family: var(--theme-font-family-mono); }
  .report-issues { display: grid; gap: 8px; margin-bottom: 10px; }
  .report-issue { display: grid; grid-template-columns: 50px minmax(0, 1fr); gap: 8px; padding: 8px; border-radius: 7px; background: rgba(251, 191, 36, 0.06); border-left: 3px solid #fbbf24; }
  .report-items { display: grid; gap: 6px; }
  .report-kv { display: grid; grid-template-columns: minmax(120px, .7fr) minmax(0, 1fr) auto; gap: 10px; align-items: center; padding: 9px 10px; border-radius: 8px; border: 1px solid var(--border-secondary); background: rgba(15, 23, 42, .45); }
  .report-kv span, .report-metric span { color: var(--text-tertiary); font-size: 10px; font-family: var(--theme-font-family-mono); text-transform: uppercase; }
  .report-kv strong { min-width: 0; color: var(--text-primary); font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .report-kv em { font-style: normal; font-size: 10px; font-family: var(--theme-font-family-mono); }
  .report-kv.value-ok, .report-metric.value-ok { border-color: rgba(16,185,129,.26); background: rgba(16,185,129,.07); }
  .report-kv.value-ok strong, .report-kv.value-ok em { color: #34d399; }
  .report-kv.value-warn, .report-metric.value-warn { border-color: rgba(251,191,36,.30); background: rgba(251,191,36,.08); }
  .report-kv.value-warn strong, .report-kv.value-warn em { color: #fbbf24; }
  .report-kv.value-error, .report-metric.value-error { border-color: rgba(248,113,113,.34); background: rgba(248,113,113,.09); }
  .report-kv.value-error strong, .report-kv.value-error em { color: #f87171; }
  .report-kv.value-number strong { color: #67e8f9; font-family: var(--theme-font-family-mono); }
  .report-metric { display: grid; gap: 8px; padding: 10px; border-radius: 8px; border: 1px solid var(--border-secondary); background: rgba(15, 23, 42, .45); }
  .metric-head { display: grid; grid-template-columns: minmax(120px, .7fr) auto auto; gap: 10px; align-items: center; }
  .metric-head strong { color: #67e8f9; font-family: var(--theme-font-family-mono); font-size: 13px; }
  .metric-head em { color: var(--text-tertiary); font-style: normal; font-size: 10px; font-family: var(--theme-font-family-mono); }
  .metric-track { height: 7px; overflow: hidden; border-radius: 999px; background: rgba(148,163,184,.13); }
  .metric-fill { height: 100%; border-radius: inherit; background: linear-gradient(90deg, #22d3ee, #10b981, #fbbf24); box-shadow: 0 0 12px rgba(34,211,238,.18); }
  .report-line { display: grid; gap: 5px; padding: 10px; border-radius: 9px; background: rgba(2,6,23,.38); border: 1px solid rgba(148,163,184,.14); }
  .report-line.line-warn { border-left: 3px solid #fbbf24; }
  .report-line.line-error { border-left: 3px solid #f87171; }
  .report-line span { color: var(--text-tertiary); font-size: 10px; font-family: var(--theme-font-family-mono); text-transform: uppercase; }
  .report-line strong { color: var(--text-primary); font-size: 12px; overflow-wrap: anywhere; }
  .report-line pre { margin: 0; color: var(--text-secondary); font-family: var(--theme-font-family-mono); font-size: 11px; line-height: 1.55; white-space: pre-wrap; overflow-wrap: anywhere; }
  .report-divider { height: 1px; background: var(--border-secondary); margin: 4px 0; }
  .report-table-wrap { overflow-x: auto; border: 1px solid var(--border-secondary); border-radius: 8px; }
  .report-table { width: 100%; min-width: 720px; border-collapse: collapse; font-size: 11px; }
  .report-table th, .report-table td { padding: 6px 8px; border-bottom: 1px solid var(--border-secondary); text-align: left; vertical-align: top; }
  .report-table th { color: var(--text-secondary); background: var(--bg-secondary); }
  .report-table td { color: var(--text-primary); font-family: var(--theme-font-family-mono); overflow-wrap: anywhere; }
  .report-table td.cell-good { color: #34d399; background: rgba(16,185,129,.06); }
  .report-table td.cell-warn { color: #fbbf24; background: rgba(251,191,36,.07); }
  .report-table td.cell-bad { color: #f87171; background: rgba(248,113,113,.08); }
  .report-table td.cell-number { color: #67e8f9; font-variant-numeric: tabular-nums; }

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
  .health-modal .check-main,
  .health-modal .alert-main { color: inherit; }
  .health-modal .stat-value,
  .health-modal .check-name,
  .health-modal .alert-title { color: #f8fafc; }
  .health-modal .stat-label,
  .health-modal .check-duration,
  .health-modal .alert-target,
  .health-modal .alert-handling { color: #94a3b8; }

  :global(.pdf-export-root) { position: fixed; left: 24px; top: 24px; z-index: 999990; width: 980px; max-height: calc(100vh - 48px); overflow: auto; background: #fff; pointer-events: none; }
  :global(.pdf-export-screen) { position: fixed; inset: 0; z-index: 999991; display: grid; place-items: center; background: rgba(2, 6, 23, .86); backdrop-filter: blur(3px); }
  :global(.pdf-export-screen-card) { display: grid; justify-items: center; gap: 12px; min-width: 240px; padding: 22px 26px; border-radius: 14px; border: 1px solid rgba(34,211,238,.28); background: rgba(15,23,42,.94); box-shadow: 0 24px 80px rgba(0,0,0,.38); }
  :global(.pdf-export-screen-spinner) { width: 34px; height: 34px; border-radius: 50%; border: 3px solid rgba(34,211,238,.18); border-top-color: #67e8f9; animation: spin .8s linear infinite; }
  :global(.pdf-export-screen-text) { color: #a7f3d0; font-family: var(--theme-font-family-mono); font-size: 12px; font-weight: 900; }
  :global(.pdf-export-report) { width: 980px; padding: 0; background: #fff; color: #111827; font-family: "Microsoft YaHei", "PingFang SC", Arial, sans-serif; }
  :global(.pdf-report-cover),
  :global(.pdf-report-block) { width: 980px; box-sizing: border-box; padding: 24px 28px; background: #fff; color: #111827; border-bottom: 1px solid #e5e7eb; }
  :global(.pdf-report-cover h1) { margin: 0 0 8px; font-size: 28px; color: #0f172a; }
  :global(.pdf-report-subtitle),
  :global(.pdf-report-meta),
  :global(.pdf-report-desc) { margin: 0 0 12px; color: #4b5563; font-size: 13px; line-height: 1.6; }
  :global(.pdf-summary-grid) { display: grid; grid-template-columns: repeat(4, 1fr); gap: 10px; margin-top: 18px; }
  :global(.pdf-summary-grid div) { padding: 12px; border: 1px solid #d1d5db; border-radius: 10px; background: #f8fafc; }
  :global(.pdf-summary-grid span) { display: block; color: #64748b; font-size: 12px; }
  :global(.pdf-summary-grid strong) { display: block; margin-top: 5px; color: #0f172a; font-size: 20px; }
  :global(.pdf-report-block h2) { margin: 0 0 8px; color: #0f172a; font-size: 20px; }
  :global(.pdf-section) { margin-top: 14px; padding: 12px; border: 1px solid #e5e7eb; border-radius: 10px; background: #fff; }
  :global(.pdf-section h3) { margin: 0 0 8px; color: #111827; font-size: 15px; }
  :global(.pdf-item) { margin-top: 8px; padding: 9px 10px; border-left: 3px solid #94a3b8; border-radius: 8px; background: #f8fafc; }
  :global(.pdf-item-warn) { border-left-color: #f59e0b; background: #fffbeb; }
  :global(.pdf-item-error) { border-left-color: #ef4444; background: #fef2f2; }
  :global(.pdf-item strong) { display: block; color: #111827; font-size: 13px; }
  :global(.pdf-item pre),
  :global(.pdf-alert-item pre) { margin: 6px 0 0; white-space: pre-wrap; overflow-wrap: anywhere; color: #374151; font-family: Consolas, "Microsoft YaHei UI", monospace; font-size: 11px; line-height: 1.5; }
  :global(.pdf-table-wrap) { overflow: hidden; margin-top: 8px; border: 1px solid #d1d5db; border-radius: 8px; }
  :global(.pdf-table-wrap table) { width: 100%; border-collapse: collapse; table-layout: fixed; font-size: 11px; }
  :global(.pdf-table-wrap th),
  :global(.pdf-table-wrap td) { padding: 6px 7px; border-bottom: 1px solid #e5e7eb; text-align: left; vertical-align: top; overflow-wrap: anywhere; }
  :global(.pdf-table-wrap th) { color: #374151; background: #f3f4f6; font-weight: 800; }
  :global(.pdf-table-wrap td) { color: #111827; font-family: Consolas, "Microsoft YaHei UI", monospace; }
  :global(.pdf-alert-item) { margin-top: 10px; padding: 12px; border: 1px solid #fde68a; border-radius: 10px; background: #fffbeb; }
  :global(.pdf-alert-item h3) { margin: 0 0 6px; color: #92400e; font-size: 14px; }
  :global(.pdf-alert-item p) { margin: 4px 0; color: #374151; font-size: 12px; line-height: 1.55; }
  :global(.pdf-suggestion) { color: #065f46 !important; }
  :global(.pdf-export-clone) { width: 980px !important; margin: 0 !important; border: 0 !important; border-radius: 0 !important; overflow: visible !important; background: #fff !important; color: #111827 !important; box-shadow: none !important; }
  :global(.pdf-export-clone .result-header),
  :global(.pdf-export-clone .result-check-item),
  :global(.pdf-export-clone .result-alert-item),
  :global(.pdf-export-clone .report-detail),
  :global(.pdf-export-clone .report-subsection),
  :global(.pdf-export-clone .report-line),
  :global(.pdf-export-clone .report-meta-grid div) { background: #fff !important; color: #111827 !important; border-color: #d1d5db !important; box-shadow: none !important; }
  :global(.pdf-export-clone .result-actions),
  :global(.pdf-export-clone .check-arrow) { display: none !important; }
  :global(.pdf-export-clone .stat-value),
  :global(.pdf-export-clone .check-name),
  :global(.pdf-export-clone .alert-title),
  :global(.pdf-export-clone .report-line strong),
  :global(.pdf-export-clone .report-meta-grid strong),
  :global(.pdf-export-clone .report-section-title),
  :global(.pdf-export-clone .report-subsection-head) { color: #111827 !important; }
  :global(.pdf-export-clone .report-line pre),
  :global(.pdf-export-clone .issue-details),
  :global(.pdf-export-clone .report-table td),
  :global(.pdf-export-clone .check-duration),
  :global(.pdf-export-clone .alert-target),
  :global(.pdf-export-clone .alert-handling),
  :global(.pdf-export-clone .report-desc) { color: #374151 !important; }
  :global(.pdf-export-clone .report-table th) { color: #374151 !important; background: #f3f4f6 !important; }
  :global(.pdf-export-clone .result-alerts) { background: #fff !important; color: #111827 !important; }

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
    .report-meta-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
    .alert-main { grid-template-columns: 62px minmax(0, 1fr) 24px; }
    .alert-target, .alert-handling { display: none; }
    .report-detail { margin-left: 12px; }
  }
</style>
