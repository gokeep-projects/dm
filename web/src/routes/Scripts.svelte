<script>
  import { onMount } from 'svelte';
  let scripts = $state([]);
  let search = $state('');
  let cat = $state('');
  let cats = $state([]);
  let err = $state(null);
  let loading = $state(true);
  let view = $state('grid');
  let sort = $state('name');
  let execStats = $state({});
  let favorites = $state(new Set());
  let showFavOnly = $state(false);
  let sourceFilter = $state('');
  let showUpload = $state(false);
  let showFilters = $state(false);
  let uploadFile = $state(null);
  let uploadId = $state('');
  let uploadTitle = $state('');
  let uploadDescription = $state('');
  let uploadFeature = $state('');
  let uploadCategory = $state('维护脚本');
  let uploadAuthor = $state('user');
  let uploadLoading = $state(false);
  let uploadError = $state(null);
  let showUploadAdvanced = $state(false);
  let editingScript = $state(null);
  let editName = $state('');
  let editDescription = $state('');
  let editFeature = $state('');
  let editCategory = $state('');
  let editAuthor = $state('');
  let editVersion = $state('');
  let editContent = $state('');
  let editLoading = $state(false);
  let editError = $state(null);
  let runResults = $state({});

  const categoryIcons = { '系统检查': 'SYS', '系统安全': 'SEC', '日志管理': 'LOG', '服务管理': 'SVC', '网络诊断': 'NET', '网络': 'NET', '性能监控': 'MON', '中间件': 'MID', '系统管理': 'ADM' };
  const categoryColors = { '系统检查': ['#0ea5e9', '#0369a1'], '系统安全': ['#dc2626', '#991b1b'], '日志管理': ['#7c3aed', '#5b21b6'], '服务管理': ['#2563eb', '#1d4ed8'], '网络诊断': ['#059669', '#047857'], '网络': ['#059669', '#047857'], '性能监控': ['#d97706', '#b45309'], '中间件': ['#9333ea', '#7e22ce'], '系统管理': ['#64748b', '#475569'] };

  function getIcon(cat) { return categoryIcons[cat] || 'GEN'; }
  function getCatColors(cat) { return categoryColors[cat] || ['#6b7280', '#4b5563']; }

  async function load() {
    loading = true;
    err = null;
    try {
      const r = await fetch('/api/scripts');
      if (r.ok) {
        const d = await r.json();
        scripts = d.scripts || [];
        const catSet = new Set();
        for (const s of scripts) catSet.add(s.category);
        cats = [...catSet].sort();
      } else { err = '加载失败: ' + r.status; }
    } catch (e) { err = '网络错误: ' + e.message; }
    loading = false;
    loadExecStats();
  }

  async function loadExecStats() {
    try {
      const r = await fetch('/api/scripts/stats/all');
      if (r.ok) {
        const d = await r.json();
        execStats = d.stats || {};
      }
    } catch (e) { console.warn('加载执行统计失败:', e); }
  }

  function execCount(id) { return execStats[id]?.total_executions || 0; }
  function lastExec(id) { return execStats[id]?.last_execution || null; }
  function categoryCount(c) { return scripts.filter(s => s.category === c).length; }
  function sourceLabel(s) { return s.user_managed ? '用户脚本' : '内置脚本'; }
  function sourceClass(s) { return s.user_managed ? 'user-source' : 'builtin-source'; }
  function sourceCount(source) {
    return scripts.filter(s => source === 'user' ? s.user_managed : !s.user_managed).length;
  }

  function onSearch(e) { search = e.target.value; }

  async function uploadScript() {
    if (!uploadFile) return;
    uploadLoading = true;
    uploadError = null;
    try {
      const inferredTitle = uploadTitle.trim() || uploadFile.name.replace(/\.[^.]+$/, '');
      const inferredId = uploadId.trim() || inferredTitle.toLowerCase().replace(/[^a-z0-9_-]+/g, '-').replace(/^-+|-+$/g, '') || `script-${Date.now()}`;
      const formData = new FormData();
      formData.append('id', inferredId);
      formData.append('title', inferredTitle);
      formData.append('description', uploadDescription.trim() || inferredTitle);
      formData.append('feature', uploadFeature.trim() || inferredTitle);
      formData.append('category', uploadCategory.trim() || '维护脚本');
      formData.append('author', uploadAuthor.trim() || 'user');
      formData.append('file', uploadFile);
      const r = await fetch('/api/scripts/upload', { method: 'POST', body: formData });
      if (r.ok) {
        closeUpload();
        load();
      } else {
        const d = await r.json();
        uploadError = d.error || '上传失败';
      }
    } catch (e) { uploadError = e.message; }
    uploadLoading = false;
  }

  function closeUpload() {
    showUpload = false;
    uploadFile = null;
    uploadId = '';
    uploadTitle = '';
    uploadDescription = '';
    uploadFeature = '';
    uploadCategory = '维护脚本';
    uploadAuthor = 'user';
    uploadError = null;
    showUploadAdvanced = false;
  }

  function handleFileSelect(e) {
    const file = e.target.files[0];
    if (file) {
      uploadFile = file;
      if (!uploadId) uploadId = file.name.split('.')[0].toLowerCase().replace(/[^a-z0-9_-]+/g, '-').replace(/^-+|-+$/g, '');
      if (!uploadTitle) uploadTitle = file.name.replace(/\.[^.]+$/, '');
    }
  }

  async function openEditScript(script) {
    if (!script?.user_managed) return;
    editingScript = script;
    editName = script.name || script.id;
    editDescription = script.description || '';
    editFeature = script.feature || '';
    editCategory = script.category || '维护脚本';
    editAuthor = script.metadata?.author || '';
    editVersion = script.metadata?.version || '1.0.0';
    editContent = '';
    editError = null;
    editLoading = true;
    try {
      const r = await fetch('/api/scripts/' + encodeURIComponent(script.id) + '/source');
      if (r.ok) {
        const d = await r.json();
        editContent = d.content || '';
      }
    } catch (e) { editError = e.message; }
    editLoading = false;
  }

  function closeEditScript() {
    editingScript = null;
    editError = null;
    editContent = '';
  }

  async function updateScript() {
    if (!editingScript) return;
    editLoading = true;
    editError = null;
    try {
      const r = await fetch('/api/scripts/' + encodeURIComponent(editingScript.id), {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          name: editName,
          description: editDescription,
          feature: editFeature,
          category: editCategory,
          author: editAuthor,
          version: editVersion,
          content: editContent,
        }),
      });
      if (r.ok) {
        closeEditScript();
        await load();
      } else {
        const d = await r.json().catch(() => ({}));
        editError = d.error || (r.status === 403 ? '内置脚本不可修改' : '保存失败: ' + r.status);
      }
    } catch (e) { editError = e.message; }
    editLoading = false;
  }

  async function deleteScript(script) {
    if (!script?.user_managed) return;
    if (!confirm('确定删除脚本 "' + script.name + '"？')) return;
    try {
      const r = await fetch('/api/scripts/' + encodeURIComponent(script.id), { method: 'DELETE' });
      if (r.ok) await load();
      else alert(r.status === 403 ? '内置脚本不可删除' : '删除失败: ' + r.status);
    } catch (e) { alert('删除失败: ' + e.message); }
  }

  function loadFavorites() {
    try {
      const raw = localStorage.getItem('dm-favorites');
      if (raw) favorites = new Set(JSON.parse(raw));
    } catch (_) {}
  }

  function saveFavorites() {
    try { localStorage.setItem('dm-favorites', JSON.stringify([...favorites])); } catch (_) {}
  }

  function toggleFav(id) {
    const next = new Set(favorites);
    if (next.has(id)) next.delete(id); else next.add(id);
    favorites = next;
    saveFavorites();
  }

  function isFav(id) { return favorites.has(id); }

  function highlightMatch(text, q) {
    if (!q || !text) return text;
    const escaped = q.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    const regex = new RegExp('(' + escaped + ')', 'gi');
    return text.replace(regex, '<mark class="search-highlight">$1</mark>');
  }

  function scriptMatchesSearch(s, q) {
    if (!q) return true;
    const number = s.number || 0;
    const label = s.numberLabel || String(number).padStart(2, '0');
    return String(number).includes(q) ||
      label.includes(q) ||
      ('#' + number).includes(q) ||
      ('#' + label).includes(q) ||
      s.name?.toLowerCase().includes(q) ||
      s.id?.toLowerCase().includes(q) ||
      s.category?.toLowerCase().includes(q) ||
      s.feature?.toLowerCase().includes(q) ||
      s.description?.toLowerCase().includes(q);
  }

  let numberedScripts = $derived.by(() => scripts.map((script, index) => {
    const number = index + 1;
    return {
      ...script,
      number,
      numberLabel: String(number).padStart(2, '0'),
    };
  }));

  let sortedScripts = $derived.by(() => {
    const q = search.trim().toLowerCase();
    let arr = numberedScripts.filter(s => {
      const matchesCategory = !cat || s.category === cat;
      const matchesSearch = scriptMatchesSearch(s, q);
      const matchesSource = !sourceFilter ||
        (sourceFilter === 'user' ? s.user_managed : !s.user_managed);
      return matchesCategory && matchesSearch && matchesSource;
    });
    if (showFavOnly) arr = arr.filter(s => favorites.has(s.id));
    if (sort === 'name') arr.sort((a, b) => a.name.localeCompare(b.name, 'zh-CN'));
    else if (sort === 'category') arr.sort((a, b) => a.category.localeCompare(b.category, 'zh-CN') || a.name.localeCompare(b.name, 'zh-CN'));
    else if (sort === 'exec') arr.sort((a, b) => execCount(b.id) - execCount(a.id));
    else if (sort === 'recent') {
      arr.sort((a, b) => {
        const la = lastExec(a.id), lb = lastExec(b.id);
        if (!la && !lb) return 0;
        if (!la) return 1;
        if (!lb) return -1;
        return lb.timestamp.localeCompare(la.timestamp);
      });
    }
    arr.sort((a, b) => (favorites.has(b.id) ? 1 : 0) - (favorites.has(a.id) ? 1 : 0));
    return arr;
  });

  let totalExecutions = $derived.by(() => {
    return Object.values(execStats).reduce((sum, item) => sum + (item?.total_executions || 0), 0);
  });

  let successRate = $derived.by(() => {
    const values = Object.values(execStats);
    const total = values.reduce((sum, item) => sum + (item?.total_executions || 0), 0);
    const success = values.reduce((sum, item) => sum + (item?.success_count || 0), 0);
    if (!total) return '-';
    return Math.round(success / total * 100) + '%';
  });

  onMount(() => {
    loadFavorites();
    load();
  });
</script>

<div class="scripts-page">
  <div class="page-header">
    <div class="header-left">
      <div>
        <h2 class="page-title">脚本中心</h2>
        <p class="page-subtitle">共 {scripts.length} 个脚本，当前显示 {sortedScripts.length} 个</p>
      </div>
      <div class="summary-pills">
        <span class="summary-pill">分类 {cats.length}</span>
        <span class="summary-pill">执行 {totalExecutions}</span>
        <span class="summary-pill">成功率 {successRate}</span>
      </div>
    </div>
    <div class="header-right">
      <button class="upload-btn" onclick={() => { showUpload = true; uploadError = null; }}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="17 8 12 3 7 8"/><line x1="12" y1="3" x2="12" y2="15"/></svg>
        上传脚本
      </button>
      <button class="upload-btn filter-toggle-btn" onclick={() => showFilters = !showFilters}>
        筛选{cat || search || showFavOnly || sourceFilter ? ' *' : ''} {showFilters ? '↑' : '↓'}
      </button>
      <select class="sort-select" bind:value={sort} aria-label="排序方式">
        <option value="name">按名称</option>
        <option value="category">按分类</option>
        <option value="exec">按执行次数</option>
        <option value="recent">按最近执行</option>
      </select>

      <div class="view-toggle">
        <button class="view-btn" class:active={view === 'grid'} onclick={() => view = 'grid'} title="网格视图">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <rect x="3" y="3" width="7" height="7" rx="1" />
            <rect x="14" y="3" width="7" height="7" rx="1" />
            <rect x="3" y="14" width="7" height="7" rx="1" />
            <rect x="14" y="14" width="7" height="7" rx="1" />
          </svg>
        </button>
        <button class="view-btn" class:active={view === 'list'} onclick={() => view = 'list'} title="列表视图">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <line x1="8" y1="6" x2="21" y2="6" />
            <line x1="8" y1="12" x2="21" y2="12" />
            <line x1="8" y1="18" x2="21" y2="18" />
            <line x1="3" y1="6" x2="3.01" y2="6" />
            <line x1="3" y1="12" x2="3.01" y2="12" />
            <line x1="3" y1="18" x2="3.01" y2="18" />
          </svg>
        </button>
      </div>
    </div>
  </div>

  {#if showFilters}
  <div class="filter-section">
    <div class="search-wrap">
      <svg class="search-icon" width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <circle cx="11" cy="11" r="8"/>
        <path d="m21 21-4.35-4.35" stroke-linecap="round"/>
      </svg>
      <input
        class="search-input"
        value={search}
        oninput={onSearch}
        placeholder="输入编号、名称、ID、分类、功能或描述"
        autocomplete="off"
        spellcheck="false"
        aria-label="搜索脚本" />
      {#if search}
        <button class="search-clear" onclick={() => search = ''} aria-label="清空搜索">
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M18 6 6 18M6 6l12 12" stroke-linecap="round"/>
          </svg>
        </button>
      {/if}
    </div>
    <div class="category-chips" aria-label="分类筛选">
      <button class="chip" class:active={!cat} onclick={() => cat = ''}>
        <span>全部</span>
        <span class="chip-count">{scripts.length}</span>
      </button>
      <button class="chip fav-chip" class:active={showFavOnly} onclick={() => showFavOnly = !showFavOnly}>
        <span>收藏</span>
        <span class="chip-count">{favorites.size}</span>
      </button>
      <button class="chip" class:active={sourceFilter === 'user'} onclick={() => sourceFilter = sourceFilter === 'user' ? '' : 'user'}>
        <span>用户脚本</span>
        <span class="chip-count">{sourceCount('user')}</span>
      </button>
      <button class="chip" class:active={sourceFilter === 'builtin'} onclick={() => sourceFilter = sourceFilter === 'builtin' ? '' : 'builtin'}>
        <span>内置脚本</span>
        <span class="chip-count">{sourceCount('builtin')}</span>
      </button>
      {#each cats as c}
        <button class="chip" class:active={cat === c} onclick={() => cat = c}>
          <span class="chip-icon">{getIcon(c)}</span>
          <span>{c}</span>
          <span class="chip-count">{categoryCount(c)}</span>
        </button>
      {/each}
    </div>
  </div>
  {/if}

  {#if loading}
    <div class="loading-wrap">
      <div class="spinner"></div>
    </div>
  {:else if scripts.length === 0}
    <div class="empty-wrap">
      <span class="empty-icon">--</span>
      <span class="empty-title">暂无维护脚本</span>
      <span class="empty-hint">点击上传脚本添加用于现场维护的脚本</span>
    </div>
  {:else if sortedScripts.length === 0}
    <div class="empty-wrap">
      <span class="empty-icon">0</span>
      <span class="empty-title">没有匹配的脚本</span>
      <span class="empty-hint">尝试调整搜索关键词或选择其他分类</span>
    </div>
  {:else if view === 'grid'}
    <div class="scripts-grid">
      {#each sortedScripts as s, i (s.id)}
        <a href="#/script/{s.id}" class="script-card" style="animation-delay:{Math.min(i * 40, 600)}ms" title="{s.name}（{s.id}）\n分类：{s.category}\n{s.feature || s.description}">
          <div class="card-glow" style="background:radial-gradient(circle, {getCatColors(s.category)[0]}15, transparent 70%)"></div>
          <div class="card-inner">
            <div class="card-header">
              <span class="script-number">#{s.numberLabel}</span>
              <div class="card-icon" style="background:linear-gradient(135deg,{getCatColors(s.category)[0]}, {getCatColors(s.category)[1]})">
                <span>{getIcon(s.category)}</span>
              </div>
              <div class="card-meta">
                <span class="card-category">{s.category}</span>
                <span class="source-pill {sourceClass(s)}">{sourceLabel(s)}</span>
                {#if s.metadata?.version}
                  <span class="card-version">v{s.metadata.version}</span>
                {/if}
              </div>
              <button
                class="card-fav-btn"
                class:faved={isFav(s.id)}
                onclick={(e) => { e.preventDefault(); e.stopPropagation(); toggleFav(s.id); }}
                aria-label="{isFav(s.id) ? '取消收藏' : '收藏'} {s.name}"
                title="{isFav(s.id) ? '取消收藏' : '收藏'}">
                {isFav(s.id) ? '★' : '☆'}
              </button>
              <button
                class="card-run-btn"
                onclick={(e) => { e.preventDefault(); e.stopPropagation(); location.hash = '#/script/' + s.id + '/run'; }}
                aria-label="快速执行 {s.name}"
                title="快速执行 {s.name}">
                <svg width="11" height="11" viewBox="0 0 24 24" fill="currentColor">
                  <path d="M8 5v14l11-7z"/>
                </svg>
              </button>
              {#if s.user_managed}
                <button
                  class="card-edit-btn"
                  onclick={(e) => { e.preventDefault(); e.stopPropagation(); openEditScript(s); }}
                  aria-label="编辑 {s.name}"
                  title="编辑脚本">
                  编辑
                </button>
                <button
                  class="card-delete-btn"
                  onclick={(e) => { e.preventDefault(); e.stopPropagation(); deleteScript(s); }}
                  aria-label="删除 {s.name}"
                  title="删除脚本">
                  删除
                </button>
              {/if}
              {#if execCount(s.id) > 0}
                <span class="card-badge" title="最近执行 {execCount(s.id)} 次">
                  <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                    <polygon points="6 3 20 12 6 21 6 3" fill="currentColor"/>
                  </svg>
                  {execCount(s.id)}
                </span>
              {/if}
            </div>
             {#if search.trim()}
              <h3 class="card-title">{@html highlightMatch(s.name, search)}</h3>
              {#if s.feature}
                <p class="card-feature">{@html highlightMatch(s.feature, search)}</p>
              {/if}
              <p class="card-desc">{@html highlightMatch(s.description, search)}</p>
            {:else}
              <h3 class="card-title">{s.name}</h3>
              {#if s.feature}
                <p class="card-feature">{s.feature}</p>
              {/if}
              <p class="card-desc">{s.description}</p>
            {/if}
            {#if s.metadata?.example}
              <div class="card-example">
                <span class="example-label">示例</span>
                <code class="example-cmd">{s.metadata.example}</code>
              </div>
            {/if}
            <div class="card-footer">
              <code class="card-cmd">$ dm run {s.id}</code>
              {#if s.metadata?.author}
                <span class="card-author">· {s.metadata.author}</span>
              {/if}
            </div>
          </div>
        </a>
      {/each}
    </div>
  {:else}
    <div class="scripts-list">
      {#each sortedScripts as s, i (s.id)}
        <a href="#/script/{s.id}" class="script-row" style="animation-delay:{Math.min(i * 30, 500)}ms">
          <span class="row-number">#{s.numberLabel}</span>
          <div class="row-icon" style="background:linear-gradient(135deg,{getCatColors(s.category)[0]}, {getCatColors(s.category)[1]})">
            {getIcon(s.category)}
          </div>
          <div class="row-info">
            <div class="row-title-row">
              {#if search.trim()}
                <h3 class="row-title">{@html highlightMatch(s.name, search)}</h3>
              {:else}
                <h3 class="row-title">{s.name}</h3>
              {/if}
              <span class="row-id">{s.id}</span>
              <span class="source-pill {sourceClass(s)}">{sourceLabel(s)}</span>
              {#if s.metadata?.version}
                <span class="row-version">v{s.metadata.version}</span>
              {/if}
              {#if execCount(s.id) > 0}
                <span class="row-badge">▶ {execCount(s.id)}</span>
              {/if}
            </div>
            {#if search.trim()}
              <p class="row-desc">{@html highlightMatch(s.feature || s.description, search)}</p>
            {:else}
              <p class="row-desc">{s.feature || s.description}</p>
            {/if}
          </div>
          <div class="row-category">
            <span class="cat-pill" style="background:{getCatColors(s.category)[0]}1a;color:{getCatColors(s.category)[0]};border-color:{getCatColors(s.category)[0]}33">{s.category}</span>
          </div>
          <div class="row-cmd">$ dm run {s.id}</div>
          <button
            class="row-fav-btn"
            class:faved={isFav(s.id)}
            onclick={(e) => { e.preventDefault(); e.stopPropagation(); toggleFav(s.id); }}
            aria-label="{isFav(s.id) ? '取消收藏' : '收藏'} {s.name}"
            title="{isFav(s.id) ? '取消收藏' : '收藏'}">
            {isFav(s.id) ? '★' : '☆'}
          </button>
          <button
            class="row-run-btn"
            onclick={(e) => { e.preventDefault(); e.stopPropagation(); location.hash = '#/script/' + s.id + '/run'; }}
            aria-label="快速执行 {s.name}"
            title="快速执行">
            <svg width="11" height="11" viewBox="0 0 24 24" fill="currentColor">
              <path d="M8 5v14l11-7z"/>
            </svg>
          </button>
          {#if s.user_managed}
            <button
              class="row-edit-btn"
              onclick={(e) => { e.preventDefault(); e.stopPropagation(); openEditScript(s); }}
              aria-label="编辑 {s.name}">
              编辑
            </button>
            <button
              class="row-delete-btn"
              onclick={(e) => { e.preventDefault(); e.stopPropagation(); deleteScript(s); }}
              aria-label="删除 {s.name}">
              删除
            </button>
          {/if}
          <div class="row-arrow">→</div>
        </a>
      {/each}
    </div>
  {/if}
</div>

{#if showUpload}
  <div class="modal-overlay" onclick={closeUpload} role="presentation">
    <div class="modal" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="dialog" aria-modal="true" tabindex="-1">
      <div class="modal-header">
        <h3>上传脚本</h3>
        <button class="modal-close" onclick={closeUpload}>✕</button>
      </div>
      <div class="modal-body">
        <div class="upload-area">
          <input type="file" id="script-file" accept=".sh,.py,.pl,.js,.bash" onchange={handleFileSelect} class="file-input" />
          <label for="script-file" class="file-label">
            <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="17 8 12 3 7 8"/><line x1="12" y1="3" x2="12" y2="15"/></svg>
            <span>{uploadFile ? uploadFile.name : '选择脚本文件'}</span>
          </label>
        </div>
        <label class="form-field full">
          <span>标题</span>
          <input bind:value={uploadTitle} placeholder="自动使用文件名，可按需修改" />
        </label>
        <button class="advanced-toggle" onclick={() => showUploadAdvanced = !showUploadAdvanced}>
          <span>{showUploadAdvanced ? '收起高级配置' : '高级配置'}</span>
          <span>{showUploadAdvanced ? '↑' : '↓'}</span>
        </button>
        {#if showUploadAdvanced}
          <div class="advanced-panel">
            <div class="form-grid">
              <label class="form-field">
                <span>脚本 ID</span>
                <input bind:value={uploadId} placeholder="自动生成，例如 restart-nginx" />
              </label>
              <label class="form-field">
                <span>分类</span>
                <input bind:value={uploadCategory} placeholder="维护脚本" />
              </label>
              <label class="form-field">
                <span>作者</span>
                <input bind:value={uploadAuthor} placeholder="user" />
              </label>
              <label class="form-field">
                <span>功能摘要</span>
                <input bind:value={uploadFeature} placeholder="默认使用标题" />
              </label>
            </div>
            <label class="form-field full">
              <span>描述</span>
              <textarea bind:value={uploadDescription} rows="3" placeholder="适用场景、注意事项、输入输出说明"></textarea>
            </label>
          </div>
        {/if}
        {#if uploadError}
          <p class="upload-error">{uploadError}</p>
        {/if}
        <div class="upload-actions">
          <button class="cancel-btn" onclick={closeUpload}>取消</button>
          <button class="submit-btn" onclick={uploadScript} disabled={!uploadFile || uploadLoading}>
            {uploadLoading ? '上传中...' : '上传'}
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}

{#if editingScript}
  <div class="modal-overlay" onclick={closeEditScript} role="presentation">
    <div class="modal script-edit-modal" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="dialog" aria-modal="true" tabindex="-1">
      <div class="modal-header">
        <h3>编辑脚本 - {editingScript.id}</h3>
        <button class="modal-close" onclick={closeEditScript}>✕</button>
      </div>
      <div class="modal-body">
        <div class="form-grid">
          <label class="form-field">
            <span>标题</span>
            <input bind:value={editName} />
          </label>
          <label class="form-field">
            <span>分类</span>
            <input bind:value={editCategory} />
          </label>
          <label class="form-field">
            <span>作者</span>
            <input bind:value={editAuthor} />
          </label>
          <label class="form-field">
            <span>版本</span>
            <input bind:value={editVersion} />
          </label>
        </div>
        <label class="form-field full">
          <span>功能摘要</span>
          <input bind:value={editFeature} />
        </label>
        <label class="form-field full">
          <span>描述</span>
          <textarea bind:value={editDescription} rows="3"></textarea>
        </label>
        <label class="form-field full">
          <span>脚本内容</span>
          <textarea bind:value={editContent} rows="14" class="code-editor" spellcheck="false"></textarea>
        </label>
        {#if editError}
          <p class="upload-error">{editError}</p>
        {/if}
        <div class="upload-actions">
          <button class="cancel-btn" onclick={closeEditScript}>取消</button>
          <button class="submit-btn" onclick={updateScript} disabled={editLoading}>
            {editLoading ? '保存中...' : '保存更新'}
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .scripts-page {
    width: 100%;
    max-width: none;
    margin: 0;
  }

  .page-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 18px;
  }

  .header-left {
    display: flex;
    align-items: baseline;
    gap: 12px;
  }

  .page-title {
    font-size: 20px;
    font-weight: 600;
    color: var(--text-primary);
    letter-spacing: -0.3px;
  }

  .view-toggle {
    display: flex;
    background: var(--bg-secondary);
    border: 1px solid var(--border-primary);
    border-radius: 8px;
    padding: 2px;
  }

  .sort-select {
    background: var(--bg-input);
    border: 1px solid var(--border-primary);
    color: var(--text-secondary);
    font-size: 12px;
    padding: 6px 28px 6px 10px;
    border-radius: 8px;
    cursor: pointer;
    transition: all 0.2s;
    appearance: none;
    background-image: url("data:image/svg+xml;charset=utf-8,%3Csvg xmlns='http://www.w3.org/2000/svg' width='10' height='10' viewBox='0 0 24 24' fill='none' stroke='%2394a3b8' stroke-width='2.5'%3E%3Cpath d='m6 9 6 6 6-6'/%3E%3C/svg%3E");
    background-repeat: no-repeat;
    background-position: right 10px center;
    outline: none;
  }
  .sort-select:hover { color: var(--text-primary); border-color: rgba(34, 211, 238, 0.2); }
  .sort-select:focus { border-color: rgba(34, 211, 238, 0.4); box-shadow: 0 0 0 3px rgba(34, 211, 238, 0.08); }
  .sort-select option { background: var(--bg-card); color: var(--text-primary); }

  .upload-btn {
    display: flex; align-items: center; gap: 6px;
    padding: 6px 14px; border-radius: 8px;
    background: linear-gradient(135deg, rgba(16, 185, 129, 0.15), rgba(5, 150, 105, 0.15));
    border: 1px solid rgba(16, 185, 129, 0.25);
    color: #10b981; font-size: 12px; font-weight: 600;
    cursor: pointer; transition: all 0.2s;
  }
  .upload-btn:hover { background: linear-gradient(135deg, #10b981, #059669); color: #fff; transform: translateY(-1px); box-shadow: 0 4px 14px rgba(16, 185, 129, 0.3); }
  .filter-toggle-btn { background: var(--bg-secondary); border-color: rgba(34, 211, 238, 0.18); color: var(--accent-primary); }

  .modal-overlay { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.6); z-index: 100; display: flex; align-items: center; justify-content: center; }
  .modal { width: 400px; max-width: 90vw; max-height: 86vh; background: var(--bg-card); border: 1px solid var(--border-primary); border-radius: 14px; overflow: hidden; display: flex; flex-direction: column; }
  .script-edit-modal { width: min(860px, 92vw); }
  .modal-header { display: flex; justify-content: space-between; align-items: center; padding: 14px 18px; border-bottom: 1px solid var(--border-primary); }
  .modal-header h3 { margin: 0; font-size: 15px; color: var(--text-primary); }
  .modal-close { background: none; border: none; color: var(--text-tertiary); font-size: 18px; cursor: pointer; padding: 4px 8px; border-radius: 6px; }
  .modal-close:hover { background: var(--bg-hover); }
  .modal-body { padding: 18px; overflow: auto; min-height: 0; }
  .form-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; margin-bottom: 12px; }
  .form-field { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
  .form-field.full { margin-bottom: 12px; }
  .form-field span { font-size: 12px; color: var(--text-secondary); font-weight: 700; }
  .form-field input,
  .form-field textarea {
    width: 100%;
    box-sizing: border-box;
    padding: 9px 10px;
    border-radius: 8px;
    border: 1px solid var(--border-primary);
    background: var(--bg-input);
    color: var(--text-primary);
    font-size: 13px;
    outline: none;
    resize: vertical;
  }
  .form-field input:focus,
  .form-field textarea:focus { border-color: var(--border-focus); box-shadow: 0 0 0 3px var(--accent-primary-light); }
  .code-editor { font-family: var(--theme-font-family-mono); line-height: 1.55; white-space: pre; overflow: auto; min-height: 220px; height: clamp(220px, 34vh, 420px); resize: vertical; }
  .upload-area { margin-bottom: 16px; }
  .file-input { display: none; }
  .file-label { display: flex; flex-direction: column; align-items: center; gap: 8px; padding: 24px; border: 2px dashed var(--border-primary); border-radius: 10px; cursor: pointer; transition: all 0.2s; color: var(--text-secondary); }
  .file-label:hover { border-color: var(--accent-primary); color: var(--accent-primary); }
  .upload-error { color: #ef4444; font-size: 12px; margin-bottom: 12px; }
  .upload-actions { display: flex; gap: 10px; justify-content: flex-end; }
  .advanced-toggle { width: 100%; display: flex; align-items: center; justify-content: space-between; margin: 0 0 12px; padding: 9px 12px; border: 1px solid var(--border-primary); border-radius: 8px; background: var(--bg-secondary); color: var(--text-secondary); cursor: pointer; font-size: 12px; }
  .advanced-toggle:hover { border-color: var(--accent-primary); color: var(--accent-primary); }
  .advanced-panel { margin-bottom: 12px; padding: 12px; border: 1px solid var(--border-secondary); border-radius: 10px; background: var(--bg-secondary); }
  .cancel-btn { padding: 8px 16px; background: var(--bg-secondary); border: 1px solid var(--border-primary); border-radius: 8px; color: var(--text-secondary); font-size: 13px; cursor: pointer; }
  .submit-btn { padding: 8px 16px; background: var(--accent-primary); color: white; border: none; border-radius: 8px; font-size: 13px; font-weight: 600; cursor: pointer; }
  .submit-btn:disabled { opacity: 0.5; cursor: not-allowed; }

  .card-badge {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    font-size: 10px;
    font-weight: 600;
    color: #22d3ee;
    background: rgba(34, 211, 238, 0.1);
    border: 1px solid rgba(34, 211, 238, 0.2);
    padding: 2px 7px;
    border-radius: 10px;
    flex-shrink: 0;
  }

  .card-run-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    border: none;
    background: linear-gradient(135deg, rgba(34, 211, 238, 0.15), rgba(6, 182, 212, 0.15));
    color: #22d3ee;
    border-radius: 8px;
    cursor: pointer;
    transition: all 0.2s;
    flex-shrink: 0;
    opacity: 0.5;
    border: 1px solid rgba(34, 211, 238, 0.15);
  }
  .script-card:hover .card-run-btn { opacity: 1; }
  .card-run-btn:hover {
    background: linear-gradient(135deg, #22d3ee, #0891b2);
    color: #fff;
    transform: scale(1.08);
    box-shadow: 0 4px 14px rgba(34, 211, 238, 0.4);
  }

  .card-fav-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    border: none;
    background: transparent;
    color: #4b5563;
    font-size: 14px;
    cursor: pointer;
    transition: all 0.15s;
    flex-shrink: 0;
    opacity: 0.4;
    border-radius: 6px;
  }
  .script-card:hover .card-fav-btn { opacity: 0.8; }
  .card-fav-btn:hover { color: #fbbf24; background: rgba(251, 191, 36, 0.1); opacity: 1; }
  .card-fav-btn.faved { color: #fbbf24; opacity: 1; }

  .fav-chip {
    border-color: rgba(251, 191, 36, 0.15) !important;
  }
  .fav-chip.active {
    background: rgba(251, 191, 36, 0.1) !important;
    border-color: rgba(251, 191, 36, 0.3) !important;
    color: #fbbf24 !important;
  }
  .fav-chip.active .chip-count {
    background: rgba(251, 191, 36, 0.15) !important;
    color: #fbbf24 !important;
  }

  .source-pill {
    display: inline-flex;
    align-items: center;
    height: 20px;
    padding: 0 7px;
    border-radius: 999px;
    border: 1px solid rgba(148, 163, 184, .18);
    background: rgba(148, 163, 184, .08);
    color: var(--text-secondary);
    font-size: 10px;
    font-weight: 800;
    line-height: 1;
    white-space: nowrap;
  }
  .source-pill.user-source {
    color: #34d399;
    border-color: rgba(52, 211, 153, .28);
    background: rgba(52, 211, 153, .1);
  }
  .source-pill.builtin-source {
    color: #93c5fd;
    border-color: rgba(96, 165, 250, .28);
    background: rgba(96, 165, 250, .1);
  }

  .row-run-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border: none;
    background: rgba(34, 211, 238, 0.08);
    color: #22d3ee;
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.2s;
    flex-shrink: 0;
    margin-left: 4px;
    border: 1px solid rgba(34, 211, 238, 0.12);
  }

  .card-edit-btn,
  .card-delete-btn,
  .row-edit-btn,
  .row-delete-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    height: 26px;
    padding: 0 8px;
    border-radius: 7px;
    border: 1px solid var(--border-primary);
    background: var(--bg-secondary);
    color: var(--text-secondary);
    font-size: 11px;
    cursor: pointer;
    flex-shrink: 0;
  }
  .card-edit-btn:hover,
  .row-edit-btn:hover { color: var(--accent-primary); border-color: var(--border-focus); background: var(--accent-primary-light); }
  .card-delete-btn:hover,
  .row-delete-btn:hover { color: #ef4444; border-color: rgba(239, 68, 68, 0.32); background: rgba(239, 68, 68, 0.08); }
  .script-row:hover .row-run-btn { background: rgba(34, 211, 238, 0.15); }
  .row-run-btn:hover {
    background: linear-gradient(135deg, #22d3ee, #0891b2);
    color: #fff;
    transform: scale(1.08);
  }

  .row-fav-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border: none;
    background: transparent;
    color: #4b5563;
    font-size: 13px;
    cursor: pointer;
    transition: all 0.15s;
    flex-shrink: 0;
    border-radius: 6px;
    opacity: 0.4;
  }
  .script-row:hover .row-fav-btn { opacity: 0.8; }
  .row-fav-btn:hover { color: #fbbf24; background: rgba(251, 191, 36, 0.1); opacity: 1; }
  .row-fav-btn.faved { color: #fbbf24; opacity: 1; }

  .row-badge {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    font-size: 10px;
    font-weight: 600;
    color: #22d3ee;
    background: rgba(34, 211, 238, 0.08);
    border: 1px solid rgba(34, 211, 238, 0.15);
    padding: 1px 6px;
    border-radius: 8px;
    margin-left: 2px;
  }

  .view-btn {
    background: none;
    border: none;
    color: #4b5563;
    padding: 6px 8px;
    border-radius: 6px;
    cursor: pointer;
    display: flex;
    align-items: center;
    transition: all 0.2s;
  }

  .view-btn:hover {
    color: #94a3b8;
  }

  .view-btn.active {
    background: rgba(34, 211, 238, 0.1);
    color: #22d3ee;
  }

  .filter-section {
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin-bottom: 20px;
  }

  .search-wrap {
    position: relative;
  }

  .search-icon {
    position: absolute;
    left: 14px;
    top: 50%;
    transform: translateY(-50%);
    color: #4b5563;
  }

  .search-input {
    width: 100%;
    padding: 12px 40px 12px 44px;
    background: var(--bg-input);
    border: 1px solid var(--border-primary);
    border-radius: 12px;
    font-size: 15px;
    color: var(--text-primary);
    transition: all 0.2s;
    outline: none;
    box-sizing: border-box;
  }

  .search-input::placeholder {
    color: var(--text-tertiary);
  }

  .search-input:focus {
    border-color: rgba(34, 211, 238, 0.3);
    box-shadow: 0 0 0 3px rgba(34, 211, 238, 0.1);
  }

  .search-clear {
    position: absolute;
    right: 10px;
    top: 50%;
    transform: translateY(-50%);
    background: none;
    border: none;
    color: var(--text-tertiary);
    padding: 4px;
    border-radius: 4px;
    cursor: pointer;
    display: flex;
    transition: all 0.2s;
  }

  .search-clear:hover {
    color: var(--text-primary);
    background: var(--bg-hover);
  }

  .category-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .chip {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    border-radius: 18px;
    border: 1px solid var(--border-primary);
    background: var(--bg-secondary);
    color: var(--text-secondary);
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s;
  }

  .chip:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .chip.active {
    background: rgba(34, 211, 238, 0.1);
    border-color: rgba(34, 211, 238, 0.3);
    color: #22d3ee;
  }

  .chip-icon {
    font-size: 13px;
  }

  .chip-count {
    font-size: 10px;
    color: var(--text-tertiary);
    padding: 1px 6px;
    border-radius: 8px;
    background: var(--bg-tertiary);
    font-weight: 600;
  }

  .chip.active .chip-count {
    background: rgba(34, 211, 238, 0.15);
    color: #22d3ee;
  }

  .loading-wrap {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 200px;
  }

  .spinner {
    width: 32px;
    height: 32px;
    border-radius: 50%;
    border: 3px solid rgba(34, 211, 238, 0.15);
    border-top-color: #22d3ee;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .empty-wrap {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 80px 0;
    color: #6b7280;
  }

  .empty-icon {
    font-size: 48px;
    margin-bottom: 8px;
  }

  .empty-title {
    font-size: 15px;
    color: #94a3b8;
  }

  .empty-hint {
    font-size: 12px;
    color: #4b5563;
  }

  .scripts-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: 14px;
  }

  .script-card {
    position: relative;
    display: block;
    background: var(--bg-card);
    backdrop-filter: blur(16px);
    border: 1px solid var(--border-primary);
    border-radius: 14px;
    padding: 18px;
    text-decoration: none;
    color: inherit;
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
    animation: cardSlideIn 0.4s ease-out both;
    overflow: hidden;
  }

  .script-number,
  .row-number {
    display: inline-grid;
    place-items: center;
    border-radius: 8px;
    border: 1px solid rgba(34, 211, 238, 0.24);
    background: rgba(34, 211, 238, 0.08);
    color: var(--accent-primary);
    font-family: var(--theme-font-family-mono);
    font-size: 11px;
    font-weight: 900;
    letter-spacing: 0;
    flex-shrink: 0;
  }

  .script-number {
    min-width: 44px;
    height: 24px;
  }

  .row-number {
    min-width: 44px;
    height: 26px;
  }

  @keyframes cardSlideIn {
    from { opacity: 0; transform: translateY(12px) scale(0.97); }
    to { opacity: 1; transform: translateY(0) scale(1); }
  }

  .script-card:hover {
    transform: translateY(-4px);
    border-color: rgba(34, 211, 238, 0.2);
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.3), 0 0 0 1px rgba(34, 211, 238, 0.1);
  }

  .card-glow {
    position: absolute;
    top: -50%;
    right: -50%;
    width: 100%;
    height: 100%;
    border-radius: 50%;
    opacity: 0;
    transition: opacity 0.4s;
    pointer-events: none;
  }

  .script-card:hover .card-glow {
    opacity: 1;
  }

  .card-inner {
    position: relative;
    z-index: 1;
  }

  .card-header {
    display: flex;
    align-items: center;
    justify-content: flex-start;
    gap: 8px;
    margin-bottom: 12px;
    min-width: 0;
  }

  .card-icon {
    width: 38px;
    height: 38px;
    border-radius: 11px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 17px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.25);
    transition: transform 0.3s;
  }

  .script-card:hover .card-icon {
    transform: scale(1.08) rotate(-5deg);
  }

  .card-meta {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
    min-width: 0;
  }

  .card-category {
    font-size: 10px;
    padding: 3px 9px;
    border-radius: 14px;
    background: var(--bg-secondary);
    color: var(--text-secondary);
    border: 1px solid var(--border-primary);
    font-weight: 500;
  }

  .card-version {
    font-size: 10px;
    color: var(--text-tertiary);
    font-family: var(--theme-font-family-mono);
  }

  .card-title {
    font-size: 15px;
    font-weight: 600;
    color: var(--text-primary);
    margin-bottom: 4px;
    transition: color 0.2s;
    letter-spacing: -0.2px;
  }

  .script-card:hover .card-title {
    color: #22d3ee;
  }

  .card-feature {
    font-size: 11px;
    color: #22d3ee;
    margin-bottom: 6px;
    font-weight: 500;
  }

  .card-desc {
    font-size: 12px;
    color: var(--text-secondary);
    line-height: 1.6;
    margin-bottom: 12px;
  }

  .card-example {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 12px;
    padding: 7px 10px;
    background: var(--bg-secondary);
    border-radius: 7px;
    border: 1px solid var(--border-secondary);
  }

  .example-label {
    font-size: 10px;
    color: var(--text-tertiary);
    font-weight: 500;
    flex-shrink: 0;
  }

  .example-cmd {
    font-family: var(--theme-font-family-mono);
    font-size: 11px;
    color: var(--text-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .card-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 11px;
    gap: 8px;
  }

  .card-cmd {
    font-family: var(--theme-font-family-mono);
    color: var(--text-tertiary);
    padding: 3px 8px;
    border-radius: 6px;
    background: var(--bg-secondary);
    border: 1px solid var(--border-secondary);
    font-size: 10px;
  }

  .card-author {
    color: var(--text-tertiary);
    font-size: 10px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .scripts-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .script-row {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 12px 16px;
    background: var(--bg-card);
    backdrop-filter: blur(16px);
    border: 1px solid var(--border-primary);
    border-radius: 10px;
    text-decoration: none;
    color: inherit;
    transition: all 0.2s;
    animation: rowSlideIn 0.3s ease-out both;
  }

  @keyframes rowSlideIn {
    from { opacity: 0; transform: translateX(-8px); }
    to { opacity: 1; transform: translateX(0); }
  }

  .script-row:hover {
    background: var(--bg-hover);
    border-color: rgba(34, 211, 238, 0.15);
    transform: translateX(4px);
  }

  .row-icon {
    width: 36px;
    height: 36px;
    border-radius: 10px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 16px;
    flex-shrink: 0;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
  }

  .row-info {
    flex: 1;
    min-width: 0;
  }

  .row-title-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 2px;
  }

  .row-title {
    font-size: 14px;
    font-weight: 600;
    color: var(--text-primary);
  }

  .row-id {
    font-family: var(--theme-font-family-mono);
    font-size: 11px;
    color: var(--text-tertiary);
  }

  .row-version {
    font-size: 10px;
    color: var(--text-tertiary);
    font-family: var(--theme-font-family-mono);
  }

  .row-desc {
    font-size: 12px;
    color: var(--text-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .cat-pill {
    font-size: 11px;
    padding: 3px 10px;
    border-radius: 14px;
    border: 1px solid;
    font-weight: 500;
    white-space: nowrap;
  }

  .row-cmd {
    font-family: var(--theme-font-family-mono);
    font-size: 11px;
    color: #4b5563;
    padding: 3px 8px;
    border-radius: 5px;
    background: rgba(0, 0, 0, 0.2);
    white-space: nowrap;
  }

  .row-arrow {
    color: #4b5563;
    font-size: 16px;
    transition: all 0.2s;
  }

  .script-row:hover .row-arrow {
    color: #22d3ee;
    transform: translateX(4px);
  }

  :global(.search-highlight) {
    background: rgba(34, 211, 238, 0.2);
    color: #22d3ee;
    padding: 0 2px;
    border-radius: 3px;
    font-weight: 600;
  }

  .scripts-page {
    width: 100%;
    max-width: none;
  }

  .page-header {
    align-items: flex-start;
    gap: 16px;
    margin-bottom: 16px;
  }

  .header-left {
    flex-direction: column;
    align-items: flex-start;
    gap: 10px;
  }

  .page-title {
    color: var(--text-primary);
    font-size: 20px;
    letter-spacing: 0;
  }

  .page-subtitle {
    color: var(--text-secondary);
    font-size: 13px;
    margin-top: 4px;
  }

  .summary-pills {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .summary-pill {
    display: inline-flex;
    align-items: center;
    height: 26px;
    padding: 0 9px;
    border: 1px solid var(--border-primary);
    border-radius: 8px;
    background: var(--bg-card);
    color: var(--text-secondary);
    font-size: 12px;
  }

  .header-right {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-left: auto;
  }

  .upload-btn,
  .submit-btn {
    background: var(--accent-primary);
    border: 1px solid var(--accent-primary);
    color: #fff;
    border-radius: 8px;
    box-shadow: none;
  }

  .upload-btn:hover,
  .submit-btn:hover {
    background: var(--accent-primary-hover);
    color: #fff;
    transform: none;
    box-shadow: none;
  }

  .sort-select,
  .view-toggle,
  .search-input,
  .chip {
    background: var(--bg-card);
    border-color: var(--border-primary);
    color: var(--text-secondary);
  }

  .sort-select {
    min-height: 34px;
    background-image: none;
  }

  .sort-select option {
    background: var(--bg-card);
    color: var(--text-primary);
  }

  .view-toggle {
    height: 34px;
  }

  .view-btn {
    color: var(--text-tertiary);
    width: 32px;
    justify-content: center;
  }

  .view-btn:hover {
    color: var(--text-primary);
    background: var(--bg-hover);
  }

  .view-btn.active {
    background: var(--accent-primary-light);
    color: var(--accent-primary);
  }

  .filter-section {
    padding: 14px;
    background: var(--bg-card);
    border: 1px solid var(--border-primary);
    border-radius: 10px;
  }

  .search-icon,
  .search-clear {
    color: var(--text-tertiary);
  }

  .search-input {
    height: 42px;
    border-radius: 8px;
    color: var(--text-primary);
  }

  .search-input::placeholder {
    color: var(--text-tertiary);
  }

  .search-input:focus {
    border-color: var(--border-focus);
    box-shadow: 0 0 0 3px var(--accent-primary-light);
  }

  .chip {
    height: 30px;
    border-radius: 8px;
    font-weight: 600;
  }

  .chip:hover {
    color: var(--text-primary);
    background: var(--bg-hover);
  }

  .chip.active {
    background: var(--accent-primary-light);
    border-color: var(--border-focus);
    color: var(--accent-primary);
  }

  .chip-icon {
    min-width: 26px;
    font-family: var(--theme-font-family-mono);
    font-size: 10px;
    color: inherit;
  }

  .chip-count,
  .chip.active .chip-count,
  .fav-chip.active .chip-count {
    background: var(--bg-secondary) !important;
    color: inherit !important;
  }

  .empty-wrap {
    background: var(--bg-card);
    border: 1px solid var(--border-primary);
    border-radius: 10px;
    color: var(--text-secondary);
  }

  .empty-icon {
    font-family: var(--theme-font-family-mono);
    font-size: 36px;
    color: var(--text-tertiary);
  }

  .empty-title {
    color: var(--text-primary);
  }

  .empty-hint {
    color: var(--text-secondary);
  }

  .script-card,
  .script-row {
    background: var(--bg-card);
    border-color: var(--border-primary);
    border-radius: 10px;
    box-shadow: none;
    backdrop-filter: none;
    animation: none;
  }

  .script-card {
    padding: 0;
  }

  .script-card:hover,
  .script-row:hover {
    transform: none;
    border-color: var(--border-focus);
    box-shadow: 0 10px 28px rgba(15, 23, 42, 0.08);
    background: var(--bg-card);
  }

  .card-glow {
    display: none;
  }

  .card-inner {
    padding: 16px;
  }

  .card-header {
    gap: 8px;
    justify-content: flex-start;
  }

  .card-icon,
  .row-icon {
    border-radius: 8px;
    box-shadow: none;
    font-family: var(--theme-font-family-mono);
    font-size: 11px;
    font-weight: 700;
    color: #fff;
  }

  .script-card:hover .card-icon {
    transform: none;
  }

  .card-meta {
    margin-right: auto;
  }

  .card-category,
  .cat-pill {
    background: var(--bg-secondary) !important;
    border-color: var(--border-primary) !important;
    color: var(--text-secondary) !important;
    border-radius: 8px;
  }

  .card-title,
  .row-title {
    color: var(--text-primary);
    letter-spacing: 0;
  }

  .script-card:hover .card-title {
    color: var(--accent-primary);
  }

  .card-feature {
    color: var(--accent-primary);
  }

  .card-desc,
  .row-desc,
  .card-author,
  .row-id,
  .row-version,
  .card-version {
    color: var(--text-secondary);
  }

  .card-example,
  .card-cmd,
  .row-cmd {
    background: var(--bg-secondary);
    border-color: var(--border-secondary);
    color: var(--text-secondary);
  }

  .example-cmd {
    color: var(--text-primary);
  }

  .card-run-btn,
  .row-run-btn {
    background: var(--accent-primary-light);
    border-color: transparent;
    color: var(--accent-primary);
    opacity: 1;
    border-radius: 8px;
  }

  .card-run-btn:hover,
  .row-run-btn:hover {
    background: var(--accent-primary);
    color: #fff;
    transform: none;
    box-shadow: none;
  }

  .card-fav-btn,
  .row-fav-btn {
    color: var(--text-tertiary);
    opacity: 1;
  }

  .card-fav-btn.faved,
  .row-fav-btn.faved,
  .card-fav-btn:hover,
  .row-fav-btn:hover {
    color: #d97706;
    background: rgba(217, 119, 6, 0.1);
  }

  .card-badge,
  .row-badge {
    color: var(--accent-primary);
    background: var(--accent-primary-light);
    border-color: transparent;
    border-radius: 8px;
  }

  .row-arrow {
    color: var(--text-tertiary);
  }

  .script-row:hover .row-arrow {
    color: var(--accent-primary);
    transform: none;
  }

  .modal-overlay {
    background: rgba(15, 18, 24, 0.55);
    backdrop-filter: blur(4px);
  }

  .modal {
    border-radius: 10px;
    box-shadow: 0 20px 60px rgba(15, 23, 42, 0.22);
  }

  @media (max-width: 768px) {
    .scripts-grid {
      grid-template-columns: 1fr;
    }
    .category-chips {
      overflow-x: auto;
      flex-wrap: nowrap;
      padding-bottom: 4px;
    }
    .chip {
      flex-shrink: 0;
    }
    .page-header { flex-direction: column; align-items: flex-start; gap: 10px; }
    .header-right { width: 100%; }
    .sort-select { flex: 1; }
    .script-card { padding: 14px; }
    .script-row { padding: 10px 12px; }
    .row-cmd { display: none; }
  }
</style>
