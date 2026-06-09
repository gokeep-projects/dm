<script>
  import { onMount } from 'svelte';

  let rules = $state([]);
  let loading = $state(true);
  let savingId = $state('');
  let selectedId = $state('');
  let search = $state('');
  let category = $state('');
  let categorySearch = $state('');
  let showCategoryMenu = $state(false);
  let message = $state('');
  let error = $state('');
  let importing = $state(false);
  let importInput = $state(null);
  let ruleList = $state(null);
  let ruleSearchInput = $state(null);
  let categorySearchInput = $state(null);
  let showCreateRule = $state(false);
  let creatingRule = $state(false);
  let newRule = $state(defaultNewRule());
  let newRuleCommands = $state('');

  function defaultNewRule() {
    return {
      id: '',
      title: '',
      level: 'warn',
      category: '自定义规则',
      target: 'system',
      condition: '',
      summary: '',
      suggestion: '',
      description: '',
      enabled: true,
    };
  }

  async function load() {
    loading = true;
    error = '';
    try {
      const r = await fetch('/api/rules', { cache: 'no-store' });
      if (!r.ok) throw new Error('规则加载失败');
      const d = await r.json();
      rules = d.rules || [];
      if (!selectedId && rules.length) selectedId = rules[0].id;
    } catch (e) {
      error = e.message || String(e);
    }
    loading = false;
  }

  function updateSelected(key, value) {
    rules = rules.map(rule => rule.id === selectedId ? { ...rule, [key]: value } : rule);
  }

  async function saveRule(rule) {
    savingId = rule.id;
    message = '';
    error = '';
    try {
      const payload = {
        enabled: rule.enabled !== false,
        level: rule.level || 'warn',
        title: rule.title || '',
        summary: rule.summary || '',
        suggestion: rule.suggestion || '',
        commands: Array.isArray(rule.commands) ? rule.commands : commandText(rule).split('\n').map(v => v.trim()).filter(Boolean),
        category: rule.category || '',
        target: rule.target || '',
        condition: rule.condition || '',
        description: rule.description || '',
      };
      const r = await fetch('/api/rules/' + encodeURIComponent(rule.id), {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });
      if (!r.ok) throw new Error('规则保存失败');
      const d = await r.json();
      message = d.message || '规则已保存并实时生效';
      window.dispatchEvent(new CustomEvent('dm-alerts-refresh'));
      await load();
    } catch (e) {
      error = e.message || String(e);
    }
    savingId = '';
  }

  async function createRule() {
    creatingRule = true;
    message = '';
    error = '';
    try {
      const payload = {
        ...newRule,
        commands: newRuleCommands.split('\n').map(v => v.trim()).filter(Boolean),
      };
      const r = await fetch('/api/rules', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });
      const d = await r.json().catch(() => ({}));
      if (!r.ok || d.status === 'error') throw new Error(d.message || '新增规则失败，请检查 ID、标题和条件');
      message = d.message || '自定义规则已新增并实时生效';
      selectedId = d.rule_id || newRule.id;
      showCreateRule = false;
      newRule = defaultNewRule();
      newRuleCommands = '';
      window.dispatchEvent(new CustomEvent('dm-alerts-refresh'));
      await load();
    } catch (e) {
      error = e.message || String(e);
    }
    creatingRule = false;
  }

  function ruleTemplate() {
    return {
      schema: 'dm-rule-overrides/v1',
      exported_at: new Date().toISOString(),
      usage: '只编辑 rules 数组中 enabled、level、title、summary、suggestion、commands 字段；id 必须来自规则引擎内置规则。',
      rules: rules.slice(0, 8).map(rule => ({
        id: rule.id,
        enabled: rule.enabled !== false,
        level: rule.level || 'warn',
        title: rule.title || '',
        summary: rule.summary || '',
        suggestion: rule.suggestion || '',
        commands: Array.isArray(rule.commands) ? rule.commands : [],
      })),
    };
  }

  function downloadTemplate() {
    const blob = new Blob([JSON.stringify(ruleTemplate(), null, 2)], { type: 'application/json;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'dm-rule-import-template.json';
    document.body.appendChild(a);
    a.click();
    a.remove();
    URL.revokeObjectURL(url);
  }

  async function importRules(event) {
    const file = event.currentTarget.files?.[0];
    if (!file) return;
    importing = true;
    message = '';
    error = '';
    try {
      const payload = JSON.parse(await file.text());
      const r = await fetch('/api/rules/import', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });
      const d = await r.json().catch(() => ({}));
      if (!r.ok || d.status === 'error') throw new Error(d.message || '规则导入失败');
      message = `${d.message || '规则已导入'}；跳过 ${d.skipped?.length || 0} 条，错误 ${d.errors?.length || 0} 条`;
      if (d.errors?.length) error = d.errors.slice(0, 5).join('；');
      window.dispatchEvent(new CustomEvent('dm-alerts-refresh'));
      await load();
    } catch (e) {
      error = e.message || String(e);
    }
    importing = false;
    event.currentTarget.value = '';
  }

  function commandText(rule) {
    return Array.isArray(rule.commands) ? rule.commands.join('\n') : '';
  }

  function setCommandText(text) {
    updateSelected('commands', text.split('\n').map(v => v.trim()).filter(Boolean));
  }

  function scrollSelectedRuleIntoView() {
    requestAnimationFrame(() => {
      ruleList?.querySelector(`[data-rule-id="${CSS.escape(selectedId || '')}"]`)?.scrollIntoView({ block: 'nearest' });
    });
  }

  function locateFirstRule() {
    if (!filtered.length) return;
    selectedId = filtered[0].id;
    scrollSelectedRuleIntoView();
  }

  function onSearchKeydown(event) {
    if (event.key === 'Enter') {
      event.preventDefault();
      locateFirstRule();
    }
  }

  function selectCategory(value) {
    category = value;
    showCategoryMenu = false;
    categorySearch = '';
  }

  function toggleCategoryMenu(event) {
    event.stopPropagation();
    showCategoryMenu = !showCategoryMenu;
    if (showCategoryMenu) {
      requestAnimationFrame(() => categorySearchInput?.focus());
    }
  }

  function closeCategoryMenu(event) {
    if (!event.target.closest?.('.category-filter')) showCategoryMenu = false;
  }

  let categories = $derived.by(() => [...new Set(rules.map(r => r.category).filter(Boolean))].sort());
  let filteredCategories = $derived.by(() => {
    const q = categorySearch.trim().toLowerCase();
    if (!q) return categories;
    return categories.filter(c => c.toLowerCase().includes(q));
  });
  let categoryCounts = $derived.by(() => {
    const counts = {};
    for (const rule of rules) {
      if (!rule.category) continue;
      counts[rule.category] = (counts[rule.category] || 0) + 1;
    }
    return counts;
  });
  let filtered = $derived.by(() => {
    const q = search.trim().toLowerCase();
    return rules.filter(rule => {
      if (category && rule.category !== category) return false;
      if (!q) return true;
      return JSON.stringify(rule).toLowerCase().includes(q);
    });
  });
  let selected = $derived.by(() => rules.find(r => r.id === selectedId) || filtered[0] || null);
  let enabledCount = $derived(rules.filter(r => r.enabled !== false).length);
  let errorCount = $derived(rules.filter(r => r.level === 'error').length);
  let warnCount = $derived(rules.filter(r => r.level === 'warn').length);

  $effect(() => {
    if (!filtered.length) return;
    if (!filtered.some(rule => rule.id === selectedId)) selectedId = filtered[0].id;
  });

  onMount(() => {
    load();
    document.addEventListener('click', closeCategoryMenu);
    return () => document.removeEventListener('click', closeCategoryMenu);
  });
</script>

<div class="rules-page">
  <div class="rules-hero">
    <div>
      <div class="eyebrow">Anomaly Rule Engine</div>
      <h2>规则引擎</h2>
      <p>集中管理所有异常命中规则，保存后立即影响系统告警和后台分析。</p>
    </div>
    <div class="hero-metrics">
      <div><span>总规则</span><strong>{rules.length}</strong></div>
      <div><span>启用</span><strong>{enabledCount}</strong></div>
      <div><span>错误</span><strong>{errorCount}</strong></div>
      <div><span>警告</span><strong>{warnCount}</strong></div>
    </div>
  </div>

  <div class="rules-toolbar">
    <div class="search-wrap">
      <input bind:this={ruleSearchInput} bind:value={search} onkeydown={onSearchKeydown} placeholder="搜索规则 ID、标题、条件、建议，回车定位..." />
    </div>
    <div class="category-filter">
      <button class="category-trigger" onclick={toggleCategoryMenu}>
        <span>{category || '全部分类'}</span>
        <em>{filtered.length}/{rules.length}</em>
        <b>{showCategoryMenu ? '↑' : '↓'}</b>
      </button>
      {#if showCategoryMenu}
        <div class="category-menu" role="listbox" tabindex="-1" onclick={(event) => event.stopPropagation()} onkeydown={(event) => {
          if (event.key === 'Escape') showCategoryMenu = false;
        }}>
          <input class="category-search" bind:this={categorySearchInput} bind:value={categorySearch} placeholder="搜索分类..." aria-label="搜索规则分类" onkeydown={(event) => {
            if (event.key === 'Escape') showCategoryMenu = false;
            if (event.key === 'Enter' && filteredCategories[0]) selectCategory(filteredCategories[0]);
          }} />
          <button class="category-option" class:active={!category} onclick={() => selectCategory('')}>
            <span>全部分类</span>
            <em>{rules.length}</em>
          </button>
          {#each filteredCategories as c}
            <button class="category-option" class:active={category === c} onclick={() => selectCategory(c)}>
              <span>{c}</span>
              <em>{categoryCounts[c] || 0}</em>
            </button>
          {:else}
            <div class="category-empty">没有匹配分类</div>
          {/each}
        </div>
      {/if}
    </div>
    <button onclick={locateFirstRule} disabled={!filtered.length}>定位</button>
    <button class="create-trigger" onclick={() => showCreateRule = !showCreateRule}>{showCreateRule ? '收起新增' : '新增规则'}</button>
    <button onclick={load}>重新加载</button>
    <button onclick={downloadTemplate}>下载模板</button>
    <button onclick={() => importInput?.click()} disabled={importing}>{importing ? '导入中...' : '导入JSON'}</button>
    <input bind:this={importInput} class="hidden-file" type="file" accept="application/json,.json" onchange={importRules} />
  </div>

  {#if message}<div class="notice ok">{message}</div>{/if}
  {#if error}<div class="notice error">{error}</div>{/if}
  {#if showCreateRule}
    <div class="create-rule-panel">
      <div class="create-head">
        <div>
          <h3>新增自定义规则</h3>
          <p>条件用逗号、换行或竖线分隔，所有关键词都命中当前检查/告警上下文时生成告警。</p>
        </div>
        <label class="switch compact-switch">
          <input type="checkbox" bind:checked={newRule.enabled} />
          <span>启用</span>
        </label>
      </div>
      <div class="create-grid">
        <label><span>规则 ID *</span><input bind:value={newRule.id} placeholder="custom.nginx.5xx" /></label>
        <label><span>标题 *</span><input bind:value={newRule.title} placeholder="Nginx 5xx 激增" /></label>
        <label><span>级别</span><select bind:value={newRule.level}><option value="warn">warn</option><option value="error">error</option><option value="info">info</option></select></label>
        <label><span>分类</span><input bind:value={newRule.category} placeholder="nginx / java / redis" /></label>
        <label><span>目标</span><input bind:value={newRule.target} placeholder="system / nginx / service-name" /></label>
        <label><span>命中条件 *</span><input bind:value={newRule.condition} placeholder="nginx, 5xx, upstream" /></label>
      </div>
      <label class="block-field"><span>概要</span><textarea rows="2" bind:value={newRule.summary} placeholder="命中后展示的摘要"></textarea></label>
      <label class="block-field"><span>处理建议</span><textarea rows="2" bind:value={newRule.suggestion} placeholder="命中后展示的处理建议"></textarea></label>
      <label class="block-field"><span>建议命令，每行一条</span><textarea rows="3" bind:value={newRuleCommands} placeholder="tail -n 200 /var/log/nginx/error.log"></textarea></label>
      <div class="editor-actions">
        <button class="save-rule" onclick={createRule} disabled={creatingRule || !newRule.id.trim() || !newRule.title.trim()}>
          {creatingRule ? '新增中...' : '新增并实时生效'}
        </button>
      </div>
    </div>
  {/if}

  {#if loading}
    <div class="loading"><div class="loader"></div><span>正在加载规则...</span></div>
  {:else}
    <div class="rules-layout">
      <div class="rule-list" bind:this={ruleList}>
        {#each filtered as rule}
          <button class="rule-row" data-rule-id={rule.id} class:active={selectedId === rule.id} class:disabled-rule={rule.enabled === false} onclick={() => selectedId = rule.id}>
            <span class="level {rule.level || 'warn'}">{rule.level || 'warn'}</span>
            <span class="rule-main">
              <strong>{rule.title}</strong>
              <small>{rule.id}</small>
            </span>
            <span class="rule-cat">{rule.category}</span>
          </button>
        {/each}
        {#if filtered.length === 0}
          <div class="empty">没有匹配规则</div>
        {/if}
      </div>

      {#if selected}
        <div class="rule-editor">
          <div class="editor-head">
            <div>
              <h3>{selected.title}</h3>
              <p>{selected.id}</p>
            </div>
            <label class="switch">
              <input type="checkbox" checked={selected.enabled !== false} onchange={(e) => updateSelected('enabled', e.currentTarget.checked)} />
              <span>启用</span>
            </label>
          </div>

          <div class="editor-grid">
            <label>
              <span>级别</span>
              <select value={selected.level || 'warn'} onchange={(e) => updateSelected('level', e.currentTarget.value)}>
                <option value="warn">warn</option>
                <option value="error">error</option>
                <option value="info">info</option>
              </select>
            </label>
            <label>
              <span>分类</span>
              <input value={selected.category || ''} readonly={!selected.custom} oninput={(e) => updateSelected('category', e.currentTarget.value)} />
            </label>
            <label>
              <span>目标</span>
              <input value={selected.target || ''} readonly={!selected.custom} oninput={(e) => updateSelected('target', e.currentTarget.value)} />
            </label>
            <label>
              <span>条件</span>
              <input value={selected.condition || ''} readonly={!selected.custom} oninput={(e) => updateSelected('condition', e.currentTarget.value)} />
            </label>
          </div>

          <div class="rule-intel">
            <div class="intel-card">
              <span>规则说明</span>
              <p>{selected.description || '内置异常规则，可通过规则引擎页面覆盖启用状态、级别、标题、建议和命令。'}</p>
            </div>
            <div class="intel-card">
              <span>检测信号</span>
              {#if selected.signals?.length}
                <div class="token-list">
                  {#each selected.signals as signal}
                    <code>{signal}</code>
                  {/each}
                </div>
              {:else}
                <p>由对应检查项的结构化输出、系统采集或后台分析结果触发。</p>
              {/if}
            </div>
            <div class="intel-card">
              <span>默认定位命令</span>
              {#if selected.commands?.length}
                <div class="command-preview">
                  {#each selected.commands.slice(0, 4) as cmd}
                    <code>{cmd}</code>
                  {/each}
                </div>
              {:else}
                <p>命中后会根据检查项和证据自动生成定位建议。</p>
              {/if}
            </div>
            <div class="intel-card">
              <span>生效状态</span>
              <p>{selected.enabled === false ? '当前已禁用，后台分析不会产生该规则的告警。' : selected.override ? '当前规则存在本地覆盖，保存后会立即影响告警分析。' : '当前使用内置规则定义，命中后会同步到系统告警。'}</p>
            </div>
          </div>

          <label class="block-field">
            <span>规则标题</span>
            <input value={selected.title || ''} oninput={(e) => updateSelected('title', e.currentTarget.value)} />
          </label>
          <label class="block-field">
            <span>概要覆盖</span>
            <textarea rows="3" value={selected.summary || ''} oninput={(e) => updateSelected('summary', e.currentTarget.value)} placeholder="为空则使用规则命中时生成的动态概要"></textarea>
          </label>
          <label class="block-field">
            <span>处理建议</span>
            <textarea rows="4" value={selected.suggestion || ''} oninput={(e) => updateSelected('suggestion', e.currentTarget.value)} placeholder="保存后告警详情会显示该建议"></textarea>
          </label>
          <label class="block-field">
            <span>建议命令，每行一条</span>
            <textarea rows="5" value={commandText(selected)} oninput={(e) => setCommandText(e.currentTarget.value)} placeholder="top -o %CPU"></textarea>
          </label>

          <details class="raw-rule">
            <summary>原始规则数据</summary>
            <pre>{JSON.stringify(selected, null, 2)}</pre>
          </details>

          <div class="editor-actions">
            <button class="save-rule" onclick={() => saveRule(selected)} disabled={savingId === selected.id}>
              {savingId === selected.id ? '保存中...' : '保存并实时生效'}
            </button>
          </div>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .rules-page { width: 100%; max-width: none; margin: 0; }
  .rules-hero { display: flex; align-items: center; justify-content: space-between; gap: 12px; min-height: 46px; padding: 8px 10px 8px 12px; border-radius: 11px; border: 1px solid rgba(34,211,238,.18); background: linear-gradient(135deg, var(--bg-card), rgba(34,211,238,.055)); box-shadow: inset 0 0 24px rgba(34,211,238,.035); }
  .eyebrow { color: #67e8f9; font-family: var(--theme-font-family-mono); font-size: 10px; font-weight: 900; text-transform: uppercase; line-height: 1; }
  .rules-hero h2 { margin: 2px 0 0; color: var(--text-primary); font-size: 16px; line-height: 1.15; }
  .rules-hero p { display: none; }
  .hero-metrics { display: flex; align-items: center; justify-content: flex-end; flex-wrap: wrap; gap: 6px; min-width: 0; }
  .hero-metrics div { display: inline-flex; align-items: center; gap: 6px; min-height: 28px; padding: 3px 8px; border-radius: 999px; border: 1px solid var(--border-primary); background: var(--bg-secondary); }
  .hero-metrics span { color: var(--text-secondary); font-size: 10px; white-space: nowrap; }
  .hero-metrics strong { color: var(--text-primary); font-family: var(--theme-font-family-mono); font-size: 13px; line-height: 1; }
  .rules-toolbar { display: flex; align-items: center; flex-wrap: wrap; gap: 10px; margin: 10px 0; }
  .rules-toolbar input, .rules-toolbar button { height: 36px; border-radius: 9px; border: 1px solid var(--border-primary); background: var(--bg-card); color: var(--text-primary); padding: 0 12px; outline: none; }
  .rules-toolbar button { cursor: pointer; font-weight: 700; white-space: nowrap; }
  .rules-toolbar button:hover:not(:disabled) { border-color: var(--border-focus); color: var(--accent-primary); background: var(--accent-primary-light); }
  .rules-toolbar button:disabled { opacity: .58; cursor: not-allowed; }
  .create-trigger { color: #051014 !important; border-color: rgba(103,232,249,.35) !important; background: linear-gradient(135deg, #67e8f9, #34d399) !important; }
  .hidden-file { display: none; }
  .search-wrap { flex: 1 1 520px; min-width: min(520px, 100%); }
  .search-wrap input { width: 100%; box-sizing: border-box; }
  .category-filter { position: relative; flex: 0 0 260px; min-width: 220px; }
  .category-trigger { width: 100%; display: grid; grid-template-columns: minmax(0, 1fr) auto auto; align-items: center; gap: 8px; text-align: left; }
  .category-trigger span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .category-trigger em { color: var(--text-tertiary); font-family: var(--theme-font-family-mono); font-size: 11px; font-style: normal; }
  .category-trigger b { color: var(--text-tertiary); font-size: 11px; }
  .category-menu { position: absolute; left: 0; right: 0; top: calc(100% + 6px); z-index: 40; max-height: 320px; overflow: auto; padding: 8px; border-radius: 12px; border: 1px solid rgba(34,211,238,.24); background: var(--bg-card); box-shadow: 0 18px 46px rgba(0,0,0,.28); }
  .category-search { width: 100%; margin-bottom: 7px; background: var(--bg-input) !important; }
  .category-option { width: 100%; display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: center; gap: 8px; margin: 0 0 4px; border-color: transparent !important; background: transparent !important; text-align: left; }
  .category-option:hover, .category-option.active { border-color: rgba(34,211,238,.24) !important; background: rgba(34,211,238,.08) !important; }
  .category-option span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .category-option em { color: var(--text-tertiary); font-family: var(--theme-font-family-mono); font-size: 11px; font-style: normal; }
  .category-empty { padding: 18px 10px; color: var(--text-tertiary); font-size: 12px; text-align: center; }
  .notice { margin-bottom: 10px; padding: 10px 12px; border-radius: 9px; font-size: 13px; }
  .notice.ok { color: #34d399; background: rgba(52,211,153,.08); border: 1px solid rgba(52,211,153,.18); }
  .notice.error { color: #f87171; background: rgba(248,113,113,.08); border: 1px solid rgba(248,113,113,.18); }
  .create-rule-panel { margin: 0 0 12px; padding: 13px; border-radius: 12px; border: 1px solid rgba(103,232,249,.24); background: radial-gradient(circle at top right, rgba(52,211,153,.12), transparent 34%), var(--bg-card); box-shadow: inset 0 0 32px rgba(34,211,238,.04); }
  .create-head { display: flex; justify-content: space-between; gap: 14px; margin-bottom: 10px; }
  .create-head h3 { margin: 0; color: var(--text-primary); font-size: 15px; }
  .create-head p { margin: 4px 0 0; color: var(--text-secondary); font-size: 12px; }
  .compact-switch { flex-shrink: 0; margin-top: 2px; }
  .create-grid { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 10px; }
  .rules-layout { display: grid; grid-template-columns: minmax(320px, 38%) minmax(0, 1fr); gap: 12px; }
  .rule-list, .rule-editor { border: 1px solid var(--border-primary); border-radius: 12px; background: var(--bg-card); }
  .rule-list { max-height: calc(100vh - 205px); overflow: auto; padding: 8px; }
  .rule-row { width: 100%; display: grid; grid-template-columns: 58px minmax(0, 1fr) 86px; gap: 10px; align-items: center; margin-bottom: 6px; padding: 10px; border: 1px solid transparent; border-radius: 9px; background: transparent; color: inherit; cursor: pointer; text-align: left; }
  .rule-row:hover, .rule-row.active { border-color: rgba(34,211,238,.28); background: rgba(34,211,238,.07); }
  .rule-row.disabled-rule { opacity: .52; }
  .level { display: inline-grid; place-items: center; min-width: 48px; height: 24px; border-radius: 7px; font-family: var(--theme-font-family-mono); font-size: 10px; font-weight: 900; text-transform: uppercase; }
  .level.warn { color: #fbbf24; background: rgba(245,158,11,.12); }
  .level.error { color: #f87171; background: rgba(239,68,68,.12); }
  .level.info { color: #93c5fd; background: rgba(59,130,246,.12); }
  .rule-main { min-width: 0; }
  .rule-main strong, .rule-main small { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .rule-main strong { color: var(--text-primary); font-size: 13px; }
  .rule-main small { margin-top: 3px; color: var(--text-tertiary); font-family: var(--theme-font-family-mono); font-size: 10px; }
  .rule-cat { color: var(--text-secondary); font-size: 11px; text-align: right; }
  .rule-editor { padding: 16px; min-width: 0; }
  .editor-head { display: flex; justify-content: space-between; gap: 12px; margin-bottom: 14px; }
  .editor-head h3 { margin: 0 0 4px; color: var(--text-primary); font-size: 18px; }
  .editor-head p { margin: 0; color: var(--text-tertiary); font-family: var(--theme-font-family-mono); font-size: 11px; }
  .switch { display: inline-flex; align-items: center; gap: 8px; color: var(--text-secondary); font-size: 13px; }
  .editor-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px; margin-bottom: 10px; }
  label span { display: block; margin-bottom: 6px; color: var(--text-secondary); font-size: 12px; font-weight: 700; }
  input, select, textarea { width: 100%; box-sizing: border-box; border-radius: 9px; border: 1px solid var(--border-primary); background: var(--bg-input); color: var(--text-primary); padding: 9px 10px; outline: none; font-family: inherit; }
  textarea { resize: vertical; line-height: 1.55; }
  input:focus, select:focus, textarea:focus { border-color: rgba(34,211,238,.45); box-shadow: 0 0 0 3px rgba(34,211,238,.08); }
  .rule-intel { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px; margin: 12px 0; }
  .intel-card { min-width: 0; padding: 11px 12px; border-radius: 10px; border: 1px solid var(--border-primary); background: var(--bg-secondary); }
  .intel-card span { display: block; margin-bottom: 7px; color: #67e8f9; font-size: 11px; font-weight: 900; }
  .intel-card p { margin: 0; color: var(--text-secondary); font-size: 12px; line-height: 1.55; }
  .token-list, .command-preview { display: flex; flex-wrap: wrap; gap: 6px; }
  .token-list code, .command-preview code { max-width: 100%; padding: 4px 7px; border-radius: 7px; border: 1px solid rgba(34,211,238,.14); background: rgba(34,211,238,.07); color: var(--text-primary); font-family: var(--theme-font-family-mono); font-size: 10px; overflow-wrap: anywhere; }
  .command-preview { flex-direction: column; }
  .command-preview code { display: block; }
  .block-field { display: block; margin-top: 10px; }
  .raw-rule { margin-top: 12px; }
  summary { color: #67e8f9; cursor: pointer; font-size: 12px; font-weight: 800; }
  pre { max-height: 260px; overflow: auto; padding: 10px; border-radius: 8px; background: var(--bg-secondary); color: var(--text-primary); font-family: var(--theme-font-family-mono); font-size: 11px; white-space: pre-wrap; }
  .editor-actions { display: flex; justify-content: flex-end; margin-top: 14px; }
  .save-rule { padding: 10px 16px; border: none; border-radius: 9px; background: linear-gradient(135deg, #0891b2, #0f766e); color: white; font-weight: 800; cursor: pointer; }
  .save-rule:disabled { opacity: .6; cursor: not-allowed; }
  .loading, .empty { display: flex; align-items: center; justify-content: center; gap: 10px; padding: 44px; color: var(--text-secondary); }
  .loader { width: 30px; height: 30px; border-radius: 50%; border: 3px solid rgba(34,211,238,.15); border-top-color: #22d3ee; animation: spin .8s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (max-width: 900px) {
    .rules-hero, .rules-toolbar { flex-direction: column; }
    .rules-hero { align-items: stretch; }
    .search-wrap, .category-filter { flex-basis: auto; width: 100%; min-width: 0; }
    .hero-metrics { justify-content: flex-start; }
    .create-head { flex-direction: column; }
    .create-grid { grid-template-columns: 1fr; }
    .rules-layout { grid-template-columns: 1fr; }
    .rule-intel { grid-template-columns: 1fr; }
  }
</style>
