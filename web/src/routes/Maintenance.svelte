<script>
  import { onMount } from 'svelte';
  import ConfirmDialog from '../lib/ConfirmDialog.svelte';

  let records = $state([]);
  let loading = $state(true);
  let search = $state('');
  let filterCategory = $state('');
  let editingId = $state(null);
  let editTitle = $state('');
  let editContent = $state('');
  let editCategory = $state('常规维护');
  let editOperator = $state('system');
  let saveTimer = null;
  let saving = $state(false);
  let lastSaved = $state('');
  let showNew = $state(false);
  let newTitle = $state('');
  let newContent = $state('');
  let newCategory = $state('常规维护');
  let newOperator = $state('system');
  let newError = $state(null);
  let sortKey = $state('timestamp');
  let sortDir = $state('desc');
  let pendingDeleteId = $state('');
  let deleteLoading = $state(false);
  let importInput = $state(null);
  let importingRecord = $state(false);
  let importMessage = $state('');
  let importError = $state('');

  async function load() {
    loading = true;
    try {
      const r = await fetch('/api/maintenance');
      if (r.ok) { const d = await r.json(); records = d.records || []; }
    } catch (e) { console.warn('加载维护记录失败:', e); }
    loading = false;
  }

  function openEditor(record) {
    editingId = record.id;
    editTitle = record.title;
    editContent = record.description || '';
    editCategory = record.category;
    editOperator = record.operator;
    lastSaved = '';
  }

  function closeEditor() {
    editingId = null;
    editTitle = '';
    editContent = '';
    lastSaved = '';
    if (saveTimer) { clearTimeout(saveTimer); saveTimer = null; }
  }

  function scheduleSave() {
    lastSaved = '未保存';
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(autoSave, 1000);
  }

  async function autoSave() {
    if (!editingId) return;
    saving = true;
    try {
      const r = await fetch('/api/maintenance/' + editingId + '/complete', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ result: editContent }),
      });
      if (r.ok) {
        lastSaved = '已保存 ' + new Date().toLocaleTimeString('zh-CN', { hour12: false });
        load();
      }
    } catch (e) { console.warn('自动保存失败:', e); }
    saving = false;
  }

  async function createRecord() {
    if (!newTitle.trim()) { newError = '请填写标题'; return; }
    newError = null;
    try {
      const r = await fetch('/api/maintenance', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ title: newTitle, description: newContent, category: newCategory, operator: newOperator }),
      });
      if (r.ok) {
        showNew = false;
        newTitle = '';
        newContent = '';
        load();
      } else { newError = '创建失败'; }
    } catch (e) { newError = e.message; }
  }

  function statusColor(s) {
    if (s === 'open') return '#fbbf24';
    if (s === 'completed') return '#34d399';
    return '#6b7280';
  }

  function statusLabel(s) {
    if (s === 'open') return '进行中';
    if (s === 'completed') return '已完成';
    return s;
  }

  function changeSort(key) {
    if (sortKey === key) sortDir = sortDir === 'asc' ? 'desc' : 'asc';
    else {
      sortKey = key;
      sortDir = key === 'timestamp' ? 'desc' : 'asc';
    }
  }

  function sortMark(key) {
    if (sortKey !== key) return '';
    return sortDir === 'asc' ? ' ↑' : ' ↓';
  }

  function recordSortValue(record, key) {
    if (key === 'timestamp') return record.timestamp || '';
    if (key === 'status') return record.status || '';
    if (key === 'category') return record.category || '';
    if (key === 'operator') return record.operator || '';
    return record.title || '';
  }

  async function deleteRecord(id) {
    pendingDeleteId = id;
  }

  async function confirmDeleteRecord() {
    const id = pendingDeleteId;
    if (!id) return;
    deleteLoading = true;
    try {
      const r = await fetch('/api/maintenance/' + id, { method: 'DELETE' });
      if (r.ok) {
        pendingDeleteId = '';
        if (editingId === id) closeEditor();
        load();
      }
    } catch (e) { console.warn('删除记录失败:', e); }
    deleteLoading = false;
  }

  async function toggleComplete(record) {
    if (record.status === 'completed') {
      try {
        const r = await fetch('/api/maintenance/' + record.id + '/complete', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ result: record.result || '' }),
        });
        if (r.ok) load();
      } catch (e) { console.warn('切换状态失败:', e); }
    } else {
      openEditor(record);
    }
  }

  async function importMaintenanceFile(event) {
    const file = event.currentTarget.files?.[0];
    if (!file) return;
    importingRecord = true;
    importMessage = '';
    importError = '';
    try {
      const form = new FormData();
      form.append('file', file);
      const r = await fetch('/api/maintenance/import', { method: 'POST', body: form });
      const d = await r.json().catch(() => ({}));
      if (!r.ok || d.status === 'error') throw new Error(d.message || '导入失败');
      importMessage = d.message || '维护记录已导入';
      await load();
    } catch (e) {
      importError = e.message || '导入失败';
    } finally {
      importingRecord = false;
      event.currentTarget.value = '';
      setTimeout(() => { importMessage = ''; importError = ''; }, 3600);
    }
  }

  let categories = $derived.by(() => {
    const cats = new Set();
    for (const r of records) cats.add(r.category);
    return [...cats].sort();
  });

  let filtered = $derived.by(() => {
    let result = records;
    if (filterCategory) result = result.filter(r => r.category === filterCategory);
    if (search.trim()) {
      const q = search.toLowerCase();
      result = result.filter(r => r.title.toLowerCase().includes(q) || r.description.toLowerCase().includes(q) || r.operator.toLowerCase().includes(q));
    }
    return [...result].sort((a, b) => {
      const av = recordSortValue(a, sortKey);
      const bv = recordSortValue(b, sortKey);
      let cmp = String(av).localeCompare(String(bv), 'zh-CN');
      if (sortDir === 'desc') cmp = -cmp;
      return cmp;
    });
  });

  onMount(load);
</script>

<div class="maint-page">
  <div class="page-header">
    <div class="header-left">
      <span class="record-count">{records.length} 条记录</span>
      {#if saving}
        <span class="save-status saving">保存中...</span>
      {:else if lastSaved}
        <span class="save-status">{lastSaved}</span>
      {/if}
    </div>
    <div class="header-right">
      <div class="sort-group" aria-label="维护记录排序">
        <button class="sort-chip" onclick={() => changeSort('timestamp')}>时间{sortMark('timestamp')}</button>
        <button class="sort-chip" onclick={() => changeSort('title')}>标题{sortMark('title')}</button>
        <button class="sort-chip" onclick={() => changeSort('category')}>分类{sortMark('category')}</button>
        <button class="sort-chip" onclick={() => changeSort('status')}>状态{sortMark('status')}</button>
        <button class="sort-chip" onclick={() => changeSort('operator')}>操作人{sortMark('operator')}</button>
      </div>
      <div class="search-wrap">
        <svg class="search-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.35-4.35" stroke-linecap="round"/></svg>
        <input type="text" placeholder="搜索记录..." bind:value={search} class="search-input" />
        {#if search}
          <button class="search-clear" onclick={() => search = ''}>✕</button>
        {/if}
      </div>
      <button class="create-btn" onclick={() => { showNew = true; newError = null; }}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
        新建记录
      </button>
      <button class="create-btn import-btn" onclick={() => importInput?.click()} disabled={importingRecord}>
        {importingRecord ? '解析中...' : '上传解析'}
      </button>
      <input bind:this={importInput} class="hidden-file" type="file" accept=".md,.txt,.json,text/markdown,text/plain,application/json" onchange={importMaintenanceFile} />
      <button class="action-btn" onclick={load}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 12a9 9 0 0 0-9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"/><path d="M3 3v5h5"/><path d="M3 12a9 9 0 0 0 9 9 9.75 9.75 0 0 0 6.74-2.74L21 16"/><path d="M16 16h5v5"/></svg>
      </button>
    </div>
  </div>
  {#if importMessage}<div class="import-notice ok">{importMessage}</div>{/if}
  {#if importError}<div class="import-notice error">{importError}</div>{/if}

  {#if categories.length > 0}
    <div class="category-chips">
      <button class="chip" class:active={!filterCategory} onclick={() => filterCategory = ''}>
        <span>全部</span>
        <span class="chip-count">{records.length}</span>
      </button>
      {#each categories as cat}
        <button class="chip" class:active={filterCategory === cat} onclick={() => filterCategory = cat}>
          <span>{cat}</span>
          <span class="chip-count">{records.filter(r => r.category === cat).length}</span>
        </button>
      {/each}
    </div>
  {/if}

  {#if showNew}
    <div class="editor-panel new-record">
      <div class="editor-header">
        <h3>新建维护记录</h3>
        <button class="close-btn" onclick={() => { showNew = false; newError = null; }}>✕</button>
      </div>
      <div class="editor-body">
        <div class="editor-meta">
          <input type="text" bind:value={newTitle} placeholder="记录标题" class="editor-title-input" />
          <div class="meta-row">
            <select bind:value={newCategory} class="meta-select">
              <option>常规维护</option>
              <option>紧急修复</option>
              <option>系统升级</option>
              <option>数据迁移</option>
            </select>
            <input type="text" bind:value={newOperator} placeholder="操作人" class="meta-input" />
          </div>
        </div>
        <textarea bind:value={newContent} placeholder="开始记录维护内容..." class="editor-textarea" rows="8"></textarea>
        {#if newError}
          <p class="form-error">{newError}</p>
        {/if}
        <div class="editor-actions">
          <button class="cancel-btn" onclick={() => { showNew = false; newError = null; }}>取消</button>
          <button class="submit-btn" onclick={createRecord}>创建记录</button>
        </div>
      </div>
    </div>
  {/if}

  {#if editingId}
    <div class="editor-panel">
      <div class="editor-header">
        <div class="editor-title-row">
          <span class="editor-icon">📝</span>
          <input type="text" bind:value={editTitle} class="editor-title-input" oninput={scheduleSave} />
        </div>
        <div class="editor-tools">
          {#if saving}
            <span class="save-indicator">保存中...</span>
          {:else if lastSaved}
            <span class="save-indicator saved">{lastSaved}</span>
          {/if}
          <button class="close-btn" onclick={closeEditor}>✕</button>
        </div>
      </div>
      <div class="editor-toolbar">
        <span class="toolbar-label">分类: {editCategory}</span>
        <span class="toolbar-sep">|</span>
        <span class="toolbar-label">操作人: {editOperator}</span>
      </div>
      <textarea bind:value={editContent} oninput={scheduleSave} placeholder="输入维护内容..." class="editor-textarea" rows="16"></textarea>
    </div>
  {/if}

  {#if loading}
    <div class="loading"><div class="spinner"></div><span>加载中...</span></div>
  {:else if filtered.length === 0}
    <div class="empty-state">
      <span class="empty-icon">📋</span>
      <span class="empty-text">暂无维护记录</span>
      <span class="empty-hint">点击"新建记录"创建第一条维护记录</span>
    </div>
  {:else}
    <div class="timeline">
      {#each filtered as record}
        <div class="timeline-item" class:completed={record.status === 'completed'} class:editing={editingId === record.id}>
          <div class="timeline-dot" style="background:{statusColor(record.status)}"></div>
          <div class="timeline-content" onclick={() => record.status === 'open' && openEditor(record)}>
            <div class="timeline-header">
              <h3 class="timeline-title">{record.title}</h3>
              <div class="timeline-badges">
                <span class="badge badge-cat">{record.category}</span>
                <span class="badge" style="color:{statusColor(record.status)};border-color:{statusColor(record.status)}33">{statusLabel(record.status)}</span>
              </div>
            </div>
            <div class="timeline-meta">
              <span>👤 {record.operator}</span>
              <span>🕐 {record.timestamp}</span>
            </div>
            {#if record.description}
              <p class="timeline-desc">{record.description}</p>
            {/if}
            {#if record.status === 'completed' && record.result}
              <div class="timeline-result">
                <span class="result-icon">✓</span>
                <span class="result-text">{record.result}</span>
              </div>
            {/if}
            <div class="timeline-actions">
              {#if record.status === 'open'}
                <button class="timeline-btn btn-complete" onclick={() => openEditor(record)}>编辑</button>
              {/if}
              <button class="timeline-btn btn-delete" onclick={() => deleteRecord(record.id)}>删除</button>
            </div>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<ConfirmDialog
  open={Boolean(pendingDeleteId)}
  title="删除维护记录"
  message="确认删除这条维护记录？删除后不会再出现在维护时间线中。"
  detail={pendingDeleteId ? `记录 ID: ${pendingDeleteId}` : ''}
  confirmText="删除记录"
  loading={deleteLoading}
  onCancel={() => pendingDeleteId = ''}
  onConfirm={confirmDeleteRecord}
/>

<style>
  .maint-page { max-width: 1200px; margin: 0 auto; }
  .page-header { display: flex; align-items: center; justify-content: space-between; gap: 16px; margin-bottom: 20px; }
  .header-left { display: flex; align-items: baseline; gap: 10px; }
  .record-count { font-size: 14px; color: #6b7280; }
  .save-status { font-size: 12px; color: #6b7280; padding: 2px 8px; background: rgba(255, 255, 255, 0.03); border-radius: 4px; }
  .save-status.saving { color: #fbbf24; }
  .header-right { display: flex; align-items: center; gap: 10px; }
  .sort-group { display: flex; align-items: center; gap: 4px; background: rgba(255, 255, 255, 0.03); border: 1px solid rgba(255, 255, 255, 0.06); border-radius: 10px; padding: 4px; }
  .sort-chip { min-height: 30px; padding: 0 9px; border: none; border-radius: 7px; background: transparent; color: #94a3b8; font-size: 12px; cursor: pointer; white-space: nowrap; }
  .sort-chip:hover { color: #e2e8f0; background: rgba(255, 255, 255, 0.06); }
  .search-wrap { position: relative; }
  .search-icon { position: absolute; left: 12px; top: 50%; transform: translateY(-50%); color: #4b5563; }
  .search-input { width: 240px; padding: 10px 14px 10px 36px; background: rgba(15, 17, 23, 0.7); border: 1px solid rgba(255, 255, 255, 0.06); border-radius: 10px; font-size: 14px; color: #e2e8f0; outline: none; box-sizing: border-box; }
  .search-input::placeholder { color: #4b5563; }
  .search-input:focus { border-color: rgba(34, 211, 238, 0.3); box-shadow: 0 0 0 3px rgba(34, 211, 238, 0.08); }
  .search-clear { position: absolute; right: 10px; top: 50%; transform: translateY(-50%); background: none; border: none; color: #4b5563; cursor: pointer; font-size: 14px; }
  .create-btn, .action-btn { display: flex; align-items: center; gap: 6px; padding: 10px 16px; background: rgba(255, 255, 255, 0.05); border: 1px solid rgba(255, 255, 255, 0.06); border-radius: 10px; color: #94a3b8; font-size: 14px; cursor: pointer; transition: all 0.2s; }
  .create-btn:hover { background: rgba(34, 211, 238, 0.1); color: #22d3ee; border-color: rgba(34, 211, 238, 0.2); }
  .create-btn:disabled { opacity: .55; cursor: wait; }
  .import-btn { color: #34d399; border-color: rgba(52,211,153,.22); background: rgba(52,211,153,.08); }
  .action-btn:hover { background: rgba(255, 255, 255, 0.06); }
  .hidden-file { display: none; }
  .import-notice { margin: -8px 0 14px; padding: 8px 10px; border-radius: 8px; font-size: 12px; font-weight: 700; }
  .import-notice.ok { color: #34d399; background: rgba(52,211,153,.08); border: 1px solid rgba(52,211,153,.18); }
  .import-notice.error { color: #f87171; background: rgba(239,68,68,.08); border: 1px solid rgba(239,68,68,.18); }

  .category-chips { display: flex; flex-wrap: wrap; gap: 8px; margin-bottom: 20px; }
  .chip { display: flex; align-items: center; gap: 6px; padding: 8px 16px; border-radius: 20px; border: 1px solid rgba(255, 255, 255, 0.06); background: rgba(15, 17, 23, 0.5); color: #6b7280; font-size: 14px; cursor: pointer; transition: all 0.2s; }
  .chip:hover { background: rgba(255, 255, 255, 0.05); }
  .chip.active { background: rgba(34, 211, 238, 0.1); border-color: rgba(34, 211, 238, 0.3); color: #22d3ee; }
  .chip-count { font-size: 12px; color: #4b5563; padding: 2px 8px; border-radius: 10px; background: rgba(255, 255, 255, 0.04); }
  .chip.active .chip-count { background: rgba(34, 211, 238, 0.15); color: #22d3ee; }

  .editor-panel { background: rgba(15, 17, 23, 0.8); border: 1px solid rgba(34, 211, 238, 0.2); border-radius: 14px; margin-bottom: 20px; overflow: hidden; }
  .editor-header { display: flex; align-items: center; justify-content: space-between; padding: 16px 20px; border-bottom: 1px solid rgba(255, 255, 255, 0.06); }
  .editor-title-row { display: flex; align-items: center; gap: 10px; flex: 1; }
  .editor-icon { font-size: 18px; }
  .editor-title-input { flex: 1; background: transparent; border: none; color: #f1f5f9; font-size: 18px; font-weight: 600; outline: none; padding: 4px 0; }
  .editor-title-input::placeholder { color: #4b5563; }
  .editor-tools { display: flex; align-items: center; gap: 10px; }
  .save-indicator { font-size: 12px; color: #6b7280; }
  .save-indicator.saved { color: #34d399; }
  .close-btn { background: none; border: none; color: #6b7280; font-size: 18px; cursor: pointer; padding: 4px 8px; border-radius: 6px; }
  .close-btn:hover { background: rgba(255, 255, 255, 0.06); color: #c9d1d9; }
  .editor-toolbar { display: flex; align-items: center; gap: 12px; padding: 8px 20px; border-bottom: 1px solid rgba(255, 255, 255, 0.04); font-size: 12px; color: #6b7280; }
  .toolbar-sep { color: #333; }
  .editor-body { padding: 20px; }
  .editor-meta { margin-bottom: 16px; }
  .meta-row { display: flex; gap: 12px; margin-top: 10px; }
  .meta-select, .meta-input { padding: 8px 12px; background: rgba(0, 0, 0, 0.3); border: 1px solid rgba(255, 255, 255, 0.06); border-radius: 8px; color: #e2e8f0; font-size: 14px; outline: none; }
  .meta-select { min-width: 120px; }
  .meta-input { flex: 1; }
  .editor-textarea { width: 100%; padding: 16px 20px; background: rgba(0, 0, 0, 0.2); border: none; color: #e2e8f0; font-size: 15px; line-height: 1.7; outline: none; resize: vertical; font-family: inherit; box-sizing: border-box; }
  .editor-textarea::placeholder { color: #4b5563; }
  .editor-actions { display: flex; gap: 10px; justify-content: flex-end; margin-top: 16px; }
  .cancel-btn { padding: 10px 20px; background: transparent; border: 1px solid rgba(255, 255, 255, 0.06); color: #94a3b8; font-size: 14px; border-radius: 8px; cursor: pointer; }
  .cancel-btn:hover { background: rgba(255, 255, 255, 0.04); }
  .submit-btn { padding: 10px 20px; background: rgba(34, 211, 238, 0.15); border: 1px solid rgba(34, 211, 238, 0.25); color: #22d3ee; font-size: 14px; font-weight: 600; border-radius: 8px; cursor: pointer; }
  .submit-btn:hover { background: linear-gradient(135deg, #22d3ee, #0891b2); color: #fff; }
  .form-error { color: #ef4444; font-size: 13px; margin-top: 8px; }

  .timeline { position: relative; padding-left: 30px; }
  .timeline::before { content: ''; position: absolute; left: 10px; top: 0; bottom: 0; width: 2px; background: rgba(255, 255, 255, 0.06); }
  .timeline-item { position: relative; margin-bottom: 16px; }
  .timeline-dot { position: absolute; left: -24px; top: 18px; width: 12px; height: 12px; border-radius: 50%; border: 2px solid rgba(15, 17, 23, 0.8); z-index: 1; }
  .timeline-content { background: rgba(15, 17, 23, 0.7); border: 1px solid rgba(255, 255, 255, 0.06); border-radius: 12px; padding: 16px 20px; cursor: pointer; transition: all 0.2s; }
  .timeline-content:hover { border-color: rgba(34, 211, 238, 0.15); }
  .timeline-item.editing .timeline-content { border-color: rgba(34, 211, 238, 0.3); background: rgba(34, 211, 238, 0.03); }
  .timeline-item.completed .timeline-content { opacity: 0.7; }
  .timeline-header { display: flex; align-items: center; justify-content: space-between; gap: 12px; margin-bottom: 8px; }
  .timeline-title { font-size: 16px; font-weight: 600; color: #f1f5f9; margin: 0; }
  .timeline-badges { display: flex; gap: 6px; }
  .badge { font-size: 11px; padding: 3px 10px; border-radius: 8px; border: 1px solid rgba(255, 255, 255, 0.1); }
  .badge-cat { color: #94a3b8; }
  .timeline-meta { display: flex; gap: 16px; font-size: 13px; color: #6b7280; margin-bottom: 8px; }
  .timeline-desc { font-size: 14px; color: #94a3b8; line-height: 1.6; margin: 0; }
  .timeline-result { display: flex; align-items: center; gap: 8px; margin-top: 10px; padding: 8px 12px; background: rgba(52, 211, 153, 0.05); border-radius: 8px; }
  .result-icon { color: #34d399; font-size: 14px; }
  .result-text { font-size: 14px; color: #34d399; }
  .edit-hint { display: inline-block; margin-top: 8px; font-size: 12px; color: #22d3ee; background: none; border: none; cursor: pointer; padding: 0; }
  .edit-hint:hover { text-decoration: underline; }

  .timeline-actions { display: flex; gap: 8px; margin-top: 10px; }
  .timeline-btn { padding: 5px 12px; border-radius: 6px; font-size: 12px; cursor: pointer; transition: all 0.15s; border: 1px solid rgba(255, 255, 255, 0.08); background: transparent; }
  .btn-complete { color: #22d3ee; border-color: rgba(34, 211, 238, 0.2); }
  .btn-complete:hover { background: rgba(34, 211, 238, 0.1); }
  .btn-delete { color: #f87171; border-color: rgba(248, 113, 113, 0.2); }
  .btn-delete:hover { background: rgba(248, 113, 113, 0.1); }

  .loading { display: flex; align-items: center; justify-content: center; gap: 10px; padding: 80px 0; color: #6b7280; font-size: 16px; }
  .spinner { width: 24px; height: 24px; border-radius: 50%; border: 3px solid rgba(34, 211, 238, 0.15); border-top-color: #22d3ee; animation: spin 0.8s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
  .empty-state { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 12px; padding: 100px 0; color: #6b7280; }
  .empty-icon { font-size: 56px; opacity: 0.5; }
  .empty-text { font-size: 18px; color: #94a3b8; }
  .empty-hint { font-size: 14px; color: #4b5563; }

  @media (max-width: 768px) {
    .page-header { flex-direction: column; align-items: stretch; gap: 12px; }
    .header-right { flex-wrap: wrap; }
    .search-input { width: 100%; }
    .meta-row { flex-direction: column; }
    .timeline-header { flex-direction: column; align-items: flex-start; }
  }
</style>
