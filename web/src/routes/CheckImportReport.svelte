<script>
  import { onMount } from 'svelte';

  let report = $state(null);
  let error = $state('');
  let expandedAlerts = $state(new Set());
  let expandedChecks = $state(new Set());
  let expandedSections = $state(new Set());
  let showRaw = $state(false);

  onMount(() => {
    try {
      const raw = sessionStorage.getItem('dm-imported-check-report') || localStorage.getItem('dm-imported-check-report');
      if (!raw) {
        error = '没有找到已导入的检查报告，请先在系统检查页面导入 JSON 文件。';
        return;
      }
      const data = JSON.parse(raw);
      if (!data || !Array.isArray(data.checks)) {
        error = '导入报告格式不正确，缺少 checks 数组。';
        return;
      }
      report = data;
      const firstProblem = anomalyItems(data)[0];
      if (firstProblem) expandedAlerts = new Set([firstProblem.key]);
    } catch (e) {
      error = e.message || String(e);
    }
  });

  function toggle(kind, key) {
    const current = kind === 'alert' ? expandedAlerts : kind === 'check' ? expandedChecks : expandedSections;
    const next = new Set(current);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    if (kind === 'alert') expandedAlerts = next;
    else if (kind === 'check') expandedChecks = next;
    else expandedSections = next;
  }

  function statusText(s) {
    if (s === 'ok') return '正常';
    if (s === 'warn') return '警告';
    if (s === 'error') return '异常';
    return s || '未知';
  }

  function statusClass(s) {
    if (s === 'error') return 'error';
    if (s === 'warn') return 'warn';
    if (s === 'ok') return 'ok';
    return 'info';
  }

  function compactTime(ts) {
    if (!ts) return '-';
    return String(ts).replace('T', ' ').replace(/\.\d+Z?$/, '').replace('Z', '');
  }

  function valueText(value) {
    if (value === null || value === undefined || value === '') return '-';
    if (Array.isArray(value)) return value.length ? value.join(' / ') : '-';
    if (typeof value === 'object') return JSON.stringify(value);
    return String(value);
  }

  function itemTitle(item) {
    if (!item) return '未知项目';
    if (item.type === 'finding') return item.title || item.summary || item.rule_id || '规则命中';
    return item.title || item.key || item.name || item.text || item.summary || item.type || '检查数据';
  }

  function itemSummary(item) {
    if (!item) return '-';
    if (item.type === 'finding') return item.summary || item.message || item.suggestion || '-';
    if (item.type === 'label') return `${item.key || '指标'}: ${valueText(item.value)}`;
    if (item.type === 'bar') return `${item.key || '指标'}: ${valueText(item.value)}${item.unit || ''}`;
    return item.summary || item.message || item.text || item.details || valueText(item.value);
  }

  function itemLevel(item) {
    if (!item) return 'info';
    if (item.level === 'error' || item.status === 'error' || item.type === 'error') return 'error';
    if (item.level === 'warn' || item.status === 'warn' || item.type === 'warning' || item.type === 'finding') return 'warn';
    if (item.status === 'ok' || item.type === 'success') return 'ok';
    return 'info';
  }

  function isAnomalyItem(item) {
    const level = itemLevel(item);
    return level === 'error' || level === 'warn';
  }

  function alertTarget(alert) {
    return alert.service_name || alert.target || alert.pid || alert.log_path || alert.type || '系统';
  }

  function alertSummary(alert) {
    return alert.summary || alert.message || alert.title || alert.handling || '未提供概要';
  }

  function checkAnomalyCount(check) {
    return (check.sections || []).flatMap(s => s.items || []).filter(isAnomalyItem).length;
  }

  function anomalyItems(data) {
    const result = [];
    for (const [index, alert] of (data?.alerts || []).entries()) {
      result.push({
        key: `alert-${index}-${alert.id || alert.rule_id || alertSummary(alert)}`,
        source: '告警',
        level: alert.level || 'warn',
        title: alert.title || alertSummary(alert),
        summary: alertSummary(alert),
        target: alertTarget(alert),
        time: alert.last_seen || alert.timestamp || alert.first_seen || data?.exported_at,
        suggestion: alert.handling || (alert.suggestions || [])[0] || alert.suggestion || '',
        raw: alert,
      });
    }
    for (const check of data?.checks || []) {
      for (const [sectionIndex, section] of (check.sections || []).entries()) {
        for (const [itemIndex, item] of (section.items || []).entries()) {
          if (!isAnomalyItem(item)) continue;
          result.push({
            key: `item-${check.id}-${sectionIndex}-${itemIndex}`,
            source: check.name || check.id,
            level: itemLevel(item),
            title: itemTitle(item),
            summary: itemSummary(item),
            target: item.target || item.key || section.title || check.name,
            time: check.timestamp || data?.exported_at,
            suggestion: item.suggestion || item.handling || '',
            raw: item,
          });
        }
      }
    }
    return result;
  }

  let anomalies = $derived.by(() => anomalyItems(report));
  let summary = $derived.by(() => report?.summary || {});
  let warningCount = $derived.by(() => summary.warnings ?? report?.checks?.reduce((n, c) => n + (c.warning_count || 0), 0) ?? 0);
  let errorCount = $derived.by(() => summary.errors ?? report?.checks?.reduce((n, c) => n + (c.error_count || 0), 0) ?? 0);
</script>

<div class="import-report-page">
  {#if error}
    <div class="empty-report">
      <div class="empty-mark">IMPORT</div>
      <h2>无法展示导入报告</h2>
      <p>{error}</p>
      <a class="back-link" href="#/checks">返回系统检查</a>
    </div>
  {:else if report}
    <div class="report-hero">
      <div>
        <div class="eyebrow">Imported Check Report</div>
        <h2>全部检查导入报告</h2>
        <p>文件 {report.imported_file || '-'}，导出时间 {compactTime(report.exported_at)}，导入时间 {compactTime(report.imported_at)}</p>
      </div>
      <div class="hero-status {statusClass(report.overall_status)}">
        <span>{statusText(report.overall_status)}</span>
        <strong>{report.total || report.checks?.length || 0}</strong>
        <small>检查项</small>
      </div>
    </div>

    <div class="metric-grid">
      <div class="metric-card total"><span>检查项</span><strong>{report.total || report.checks?.length || 0}</strong></div>
      <div class="metric-card warn"><span>警告</span><strong>{warningCount}</strong></div>
      <div class="metric-card error"><span>错误</span><strong>{errorCount}</strong></div>
      <div class="metric-card alert"><span>异常线索</span><strong>{anomalies.length}</strong></div>
      <div class="metric-card"><span>规则告警</span><strong>{report.alerts?.length || 0}</strong></div>
    </div>

    <section class="anomaly-section">
      <div class="section-title">
        <div>
          <h3>异常重点</h3>
          <p>优先聚合导入文件内的告警、规则命中、警告和错误项。</p>
        </div>
        <span>{anomalies.length} 条</span>
      </div>
      {#if anomalies.length === 0}
        <div class="clean-panel">导入报告内未发现异常项。仍可在下方展开每个检查项查看完整数据。</div>
      {:else}
        <div class="anomaly-list">
          {#each anomalies as item}
            <div class="anomaly-card {statusClass(item.level)}">
              <button class="anomaly-main" onclick={() => toggle('alert', item.key)}>
                <span class="level">{statusText(item.level)}</span>
                <span class="time">{compactTime(item.time)}</span>
                <span class="title">{item.title}</span>
                <span class="target" title={item.target}>{item.target}</span>
                <span class="source">{item.source}</span>
                <span class="chevron">{expandedAlerts.has(item.key) ? '收起' : '详情'}</span>
              </button>
              {#if expandedAlerts.has(item.key)}
                <div class="anomaly-detail">
                  <div class="summary-text">{item.summary}</div>
                  {#if item.suggestion}
                    <div class="suggestion-text"><b>处理建议</b>{item.suggestion}</div>
                  {/if}
                  <pre>{JSON.stringify(item.raw, null, 2)}</pre>
                </div>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </section>

    <section class="checks-section">
      <div class="section-title">
        <div>
          <h3>检查项完整渲染</h3>
          <p>每个检查项的章节、表格、指标、发现项和原始字段均可展开查看。</p>
        </div>
      </div>

      <div class="check-list">
        {#each report.checks as check}
          <div class="check-card {statusClass(check.status)}">
            <button class="check-head" onclick={() => toggle('check', check.id)}>
              <span class="status-dot"></span>
              <span class="check-name">{check.name || check.id}</span>
              <span class="check-meta">{check.category || '-'} · {compactTime(check.timestamp)}</span>
              <span class="check-counts">{check.warning_count || 0} 警告 / {check.error_count || 0} 错误 / {check.section_count || check.sections?.length || 0} 章节</span>
              <span class="check-duration">{check.duration_ms ?? '-'}ms</span>
            </button>
            {#if expandedChecks.has(check.id)}
              <div class="check-body">
                <div class="check-desc">{check.description || '无描述'} · 版本 {check.version || '-'}</div>
                {#each (check.sections || []) as section, sectionIndex}
                  <div class="section-card">
                    <button class="section-head" onclick={() => toggle('section', `${check.id}-${sectionIndex}`)}>
                      <span>{section.title || `章节 ${sectionIndex + 1}`}</span>
                      <small>{(section.items || []).length} 项数据，{(section.items || []).filter(isAnomalyItem).length} 项异常</small>
                    </button>
                    {#if expandedSections.has(`${check.id}-${sectionIndex}`)}
                      <div class="item-grid">
                        {#each (section.items || []) as item, itemIndex}
                          <div class="data-item {statusClass(itemLevel(item))}">
                            <div class="data-title">{itemTitle(item)}</div>
                            <div class="data-summary">{itemSummary(item)}</div>
                            {#if item.rows?.length}
                              <div class="table-wrap">
                                <table>
                                  {#if item.headers?.length}
                                    <thead><tr>{#each item.headers as head}<th>{head}</th>{/each}</tr></thead>
                                  {/if}
                                  <tbody>
                                    {#each item.rows as row}
                                      <tr>{#each row as cell}<td>{valueText(cell)}</td>{/each}</tr>
                                    {/each}
                                  </tbody>
                                </table>
                              </div>
                            {/if}
                            <details>
                              <summary>原始字段</summary>
                              <pre>{JSON.stringify(item, null, 2)}</pre>
                            </details>
                          </div>
                        {/each}
                      </div>
                    {/if}
                  </div>
                {/each}
                <details class="raw-check">
                  <summary>检查项原始 JSON</summary>
                  <pre>{JSON.stringify(check, null, 2)}</pre>
                </details>
              </div>
            {/if}
          </div>
        {/each}
      </div>
    </section>

    <section class="raw-section">
      <button class="raw-toggle" onclick={() => showRaw = !showRaw}>{showRaw ? '收起原始导入数据' : '查看原始导入数据'}</button>
      {#if showRaw}
        <pre class="raw-json">{JSON.stringify(report, null, 2)}</pre>
      {/if}
    </section>
  {/if}
</div>

<style>
  .import-report-page { width: 100%; max-width: none; margin: 0; }
  .report-hero { position: relative; overflow: hidden; display: flex; justify-content: space-between; gap: 20px; padding: 22px; border-radius: 14px; border: 1px solid rgba(34,211,238,.22); background: linear-gradient(135deg, rgba(15,23,42,.98), rgba(6,78,59,.36)); box-shadow: 0 24px 70px rgba(0,0,0,.28); }
  .report-hero::before { content: ''; position: absolute; inset: 0; background-image: linear-gradient(rgba(34,211,238,.08) 1px, transparent 1px), linear-gradient(90deg, rgba(34,211,238,.07) 1px, transparent 1px); background-size: 30px 30px; opacity: .4; pointer-events: none; }
  .report-hero > * { position: relative; z-index: 1; }
  .eyebrow { color: #67e8f9; font-family: var(--theme-font-family-mono); font-size: 11px; font-weight: 800; text-transform: uppercase; }
  .report-hero h2 { margin: 6px 0; color: var(--text-primary); font-size: 24px; letter-spacing: 0; }
  .report-hero p { margin: 0; color: var(--text-secondary); font-size: 13px; }
  .hero-status { min-width: 120px; display: grid; place-items: center; padding: 12px 18px; border-radius: 12px; border: 1px solid rgba(148,163,184,.2); background: rgba(15,23,42,.66); }
  .hero-status strong { font-family: var(--theme-font-family-mono); font-size: 28px; color: var(--text-primary); }
  .hero-status span, .hero-status small { color: var(--text-secondary); font-size: 12px; }
  .hero-status.error { border-color: rgba(239,68,68,.38); box-shadow: inset 0 0 24px rgba(239,68,68,.08); }
  .hero-status.warn { border-color: rgba(245,158,11,.38); box-shadow: inset 0 0 24px rgba(245,158,11,.08); }
  .hero-status.ok { border-color: rgba(16,185,129,.38); box-shadow: inset 0 0 24px rgba(16,185,129,.08); }
  .metric-grid { display: grid; grid-template-columns: repeat(5, minmax(0, 1fr)); gap: 10px; margin: 14px 0; }
  .metric-card { padding: 13px 14px; border-radius: 10px; border: 1px solid var(--border-primary); background: var(--bg-card); }
  .metric-card span { display: block; color: var(--text-secondary); font-size: 12px; }
  .metric-card strong { display: block; margin-top: 5px; color: var(--text-primary); font-family: var(--theme-font-family-mono); font-size: 24px; }
  .metric-card.warn strong { color: #fbbf24; }
  .metric-card.error strong { color: #f87171; }
  .metric-card.alert strong { color: #67e8f9; }
  .section-title { display: flex; justify-content: space-between; align-items: flex-end; gap: 16px; margin: 18px 0 10px; }
  .section-title h3 { margin: 0 0 4px; color: var(--text-primary); font-size: 17px; }
  .section-title p { margin: 0; color: var(--text-secondary); font-size: 12px; }
  .section-title span { color: #67e8f9; font-family: var(--theme-font-family-mono); font-weight: 800; }
  .clean-panel, .empty-report { padding: 36px 18px; border-radius: 12px; border: 1px solid var(--border-primary); background: var(--bg-card); color: var(--text-secondary); text-align: center; }
  .empty-report h2 { color: var(--text-primary); margin: 8px 0; }
  .empty-mark { display: inline-flex; padding: 5px 10px; border-radius: 999px; color: #67e8f9; border: 1px solid rgba(34,211,238,.22); font-family: var(--theme-font-family-mono); font-size: 11px; }
  .back-link { color: var(--accent-primary); text-decoration: none; font-weight: 700; }
  .anomaly-list, .check-list { display: flex; flex-direction: column; gap: 8px; }
  .anomaly-card, .check-card { overflow: hidden; border-radius: 11px; border: 1px solid var(--border-primary); background: var(--bg-card); transition: border-color .18s ease, transform .18s ease; }
  .anomaly-card:hover, .check-card:hover { border-color: rgba(34,211,238,.34); }
  .anomaly-card.error, .data-item.error, .check-card.error { border-left: 4px solid #ef4444; }
  .anomaly-card.warn, .data-item.warn, .check-card.warn { border-left: 4px solid #f59e0b; }
  .anomaly-card.ok, .data-item.ok, .check-card.ok { border-left: 4px solid #10b981; }
  .anomaly-main { width: 100%; display: grid; grid-template-columns: 70px 150px minmax(220px, 1.2fr) minmax(130px, .7fr) 120px 58px; gap: 10px; align-items: center; padding: 12px 14px; border: none; background: transparent; color: inherit; cursor: pointer; text-align: left; }
  .level { width: fit-content; min-width: 48px; padding: 4px 7px; border-radius: 7px; background: rgba(245,158,11,.1); color: #fbbf24; font-family: var(--theme-font-family-mono); font-size: 10px; font-weight: 900; text-align: center; }
  .anomaly-card.error .level { background: rgba(239,68,68,.12); color: #f87171; }
  .time, .source, .target, .check-meta, .check-counts, .check-duration { min-width: 0; color: var(--text-secondary); font-family: var(--theme-font-family-mono); font-size: 11px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .title, .check-name { min-width: 0; color: var(--text-primary); font-size: 13px; font-weight: 800; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .chevron { color: #67e8f9; font-size: 12px; text-align: right; }
  .anomaly-detail { padding: 0 14px 14px 94px; }
  .summary-text, .suggestion-text { margin: 8px 0; padding: 9px 10px; border-radius: 8px; border: 1px solid var(--border-secondary); background: var(--bg-secondary); color: var(--text-secondary); font-size: 12px; line-height: 1.55; }
  .suggestion-text b { display: block; margin-bottom: 4px; color: #67e8f9; }
  pre { margin: 8px 0 0; padding: 10px; border-radius: 8px; border: 1px solid rgba(148,163,184,.14); background: #070b14; color: #cbd5e1; font-family: var(--theme-font-family-mono); font-size: 11px; line-height: 1.55; white-space: pre-wrap; overflow-wrap: anywhere; }
  .check-head { width: 100%; display: grid; grid-template-columns: 14px minmax(160px, .9fr) minmax(180px, 1fr) minmax(180px, .9fr) 80px; gap: 10px; align-items: center; padding: 13px 14px; border: none; background: transparent; color: inherit; cursor: pointer; text-align: left; }
  .status-dot { width: 9px; height: 9px; border-radius: 50%; background: #94a3b8; }
  .check-card.ok .status-dot { background: #10b981; box-shadow: 0 0 0 3px rgba(16,185,129,.12); }
  .check-card.warn .status-dot { background: #f59e0b; box-shadow: 0 0 0 3px rgba(245,158,11,.12); }
  .check-card.error .status-dot { background: #ef4444; box-shadow: 0 0 0 3px rgba(239,68,68,.12); }
  .check-body { padding: 0 14px 14px 28px; }
  .check-desc { margin: 2px 0 12px; color: var(--text-secondary); font-size: 12px; }
  .section-card { margin-top: 8px; border: 1px solid var(--border-secondary); border-radius: 9px; background: var(--bg-secondary); overflow: hidden; }
  .section-head { width: 100%; display: flex; justify-content: space-between; gap: 12px; padding: 10px 12px; border: none; background: transparent; color: var(--text-primary); font-weight: 800; cursor: pointer; text-align: left; }
  .section-head small { color: var(--text-secondary); font-weight: 500; }
  .item-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 8px; padding: 10px; }
  .data-item { padding: 10px; border-radius: 8px; border: 1px solid var(--border-secondary); background: var(--bg-card); }
  .data-title { color: var(--text-primary); font-size: 13px; font-weight: 800; margin-bottom: 5px; }
  .data-summary { color: var(--text-secondary); font-size: 12px; line-height: 1.5; overflow-wrap: anywhere; }
  details { margin-top: 8px; }
  summary { color: #67e8f9; cursor: pointer; font-size: 12px; font-weight: 700; }
  .table-wrap { max-width: 100%; overflow: auto; margin-top: 8px; border-radius: 8px; border: 1px solid var(--border-secondary); }
  table { width: 100%; border-collapse: collapse; min-width: 420px; }
  th, td { padding: 7px 8px; border-bottom: 1px solid var(--border-secondary); color: var(--text-secondary); font-size: 11px; text-align: left; }
  th { color: var(--text-primary); background: var(--bg-secondary); }
  .raw-check { margin-top: 10px; }
  .raw-section { margin-top: 18px; }
  .raw-toggle { padding: 8px 12px; border-radius: 8px; border: 1px solid rgba(34,211,238,.2); background: var(--bg-card); color: #67e8f9; cursor: pointer; font-size: 12px; font-weight: 800; }
  .raw-json { max-height: 560px; overflow: auto; }
  @media (max-width: 900px) {
    .report-hero { flex-direction: column; }
    .metric-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
    .anomaly-main, .check-head { grid-template-columns: 1fr; }
    .anomaly-detail { padding: 0 12px 12px; }
  }
</style>
