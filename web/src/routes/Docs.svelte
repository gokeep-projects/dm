<script>
  import { onMount } from 'svelte';

  let docs = $state([]);
  let loading = $state(true);
  let search = $state('');
  let showCreate = $state(false);
  let createId = $state('');
  let createTitle = $state('');
  let createCategory = $state('通用');
  let createError = $state(null);
  let viewingDoc = $state(null);
  let viewLoading = $state(false);
  let editing = $state(false);
  let editContent = $state('');
  let editTitle = $state('');
  let editCategory = $state('');
  let saving = $state(false);
  let saveError = $state(null);
  let sortKey = $state('updated_at');
  let sortDir = $state('desc');

  async function load() {
    loading = true;
    try {
      const r = await fetch('/api/docs');
      if (r.ok) { const d = await r.json(); docs = d.docs || []; }
    } catch (e) { console.warn('加载文档列表失败:', e); }
    loading = false;
  }

  async function viewDoc(id) {
    viewLoading = true;
    try {
      const r = await fetch('/api/docs/' + encodeURIComponent(id));
      if (r.ok) viewingDoc = await r.json();
    } catch (e) { console.warn('加载文档详情失败:', e); }
    viewLoading = false;
  }

  async function createDoc() {
    if (!createId.trim() || !createTitle.trim()) { createError = '请填写文档 ID 和标题'; return; }
    createError = null;
    try {
      const r = await fetch('/api/docs', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ id: createId.trim(), title: createTitle.trim(), category: createCategory }),
      });
      if (r.ok) { showCreate = false; createId = ''; createTitle = ''; load(); }
      else { createError = '创建失败'; }
    } catch (e) { createError = e.message; }
  }

  async function deleteDoc(id) {
    if (!confirm(`确定删除文档 "${id}"？`)) return;
    try {
      await fetch('/api/docs/' + encodeURIComponent(id), { method: 'DELETE' });
      if (viewingDoc?.meta?.id === id) viewingDoc = null;
      load();
    } catch (_) {}
  }

  function closeView() { viewingDoc = null; editing = false; editContent = ''; }

  function changeSort(key) {
    if (sortKey === key) sortDir = sortDir === 'asc' ? 'desc' : 'asc';
    else {
      sortKey = key;
      sortDir = key === 'updated_at' ? 'desc' : 'asc';
    }
  }

  function sortMark(key) {
    if (sortKey !== key) return '';
    return sortDir === 'asc' ? ' ↑' : ' ↓';
  }

  function docSortValue(doc, key) {
    if (key === 'updated_at') return doc.updated_at || '';
    if (key === 'category') return doc.category || '';
    if (key === 'id') return doc.id || '';
    return doc.title || '';
  }

  function startEdit() {
    if (!viewingDoc) return;
    editing = true;
    editContent = viewingDoc.content || '';
    editTitle = viewingDoc.meta?.title || '';
    editCategory = viewingDoc.meta?.category || '通用';
  }

  async function saveDoc() {
    if (!viewingDoc) return;
    saving = true;
    saveError = null;
    try {
      const r = await fetch('/api/docs/' + encodeURIComponent(viewingDoc.meta.id), {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ title: editTitle, category: editCategory, content: editContent }),
      });
      if (r.ok) {
        editing = false;
        viewDoc(viewingDoc.meta.id);
        load();
      } else {
        saveError = '保存失败';
      }
    } catch (e) { saveError = e.message; }
    saving = false;
  }

  let filtered = $derived.by(() => {
    let result = docs;
    if (search.trim()) {
      const q = search.toLowerCase();
      result = result.filter(d => d.title.toLowerCase().includes(q) || d.id.toLowerCase().includes(q) || d.category.toLowerCase().includes(q));
    }
    return [...result].sort((a, b) => {
      const av = docSortValue(a, sortKey);
      const bv = docSortValue(b, sortKey);
      let cmp = String(av).localeCompare(String(bv), 'zh-CN');
      if (sortDir === 'desc') cmp = -cmp;
      return cmp;
    });
  });

  onMount(load);
</script>

<div class="docs-page">
  <div class="page-header">
    <div class="header-left">
      <span class="doc-count">{docs.length} 篇文档</span>
    </div>
    <div class="header-right">
      <div class="sort-group" aria-label="文档排序">
        <button class="sort-chip" onclick={() => changeSort('updated_at')}>更新{sortMark('updated_at')}</button>
        <button class="sort-chip" onclick={() => changeSort('title')}>标题{sortMark('title')}</button>
        <button class="sort-chip" onclick={() => changeSort('category')}>分类{sortMark('category')}</button>
        <button class="sort-chip" onclick={() => changeSort('id')}>ID{sortMark('id')}</button>
      </div>
      <div class="search-wrap">
        <svg class="search-icon" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.35-4.35" stroke-linecap="round"/></svg>
        <input type="text" placeholder="搜索文档..." bind:value={search} class="search-input" />
      </div>
      <button class="create-btn" onclick={() => showCreate = true}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
        新建文档
      </button>
    </div>
  </div>

  {#if showCreate}
    <div class="create-panel">
      <div class="create-head">
        <h3>新建文档</h3>
        <button class="close-btn" onclick={() => { showCreate = false; createError = null; }}>✕</button>
      </div>
      <div class="create-form">
        <div class="form-group">
          <label>文档 ID</label>
          <input type="text" bind:value={createId} placeholder="如: onboarding-guide" class="form-input" />
        </div>
        <div class="form-group">
          <label>标题</label>
          <input type="text" bind:value={createTitle} placeholder="文档标题" class="form-input" />
        </div>
        <div class="form-group">
          <label>分类</label>
          <input type="text" bind:value={createCategory} placeholder="通用" class="form-input" />
        </div>
        {#if createError}
          <p class="form-error">{createError}</p>
        {/if}
        <div class="form-actions">
          <button class="cancel-btn" onclick={() => { showCreate = false; createError = null; }}>取消</button>
          <button class="submit-btn" onclick={createDoc}>创建</button>
        </div>
      </div>
    </div>
  {/if}

  {#if viewingDoc}
    <div class="doc-viewer">
      <div class="viewer-head">
        <div class="viewer-info">
          {#if editing}
            <input type="text" bind:value={editTitle} class="edit-title-input" placeholder="文档标题" />
          {:else}
            <h2>{viewingDoc.meta?.title || viewingDoc.meta?.id}</h2>
          {/if}
          <div class="viewer-meta">
            {#if editing}
              <input type="text" bind:value={editCategory} class="edit-cat-input" placeholder="分类" />
            {:else}
              <span class="meta-tag">{viewingDoc.meta?.category}</span>
              {#each viewingDoc.meta?.tags || [] as tag}
                <span class="meta-tag">{tag}</span>
              {/each}
              <span class="meta-time">{viewingDoc.meta?.updated_at}</span>
            {/if}
          </div>
        </div>
        <div class="viewer-actions">
          {#if editing}
            <button class="save-btn" onclick={saveDoc} disabled={saving}>
              {saving ? '保存中...' : '保存'}
            </button>
            <button class="cancel-edit-btn" onclick={() => editing = false}>取消</button>
          {:else}
            <button class="edit-btn" onclick={startEdit} title="编辑">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
              <span>编辑</span>
            </button>
            <button class="action-btn" onclick={() => deleteDoc(viewingDoc.meta.id)} title="删除">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/></svg>
            </button>
          {/if}
          <button class="action-btn" onclick={closeView}>✕</button>
        </div>
      </div>
      {#if saveError}
        <div class="save-error">{saveError}</div>
      {/if}
      <div class="viewer-body">
        {#if viewLoading}
          <div class="loading"><div class="spinner"></div><span>加载中...</span></div>
        {:else if editing}
          <textarea bind:value={editContent} class="doc-editor" placeholder="输入文档内容..."></textarea>
        {:else}
          <div class="doc-content">{viewingDoc.content}</div>
        {/if}
      </div>
    </div>
  {/if}

  {#if loading}
    <div class="loading"><div class="spinner"></div><span>加载中...</span></div>
  {:else if filtered.length === 0}
    <div class="empty-state">
      <span class="empty-icon">📄</span>
      <span class="empty-text">暂无文档</span>
      <span class="empty-hint">点击"新建文档"创建第一篇维护文档</span>
    </div>
  {:else}
    <div class="docs-grid">
      {#each filtered as doc, i (doc.id)}
        <button class="doc-card" style="animation-delay:{Math.min(i * 30, 500)}ms" onclick={() => viewDoc(doc.id)}>
          <div class="doc-icon">📄</div>
          <div class="doc-info">
            <h3 class="doc-title">{doc.title}</h3>
            <p class="doc-id">{doc.id}</p>
            <div class="doc-footer">
              <span class="doc-cat">{doc.category}</span>
              <span class="doc-time">{doc.updated_at}</span>
            </div>
          </div>
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .docs-page { max-width: 1200px; margin: 0 auto; }
  .page-header { display: flex; align-items: center; justify-content: space-between; gap: 16px; margin-bottom: 20px; }
  .header-left { display: flex; align-items: baseline; gap: 10px; }
  .doc-count { font-size: 14px; color: #6b7280; }
  .header-right { display: flex; align-items: center; gap: 10px; }
  .sort-group { display: flex; align-items: center; gap: 4px; background: rgba(255, 255, 255, 0.03); border: 1px solid rgba(255, 255, 255, 0.06); border-radius: 10px; padding: 4px; }
  .sort-chip { min-height: 30px; padding: 0 9px; border: none; border-radius: 7px; background: transparent; color: #94a3b8; font-size: 12px; cursor: pointer; }
  .sort-chip:hover { color: #e2e8f0; background: rgba(255, 255, 255, 0.06); }
  .search-wrap { position: relative; }
  .search-icon { position: absolute; left: 12px; top: 50%; transform: translateY(-50%); color: #4b5563; }
  .search-input { width: 240px; padding: 10px 14px 10px 36px; background: rgba(15, 17, 23, 0.7); border: 1px solid rgba(255, 255, 255, 0.06); border-radius: 10px; font-size: 14px; color: #e2e8f0; outline: none; transition: all 0.2s; box-sizing: border-box; }
  .search-input::placeholder { color: #4b5563; }
  .search-input:focus { border-color: rgba(34, 211, 238, 0.3); box-shadow: 0 0 0 3px rgba(34, 211, 238, 0.08); }
  .create-btn { display: flex; align-items: center; gap: 6px; padding: 10px 18px; background: linear-gradient(135deg, rgba(34, 211, 238, 0.15), rgba(6, 182, 212, 0.15)); border: 1px solid rgba(34, 211, 238, 0.25); border-radius: 10px; color: #22d3ee; font-size: 14px; font-weight: 600; cursor: pointer; transition: all 0.2s; }
  .create-btn:hover { background: linear-gradient(135deg, #22d3ee, #0891b2); color: #fff; transform: translateY(-1px); box-shadow: 0 4px 14px rgba(34, 211, 238, 0.3); }

  .create-panel { background: rgba(15, 17, 23, 0.7); border: 1px solid rgba(255, 255, 255, 0.06); border-radius: 14px; padding: 20px; margin-bottom: 16px; }
  .create-head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 16px; }
  .create-head h3 { margin: 0; font-size: 16px; font-weight: 600; color: #f1f5f9; }
  .close-btn { background: none; border: none; color: #6b7280; font-size: 18px; cursor: pointer; padding: 4px 8px; border-radius: 6px; }
  .close-btn:hover { background: rgba(255, 255, 255, 0.06); color: #c9d1d9; }
  .create-form { display: flex; flex-wrap: wrap; gap: 14px; align-items: flex-end; }
  .form-group { display: flex; flex-direction: column; gap: 6px; flex: 1; min-width: 150px; }
  .form-group label { font-size: 13px; color: #94a3b8; font-weight: 500; }
  .form-input { padding: 10px 14px; background: rgba(0, 0, 0, 0.3); border: 1px solid rgba(255, 255, 255, 0.06); border-radius: 8px; color: #e2e8f0; font-size: 14px; outline: none; transition: all 0.15s; box-sizing: border-box; }
  .form-input:focus { border-color: rgba(34, 211, 238, 0.3); }
  .form-error { color: #ef4444; font-size: 13px; width: 100%; }
  .form-actions { display: flex; gap: 10px; }
  .cancel-btn { padding: 10px 18px; background: transparent; border: 1px solid rgba(255, 255, 255, 0.06); color: #94a3b8; font-size: 14px; border-radius: 8px; cursor: pointer; }
  .cancel-btn:hover { background: rgba(255, 255, 255, 0.04); }
  .submit-btn { padding: 10px 18px; background: rgba(34, 211, 238, 0.15); border: 1px solid rgba(34, 211, 238, 0.25); color: #22d3ee; font-size: 14px; font-weight: 600; border-radius: 8px; cursor: pointer; }
  .submit-btn:hover { background: linear-gradient(135deg, #22d3ee, #0891b2); color: #fff; }

  .doc-viewer { background: rgba(15, 17, 23, 0.7); border: 1px solid rgba(255, 255, 255, 0.06); border-radius: 14px; margin-bottom: 16px; overflow: hidden; }
  .viewer-head { display: flex; align-items: center; justify-content: space-between; padding: 14px 16px; border-bottom: 1px solid rgba(255, 255, 255, 0.06); }
  .viewer-info h2 { margin: 0 0 6px; font-size: 16px; font-weight: 600; color: #f1f5f9; }
  .viewer-meta { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; }
  .meta-tag { font-size: 10px; color: #94a3b8; background: rgba(255, 255, 255, 0.04); padding: 2px 7px; border-radius: 4px; border: 1px solid rgba(255, 255, 255, 0.06); }
  .meta-time { font-size: 10px; color: #6b7280; }
  .viewer-actions { display: flex; gap: 6px; align-items: center; }
  .action-btn { display: flex; align-items: center; justify-content: center; width: 28px; height: 28px; border: none; background: transparent; color: #6b7280; border-radius: 6px; cursor: pointer; transition: all 0.15s; }
  .action-btn:hover { background: rgba(255, 255, 255, 0.06); color: #c9d1d9; }
  .edit-btn { display: flex; align-items: center; gap: 5px; padding: 6px 12px; background: rgba(255, 255, 255, 0.05); border: 1px solid rgba(255, 255, 255, 0.08); color: #94a3b8; font-size: 12px; border-radius: 6px; cursor: pointer; transition: all 0.15s; }
  .edit-btn:hover { background: rgba(34, 211, 238, 0.1); color: #22d3ee; border-color: rgba(34, 211, 238, 0.2); }
  .save-btn { padding: 6px 14px; background: rgba(34, 211, 238, 0.15); border: 1px solid rgba(34, 211, 238, 0.25); color: #22d3ee; font-size: 12px; font-weight: 600; border-radius: 6px; cursor: pointer; transition: all 0.15s; }
  .save-btn:hover { background: linear-gradient(135deg, #22d3ee, #0891b2); color: #fff; }
  .save-btn:disabled { opacity: 0.5; cursor: not-allowed; }
  .cancel-edit-btn { padding: 6px 12px; background: transparent; border: 1px solid rgba(255, 255, 255, 0.08); color: #94a3b8; font-size: 12px; border-radius: 6px; cursor: pointer; }
  .cancel-edit-btn:hover { background: rgba(255, 255, 255, 0.04); }
  .edit-title-input { flex: 1; background: rgba(0, 0, 0, 0.3); border: 1px solid rgba(255, 255, 255, 0.1); border-radius: 6px; color: #f1f5f9; font-size: 16px; font-weight: 600; padding: 6px 10px; outline: none; }
  .edit-title-input:focus { border-color: rgba(34, 211, 238, 0.3); }
  .edit-cat-input { width: 100px; background: rgba(0, 0, 0, 0.3); border: 1px solid rgba(255, 255, 255, 0.1); border-radius: 4px; color: #94a3b8; font-size: 12px; padding: 3px 8px; outline: none; }
  .edit-cat-input:focus { border-color: rgba(34, 211, 238, 0.3); }
  .save-error { padding: 8px 16px; background: rgba(239, 68, 68, 0.08); border-top: 1px solid rgba(239, 68, 68, 0.15); color: #ef4444; font-size: 13px; }
  .viewer-body { padding: 16px; max-height: 60vh; overflow-y: auto; }
  .doc-content { font-size: 14px; color: #c9d1d9; line-height: 1.7; white-space: pre-wrap; font-family: var(--theme-font-family-mono); }
  .doc-editor { width: 100%; min-height: 400px; padding: 14px; background: rgba(0, 0, 0, 0.3); border: 1px solid rgba(255, 255, 255, 0.08); border-radius: 8px; color: #c9d1d9; font-size: 14px; line-height: 1.7; font-family: var(--theme-font-family-mono); outline: none; resize: vertical; box-sizing: border-box; }
  .doc-editor:focus { border-color: rgba(34, 211, 238, 0.3); }

  .docs-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 12px; }
  .doc-card { display: flex; align-items: flex-start; gap: 14px; padding: 16px; background: rgba(15, 17, 23, 0.7); border: 1px solid rgba(255, 255, 255, 0.06); border-radius: 12px; cursor: pointer; text-align: left; transition: all 0.2s; animation: cardIn 0.3s ease-out both; color: inherit; width: 100%; }
  @keyframes cardIn { from { opacity: 0; transform: translateY(8px); } to { opacity: 1; transform: translateY(0); } }
  .doc-card:hover { transform: translateY(-2px); border-color: rgba(34, 211, 238, 0.2); box-shadow: 0 8px 24px rgba(0, 0, 0, 0.2); }
  .doc-icon { font-size: 24px; flex-shrink: 0; }
  .doc-info { flex: 1; min-width: 0; }
  .doc-title { font-size: 14px; font-weight: 600; color: #f1f5f9; margin: 0 0 4px; }
  .doc-id { font-size: 11px; color: #6b7280; font-family: var(--theme-font-family-mono); margin: 0 0 8px; }
  .doc-footer { display: flex; align-items: center; gap: 8px; }
  .doc-cat { font-size: 10px; color: #94a3b8; background: rgba(255, 255, 255, 0.04); padding: 2px 6px; border-radius: 4px; }
  .doc-time { font-size: 10px; color: #6b7280; }

  .loading { display: flex; align-items: center; justify-content: center; gap: 10px; padding: 60px 0; color: #6b7280; font-size: 14px; }
  .spinner { width: 24px; height: 24px; border-radius: 50%; border: 3px solid rgba(34, 211, 238, 0.15); border-top-color: #22d3ee; animation: spin 0.8s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
  .empty-state { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px; padding: 80px 0; color: #6b7280; }
  .empty-icon { font-size: 48px; opacity: 0.5; }
  .empty-text { font-size: 15px; color: #94a3b8; }
  .empty-hint { font-size: 12px; color: #4b5563; }

  @media (max-width: 768px) {
    .page-header { flex-direction: column; align-items: stretch; gap: 10px; }
    .header-right { flex-wrap: wrap; }
    .search-input { width: 100%; }
    .create-form { flex-direction: column; }
    .form-group { min-width: auto; }
  }
</style>
