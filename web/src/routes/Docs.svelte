<script>
  import { onDestroy, onMount, tick } from 'svelte';
  import ConfirmDialog from '../lib/ConfirmDialog.svelte';

  let { detailId = '' } = $props();

  let docs = $state([]);
  let dirs = $state([]);
  let loading = $state(true);
  let search = $state('');
  let selectedCategory = $state('全部');
  let selectedId = $state('');
  let doc = $state(null);
  let viewLoading = $state(false);
  let title = $state('');
  let category = $state('通用');
  let content = $state('');
  let saveState = $state('idle');
  let saveMessage = $state('已同步');
  let saveError = $state('');
  let createOpen = $state(false);
  let createDirOpen = $state(false);
  let createTitle = $state('');
  let createId = $state('');
  let createCategory = $state('通用');
  let createDirName = $state('');
  let createError = $state('');
  let importInput = $state(null);
  let attachmentInput = $state(null);
  let deleteId = $state('');
  let deleteLoading = $state(false);
  let dropActive = $state(false);
  let isEditing = $state(false);
  let editorEl = $state(null);
  let saveTimer = null;
  let lastSaved = '';

  let docDirs = $derived.by(() => {
    const set = new Set(['全部', ...dirs, ...docs.map((item) => item.category || '通用')]);
    return Array.from(set).filter(Boolean);
  });

  let filteredDocs = $derived.by(() => {
    const q = search.trim().toLowerCase();
    const scoped = selectedCategory === '全部'
      ? docs
      : docs.filter((item) => (item.category || '通用') === selectedCategory);
    const list = q
      ? scoped.filter((item) => `${item.title} ${item.id} ${item.category}`.toLowerCase().includes(q))
      : scoped;
    return [...list].sort((a, b) => String(b.updated_at || '').localeCompare(String(a.updated_at || '')));
  });

  let activeDocMeta = $derived(docs.find((item) => item.id === selectedId) || doc?.meta || null);
  let isDefaultDoc = $derived(selectedId === 'first-use-quick-start');
  let renderedContent = $derived(renderMarkdown(content));

  onMount(async () => {
    await loadDocs();
    const target = detailId || selectedId || docs[0]?.id;
    if (target) await openDoc(target);
  });

  onDestroy(() => {
    if (saveTimer) clearTimeout(saveTimer);
  });

  async function loadDocs() {
    loading = true;
    try {
      const r = await fetch('/api/docs?ts=' + Date.now(), { cache: 'no-store' });
      if (r.ok) {
        const data = await r.json();
        docs = data.docs || [];
        dirs = data.dirs || [];
      }
    } catch (e) {
      saveError = '加载文档列表失败: ' + (e.message || e);
    } finally {
      loading = false;
    }
  }

  async function openDoc(id) {
    if (!id) return;
    viewLoading = true;
    saveError = '';
    try {
      const r = await fetch('/api/docs/' + encodeURIComponent(id) + '?ts=' + Date.now(), { cache: 'no-store' });
      if (!r.ok) throw new Error('文档不存在');
      const data = await r.json();
      doc = data;
      selectedId = data.meta?.id || id;
      title = data.meta?.title || data.meta?.id || '';
      category = data.meta?.category || '通用';
      selectedCategory = category || selectedCategory;
      content = stripEnvelope(data.content || '');
      isEditing = false;
      lastSaved = snapshot();
      saveState = 'idle';
      saveMessage = '已同步';
    } catch (e) {
      saveError = e.message || '加载文档失败';
    } finally {
      viewLoading = false;
    }
  }

  function stripEnvelope(raw) {
    const lines = String(raw || '').split(/\r?\n/);
    const out = [];
    let afterTitle = false;
    for (const line of lines) {
      if (line.startsWith('<!-- ')) continue;
      if (!afterTitle && line.startsWith('# ')) {
        afterTitle = true;
        continue;
      }
      if (afterTitle || line.trim()) {
        out.push(line);
        afterTitle = true;
      }
    }
    return out.join('\n').trimStart();
  }

  function snapshot() {
    return JSON.stringify({ title, category, content });
  }

  function markDirty() {
    if (!selectedId || !isEditing) return;
    saveError = '';
    saveState = 'dirty';
    saveMessage = '等待自动保存';
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => saveDoc({ silent: true }), 900);
  }

  async function saveDoc({ silent = false } = {}) {
    if (!selectedId) return;
    const current = snapshot();
    if (current === lastSaved && silent) return;
    saveState = 'saving';
    saveMessage = '自动保存中...';
    try {
      const r = await fetch('/api/docs/' + encodeURIComponent(selectedId), {
        method: 'PUT',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ title, category, content }),
      });
      if (!r.ok) throw new Error('保存失败');
      const meta = await r.json();
      lastSaved = current;
      saveState = 'saved';
      saveMessage = '已自动保存';
      doc = { meta, content: `# ${title}\n\n${content}` };
      await loadDocs();
      setTimeout(() => {
        if (saveState === 'saved') {
          saveState = 'idle';
          saveMessage = '已同步';
        }
      }, 1600);
    } catch (e) {
      saveState = 'error';
      saveMessage = '保存失败';
      saveError = e.message || '保存失败';
    }
  }

  async function createDoc() {
    const nextTitle = createTitle.trim();
    const nextId = (createId.trim() || slug(nextTitle)).trim();
    const targetCategory = (createCategory || (selectedCategory === '全部' ? '通用' : selectedCategory) || '通用').trim();
    if (!nextTitle || !nextId) {
      createError = '请填写标题';
      return;
    }
    createError = '';
    try {
      const r = await fetch('/api/docs', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ id: nextId, title: nextTitle, category: targetCategory }),
      });
      if (!r.ok) throw new Error('创建失败，可能是 ID 已存在');
      createOpen = false;
      createTitle = '';
      createId = '';
      createCategory = targetCategory;
      selectedCategory = targetCategory;
      await loadDocs();
      await openDoc(nextId);
      isEditing = true;
      await tick();
      editorEl?.focus();
    } catch (e) {
      createError = e.message || '创建失败';
    }
  }

  function openCreateDoc() {
    createError = '';
    createTitle = '';
    createId = '';
    createCategory = selectedCategory === '全部' ? '通用' : selectedCategory;
    createOpen = true;
  }

  async function createDirectory() {
    const name = createDirName.trim();
    if (!name) {
      createError = '请填写目录名称';
      return;
    }
    createError = '';
    try {
      const r = await fetch('/api/docs/dirs', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ name }),
      });
      const data = await r.json().catch(() => ({}));
      if (!r.ok || data.status === 'error') throw new Error(data.message || '创建目录失败');
      dirs = data.dirs || [...dirs, name];
      selectedCategory = name;
      createCategory = name;
      createDirName = '';
      createDirOpen = false;
      await loadDocs();
    } catch (e) {
      createError = e.message || '创建目录失败';
    }
  }

  function slug(value) {
    const base = String(value || '')
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9\u4e00-\u9fa5_-]+/g, '-')
      .replace(/^-+|-+$/g, '');
    return base || `doc-${Date.now()}`;
  }

  async function importDocFile(event) {
    const file = event.currentTarget.files?.[0];
    if (!file) return;
    saveState = 'saving';
    saveMessage = '导入解析中...';
    try {
      const form = new FormData();
      form.append('file', file);
      form.append('category', selectedCategory === '全部' ? (category || '导入文档') : selectedCategory);
      const r = await fetch('/api/docs/import', { method: 'POST', body: form });
      const data = await r.json().catch(() => ({}));
      if (!r.ok || data.status === 'error') throw new Error(data.message || '导入失败');
      await loadDocs();
      if (data.doc?.id) await openDoc(data.doc.id);
      saveState = 'saved';
      saveMessage = '文档已导入';
    } catch (e) {
      saveState = 'error';
      saveMessage = '导入失败';
      saveError = e.message || '导入失败';
    } finally {
      event.currentTarget.value = '';
    }
  }

  async function uploadAttachment(event) {
    const files = Array.from(event.currentTarget.files || []);
    if (!files.length || !selectedId) return;
    await handleDocFiles(files);
    event.currentTarget.value = '';
  }

  async function handleDocFiles(files) {
    if (!selectedId || !files?.length) return;
    if (!isEditing) return;
    saveState = 'saving';
    saveMessage = files.length > 1 ? `上传 ${files.length} 个附件中...` : '上传附件中...';
    try {
      const snippets = [];
      for (const file of files) {
        snippets.push(await uploadOneAttachment(file));
      }
      insertAtCursor('\n' + snippets.join('\n') + '\n');
      saveState = 'saved';
      saveMessage = files.some((file) => isImageFile(file)) ? '图片/附件已插入' : '附件已插入';
      markDirty();
    } catch (e) {
      saveState = 'error';
      saveMessage = '上传失败';
      saveError = e.message || '上传失败';
    }
  }

  async function uploadOneAttachment(file) {
      const bytes = await file.arrayBuffer();
      const fallback = `paste-${new Date().toISOString().replace(/[-:TZ.]/g, '').slice(0, 14)}.${mimeExt(file.type)}`;
      const safeName = (file.name || fallback).replace(/[^\w.\-\u4e00-\u9fa5]/g, '_');
      const r = await fetch(`/api/docs/${encodeURIComponent(selectedId)}/attachments/${encodeURIComponent(safeName)}`, {
        method: 'PUT',
        body: bytes,
      });
      if (!r.ok) throw new Error('附件上传失败');
      const url = `/api/docs/${encodeURIComponent(selectedId)}/attachments/${encodeURIComponent(safeName)}`;
      return isImageFile(file) ? `![${safeName}](${url})` : `[${safeName}](${url})`;
  }

  function isImageFile(file) {
    return /^image\//.test(file?.type || '') || /\.(png|jpe?g|gif|webp|svg)$/i.test(file?.name || '');
  }

  function mimeExt(type) {
    if (type === 'image/jpeg') return 'jpg';
    if (type === 'image/gif') return 'gif';
    if (type === 'image/webp') return 'webp';
    if (type === 'image/svg+xml') return 'svg';
    return 'png';
  }

  function handleDragOver(event) {
    if (!selectedId) return;
    if (!isEditing) return;
    event.preventDefault();
    dropActive = true;
  }

  function handleDragLeave(event) {
    if (!event.currentTarget.contains(event.relatedTarget)) dropActive = false;
  }

  async function handleDrop(event) {
    if (!selectedId) return;
    if (!isEditing) return;
    event.preventDefault();
    dropActive = false;
    const files = Array.from(event.dataTransfer?.files || []);
    if (files.length) await handleDocFiles(files);
  }

  async function handlePaste(event) {
    if (!selectedId) return;
    if (!isEditing) return;
    const files = Array.from(event.clipboardData?.files || []);
    const imageItems = Array.from(event.clipboardData?.items || [])
      .filter((item) => item.kind === 'file')
      .map((item) => item.getAsFile())
      .filter(Boolean);
    const uploadFiles = files.length ? files : imageItems;
    if (uploadFiles.length) {
      event.preventDefault();
      await handleDocFiles(uploadFiles);
    }
  }

  async function insertAtCursor(text) {
    const el = editorEl;
    if (!el) {
      content += text;
      markDirty();
      return;
    }
    const start = el.selectionStart ?? content.length;
    const end = el.selectionEnd ?? content.length;
    content = content.slice(0, start) + text + content.slice(end);
    await tick();
    el.focus();
    el.selectionStart = el.selectionEnd = start + text.length;
    markDirty();
  }

  function insertCodeBlock() {
    insertAtCursor('\n```bash\n# 在这里粘贴命令或代码\n\n```\n');
  }

  function escapeHtml(value) {
    return String(value || '')
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
      .replace(/'/g, '&#39;');
  }

  function safeUrl(value) {
    const url = String(value || '').trim();
    if (/^(https?:|\/api\/docs\/|\.\/|\/|#)/i.test(url)) return escapeHtml(url);
    return '#';
  }

  function renderInline(value) {
    let html = escapeHtml(value);
    html = html.replace(/!\[([^\]]*)\]\(([^)]+)\)/g, (_, alt, url) => `<img src="${safeUrl(url)}" alt="${escapeHtml(alt)}" loading="lazy" />`);
    html = html.replace(/\[([^\]]+)\]\(([^)]+)\)/g, (_, text, url) => `<a href="${safeUrl(url)}" target="_blank" rel="noreferrer">${escapeHtml(text)}</a>`);
    html = html.replace(/`([^`]+)`/g, '<code>$1</code>');
    html = html.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');
    html = html.replace(/\*([^*]+)\*/g, '<em>$1</em>');
    return html;
  }

  function renderMarkdown(markdown) {
    const lines = String(markdown || '').split(/\r?\n/);
    const html = [];
    let inCode = false;
    let codeLang = '';
    let codeLines = [];
    let inList = false;
    const closeList = () => {
      if (inList) {
        html.push('</ul>');
        inList = false;
      }
    };
    const flushCode = () => {
      html.push(`<pre><span>${escapeHtml(codeLang || 'code')}</span><code>${escapeHtml(codeLines.join('\n'))}</code></pre>`);
      codeLines = [];
      codeLang = '';
      inCode = false;
    };

    for (const line of lines) {
      const raw = line;
      const trimmed = raw.trim();
      if (trimmed.startsWith('```')) {
        if (inCode) {
          flushCode();
        } else {
          closeList();
          inCode = true;
          codeLang = trimmed.slice(3).trim();
          codeLines = [];
        }
        continue;
      }
      if (inCode) {
        codeLines.push(raw);
        continue;
      }
      if (!trimmed) {
        closeList();
        html.push('<div class="md-gap"></div>');
        continue;
      }
      if (/^---+$/.test(trimmed)) {
        closeList();
        html.push('<hr />');
        continue;
      }
      const heading = /^(#{1,4})\s+(.+)$/.exec(raw);
      if (heading) {
        closeList();
        const level = heading[1].length;
        html.push(`<h${level}>${renderInline(heading[2])}</h${level}>`);
        continue;
      }
      const list = /^\s*[-*]\s+(.+)$/.exec(raw);
      if (list) {
        if (!inList) {
          html.push('<ul>');
          inList = true;
        }
        html.push(`<li>${renderInline(list[1])}</li>`);
        continue;
      }
      if (trimmed.startsWith('>')) {
        closeList();
        html.push(`<blockquote>${renderInline(trimmed.replace(/^>\s?/, ''))}</blockquote>`);
        continue;
      }
      closeList();
      html.push(`<p>${renderInline(raw)}</p>`);
    }
    if (inCode) flushCode();
    closeList();
    return html.join('');
  }

  async function confirmDeleteDoc() {
    if (!deleteId) return;
    deleteLoading = true;
    try {
      await fetch('/api/docs/' + encodeURIComponent(deleteId), { method: 'DELETE' });
      if (selectedId === deleteId) {
        doc = null;
        selectedId = '';
      }
      deleteId = '';
      await loadDocs();
      if (!selectedId && docs[0]?.id) await openDoc(docs[0].id);
    } finally {
      deleteLoading = false;
    }
  }

</script>

<div class="docs-workbench">
  <aside class="doc-sidebar">
    <div class="doc-side-head">
      <div>
        <span>{docs.length} 篇</span>
        <strong>维护文档</strong>
      </div>
        <div class="head-actions">
          <button class="icon-btn" onclick={() => createDirOpen = true} title="新建目录">◇</button>
          <button class="icon-btn" onclick={openCreateDoc} title="新建文档">+</button>
        </div>
      </div>
      <input class="doc-search" bind:value={search} placeholder="搜索标题、分类、ID" />
      <div class="dir-list" aria-label="文档目录">
        {#each docDirs as dir}
          <button class:active={dir === selectedCategory} onclick={() => { selectedCategory = dir; createCategory = dir === '全部' ? '通用' : dir; }}>
            <span>{dir}</span>
            <em>{dir === '全部' ? docs.length : docs.filter((item) => (item.category || '通用') === dir).length}</em>
          </button>
        {/each}
      </div>
    <div class="doc-list">
      {#if loading}
        <div class="doc-list-empty">正在加载...</div>
      {:else}
        {#each filteredDocs as item}
          <button class="doc-list-item" class:active={item.id === selectedId} onclick={() => openDoc(item.id)}>
            <strong>{item.title}</strong>
            <span>{item.category} · {item.updated_at}</span>
          </button>
        {:else}
          <div class="doc-list-empty">没有匹配文档</div>
        {/each}
      {/if}
    </div>
  </aside>

  <main class="doc-main">
    <header class="doc-toolbar">
      <div class="title-fields">
        <input class="title-input" bind:value={title} oninput={markDirty} placeholder="文档标题" disabled={!selectedId || !isEditing} />
        <input class="category-input" bind:value={category} oninput={markDirty} placeholder="分类" disabled={!selectedId || !isEditing} />
      </div>
      <div class="toolbar-actions">
        <span class="save-hint {saveState}">{saveMessage}</span>
        {#if isEditing}
          <button onclick={() => saveDoc()} disabled={!selectedId || saveState === 'saving'}>保存</button>
          <button onclick={() => { isEditing = false; }}>阅读</button>
        {:else}
          <button onclick={() => { isEditing = true; tick().then(() => editorEl?.focus()); }} disabled={!selectedId}>编辑</button>
        {/if}
        <button class="danger" onclick={() => deleteId = selectedId} disabled={!selectedId || isDefaultDoc}>删除</button>
      </div>
      <input bind:this={attachmentInput} class="hidden-file" type="file" accept="image/*,.md,.txt,.log,.json,.yaml,.yml,.zip,.tar,.gz,.pdf,.sh,.py,.pl,.conf,.ini" onchange={uploadAttachment} />
      <input bind:this={importInput} class="hidden-file" type="file" accept=".md,.txt,.json,text/markdown,text/plain,application/json" onchange={importDocFile} />
    </header>

    {#if saveError}
      <div class="doc-error">{saveError}</div>
    {/if}

    {#if viewLoading}
      <div class="doc-loading">正在打开文档...</div>
    {:else if !selectedId}
      <div class="doc-empty">左侧选择文档，或新建一篇维护文档。</div>
    {:else}
      <section
        class="editor-shell"
        class:editing={isEditing}
        class:drop-active={dropActive}
        role="group"
        aria-label="维护文档编辑区，支持拖入文件和粘贴附件"
        ondragover={handleDragOver}
        ondragleave={handleDragLeave}
        ondrop={handleDrop}>
        <div class="typora-page" class:editing={isEditing}>
          <article class="markdown-render">
            {#if renderedContent}
              {@html renderedContent}
            {:else}
              <p class="empty-render">开始输入 Markdown、日志、截图或处理步骤...</p>
            {/if}
          </article>
          {#if isEditing}
            <textarea
              bind:this={editorEl}
              class="markdown-source"
              bind:value={content}
              oninput={markDirty}
              onpaste={handlePaste}
              spellcheck="false"
              placeholder="编辑 Markdown，正文会实时渲染..."></textarea>
          {/if}
        </div>
        <div class="drop-mask">
          <strong>释放文件</strong>
          <span>图片、文档、日志、脚本都会作为附件插入当前文档</span>
        </div>
      </section>
      <div class="doc-action-rail">
        <div>
          <strong>{activeDocMeta ? activeDocMeta.id : '未选择文档'}</strong>
          <span>{isEditing ? '编辑中：支持拖入文件、粘贴截图、粘贴附件和实时自动保存' : isDefaultDoc ? '系统默认文档会一直保留，不能删除' : '阅读模式，点击编辑后修改内容'}</span>
        </div>
        <div class="rail-actions">
          {#if isEditing}
            <button onclick={insertCodeBlock} disabled={!selectedId}>代码片段</button>
            <button onclick={() => attachmentInput?.click()} disabled={!selectedId}>上传图片/附件</button>
          {:else}
            <button onclick={() => { isEditing = true; tick().then(() => editorEl?.focus()); }} disabled={!selectedId}>编辑文档</button>
          {/if}
          <button onclick={() => importInput?.click()}>导入文档</button>
        </div>
      </div>
    {/if}
  </main>
</div>

{#if createOpen}
  <div class="doc-create-layer" role="presentation">
    <div class="doc-create-modal" role="dialog" aria-modal="true">
      <header>
        <h3>新建维护文档</h3>
        <button onclick={() => createOpen = false}>x</button>
      </header>
      <label>标题<input bind:value={createTitle} placeholder="例如: Redis 故障处理步骤" /></label>
      <label>ID<input bind:value={createId} placeholder="留空自动生成" /></label>
      <label>目录<input bind:value={createCategory} placeholder="通用" /></label>
      {#if createError}<p>{createError}</p>{/if}
      <footer>
        <button onclick={() => createOpen = false}>取消</button>
        <button onclick={createDoc}>创建</button>
      </footer>
    </div>
  </div>
{/if}

{#if createDirOpen}
  <div class="doc-create-layer" role="presentation">
    <div class="doc-create-modal compact" role="dialog" aria-modal="true">
      <header>
        <h3>新建文档目录</h3>
        <button onclick={() => createDirOpen = false}>x</button>
      </header>
      <label>目录名称<input bind:value={createDirName} placeholder="例如: 数据库 / Java / 网络" /></label>
      {#if createError}<p>{createError}</p>{/if}
      <footer>
        <button onclick={() => createDirOpen = false}>取消</button>
        <button onclick={createDirectory}>创建</button>
      </footer>
    </div>
  </div>
{/if}

<ConfirmDialog
  open={Boolean(deleteId)}
  title="删除维护文档"
  message={`确认删除文档「${deleteId}」？`}
  detail={deleteId ? `文档 ID: ${deleteId}` : ''}
  confirmText="删除文档"
  loading={deleteLoading}
  onCancel={() => deleteId = ''}
  onConfirm={confirmDeleteDoc}
/>

<style>
  .docs-workbench { width: 100%; max-width: 100%; height: calc(100vh - 96px); min-height: 620px; display: grid; grid-template-columns: minmax(240px, 310px) minmax(0, 1fr); gap: 10px; overflow: hidden; }
  .doc-sidebar, .doc-main { min-width: 0; min-height: 0; border: 1px solid rgba(45,212,191,.16); border-radius: 12px; background: rgba(8,13,24,.82); }
  .doc-sidebar { display: flex; flex-direction: column; padding: 10px; overflow: hidden; }
  .doc-side-head { display: flex; align-items: center; justify-content: space-between; gap: 10px; margin-bottom: 10px; }
  .doc-side-head span { display: block; color: #67e8f9; font-size: 11px; font-weight: 800; }
  .doc-side-head strong { display: block; color: #f8fafc; font-size: 18px; }
  .head-actions { display: flex; align-items: center; gap: 6px; flex: 0 0 auto; }
  .icon-btn { width: 30px; height: 30px; border: 1px solid rgba(45,212,191,.28); border-radius: 8px; background: rgba(20,184,166,.12); color: #99f6e4; font-size: 20px; }
  .doc-search { width: 100%; box-sizing: border-box; min-height: 34px; margin-bottom: 10px; padding: 0 10px; border-radius: 9px; border: 1px solid rgba(148,163,184,.16); background: rgba(2,6,23,.68); color: #e2e8f0; outline: none; }
  .doc-search:focus { border-color: rgba(45,212,191,.42); box-shadow: 0 0 0 2px rgba(45,212,191,.08); }
  .dir-list { max-height: 150px; min-height: 0; overflow: auto; display: grid; gap: 5px; margin-bottom: 10px; padding: 7px; border: 1px solid rgba(148,163,184,.10); border-radius: 10px; background: rgba(2,6,23,.30); }
  .dir-list button { width: 100%; min-width: 0; min-height: 30px; display: grid; grid-template-columns: minmax(0, 1fr) auto; align-items: center; gap: 8px; padding: 0 9px; border: 1px solid transparent; border-radius: 8px; background: transparent; color: #cbd5e1; text-align: left; }
  .dir-list button.active { border-color: rgba(45,212,191,.36); background: rgba(20,184,166,.13); color: #ccfbf1; }
  .dir-list span { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; font-weight: 800; }
  .dir-list em { min-width: 22px; padding: 2px 6px; border-radius: 999px; background: rgba(15,23,42,.86); color: #67e8f9; font-style: normal; font-size: 11px; text-align: center; }
  .doc-list { min-height: 0; overflow: auto; display: grid; gap: 6px; align-content: start; }
  .doc-list-item { width: 100%; min-height: 58px; padding: 9px 10px; border-radius: 9px; border: 1px solid rgba(148,163,184,.10); background: rgba(15,23,42,.54); color: #cbd5e1; text-align: left; }
  .doc-list-item strong { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 13px; color: #f8fafc; }
  .doc-list-item span { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; margin-top: 5px; color: #94a3b8; font-size: 11px; }
  .doc-list-item.active { border-color: rgba(45,212,191,.42); background: linear-gradient(135deg, rgba(20,184,166,.18), rgba(14,116,144,.14)); box-shadow: inset 0 0 22px rgba(45,212,191,.05); }
  .doc-list-empty, .doc-empty, .doc-loading { padding: 28px; color: #94a3b8; text-align: center; }
  .doc-main { display: flex; flex-direction: column; overflow: hidden; }
  .doc-toolbar { min-width: 0; display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: 10px; align-items: center; padding: 10px; border-bottom: 1px solid rgba(148,163,184,.10); }
  .title-fields { min-width: 0; display: grid; grid-template-columns: minmax(0, 1fr) 150px; gap: 8px; }
  .title-input, .category-input { min-height: 34px; box-sizing: border-box; border-radius: 9px; border: 1px solid rgba(148,163,184,.14); background: rgba(2,6,23,.58); color: #f8fafc; padding: 0 10px; outline: none; }
  .title-input { font-size: 16px; font-weight: 800; }
  .category-input { color: #99f6e4; }
  .toolbar-actions { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; justify-content: flex-end; }
  .toolbar-actions button, .doc-action-rail button { min-height: 30px; padding: 0 10px; border-radius: 8px; border: 1px solid rgba(45,212,191,.18); background: rgba(20,184,166,.10); color: #99f6e4; font-size: 12px; font-weight: 800; }
  .toolbar-actions button:disabled { opacity: .45; }
  .toolbar-actions .danger { border-color: rgba(248,113,113,.22); color: #fecaca; background: rgba(127,29,29,.16); }
  .save-hint { min-width: 78px; color: #94a3b8; font-size: 11px; text-align: right; }
  .save-hint.saving { color: #67e8f9; }
  .save-hint.saved { color: #86efac; }
  .save-hint.error { color: #fca5a5; }
  .doc-error { margin: 8px 10px 0; padding: 8px 10px; border-radius: 8px; color: #fecaca; background: rgba(127,29,29,.18); border: 1px solid rgba(248,113,113,.20); font-size: 12px; }
  .editor-shell { position: relative; min-width: 0; min-height: 0; flex: 1; overflow: hidden; padding: 12px; background: radial-gradient(circle at 20% 0%, rgba(45,212,191,.08), transparent 34%), #070d19; }
  .typora-page { position: relative; height: 100%; min-width: 0; min-height: 0; overflow: hidden; border: 1px solid rgba(45,212,191,.12); border-radius: 12px; background: linear-gradient(180deg, rgba(2,6,23,.80), rgba(8,13,24,.92)); }
  .markdown-render { height: 100%; min-width: 0; box-sizing: border-box; overflow: auto; padding: 28px clamp(360px, 45%, 520px) 28px 34px; color: #dbeafe; font: 14px/1.78 var(--theme-font-family); word-break: break-word; overflow-wrap: anywhere; }
  .typora-page:not(.editing) .markdown-render { padding: 34px min(8vw, 76px); }
  .markdown-render :global(h1), .markdown-render :global(h2), .markdown-render :global(h3), .markdown-render :global(h4) { margin: 0 0 12px; color: #f8fafc; line-height: 1.25; letter-spacing: 0; }
  .markdown-render :global(h1) { font-size: 26px; padding-bottom: 12px; border-bottom: 1px solid rgba(45,212,191,.22); }
  .markdown-render :global(h2) { margin-top: 20px; font-size: 20px; color: #ccfbf1; }
  .markdown-render :global(h3) { margin-top: 16px; font-size: 16px; color: #bae6fd; }
  .markdown-render :global(p) { margin: 0 0 9px; }
  .markdown-render :global(ul) { margin: 0 0 12px 18px; padding: 0; }
  .markdown-render :global(li) { margin: 3px 0; }
  .markdown-render :global(blockquote) { margin: 10px 0; padding: 8px 12px; border-left: 3px solid rgba(45,212,191,.72); border-radius: 8px; background: rgba(20,184,166,.09); color: #ccfbf1; }
  .markdown-render :global(pre) { position: relative; margin: 12px 0; padding: 30px 14px 13px; overflow: auto; border: 1px solid rgba(148,163,184,.14); border-radius: 10px; background: rgba(2,6,23,.86); box-shadow: inset 0 0 26px rgba(45,212,191,.035); }
  .markdown-render :global(pre span) { position: absolute; top: 8px; left: 12px; color: #67e8f9; font: 10px/1 var(--theme-font-family-mono); text-transform: uppercase; }
  .markdown-render :global(code) { border-radius: 5px; background: rgba(15,23,42,.86); color: #a7f3d0; padding: 1px 5px; font: 12px/1.7 var(--theme-font-family-mono); }
  .markdown-render :global(pre code) { display: block; min-width: max-content; padding: 0; background: transparent; color: #dbeafe; white-space: pre; }
  .markdown-render :global(a) { color: #67e8f9; text-decoration: none; border-bottom: 1px solid rgba(103,232,249,.34); }
  .markdown-render :global(img) { display: block; max-width: 100%; max-height: 360px; margin: 12px 0; border-radius: 10px; border: 1px solid rgba(148,163,184,.14); object-fit: contain; }
  .markdown-render :global(hr) { border: 0; border-top: 1px solid rgba(148,163,184,.16); margin: 16px 0; }
  .markdown-render :global(.md-gap) { height: 8px; }
  .empty-render { color: #64748b; }
  .markdown-source { position: absolute; z-index: 3; right: 14px; top: 14px; bottom: 14px; width: min(420px, 40%); min-width: 320px; box-sizing: border-box; padding: 14px 15px; resize: none; border: 1px solid rgba(45,212,191,.20); border-radius: 10px; background: rgba(2,6,23,.86); color: #e2e8f0; outline: none; font: 13px/1.72 var(--theme-font-family-mono); box-shadow: 0 18px 60px rgba(0,0,0,.34), inset 0 0 34px rgba(45,212,191,.035); backdrop-filter: blur(8px); }
  .markdown-source:focus { border-color: rgba(45,212,191,.48); box-shadow: 0 0 0 3px rgba(45,212,191,.08), 0 18px 60px rgba(0,0,0,.34), inset 0 0 34px rgba(45,212,191,.045); }
  .drop-mask { position: absolute; inset: 12px; z-index: 5; display: none; place-items: center; align-content: center; gap: 8px; border: 1px dashed rgba(45,212,191,.68); border-radius: 12px; background: rgba(2,6,23,.78); color: #ccfbf1; text-align: center; backdrop-filter: blur(4px); pointer-events: none; }
  .drop-mask strong { font-size: 20px; }
  .drop-mask span { color: #94a3b8; font-size: 12px; }
  .editor-shell.drop-active .drop-mask { display: grid; }
  .doc-action-rail { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 9px 10px; border-top: 1px solid rgba(148,163,184,.10); background: rgba(2,6,23,.42); }
  .doc-action-rail > div:first-child { min-width: 0; display: grid; gap: 2px; }
  .doc-action-rail strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: #e2e8f0; font-family: var(--theme-font-family-mono); font-size: 11px; }
  .doc-action-rail span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: #94a3b8; font-size: 11px; }
  .rail-actions { display: flex; align-items: center; justify-content: flex-end; gap: 7px; flex-wrap: wrap; }
  .hidden-file { display: none; }
  .doc-create-layer { position: fixed; inset: 0; z-index: 80; display: grid; place-items: center; background: rgba(2,6,23,.72); }
  .doc-create-modal { width: min(420px, 92vw); display: grid; gap: 12px; padding: 16px; border-radius: 12px; border: 1px solid rgba(45,212,191,.22); background: #0b1220; color: #e2e8f0; }
  .doc-create-modal.compact { width: min(360px, 92vw); }
  .doc-create-modal header, .doc-create-modal footer { display: flex; align-items: center; justify-content: space-between; gap: 10px; }
  .doc-create-modal h3 { margin: 0; }
  .doc-create-modal label { display: grid; gap: 5px; color: #94a3b8; font-size: 12px; }
  .doc-create-modal input { min-height: 34px; border-radius: 8px; border: 1px solid rgba(148,163,184,.18); background: rgba(2,6,23,.7); color: #f8fafc; padding: 0 9px; }
  .doc-create-modal button { min-height: 30px; border-radius: 8px; border: 1px solid rgba(45,212,191,.18); background: rgba(20,184,166,.10); color: #99f6e4; }
  .doc-create-modal p { color: #fca5a5; margin: 0; font-size: 12px; }
  @media (max-width: 980px) {
    .docs-workbench { grid-template-columns: 1fr; height: auto; overflow: visible; }
    .doc-sidebar { max-height: 300px; }
    .doc-toolbar { grid-template-columns: 1fr; }
    .editor-shell { min-height: 680px; }
    .markdown-source { left: 14px; right: 14px; top: auto; bottom: 14px; width: auto; min-width: 0; height: 42%; }
    .markdown-render { padding: 24px 18px 330px; }
    .doc-action-rail { align-items: stretch; flex-direction: column; }
    .rail-actions { justify-content: flex-start; }
  }
</style>
