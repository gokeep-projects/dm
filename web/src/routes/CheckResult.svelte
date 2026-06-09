<script>
  import { onMount } from 'svelte';

  let { id } = $props();
  let result = $state(null);
  let loading = $state(true);
  let error = $state(null);
  let expandedIssues = $state(new Set());
  let expandedItems = $state(new Set());
  let expandedTables = $state(new Set());
  let expandedRaw = $state(false);
  let tableSort = $state({});
  let checkConfig = $state({});
  let configLoading = $state(false);
  let configSaving = $state(false);
  let configMessage = $state('');
  let configPrompt = $state('');
  let configFilePath = $state('');
  let showConfig = $state(false);
  let lastLoadedId = $state('');

  async function load() {
    if (!id) return;
    loading = true;
    error = null;
    result = null;
    expandedIssues = new Set();
    expandedItems = new Set();
    expandedTables = new Set();
    expandedRaw = false;
    try {
      const r = await fetch('/api/checks/' + encodeURIComponent(id) + '?ts=' + Date.now(), { cache: 'no-store' });
      if (!r.ok) { error = '检查失败'; loading = false; return; }
      result = await r.json();
    } catch (e) { error = e.message; }
    loading = false;
  }

  function statusIcon(s) {
    if (s === 'ok') return 'OK';
    if (s === 'warn') return 'WARN';
    if (s === 'error') return 'FAIL';
    return 'INFO';
  }

  function statusText(s) {
    if (s === 'ok') return '正常';
    if (s === 'warn') return '警告';
    if (s === 'error') return '异常';
    return '信息';
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

  function toggleSet(setName, key) {
    const next = new Set(setName === 'issues' ? expandedIssues : setName === 'tables' ? expandedTables : expandedItems);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    if (setName === 'issues') expandedIssues = next;
    else if (setName === 'tables') expandedTables = next;
    else expandedItems = next;
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

  function changeTableSort(key, columnIndex) {
    const current = tableSort[key];
    const dir = current?.column === columnIndex && current?.dir === 'asc' ? 'desc' : 'asc';
    tableSort = { ...tableSort, [key]: { column: columnIndex, dir } };
  }

  function tableSortMark(key, columnIndex) {
    const current = tableSort[key];
    if (!current || current.column !== columnIndex) return '';
    return current.dir === 'asc' ? ' ↑' : ' ↓';
  }

  function sortedRows(item, key) {
    const rows = item.rows || [];
    const sort = tableSort[key];
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

  function itemSeverity(item) {
    if (item.type === 'finding') return item.level === 'error' ? 'error' : 'warn';
    if (item.type === 'error') return 'error';
    if (item.type === 'warning') return 'warn';
    if (item.status === 'error') return 'error';
    if (item.status === 'warn') return 'warn';
    return null;
  }

  function itemTitle(item) {
    if (item.type === 'finding') return `${item.title}: ${item.summary}`;
    if (item.type === 'label') return `${item.key}: ${item.value}`;
    if (item.type === 'bar') return `${item.key}: ${Number(item.value || 0).toFixed(1)}${item.unit || ''}`;
    if (item.type === 'warning' || item.type === 'error' || item.type === 'info' || item.type === 'success') return item.text || '';
    return item.key || item.title || item.type;
  }

  function itemDetails(item) {
    if (item.details) return item.details;
    if (item.type === 'finding') {
      const evidence = (item.evidence || []).map(v => `- ${v}`).join('\n');
      const commands = (item.commands || []).map(v => `$ ${v}`).join('\n');
      return [
        `规则: ${item.rule_id}`,
        `对象: ${item.target || '-'}`,
        `分类: ${item.category || '-'}`,
        `概要: ${item.summary || '-'}`,
        evidence ? `证据:\n${evidence}` : '',
        item.suggestion ? `处理建议: ${item.suggestion}` : '',
        commands ? `建议命令:\n${commands}` : '',
      ].filter(Boolean).join('\n');
    }
    if (item.type === 'bar') return `当前值: ${Number(item.value || 0).toFixed(2)}${item.unit || ''}\n阈值上限: ${item.max ?? '-'}${item.unit || ''}\n状态: ${statusIcon(item.status)}`;
    if (item.type === 'label') return `检查项: ${item.key}\n当前值: ${item.value}\n状态: ${statusIcon(item.status)}`;
    return item.text || '';
  }

  function sectionIssues(section) {
    return (section.items || [])
      .map((item, index) => ({ item, index, severity: itemSeverity(item), title: itemTitle(item), details: itemDetails(item) }))
      .filter(issue => issue.severity);
  }

  function sectionCounts(section) {
    const issues = sectionIssues(section);
    return {
      warn: issues.filter(i => i.severity === 'warn').length,
      error: issues.filter(i => i.severity === 'error').length,
      total: issues.length,
    };
  }

  function tableRows(item, key) {
    const rows = sortedRows(item, key);
    if (item.status === 'collapsed' && !expandedTables.has(key)) return [];
    if (expandedTables.has(key) || rows.length <= 12) return rows;
    return rows.slice(0, 12);
  }

  function tableNeedsToggle(item) {
    return item.status === 'collapsed' || (item.rows || []).length > 12;
  }

  function tableToggleText(item, key) {
    if (expandedTables.has(key)) return '收起';
    if (item.status === 'collapsed') return `展开全部 ${item.rows?.length || 0} 行`;
    return `展开全部 ${item.rows?.length || 0} 行`;
  }

  const configurableChecks = new Set(['elasticsearch', 'redis', 'nginx', 'keepalived', 'mysql', 'kafka', 'java-service']);

  function configFields(checkId) {
    const commonPaths = [
      { key: 'config_path', label: '配置路径', type: 'text', placeholder: '/etc/xxx/xxx.conf' },
      { key: 'data_path', label: '数据路径', type: 'text', placeholder: '/var/lib/xxx' },
      { key: 'log_path', label: '日志路径', type: 'text', placeholder: '/var/log/xxx/xxx.log' },
      { key: 'program_path', label: '程序路径', type: 'text', placeholder: '/usr/sbin/xxx' },
    ];
    if (checkId === 'elasticsearch') return [
      { key: 'url', label: '访问地址', type: 'text', placeholder: 'http://127.0.0.1:9200' },
      { key: 'host', label: '主机', type: 'text', placeholder: '127.0.0.1' },
      { key: 'port', label: '端口', type: 'text', placeholder: '9200' },
      { key: 'username', label: '用户名', type: 'text', placeholder: 'elastic' },
      { key: 'password', label: '密码', type: 'password', placeholder: '留空表示不修改已保存密码' },
      ...commonPaths,
    ];
    if (checkId === 'redis') return [
      { key: 'host', label: '主机', type: 'text', placeholder: '127.0.0.1' },
      { key: 'port', label: '端口', type: 'text', placeholder: '6379' },
      { key: 'password', label: '密码', type: 'password', placeholder: '留空表示不修改已保存密码' },
      ...commonPaths,
    ];
    if (checkId === 'mysql') return [
      { key: 'host', label: '主机', type: 'text', placeholder: '127.0.0.1' },
      { key: 'port', label: '端口', type: 'text', placeholder: '3306' },
      { key: 'username', label: '用户名', type: 'text', placeholder: 'root' },
      { key: 'password', label: '密码', type: 'password', placeholder: '留空表示不修改已保存密码' },
      ...commonPaths,
    ];
    if (checkId === 'kafka') return [
      { key: 'host', label: '主机', type: 'text', placeholder: '127.0.0.1' },
      { key: 'port', label: '端口', type: 'text', placeholder: '9092' },
      ...commonPaths,
    ];
    if (checkId === 'java-service') return [
      { key: 'service_prefix', label: '服务前缀', type: 'text', placeholder: 'order-,pay-,gateway' },
      { key: 'log_path', label: '日志路径', type: 'text', placeholder: '/data/logs 或 /var/log' },
      { key: 'program_path', label: '程序路径', type: 'text', placeholder: '/data/apps' },
    ];
    if (checkId === 'nginx') return [
      { key: 'config_path', label: '配置路径', type: 'text', placeholder: '/etc/nginx/nginx.conf' },
      { key: 'log_path', label: '日志路径', type: 'text', placeholder: '/var/log/nginx/error.log' },
      { key: 'program_path', label: '程序路径', type: 'text', placeholder: '/usr/sbin/nginx' },
    ];
    if (checkId === 'keepalived') return [
      { key: 'config_path', label: '配置路径', type: 'text', placeholder: '/etc/keepalived/keepalived.conf' },
      { key: 'log_path', label: '日志路径', type: 'text', placeholder: '/var/log/messages' },
      { key: 'program_path', label: '程序路径', type: 'text', placeholder: '/usr/sbin/keepalived' },
    ];
    return [];
  }

  function hasConfigPanel(checkId) {
    return configurableChecks.has(checkId);
  }

  function isBlank(value) {
    return String(value ?? '').trim() === '';
  }

  function requiredConfigMissing(checkId, cfg = checkConfig) {
    if (checkId === 'elasticsearch') {
      const hasUrl = !isBlank(cfg.url);
      const hasHostPort = !isBlank(cfg.host) && !isBlank(cfg.port);
      return hasUrl || hasHostPort ? [] : ['访问地址，或主机 + 端口'];
    }
    if (checkId === 'redis' || checkId === 'kafka') {
      const missing = [];
      if (isBlank(cfg.host)) missing.push('主机');
      if (isBlank(cfg.port)) missing.push('端口');
      return missing;
    }
    if (checkId === 'mysql') {
      const missing = [];
      if (isBlank(cfg.host)) missing.push('主机');
      if (isBlank(cfg.port)) missing.push('端口');
      if (isBlank(cfg.username)) missing.push('用户名');
      return missing;
    }
    if (checkId === 'java-service') {
      return isBlank(cfg.service_prefix) ? ['服务前缀'] : [];
    }
    return [];
  }

  function isRequiredField(checkId, key) {
    if (checkId === 'elasticsearch') return key === 'url' || key === 'host' || key === 'port';
    if (checkId === 'redis' || checkId === 'kafka') return key === 'host' || key === 'port';
    if (checkId === 'mysql') return key === 'host' || key === 'port' || key === 'username';
    if (checkId === 'java-service') return key === 'service_prefix';
    return false;
  }

  function refreshConfigPrompt(cfg = checkConfig) {
    const missing = requiredConfigMissing(id, cfg);
    if (missing.length) {
      configPrompt = `缺少关键连接配置：${missing.join('、')}。请先补充关键项，路径等辅助信息可以留空。`;
      showConfig = true;
    } else {
      configPrompt = '';
    }
  }

  function configSummary() {
    const missing = requiredConfigMissing(id);
    if (missing.length) return '需要配置';
    if (id === 'elasticsearch') return checkConfig.url || [checkConfig.host, checkConfig.port].filter(Boolean).join(':') || '已配置';
    if (id === 'redis' || id === 'mysql' || id === 'kafka') return [checkConfig.host, checkConfig.port].filter(Boolean).join(':') || '已配置';
    if (id === 'java-service') return checkConfig.service_prefix || '已配置';
    return '可选配置';
  }

  async function loadConfig() {
    if (!hasConfigPanel(id)) return;
    configLoading = true;
    configMessage = '';
    try {
      const r = await fetch('/api/check-configs/' + encodeURIComponent(id), { cache: 'no-store' });
      if (r.ok) {
        const d = await r.json();
        checkConfig = d.value || {};
        configFilePath = d.config_file || '';
        refreshConfigPrompt(checkConfig);
      } else {
        configMessage = '配置加载失败';
      }
    } catch (e) {
      configMessage = e.message;
    }
    configLoading = false;
  }

  async function saveConfig() {
    if (!hasConfigPanel(id)) return;
    configSaving = true;
    configMessage = '';
    const payload = {};
    for (const field of configFields(id)) {
      payload[field.key] = String(checkConfig[field.key] ?? '').trim();
    }
    const missing = requiredConfigMissing(id, payload);
    if (missing.length) {
      configPrompt = `缺少关键连接配置：${missing.join('、')}。`;
      showConfig = true;
      configSaving = false;
      return;
    }
    try {
      const r = await fetch('/api/check-configs/' + encodeURIComponent(id), {
        method: 'PUT',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(payload),
      });
      if (!r.ok) {
        configMessage = '配置保存失败';
      } else {
        const d = await r.json();
        checkConfig = d.value || payload;
        configFilePath = d.config_file || configFilePath;
        refreshConfigPrompt(checkConfig);
        configMessage = `配置已保存并同步到文件${configFilePath ? ': ' + configFilePath : ''}，正在刷新最新检查结果`;
        await load();
      }
    } catch (e) {
      configMessage = e.message;
    }
    configSaving = false;
  }

  onMount(() => {
    lastLoadedId = id;
    loadConfig();
    load();
  });

  $effect(() => {
    if (!id || id === lastLoadedId) return;
    lastLoadedId = id;
    checkConfig = {};
    configMessage = '';
    configPrompt = '';
    configFilePath = '';
    showConfig = false;
    loadConfig();
    load();
  });
</script>

<div class="check-page">
  <div class="check-header">
    <div class="header-info">
      <a href="#/checks" class="back-btn" aria-label="返回">
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none"><path d="m15 18-6-6 6-6" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>
      </a>
      <div>
        <h1 class="check-title">{result?.name || id}</h1>
        <p class="check-desc">{result?.description || ''}</p>
      </div>
    </div>
    <div class="check-meta">
      {#if result?.duration_ms}
        <span class="meta-badge">耗时 {result.duration_ms}ms</span>
      {/if}
      {#if result?.status}
        <span class="status-badge" style="color:{statusColor(result.status)};border-color:{statusColor(result.status)}33;background:{statusColor(result.status)}15">
          [{statusIcon(result.status)}] {statusText(result.status)}
        </span>
      {/if}
      <button class="action-btn" onclick={load}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 12a9 9 0 0 0-9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/><path d="M3 12a9 9 0 0 0 9 9 9.75 9.75 0 0 0 6.74-2.74L21 16"/><path d="M16 16h5v5"/></svg>
        <span>刷新</span>
      </button>
    </div>
  </div>

  {#if hasConfigPanel(id)}
    <div class="config-panel" class:config-needs={configPrompt}>
      <div class="config-head">
        <div>
          <h2>检查连接配置 <span class="config-summary">{configSummary()}</span></h2>
          <p>{configPrompt || `配置会同步持久化到数据库和连接配置文件${configFilePath ? '：' + configFilePath : ''}，路径等辅助信息可留空，保存后立即使用最新配置重新执行检查。`}</p>
        </div>
        <div class="config-actions">
          <button class="action-btn" onclick={() => showConfig = !showConfig}>
            {showConfig ? '收起配置' : '展开配置'}
          </button>
          {#if showConfig}
            <button class="action-btn primary" onclick={saveConfig} disabled={configSaving || configLoading}>
              {configSaving ? '保存中...' : '保存并刷新'}
            </button>
          {/if}
        </div>
      </div>
      {#if showConfig}
        <div class="config-grid">
          {#each configFields(id) as field}
            <label class="config-field" class:required-field={isRequiredField(id, field.key)}>
              <span>{field.label}{isRequiredField(id, field.key) ? ' *' : ''}</span>
              <input
                type={field.type}
                placeholder={field.placeholder}
                autocomplete={field.type === 'password' ? 'new-password' : 'off'}
                bind:value={checkConfig[field.key]}
                oninput={() => refreshConfigPrompt()}
              />
            </label>
          {/each}
        </div>
        {#if configMessage}
          <div class="config-message">{configMessage}</div>
        {/if}
      {/if}
    </div>
  {/if}

  {#if loading}
    <div class="loading">
      <div class="scan-loader">
        <span></span><span></span><span></span><span></span>
      </div>
      <div class="loading-copy">
        <strong>正在执行检查</strong>
        <span>采集进程、端口、配置、日志并交给规则引擎分析...</span>
      </div>
    </div>
  {:else if error}
    <div class="error-box">
      <span class="error-icon">✗</span>
      <span>{error}</span>
      <button class="retry-btn" onclick={load}>重试</button>
    </div>
  {:else if result}
    <div class="sections">
      {#each result.sections as section, sectionIndex}
        {@const counts = sectionCounts(section)}
        {@const issueKey = `section-${sectionIndex}`}
        <div class="section-card">
          <div class="section-header">
            {#if section.icon}<span class="section-icon">{section.icon}</span>{/if}
            <h2 class="section-title">{section.title}</h2>
            {#if counts.error > 0}
              <span class="issue-chip issue-error">FAIL {counts.error}</span>
            {/if}
            {#if counts.warn > 0}
              <span class="issue-chip issue-warn">WARN {counts.warn}</span>
            {/if}
            {#if counts.total > 0}
              <button class="section-toggle" onclick={() => toggleSet('issues', issueKey)}>
                {expandedIssues.has(issueKey) ? '收起详情' : '展开详情'}
              </button>
            {/if}
          </div>
          {#if section.description}
            <p class="section-desc">{section.description}</p>
          {/if}
          {#if counts.total > 0 && expandedIssues.has(issueKey)}
            <div class="issue-list">
              {#each sectionIssues(section) as issue}
                <div class="issue-row" class:issue-row-error={issue.severity === 'error'}>
                  <span class="issue-level">{statusIcon(issue.severity)}</span>
                  <div class="issue-body">
                    <div class="issue-title">{issue.title}</div>
                    <pre class="issue-details">{issue.details}</pre>
                  </div>
                </div>
              {/each}
            </div>
          {/if}
          <div class="section-items">
            {#each section.items as item, itemIndex}
              {@const itemKey = `item-${sectionIndex}-${itemIndex}`}
              {#if item.type === 'finding'}
                <div class="finding-card" class:finding-error={item.level === 'error'}>
                  <div class="finding-head">
                    <span class="finding-level">{statusIcon(item.level)}</span>
                    <div class="finding-main">
                      <div class="finding-title">{item.title}</div>
                      <div class="finding-summary">{item.summary}</div>
                    </div>
                    <button class="detail-toggle finding-toggle" onclick={() => toggleSet('items', itemKey)}>
                      {expandedItems.has(itemKey) ? '收起' : '详情'}
                    </button>
                  </div>
                  <div class="finding-meta">
                    <span>规则 {item.rule_id}</span>
                    <span>对象 {item.target || '-'}</span>
                    <span>分类 {item.category || '-'}</span>
                  </div>
                  {#if expandedItems.has(itemKey)}
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
                  {/if}
                </div>
              {:else if item.type === 'label'}
                <div class="item-row">
                  <span class="item-key">{item.key}</span>
                  <span class="item-value" style="color:{statusColor(item.status)}">{item.value}</span>
                  {#if item.status}
                    <button class="item-status-btn" style="color:{statusColor(item.status)}" onclick={() => toggleSet('items', itemKey)}>{statusIcon(item.status)}</button>
                  {/if}
                </div>
                {#if expandedItems.has(itemKey)}
                  <pre class="item-details open">{itemDetails(item)}</pre>
                {/if}

              {:else if item.type === 'bar'}
                <div class="item-row item-bar">
                  <span class="item-key">{item.key}</span>
                  <div class="bar-wrap">
                    <div class="bar-track">
                      <div class="bar-fill" style="width:{Math.min(item.value / item.max * 100, 100)}%;background:{barColor(item.status)}"></div>
                    </div>
                    <span class="bar-value" style="color:{barColor(item.status)}">{item.value.toFixed(1)}{item.unit}</span>
                  </div>
                </div>

              {:else if item.type === 'table'}
                <div class="item-table">
                  <table>
                    <thead>
                      <tr>
                        {#each item.headers as h, columnIndex}
                          <th>
                            <button class="table-sort-btn" onclick={() => changeTableSort(itemKey, columnIndex)}>
                              {h}{tableSortMark(itemKey, columnIndex)}
                            </button>
                          </th>
                        {/each}
                      </tr>
                    </thead>
                    <tbody>
                      {#each tableRows(item, itemKey) as row}
                        <tr>
                          {#each row as cell}
                            <td>{cell}</td>
                          {/each}
                        </tr>
                      {/each}
                    </tbody>
                  </table>
                  {#if tableNeedsToggle(item)}
                    <button class="table-toggle" onclick={() => toggleSet('tables', itemKey)}>
                      {tableToggleText(item, itemKey)}
                    </button>
                  {/if}
                </div>

              {:else if item.type === 'sparkline'}
                <div class="item-row item-sparkline">
                  <span class="item-key">{item.key}</span>
                  <div class="sparkline-bars" aria-label="{item.key} 趋势">
                    {#each sparklineBars(item.data) as point}
                      <span class="sparkline-bar" style="height:{point.height}px;background:{barColor(item.status)}" title="{point.value}{item.unit || ''}"></span>
                    {/each}
                  </div>
                  <span class="sparkline-value" style="color:{statusColor(item.status)}">
                    {item.data.length > 0 ? item.data[item.data.length - 1].toFixed(1) : '0'}{item.unit}
                  </span>
                </div>

              {:else if item.type === 'info'}
                <div class="item-msg item-info">[INFO] {item.text}</div>

              {:else if item.type === 'warning'}
                <div class="item-msg item-warn">
                  <span>[WARN] {item.text}</span>
                  {#if item.details}
                    <button class="detail-toggle" onclick={() => toggleSet('items', itemKey)}>{expandedItems.has(itemKey) ? '收起' : '详情'}</button>
                  {/if}
                </div>
                {#if item.details && expandedItems.has(itemKey)}
                  <pre class="item-details open">{item.details}</pre>
                {/if}

              {:else if item.type === 'error'}
                <div class="item-msg item-error">
                  <span>[FAIL] {item.text}</span>
                  {#if item.details}
                    <button class="detail-toggle" onclick={() => toggleSet('items', itemKey)}>{expandedItems.has(itemKey) ? '收起' : '详情'}</button>
                  {/if}
                </div>
                {#if item.details && expandedItems.has(itemKey)}
                  <pre class="item-details open">{item.details}</pre>
                {/if}

              {:else if item.type === 'success'}
                <div class="item-msg item-success">[OK] {item.text}</div>

              {:else if item.type === 'divider'}
                <div class="item-divider"></div>
              {/if}
            {/each}
          </div>
        </div>
      {/each}
    </div>
    <div class="raw-panel">
      <button class="raw-toggle" onclick={() => expandedRaw = !expandedRaw}>
        {expandedRaw ? '收起原始数据' : '查看原始数据'}
      </button>
      {#if expandedRaw}
        <pre class="raw-json">{JSON.stringify(result, null, 2)}</pre>
      {/if}
    </div>
  {/if}
</div>

<style>
  .check-page { max-width: 1200px; margin: 0 auto; }
  .check-header { display: flex; align-items: center; justify-content: space-between; gap: 16px; margin-bottom: 20px; }
  .header-info { display: flex; align-items: center; gap: 12px; }
  .back-btn { color: var(--text-tertiary); text-decoration: none; padding: 8px; border-radius: 10px; transition: all 0.2s; display: flex; }
  .back-btn:hover { color: var(--text-primary); background: var(--bg-hover); }
  .check-title { font-size: 20px; font-weight: 700; color: var(--text-primary); letter-spacing: 0; margin: 0; }
  .check-desc { font-size: 12px; color: var(--text-secondary); margin: 4px 0 0; }
  .check-meta { display: flex; align-items: center; gap: 8px; }
  .meta-badge { font-size: 11px; color: var(--text-secondary); font-family: var(--theme-font-family-mono); padding: 4px 10px; border-radius: 6px; background: var(--bg-secondary); border: 1px solid var(--border-primary); }
  .status-badge { font-size: 11px; font-weight: 600; padding: 4px 10px; border-radius: 6px; border: 1px solid; }
  .action-btn { display: flex; align-items: center; gap: 5px; padding: 6px 12px; background: var(--bg-secondary); border: 1px solid var(--border-primary); border-radius: 8px; color: var(--text-secondary); font-size: 12px; cursor: pointer; transition: all 0.2s; }
  .action-btn:hover { background: var(--bg-hover); color: var(--accent-primary); border-color: var(--border-focus); }
  .action-btn.primary { color: #051014; background: linear-gradient(135deg, #67e8f9, #34d399); border-color: rgba(103,232,249,0.5); font-weight: 700; }
  .action-btn:disabled { opacity: 0.55; cursor: not-allowed; }

  .config-panel { margin: 0 0 16px; padding: 14px; border: 1px solid rgba(34,211,238,0.18); border-radius: 10px; background: linear-gradient(135deg, rgba(34,211,238,0.08), rgba(52,211,153,0.05)); box-shadow: var(--shadow-sm); }
  .config-panel.config-needs { border-color: rgba(251, 191, 36, .38); background: linear-gradient(135deg, rgba(251,191,36,.1), rgba(34,211,238,.05)); }
  .config-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; margin-bottom: 12px; }
  .config-head h2 { margin: 0; color: var(--text-primary); font-size: 14px; font-weight: 700; }
  .config-head p { margin: 4px 0 0; color: var(--text-secondary); font-size: 12px; }
  .config-needs .config-head p { color: #fbbf24; }
  .config-summary { display: inline-flex; margin-left: 6px; padding: 2px 7px; border-radius: 999px; background: var(--bg-secondary); border: 1px solid var(--border-primary); color: var(--accent-primary); font-family: var(--theme-font-family-mono); font-size: 10px; font-weight: 800; vertical-align: middle; }
  .config-needs .config-summary { color: #fbbf24; border-color: rgba(251,191,36,.25); background: rgba(251,191,36,.08); }
  .config-actions { display: flex; align-items: center; gap: 8px; flex-shrink: 0; }
  .config-grid { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 10px; }
  .config-field { display: grid; gap: 5px; min-width: 0; }
  .config-field span { color: var(--text-secondary); font-size: 11px; }
  .config-field.required-field span { color: var(--text-primary); font-weight: 800; }
  .config-field input { min-width: 0; height: 34px; padding: 0 10px; border-radius: 8px; border: 1px solid var(--border-primary); background: var(--bg-secondary); color: var(--text-primary); font-size: 12px; outline: none; }
  .config-field.required-field input { border-color: rgba(34,211,238,.22); }
  .config-field input:focus { border-color: var(--border-focus); box-shadow: 0 0 0 3px rgba(34,211,238,0.12); }
  .config-message { margin-top: 10px; padding: 8px 10px; border-radius: 8px; background: rgba(34,211,238,0.08); color: var(--accent-primary); font-size: 12px; }

  .loading { display: flex; align-items: center; justify-content: center; gap: 16px; padding: 54px 0; color: var(--text-secondary); font-size: 14px; }
  .scan-loader { position: relative; display: grid; grid-template-columns: repeat(2, 34px); gap: 6px; padding: 8px; border: 1px solid rgba(34,211,238,0.2); border-radius: 12px; background: rgba(34,211,238,0.05); overflow: hidden; }
  .scan-loader::after { content: ""; position: absolute; inset: 0; transform: translateY(-100%); background: linear-gradient(180deg, transparent, rgba(103,232,249,0.35), transparent); animation: scan 1.3s ease-in-out infinite; }
  .scan-loader span { width: 34px; height: 24px; border-radius: 6px; background: linear-gradient(135deg, rgba(34,211,238,0.28), rgba(52,211,153,0.16)); animation: pulseCell 1.2s ease-in-out infinite; }
  .scan-loader span:nth-child(2) { animation-delay: .12s; }
  .scan-loader span:nth-child(3) { animation-delay: .24s; }
  .scan-loader span:nth-child(4) { animation-delay: .36s; }
  .loading-copy { display: grid; gap: 4px; }
  .loading-copy strong { color: var(--text-primary); font-size: 14px; }
  .loading-copy span { color: var(--text-secondary); font-size: 12px; }
  @keyframes scan { 0%, 15% { transform: translateY(-100%); } 70%, 100% { transform: translateY(100%); } }
  @keyframes pulseCell { 0%,100% { opacity: .45; transform: scale(.96); } 50% { opacity: 1; transform: scale(1); } }
  @keyframes spin { to { transform: rotate(360deg); } }
  .error-box { display: flex; align-items: center; gap: 12px; padding: 16px; background: rgba(239, 68, 68, 0.08); border: 1px solid rgba(239, 68, 68, 0.15); border-radius: 10px; color: #ef4444; font-size: 13px; }
  .error-icon { font-size: 18px; }
  .retry-btn { margin-left: auto; padding: 4px 10px; background: rgba(239, 68, 68, 0.1); border: 1px solid rgba(239, 68, 68, 0.2); color: #ef4444; border-radius: 6px; font-size: 11px; cursor: pointer; }
  .retry-btn:hover { background: rgba(239, 68, 68, 0.15); }

  .sections { display: flex; flex-direction: column; gap: 14px; }
  .section-card { background: var(--bg-card); border: 1px solid var(--border-primary); border-radius: 10px; padding: 14px; overflow: hidden; box-shadow: var(--shadow-sm); }
  .section-header { display: flex; align-items: center; gap: 8px; margin-bottom: 12px; padding-bottom: 10px; border-bottom: 1px solid var(--border-primary); }
  .section-icon { font-size: 16px; }
  .section-title { font-size: 14px; font-weight: 600; color: var(--text-primary); margin: 0; }
  .section-desc { font-size: 12px; color: var(--text-secondary); margin: 0 0 12px; }
  .section-items { display: flex; flex-direction: column; gap: 6px; }
  .issue-chip { margin-left: 2px; padding: 2px 7px; border-radius: 999px; font-size: 10px; font-weight: 700; font-family: var(--theme-font-family-mono); border: 1px solid; }
  .issue-error { color: #f87171; background: rgba(248, 113, 113, 0.08); border-color: rgba(248, 113, 113, 0.22); }
  .issue-warn { color: #fbbf24; background: rgba(251, 191, 36, 0.08); border-color: rgba(251, 191, 36, 0.22); }
  .section-toggle { margin-left: auto; padding: 4px 9px; border-radius: 6px; border: 1px solid var(--border-primary); background: var(--bg-secondary); color: var(--text-secondary); font-size: 11px; cursor: pointer; }
  .section-toggle:hover { color: var(--text-primary); background: var(--bg-hover); }
  .issue-list { display: flex; flex-direction: column; gap: 8px; margin: 0 0 12px; padding: 10px; background: var(--bg-secondary); border: 1px solid var(--border-primary); border-radius: 8px; }
  .issue-row { display: grid; grid-template-columns: 54px 1fr; gap: 10px; padding: 8px; border-radius: 7px; background: rgba(251, 191, 36, 0.06); border-left: 3px solid #fbbf24; }
  .issue-row-error { background: rgba(248, 113, 113, 0.06); border-left-color: #f87171; }
  .issue-level { font-size: 11px; font-weight: 700; font-family: var(--theme-font-family-mono); color: #fbbf24; }
  .issue-row-error .issue-level { color: #f87171; }
  .issue-body { min-width: 0; }
  .issue-title { color: var(--text-primary); font-size: 12px; font-weight: 600; margin-bottom: 4px; overflow-wrap: anywhere; }
  .issue-details { margin: 0; white-space: pre-wrap; overflow-wrap: anywhere; color: var(--text-secondary); font-size: 11px; line-height: 1.55; font-family: var(--theme-font-family-mono); }

  .item-row { display: flex; align-items: center; gap: 10px; padding: 6px 8px; border-radius: 6px; transition: background 0.15s; min-width: 0; }
  .item-row:hover { background: var(--bg-hover); }
  .item-key { font-size: 12px; color: var(--text-secondary); min-width: 80px; }
  .item-value { font-size: 12px; font-family: var(--theme-font-family-mono); flex: 1; min-width: 0; overflow-wrap: anywhere; word-break: break-word; line-height: 1.6; }
  .item-status-btn { margin-left: auto; padding: 2px 7px; border-radius: 5px; border: 1px solid var(--border-primary); background: var(--bg-secondary); font-size: 10px; font-weight: 700; font-family: var(--theme-font-family-mono); cursor: pointer; }
  .item-status-btn:hover { background: var(--bg-hover); }

  .item-bar { flex-direction: column; align-items: stretch; gap: 4px; }
  .bar-wrap { display: flex; align-items: center; gap: 10px; }
  .bar-track { flex: 1; height: 6px; background: var(--bg-tertiary); border-radius: 3px; overflow: hidden; }
  .bar-fill { height: 100%; border-radius: 3px; transition: width 0.6s cubic-bezier(0.4, 0, 0.2, 1); }
  .bar-value { font-size: 12px; font-family: var(--theme-font-family-mono); font-weight: 600; min-width: 60px; text-align: right; }

  .item-table { overflow-x: auto; }
  .item-table table { width: 100%; border-collapse: collapse; font-size: 12px; }
  .item-table th { text-align: left; padding: 0; color: var(--text-secondary); font-weight: 600; border-bottom: 1px solid var(--border-primary); background: var(--bg-secondary); }
  .table-sort-btn { width: 100%; min-height: 32px; padding: 6px 10px; border: 0; background: transparent; color: inherit; font: inherit; text-align: left; cursor: pointer; white-space: nowrap; }
  .table-sort-btn:hover { color: var(--text-primary); background: var(--bg-hover); }
  .item-table td { padding: 6px 10px; border-bottom: 1px solid var(--border-secondary); color: var(--text-primary); font-family: var(--theme-font-family-mono); }
  .item-table tr:hover td { background: var(--bg-hover); }
  .table-toggle { margin-top: 8px; padding: 5px 10px; border-radius: 6px; border: 1px solid var(--border-primary); background: var(--bg-secondary); color: var(--text-secondary); font-size: 11px; cursor: pointer; }
  .table-toggle:hover { color: var(--text-primary); background: var(--bg-hover); }

  .item-sparkline { display: grid; grid-template-columns: minmax(90px, 160px) minmax(120px, 1fr) minmax(70px, auto); align-items: end; }
  .sparkline-bars { display: flex; align-items: end; gap: 3px; min-height: 72px; padding: 8px 10px; background: var(--bg-secondary); border: 1px solid var(--border-secondary); border-radius: 8px; overflow-x: auto; }
  .sparkline-bar { width: 7px; min-width: 7px; border-radius: 4px 4px 0 0; opacity: 0.78; }
  .sparkline-value { font-size: 14px; font-weight: 700; font-family: var(--theme-font-family-mono); text-align: right; }

  .item-msg { padding: 8px 12px; border-radius: 8px; font-size: 12px; display: flex; align-items: center; gap: 8px; }
  .item-info { background: rgba(34, 211, 238, 0.06); color: #22d3ee; }
  .item-warn { background: rgba(251, 191, 36, 0.08); color: #fbbf24; }
  .item-error { background: rgba(239, 68, 68, 0.08); color: #f87171; }
  .item-success { background: rgba(52, 211, 153, 0.08); color: #34d399; }
  .item-divider { height: 1px; background: var(--border-primary); margin: 4px 0; }
  .detail-toggle { background: none; border: none; color: inherit; cursor: pointer; font-size: 11px; margin-left: auto; padding: 2px 6px; border-radius: 4px; opacity: 0.7; }
  .detail-toggle:hover { opacity: 1; background: rgba(255,255,255,0.1); }
  .item-details { padding: 8px 12px; margin: 4px 0 2px; font-size: 12px; line-height: 1.6; color: var(--text-secondary); background: var(--bg-secondary); border: 1px solid var(--border-primary); border-radius: 6px; white-space: pre-wrap; overflow-wrap: anywhere; font-family: var(--theme-font-family-mono); }
  .item-details.open { display: block; }
  .finding-card { padding: 12px; border-radius: 10px; border: 1px solid rgba(251,191,36,0.22); background: rgba(251,191,36,0.06); }
  .finding-card.finding-error { border-color: rgba(248,113,113,0.24); background: rgba(248,113,113,0.06); }
  .finding-head { display: flex; align-items: flex-start; gap: 10px; }
  .finding-level { flex: 0 0 auto; min-width: 42px; padding: 3px 7px; border-radius: 7px; background: rgba(255,255,255,0.1); font-family: var(--theme-font-family-mono); font-size: 10px; font-weight: 800; color: #fbbf24; text-align: center; }
  .finding-error .finding-level { color: #f87171; }
  .finding-main { min-width: 0; flex: 1; }
  .finding-title { color: var(--text-primary); font-size: 13px; font-weight: 700; overflow-wrap: anywhere; }
  .finding-summary { margin-top: 3px; color: var(--text-secondary); font-size: 12px; line-height: 1.55; overflow-wrap: anywhere; }
  .finding-toggle { flex: 0 0 auto; }
  .finding-meta { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 9px; }
  .finding-meta span { padding: 2px 7px; border-radius: 999px; background: var(--bg-secondary); border: 1px solid var(--border-secondary); color: var(--text-secondary); font-size: 10px; font-family: var(--theme-font-family-mono); }
  .finding-detail { display: grid; gap: 8px; margin-top: 10px; }
  .finding-block { padding: 9px; border-radius: 8px; background: var(--bg-card); border: 1px solid var(--border-primary); }
  .finding-block-title { margin-bottom: 6px; color: var(--text-primary); font-size: 11px; font-weight: 700; }
  .finding-line { color: var(--text-secondary); font-size: 12px; line-height: 1.55; overflow-wrap: anywhere; }
  .finding-command { display: block; margin-top: 4px; padding: 6px 8px; border-radius: 6px; background: var(--bg-secondary); color: var(--accent-primary); font-family: var(--theme-font-family-mono); font-size: 11px; overflow-wrap: anywhere; white-space: pre-wrap; }
  .raw-panel { margin-top: 14px; padding: 12px; border: 1px solid var(--border-primary); border-radius: 10px; background: var(--bg-card); }
  .raw-toggle { padding: 6px 10px; border-radius: 7px; border: 1px solid var(--border-primary); background: var(--bg-secondary); color: var(--text-secondary); font-size: 12px; cursor: pointer; }
  .raw-toggle:hover { color: var(--text-primary); background: var(--bg-hover); }
  .raw-json { max-height: 520px; overflow: auto; margin: 10px 0 0; padding: 12px; border-radius: 8px; border: 1px solid var(--border-secondary); background: var(--bg-secondary); color: var(--text-primary); font-size: 11px; line-height: 1.55; white-space: pre-wrap; overflow-wrap: anywhere; }

  @media (max-width: 900px) {
    .check-header, .config-head { align-items: stretch; flex-direction: column; }
    .check-meta { flex-wrap: wrap; }
    .config-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  }
  @media (max-width: 560px) {
    .config-grid { grid-template-columns: 1fr; }
    .loading { align-items: flex-start; }
  }
</style>
