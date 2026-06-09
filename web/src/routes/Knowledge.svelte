<script>
  import { onMount } from 'svelte';
  import ConfirmDialog from '../lib/ConfirmDialog.svelte';

  let { detailId = null, detailType = null } = $props();
  let activeTab = $state('docs');
  let records = $state([]);
  let docs = $state([]);
  let loading = $state(true);
  let search = $state('');
  let showCreate = $state(false);
  let createType = $state('doc');
  let createTitle = $state('');
  let createContent = $state('');
  let createCategory = $state('常规维护');
  let createError = $state(null);
  let editingId = $state(null);
  let editContent = $state('');
  let editTitle = $state('');
  let editCategory = $state('');
  let saving = $state(false);
  let viewingItem = $state(null);
  let viewLoading = $state(false);
  let sortKey = $state('time');
  let sortDir = $state('desc');
  let pendingDelete = $state(null);
  let deleteLoading = $state(false);

  async function load() {
    loading = true;
    try {
      const [r, d] = await Promise.all([
        fetch('/api/maintenance').then(r => r.ok ? r.json() : null),
        fetch('/api/docs').then(r => r.ok ? r.json() : null),
      ]);
      if (r) records = r.records || [];
      if (d) docs = d.docs || [];
      
      // 如果有detailId，自动加载详情
      if (detailId) {
        if (detailType === 'record') {
          viewRecord(detailId);
        } else if (detailType === 'doc') {
          viewDoc(detailId);
        }
      }
    } catch (e) { console.warn('加载数据失败:', e); }
    loading = false;
  }

  async function viewRecord(id) {
    viewLoading = true;
    viewingItem = { type: 'record', id };
    try {
      const r = await fetch('/api/maintenance/' + encodeURIComponent(id));
      if (r.ok) {
        const d = await r.json();
        viewingItem = { ...viewingItem, ...d };
      }
    } catch (e) { console.warn('加载记录详情失败:', e); }
    viewLoading = false;
  }

  async function viewDoc(id) {
    viewLoading = true;
    viewingItem = { type: 'doc', id };
    try {
      const r = await fetch('/api/docs/' + encodeURIComponent(id));
      if (r.ok) {
        const d = await r.json();
        viewingItem = { ...viewingItem, ...d, meta: d.meta, content: d.content };
      }
    } catch (e) { console.warn('加载文档详情失败:', e); }
    viewLoading = false;
  }

  function closeView() {
    viewingItem = null;
    editingId = false;
    editContent = '';
  }

  function startEdit() {
    if (!viewingItem) return;
    editingId = viewingItem.id;
    editTitle = viewingItem.title || viewingItem.meta?.title || '';
    editCategory = viewingItem.category || viewingItem.meta?.category || '';
    editContent = viewingItem.content || viewingItem.description || '';
  }

  async function saveEdit() {
    if (!viewingItem || !editingId) return;
    saving = true;
    try {
      const currentId = editingId;
      if (viewingItem.type === 'doc') {
        const r = await fetch('/api/docs/' + encodeURIComponent(currentId), {
          method: 'PUT',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ title: editTitle, category: editCategory, content: editContent }),
        });
        if (r.ok) {
          editingId = null;
          viewDoc(currentId);
          load();
        }
      } else {
        const r = await fetch('/api/maintenance/' + currentId, {
          method: 'PUT',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            title: editTitle,
            category: editCategory,
            description: editContent,
            result: viewingItem.result || '',
            status: viewingItem.status || 'open',
            operator: viewingItem.operator || 'user',
          }),
        });
        if (r.ok) {
          editingId = null;
          viewRecord(currentId);
          load();
        }
      }
    } catch (e) { console.warn('保存失败:', e); }
    saving = false;
  }

  async function createItem() {
    if (!createTitle.trim()) { createError = '请填写标题'; return; }
    createError = null;
    try {
      if (createType === 'record') {
        const r = await fetch('/api/maintenance', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ title: createTitle, description: createContent, category: createCategory, operator: 'user' }),
        });
        if (r.ok) { showCreate = false; createTitle = ''; createContent = ''; createCategory = '常规维护'; load(); }
        else { createError = '创建失败'; }
      } else {
        const id = createTitle.toLowerCase().replace(/[^a-z0-9]/g, '-').replace(/-+/g, '-');
        const r = await fetch('/api/docs', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ id, title: createTitle, category: createCategory }),
        });
        if (r.ok) { showCreate = false; createTitle = ''; createContent = ''; createCategory = '常规维护'; load(); }
        else { createError = '创建失败'; }
      }
    } catch (e) { createError = e.message; }
  }

  async function deleteItem(id, type) {
    pendingDelete = { id, type };
  }

  async function confirmDeleteItem() {
    const item = pendingDelete;
    if (!item) return;
    deleteLoading = true;
    try {
      if (item.type === 'record') {
        await fetch('/api/maintenance/' + item.id, { method: 'DELETE' });
      } else {
        await fetch('/api/docs/' + encodeURIComponent(item.id), { method: 'DELETE' });
      }
      if (viewingItem?.id === item.id) closeView();
      pendingDelete = null;
      load();
    } catch (e) { console.warn('删除失败:', e); }
    deleteLoading = false;
  }

  function changeSort(key) {
    if (sortKey === key) sortDir = sortDir === 'asc' ? 'desc' : 'asc';
    else {
      sortKey = key;
      sortDir = key === 'time' ? 'desc' : 'asc';
    }
  }

  function sortMark(key) {
    if (sortKey !== key) return '';
    return sortDir === 'asc' ? ' ↑' : ' ↓';
  }

  function recordSortValue(record, key) {
    if (key === 'time') return record.timestamp || '';
    if (key === 'category') return record.category || '';
    if (key === 'status') return record.status || '';
    if (key === 'operator') return record.operator || '';
    return record.title || '';
  }

  function docSortValue(doc, key) {
    if (key === 'time') return doc.updated_at || '';
    if (key === 'category') return doc.category || '';
    if (key === 'id') return doc.id || '';
    return doc.title || '';
  }

  let filteredRecords = $derived.by(() => {
    let result = records;
    if (search.trim()) {
      const q = search.toLowerCase();
      result = result.filter(r => r.title.toLowerCase().includes(q) || r.description?.toLowerCase().includes(q) || r.category?.toLowerCase().includes(q) || r.operator?.toLowerCase().includes(q));
    }
    return [...result].sort((a, b) => {
      const av = recordSortValue(a, sortKey);
      const bv = recordSortValue(b, sortKey);
      let cmp = String(av).localeCompare(String(bv), 'zh-CN');
      if (sortDir === 'desc') cmp = -cmp;
      return cmp;
    });
  });

  let filteredDocs = $derived.by(() => {
    let result = docs;
    if (search.trim()) {
      const q = search.toLowerCase();
      result = result.filter(d => d.title.toLowerCase().includes(q) || d.id.toLowerCase().includes(q) || d.category?.toLowerCase().includes(q));
    }
    return [...result].sort((a, b) => {
      const av = docSortValue(a, sortKey);
      const bv = docSortValue(b, sortKey);
      let cmp = String(av).localeCompare(String(bv), 'zh-CN');
      if (sortDir === 'desc') cmp = -cmp;
      return cmp;
    });
  });

  function formatTime(ts) {
    if (!ts) return '-';
    try {
      const d = new Date(ts.replace(' ', 'T'));
      const diff = Math.floor((Date.now() - d.getTime()) / 1000);
      if (diff < 60) return diff + '秒前';
      if (diff < 3600) return Math.floor(diff / 60) + '分钟前';
      if (diff < 86400) return Math.floor(diff / 3600) + '小时前';
      return d.toLocaleDateString('zh-CN', { month: '2-digit', day: '2-digit' });
    } catch { return ts; }
  }

  function statusColor(s) {
    if (s === 'open') return '#f59e0b';
    if (s === 'completed') return '#10b981';
    return '#94a3b8';
  }

  onMount(load);
</script>

<div class="knowledge-page">
  <div class="page-header">
    <div class="header-left">
    </div>
    <div class="header-right">
      <div class="sort-group" aria-label="维护管理排序">
        <button class="sort-chip" onclick={() => changeSort('time')}>时间{sortMark('time')}</button>
        <button class="sort-chip" onclick={() => changeSort('title')}>标题{sortMark('title')}</button>
        <button class="sort-chip" onclick={() => changeSort('category')}>分类{sortMark('category')}</button>
        {#if activeTab === 'records'}
          <button class="sort-chip" onclick={() => changeSort('status')}>状态{sortMark('status')}</button>
          <button class="sort-chip" onclick={() => changeSort('operator')}>操作人{sortMark('operator')}</button>
        {:else}
          <button class="sort-chip" onclick={() => changeSort('id')}>ID{sortMark('id')}</button>
        {/if}
      </div>
      <div class="search-wrap">
        <svg class="search-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.35-4.35" stroke-linecap="round"/></svg>
        <input type="text" placeholder="搜索..." bind:value={search} class="search-input" />
        {#if search}
          <button class="search-clear" onclick={() => search = ''}>✕</button>
        {/if}
      </div>
      <button class="create-btn" onclick={() => { createType = activeTab === 'records' ? 'record' : 'doc'; showCreate = true; createError = null; }}>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
        新建
      </button>
    </div>
  </div>

  <div class="knowledge-tabs" role="tablist" aria-label="维护管理类型">
    <button class="knowledge-tab docs-tab" role="tab" aria-selected={activeTab === 'docs'} class:active={activeTab === 'docs'} onclick={() => activeTab = 'docs'}>
      <span class="tab-icon">DOC</span>
      <span class="tab-title">维护文档</span>
      <span class="tab-hint">操作手册 / 预案</span>
      <span class="tab-count">{docs.length}</span>
    </button>
    <span class="tab-divider" aria-hidden="true"></span>
    <button class="knowledge-tab records-tab" role="tab" aria-selected={activeTab === 'records'} class:active={activeTab === 'records'} onclick={() => activeTab = 'records'}>
      <span class="tab-icon">REC</span>
      <span class="tab-title">维护记录</span>
      <span class="tab-hint">处置过程 / 状态</span>
      <span class="tab-count">{records.length}</span>
    </button>
  </div>

  {#if showCreate}
    <div class="create-panel">
      <div class="create-header">
        <h3>新建{createType === 'record' ? '维护记录' : '文档'}</h3>
        <button class="close-btn" onclick={() => { showCreate = false; createError = null; }}>✕</button>
      </div>
      <div class="create-body">
        <div class="create-type-select">
          <button class="type-btn" class:active={createType === 'record'} onclick={() => createType = 'record'}>📝 维护记录</button>
          <button class="type-btn" class:active={createType === 'doc'} onclick={() => createType = 'doc'}>📄 文档</button>
        </div>
        <input type="text" bind:value={createTitle} placeholder="标题" class="create-input" />
        <input type="text" bind:value={createCategory} placeholder="分类" class="create-input" />
        <textarea bind:value={createContent} placeholder="内容..." class="create-textarea" rows="4"></textarea>
        {#if createError}
          <p class="create-error">{createError}</p>
        {/if}
        <div class="create-actions">
          <button class="cancel-btn" onclick={() => { showCreate = false; createError = null; }}>取消</button>
          <button class="submit-btn" onclick={createItem}>创建</button>
        </div>
      </div>
    </div>
  {/if}

  {#if viewingItem && !detailId}
    <div class="viewer-panel">
      <div class="viewer-header">
        <div class="viewer-info">
          <h2>{viewingItem.title || viewingItem.meta?.title || viewingItem.id}</h2>
          <div class="viewer-meta">
            <span class="meta-tag">{viewingItem.type === 'doc' ? '文档' : '记录'}</span>
            <span class="meta-tag">{viewingItem.category || viewingItem.meta?.category || '-'}</span>
            {#if viewingItem.status}
              <span class="meta-status" style="color:{statusColor(viewingItem.status)};background:{statusColor(viewingItem.status)}15">
                {viewingItem.status === 'open' ? '进行中' : '已完成'}
              </span>
            {/if}
            <span class="meta-time">{viewingItem.timestamp || viewingItem.meta?.updated_at || '-'}</span>
          </div>
        </div>
        <div class="viewer-actions">
          {#if editingId}
            <button class="save-btn" onclick={saveEdit} disabled={saving}>
              {saving ? '保存中...' : '保存'}
            </button>
            <button class="cancel-edit-btn" onclick={() => editingId = null}>取消</button>
          {:else}
            <button class="edit-btn" onclick={startEdit}>编辑</button>
            <button class="delete-btn" onclick={() => deleteItem(viewingItem.id, viewingItem.type)}>删除</button>
          {/if}
          <button class="close-viewer-btn" onclick={closeView}>✕</button>
        </div>
      </div>
      <div class="viewer-body">
        {#if viewLoading}
          <div class="loading">
            <div class="loading-spinner"></div>
            <span>加载中...</span>
          </div>
        {:else if editingId}
          <div class="edit-fields">
            <input bind:value={editTitle} class="edit-input" placeholder="标题" />
            <input bind:value={editCategory} class="edit-input" placeholder="分类" />
            <textarea bind:value={editContent} class="edit-textarea" placeholder="内容..."></textarea>
          </div>
        {:else}
          <div class="viewer-content">{viewingItem.content || viewingItem.description || viewingItem.result || '暂无内容'}</div>
        {/if}
      </div>
    </div>
  {/if}

  {#if detailId}
    <!-- 详情页模式：只显示详情 -->
    {#if viewingItem}
      <div class="viewer-panel">
        <div class="viewer-header">
          <div class="viewer-info">
            <a href="#/knowledge" class="back-btn" aria-label="返回知识库">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none"><path d="m15 18-6-6 6-6" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>
            </a>
            <h2>{viewingItem.title || viewingItem.meta?.title || viewingItem.id}</h2>
            <div class="viewer-meta">
              <span class="meta-tag">{viewingItem.type === 'doc' ? '文档' : '记录'}</span>
              <span class="meta-tag">{viewingItem.category || viewingItem.meta?.category || '-'}</span>
              {#if viewingItem.status}
                <span class="meta-status" style="color:{statusColor(viewingItem.status)};background:{statusColor(viewingItem.status)}15">
                  {viewingItem.status === 'open' ? '进行中' : '已完成'}
                </span>
              {/if}
              <span class="meta-time">{viewingItem.timestamp || viewingItem.meta?.updated_at || '-'}</span>
            </div>
          </div>
          <div class="viewer-actions">
            {#if editingId}
              <button class="save-btn" onclick={saveEdit} disabled={saving}>
                {saving ? '保存中...' : '保存'}
              </button>
              <button class="cancel-edit-btn" onclick={() => editingId = null}>取消</button>
            {:else}
              <button class="edit-btn" onclick={startEdit}>编辑</button>
              <button class="delete-btn" onclick={() => deleteItem(viewingItem.id, viewingItem.type)}>删除</button>
            {/if}
          </div>
        </div>
        <div class="viewer-body">
          {#if editingId}
            <div class="edit-fields">
              <input bind:value={editTitle} class="edit-input" placeholder="标题" />
              <input bind:value={editCategory} class="edit-input" placeholder="分类" />
              <textarea bind:value={editContent} class="edit-textarea" placeholder="内容..."></textarea>
            </div>
          {:else}
            <div class="viewer-content">{viewingItem.content || viewingItem.description || viewingItem.result || '暂无内容'}</div>
          {/if}
        </div>
      </div>
    {:else if loading}
      <div class="loading">
        <div class="loading-spinner"></div>
        <span>加载中...</span>
      </div>
    {/if}
  {:else if loading}
    <div class="loading">
      <div class="loading-spinner"></div>
      <span>加载中...</span>
    </div>
  {:else if activeTab === 'records'}
    {#if filteredRecords.length === 0}
      <div class="empty-state">
        <span class="empty-icon">REC</span>
        <span class="empty-text">暂无维护记录</span>
      </div>
    {:else}
      <div class="items-list">
        {#each filteredRecords as record}
          <a href="#/maintenance/{record.id}" class="item-card record-card">
            <div class="record-rail" style="background:{statusColor(record.status)}"></div>
            <div class="record-token">工单</div>
            <div class="item-info">
              <h3 class="item-title">{record.title}</h3>
              <p class="item-desc">{record.description || '-'}</p>
              <div class="item-meta">
                <span class="meta-cat">{record.category}</span>
                <span class="meta-time">{formatTime(record.timestamp)}</span>
                <span class="meta-operator">操作人 {record.operator}</span>
              </div>
            </div>
            <div class="item-badge" style="color:{statusColor(record.status)};background:{statusColor(record.status)}15">
              {record.status === 'open' ? '进行中' : '已完成'}
            </div>
          </a>
        {/each}
      </div>
    {/if}
  {:else}
    {#if filteredDocs.length === 0}
      <div class="empty-state">
        <span class="empty-icon">DOC</span>
        <span class="empty-text">暂无文档</span>
      </div>
    {:else}
      <div class="items-list">
        {#each filteredDocs as doc}
          <a href="#/doc/{doc.id}" class="item-card doc-card">
            <div class="doc-icon">DOC</div>
            <div class="item-info">
              <h3 class="item-title">{doc.title}</h3>
              <p class="item-id">文档ID {doc.id}</p>
              <div class="item-meta">
                <span class="meta-cat">{doc.category}</span>
                <span class="meta-time">{formatTime(doc.updated_at)}</span>
              </div>
            </div>
            <div class="doc-arrow">阅读编辑</div>
          </a>
        {/each}
      </div>
    {/if}
  {/if}
</div>

<ConfirmDialog
  open={Boolean(pendingDelete)}
  title={pendingDelete?.type === 'record' ? '删除维护记录' : '删除维护文档'}
  message="确认删除该条目？删除后列表和详情页都会同步移除。"
  detail={pendingDelete ? `ID: ${pendingDelete.id}\n类型: ${pendingDelete.type === 'record' ? '维护记录' : '维护文档'}` : ''}
  confirmText="确认删除"
  loading={deleteLoading}
  onCancel={() => pendingDelete = null}
  onConfirm={confirmDeleteItem}
/>

<style>
  .knowledge-page { width: 100%; max-width: none; margin: 0; }
  .page-header { display: flex; align-items: center; justify-content: space-between; gap: 16px; margin-bottom: 20px; }
  .header-left { display: flex; align-items: baseline; gap: 10px; }
  .header-right { display: flex; align-items: center; gap: 10px; }
  .sort-group { display: flex; align-items: center; gap: 4px; background: var(--bg-secondary); border: 1px solid var(--border-primary); border-radius: 10px; padding: 4px; }
  .sort-chip { min-height: 30px; padding: 0 9px; border: none; border-radius: 7px; background: transparent; color: var(--text-secondary); font-size: 12px; cursor: pointer; white-space: nowrap; }
  .sort-chip:hover { color: var(--text-primary); background: var(--bg-hover); }
  .search-wrap { position: relative; }
  .search-icon { position: absolute; left: 12px; top: 50%; transform: translateY(-50%); color: var(--text-tertiary); }
  .search-input { width: 260px; padding: 10px 14px 10px 38px; background: var(--bg-input); border: 1px solid var(--border-primary); border-radius: 10px; font-size: 14px; color: var(--text-primary); outline: none; transition: all 0.2s; box-sizing: border-box; }
  .search-input:focus { border-color: var(--border-focus); box-shadow: 0 0 0 3px var(--accent-primary-light); }
  .search-clear { position: absolute; right: 10px; top: 50%; transform: translateY(-50%); background: none; border: none; color: var(--text-tertiary); cursor: pointer; font-size: 14px; }
  .create-btn { display: flex; align-items: center; gap: 6px; padding: 10px 18px; background: var(--accent-primary); color: white; border: none; border-radius: 10px; font-size: 14px; font-weight: 600; cursor: pointer; transition: all 0.2s; }
  .create-btn:hover { background: var(--accent-primary-hover); }

  .knowledge-tabs { display: flex; align-items: stretch; gap: 0; margin-bottom: 18px; padding: 4px; border: 1px solid var(--border-primary); border-bottom-color: rgba(34,211,238,.32); border-radius: 12px 12px 0 0; background: linear-gradient(180deg, var(--bg-card), var(--bg-secondary)); box-shadow: inset 0 -1px 0 rgba(34,211,238,.12); }
  .knowledge-tab { position: relative; flex: 1; min-width: 0; display: grid; grid-template-columns: 42px auto 1fr auto; align-items: center; gap: 10px; min-height: 46px; padding: 8px 12px; border: 1px solid transparent; border-radius: 9px 9px 0 0; background: transparent; color: var(--text-secondary); cursor: pointer; text-align: left; transition: background .18s ease, border-color .18s ease, color .18s ease; }
  .knowledge-tab::after { content: ''; position: absolute; left: 12px; right: 12px; bottom: -5px; height: 2px; border-radius: 999px; background: transparent; }
  .knowledge-tab:hover { color: var(--text-primary); background: var(--bg-hover); }
  .knowledge-tab.active { color: var(--text-primary); border-color: rgba(34,211,238,.24); background: rgba(34,211,238,.08); }
  .knowledge-tab.active::after { background: linear-gradient(90deg, #22d3ee, #60a5fa); box-shadow: 0 0 14px rgba(34,211,238,.35); }
  .records-tab.active { border-color: rgba(245,158,11,.25); background: rgba(245,158,11,.08); }
  .records-tab.active::after { background: linear-gradient(90deg, #f59e0b, #f97316); box-shadow: 0 0 14px rgba(245,158,11,.3); }
  .tab-divider { width: 1px; margin: 8px 3px; background: linear-gradient(180deg, transparent, var(--border-primary), transparent); }
  .tab-icon { display: grid; place-items: center; width: 34px; height: 30px; border-radius: 8px; font-family: var(--theme-font-family-mono); font-size: 10px; font-weight: 900; letter-spacing: 0; color: #67e8f9; background: rgba(34,211,238,.1); border: 1px solid rgba(34,211,238,.18); }
  .records-tab .tab-icon { color: #fbbf24; background: rgba(245,158,11,.1); border-color: rgba(245,158,11,.18); }
  .tab-title { color: var(--text-primary); font-size: 14px; font-weight: 800; white-space: nowrap; }
  .tab-hint { min-width: 0; color: var(--text-tertiary); font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tab-count { display: grid; place-items: center; min-width: 30px; height: 24px; padding: 0 8px; border-radius: 999px; background: var(--bg-secondary); color: var(--text-secondary); border: 1px solid var(--border-primary); font-family: var(--theme-font-family-mono); font-size: 11px; font-weight: 900; }
  .create-panel { background: var(--bg-card); border: 1px solid var(--border-primary); border-radius: 14px; margin-bottom: 20px; overflow: hidden; }
  .create-header { display: flex; justify-content: space-between; align-items: center; padding: 16px 20px; border-bottom: 1px solid var(--border-primary); }
  .create-header h3 { margin: 0; font-size: 16px; color: var(--text-primary); }
  .close-btn { background: none; border: none; color: var(--text-tertiary); font-size: 18px; cursor: pointer; padding: 4px 8px; border-radius: 6px; }
  .close-btn:hover { background: var(--bg-hover); }
  .create-body { padding: 20px; }
  .create-type-select { display: flex; gap: 8px; margin-bottom: 14px; }
  .type-btn { padding: 8px 14px; background: var(--bg-secondary); border: 1px solid var(--border-primary); border-radius: 8px; color: var(--text-secondary); font-size: 13px; cursor: pointer; transition: all 0.15s; }
  .type-btn.active { background: var(--accent-primary-light); border-color: var(--accent-primary); color: var(--accent-primary); }
  .create-input { width: 100%; padding: 10px 14px; background: var(--bg-input); border: 1px solid var(--border-primary); border-radius: 8px; color: var(--text-primary); font-size: 14px; outline: none; margin-bottom: 10px; box-sizing: border-box; }
  .create-input:focus { border-color: var(--border-focus); }
  .create-textarea { width: 100%; padding: 10px 14px; background: var(--bg-input); border: 1px solid var(--border-primary); border-radius: 8px; color: var(--text-primary); font-size: 14px; outline: none; resize: vertical; box-sizing: border-box; }
  .create-textarea:focus { border-color: var(--border-focus); }
  .create-error { color: var(--accent-danger); font-size: 13px; margin-bottom: 10px; }
  .create-actions { display: flex; gap: 10px; justify-content: flex-end; }
  .cancel-btn { padding: 8px 16px; background: var(--bg-secondary); border: 1px solid var(--border-primary); border-radius: 8px; color: var(--text-secondary); font-size: 13px; cursor: pointer; }
  .submit-btn { padding: 8px 16px; background: var(--accent-primary); color: white; border: none; border-radius: 8px; font-size: 13px; font-weight: 600; cursor: pointer; }
  .submit-btn:hover { background: var(--accent-primary-hover); }

  .viewer-panel { background: var(--bg-card); border: 1px solid var(--border-primary); border-radius: 14px; margin-bottom: 20px; overflow: hidden; }
  .viewer-header { display: flex; justify-content: space-between; align-items: flex-start; padding: 16px 20px; border-bottom: 1px solid var(--border-primary); }
  .viewer-info { display: flex; align-items: center; gap: 12px; flex: 1; }
  .back-btn { color: var(--text-tertiary); text-decoration: none; padding: 8px; border-radius: 10px; transition: all 0.2s; display: flex; }
  .back-btn:hover { color: var(--text-primary); background: var(--bg-hover); }
  .viewer-info h2 { margin: 0 0 8px; font-size: 18px; color: var(--text-primary); }
  .viewer-meta { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .meta-tag { font-size: 11px; color: var(--text-secondary); background: var(--bg-secondary); padding: 3px 8px; border-radius: 6px; }
  .meta-status { font-size: 11px; font-weight: 600; padding: 3px 8px; border-radius: 6px; }
  .meta-time { font-size: 11px; color: var(--text-tertiary); }
  .viewer-actions { display: flex; gap: 8px; align-items: center; }
  .edit-btn, .delete-btn { padding: 6px 12px; border: 1px solid var(--border-primary); border-radius: 6px; font-size: 12px; cursor: pointer; transition: all 0.15s; }
  .edit-btn { background: var(--bg-secondary); color: var(--text-secondary); }
  .edit-btn:hover { background: var(--accent-primary-light); color: var(--accent-primary); border-color: var(--accent-primary); }
  .delete-btn { background: var(--bg-secondary); color: var(--accent-danger); }
  .delete-btn:hover { background: rgba(239, 68, 68, 0.1); border-color: var(--accent-danger); }
  .save-btn { padding: 6px 14px; background: var(--accent-primary); color: white; border: none; border-radius: 6px; font-size: 12px; font-weight: 600; cursor: pointer; }
  .save-btn:disabled { opacity: 0.5; }
  .cancel-edit-btn { padding: 6px 12px; background: var(--bg-secondary); border: 1px solid var(--border-primary); border-radius: 6px; color: var(--text-secondary); font-size: 12px; cursor: pointer; }
  .close-viewer-btn { background: none; border: none; color: var(--text-tertiary); font-size: 16px; cursor: pointer; padding: 4px 8px; border-radius: 6px; }
  .close-viewer-btn:hover { background: var(--bg-hover); }
  .viewer-body { padding: 20px; }
  .viewer-content { font-size: 14px; color: var(--text-primary); line-height: 1.7; white-space: pre-wrap; }
  .edit-fields { display: flex; flex-direction: column; gap: 10px; }
  .edit-input { width: 100%; padding: 10px 12px; background: var(--bg-input); border: 1px solid var(--border-primary); border-radius: 8px; color: var(--text-primary); font-size: 14px; outline: none; box-sizing: border-box; }
  .edit-input:focus { border-color: var(--border-focus); }
  .edit-textarea { width: 100%; min-height: 300px; padding: 14px; background: var(--bg-input); border: 1px solid var(--border-primary); border-radius: 8px; color: var(--text-primary); font-size: 14px; line-height: 1.7; outline: none; resize: vertical; box-sizing: border-box; }
  .edit-textarea:focus { border-color: var(--border-focus); }

  .items-list { display: flex; flex-direction: column; gap: 10px; }
  .item-card { position: relative; display: flex; align-items: center; gap: 14px; padding: 16px; background: var(--bg-card); border: 1px solid var(--border-primary); border-radius: 12px; cursor: pointer; text-align: left; transition: all 0.2s; width: 100%; color: inherit; text-decoration: none; box-sizing: border-box; }
  .item-card:hover { transform: translateY(-1px); box-shadow: var(--shadow-md); border-color: var(--accent-primary); }
  .record-card { border-left: 0; background: linear-gradient(90deg, rgba(245,158,11,.08), var(--bg-card) 180px); }
  .doc-card { background: linear-gradient(90deg, rgba(34,211,238,.08), var(--bg-card) 180px); }
  .record-rail { position: absolute; left: 0; top: 0; bottom: 0; width: 4px; }
  .record-token, .doc-icon { display: grid; place-items: center; width: 48px; height: 42px; border-radius: 10px; flex-shrink: 0; font-family: var(--theme-font-family-mono); font-size: 12px; font-weight: 900; }
  .record-token { color: #fbbf24; background: rgba(245,158,11,.12); border: 1px solid rgba(245,158,11,.2); }
  .doc-icon { color: #67e8f9; background: rgba(34,211,238,.12); border: 1px solid rgba(34,211,238,.22); }
  .item-info { flex: 1; min-width: 0; }
  .item-title { font-size: 15px; font-weight: 600; color: var(--text-primary); margin: 0 0 4px; }
  .item-desc { font-size: 13px; color: var(--text-secondary); margin: 0 0 6px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .item-id { font-size: 12px; color: var(--text-tertiary); font-family: var(--theme-font-family-mono); margin: 0 0 6px; }
  .item-meta { display: flex; align-items: center; gap: 10px; }
  .meta-cat { font-size: 11px; color: var(--text-secondary); background: var(--bg-secondary); padding: 2px 6px; border-radius: 4px; }
  .meta-time { font-size: 11px; color: var(--text-tertiary); }
  .meta-operator { font-size: 11px; color: var(--text-tertiary); }
  .item-badge { font-size: 11px; font-weight: 600; padding: 4px 10px; border-radius: 8px; flex-shrink: 0; }
  .doc-arrow { color: #67e8f9; font-size: 12px; font-weight: 800; flex-shrink: 0; }

  .loading { display: flex; align-items: center; justify-content: center; gap: 10px; padding: 60px 0; color: var(--text-secondary); font-size: 14px; }
  .loading-spinner { width: 24px; height: 24px; border: 3px solid var(--border-primary); border-top-color: var(--accent-primary); border-radius: 50%; animation: spin 0.8s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }

  .empty-state { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px; padding: 80px 0; color: var(--text-secondary); }
  .empty-icon { font-size: 48px; opacity: 0.5; }
  .empty-text { font-size: 15px; }

  @media (max-width: 768px) {
    .page-header { flex-direction: column; align-items: stretch; }
    .header-right { flex-wrap: wrap; }
    .search-input { width: 100%; }
    .knowledge-tabs { flex-direction: column; border-radius: 12px; }
    .knowledge-tab { grid-template-columns: 38px auto 1fr auto; border-radius: 9px; }
    .knowledge-tab::after { bottom: 0; }
    .tab-divider { width: auto; height: 1px; margin: 3px 8px; background: linear-gradient(90deg, transparent, var(--border-primary), transparent); }
    .viewer-header { flex-direction: column; gap: 12px; }
  }
</style>
