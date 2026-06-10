<script>
  import { onMount } from 'svelte';
  import ConfirmDialog from '../lib/ConfirmDialog.svelte';
  let scripts = $state([]);
  let search = $state('');
  let cat = $state('');
  let cats = $state([]);
  let err = $state(null);
  let loading = $state(true);
  let view = $state('list');
  let sort = $state('number');
  let execStats = $state({});
  let favorites = $state(new Set());
  let showFavOnly = $state(false);
  let showSystemScripts = $state(true);
  let showFailedOnly = $state(false);
  let showParamOnly = $state(false);
  let showUpload = $state(false);
  let showFilters = $state(false);
  let showCategoryMenu = $state(false);
  let categorySearch = $state('');
  let categorySearchInput = $state(null);
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
  let uploadParamsText = $state('');
  let uploadParams = $state([]);
  let editingScript = $state(null);
  let editName = $state('');
  let editDescription = $state('');
  let editFeature = $state('');
  let editCategory = $state('');
  let editAuthor = $state('');
  let editVersion = $state('');
  let editContent = $state('');
  let editFile = $state(null);
  let editParamsText = $state('');
  let editLoading = $state(false);
  let editError = $state(null);
  let runResults = $state({});
  let runParamScript = $state(null);
  let runParamValues = $state({});
  let runParamMode = $state('named');
  let runParamError = $state('');
  let headerSearchInput = $state(null);
  let copiedCommandId = $state('');
  let confirmDeleteScript = $state(null);
  let deleteLoading = $state(false);

  const categoryIcons = { '系统检查': 'SYS', '系统安全': 'SEC', '日志管理': 'LOG', '服务管理': 'SVC', '网络诊断': 'NET', '网络': 'NET', '性能监控': 'MON', '中间件': 'MID', '系统管理': 'ADM' };
  const categoryColors = { '系统检查': ['#0ea5e9', '#0369a1'], '系统安全': ['#dc2626', '#991b1b'], '日志管理': ['#7c3aed', '#5b21b6'], '服务管理': ['#2563eb', '#1d4ed8'], '网络诊断': ['#059669', '#047857'], '网络': ['#059669', '#047857'], '性能监控': ['#d97706', '#b45309'], '中间件': ['#9333ea', '#7e22ce'], '系统管理': ['#64748b', '#475569'] };
  const paramDefinitionPlaceholder = '[{"name":"target","description":"目标服务","type":"string","default":"nginx","required":true}]';
  const executableAccept = '.sh,.bash,.zsh,.ksh,.py,.python,.pl,.perl,.rb,.lua,.js,.mjs,.php,.awk,.expect,.exp,.run,.bin';

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
  function lastExecText(id) {
    const item = lastExec(id);
    if (!item?.timestamp) return '未执行';
    return String(item.timestamp).slice(0, 19);
  }
  function lastExecTitle(id) {
    const item = lastExec(id);
    return item?.timestamp || '未执行';
  }
  function lastExecState(id) {
    const item = lastExec(id);
    if (!item) return { label: '未执行', className: 'idle' };
    if (item.exit_code === 0) return { label: '成功', className: 'ok' };
    if (item.exit_code === null || item.exit_code === undefined) return { label: '运行中', className: 'running' };
    return { label: `失败 ${item.exit_code}`, className: 'fail' };
  }
  function scriptInVisibleSource(s) { return showSystemScripts || s.user_managed; }
  function visibleSourceCount() { return scripts.filter(scriptInVisibleSource).length; }
  function categoryCount(c) { return scripts.filter(s => s.category === c && scriptInVisibleSource(s)).length; }
  function sourceLabel(s) { return s.user_managed ? '用户脚本' : '内置脚本'; }
  function sourceClass(s) { return s.user_managed ? 'user-source' : 'builtin-source'; }
  function sourceCount(source) { return scripts.filter(s => source === 'user' ? s.user_managed : !s.user_managed).length; }
  function visibleScopeText() { return showSystemScripts ? '用户脚本 + 内置脚本' : '仅用户脚本，内置脚本未勾选'; }
  function cliCommand(script) { return `dm run ${script.id}`; }
  function runParamDefs(script) { return script?.metadata?.params || []; }
  function paramType(param) { return param?.type || param?.param_type || 'string'; }
  function versionText(script) { return script.metadata?.version ? `v${script.metadata.version}` : 'v1.0.0'; }
  function scriptType(script) {
    const path = String(script.path || '');
    const ext = (path.split('.').pop() || '').toLowerCase();
    const map = {
      sh: 'Shell',
      bash: 'Bash',
      zsh: 'Zsh',
      ksh: 'Ksh',
      py: 'Python',
      python: 'Python',
      pl: 'Perl',
      perl: 'Perl',
      js: 'Node',
      mjs: 'Node',
      rb: 'Ruby',
      lua: 'Lua',
      php: 'PHP',
      awk: 'AWK',
      expect: 'Expect',
      exp: 'Expect',
      run: 'Exec',
      bin: 'Binary',
    };
    return map[ext] || (path ? 'Exec' : 'Script');
  }
  function failedExecCount() {
    return scripts.filter(s => lastExecState(s.id).className === 'fail').length;
  }
  function paramCount(script) { return script.metadata?.params?.length || 0; }
  function configurableCount() { return scripts.filter(s => paramCount(s) > 0).length; }

  function storedParamValues(scriptId) {
    try {
      const raw = localStorage.getItem('dm-params-' + scriptId);
      return raw ? JSON.parse(raw) : {};
    } catch (_) {
      return {};
    }
  }

  function openRunParams(script) {
    if (!script) return;
    const defs = runParamDefs(script);
    if (!defs.length) {
      location.hash = '#/script/' + script.id + '/run';
      return;
    }
    const saved = storedParamValues(script.id);
    const next = {};
    for (const p of defs) {
      next[p.name] = saved[p.name] ?? p.default ?? (paramType(p) === 'boolean' ? 'false' : '');
    }
    runParamScript = script;
    runParamValues = next;
    runParamMode = 'named';
    runParamError = '';
  }

  function closeRunParams() {
    runParamScript = null;
    runParamValues = {};
    runParamMode = 'named';
    runParamError = '';
  }

  function updateRunParam(name, value) {
    runParamValues = { ...runParamValues, [name]: value };
  }

  function selectedRunParams() {
    const params = {};
    for (const p of runParamDefs(runParamScript)) {
      const value = (runParamValues[p.name] ?? '').toString();
      if (value !== '' && !(paramType(p) === 'boolean' && value === 'false' && !p.required)) {
        params[p.name] = value;
      }
    }
    return params;
  }

  function validateRunParams() {
    for (const p of runParamDefs(runParamScript)) {
      const value = (runParamValues[p.name] ?? '').toString().trim();
      if (p.required && !value) {
        return `参数 ${p.name} 为必填`;
      }
    }
    return '';
  }

  function buildListRunCommand() {
    if (!runParamScript) return '';
    const defs = runParamDefs(runParamScript);
    const selected = selectedRunParams();
    const parts = ['dm', 'run', shellQuote(runParamScript.id)];
    if (runParamMode === 'positional') {
      for (const p of defs) {
        const value = (runParamValues[p.name] ?? '').toString();
        if (value !== '' && !(paramType(p) === 'boolean' && value === 'false' && !p.required)) parts.push(shellQuote(value));
      }
    } else {
      for (const [key, value] of Object.entries(selected)) {
        parts.push('--param', shellQuote(`${key}=${value}`));
      }
    }
    return parts.join(' ');
  }

  function shellQuote(value) {
    const text = String(value ?? '');
    if (/^[A-Za-z0-9_./:@%+=,-]+$/.test(text)) return text;
    return "'" + text.replace(/'/g, "'\\''") + "'";
  }

  function confirmRunParams() {
    if (!runParamScript) return;
    const error = validateRunParams();
    if (error) {
      runParamError = error;
      return;
    }
    const params = selectedRunParams();
    const args = [];
    if (runParamMode === 'positional') {
      for (const p of runParamDefs(runParamScript)) {
        const value = (runParamValues[p.name] ?? '').toString();
        if (value !== '' && !(paramType(p) === 'boolean' && value === 'false' && !p.required)) args.push(value);
      }
    }
    try {
      localStorage.setItem('dm-params-' + runParamScript.id, JSON.stringify(runParamValues));
      localStorage.setItem('dm-run-payload-' + runParamScript.id, JSON.stringify({
        params: runParamMode === 'named' ? params : {},
        args,
      }));
    } catch (_) {}
    const id = runParamScript.id;
    closeRunParams();
    location.hash = '#/script/' + id + '/run';
  }

  function onSearch(e) { search = e.target.value; }

  async function copyCommand(script) {
    const command = cliCommand(script);
    try {
      if (navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(command);
      } else {
        const textarea = document.createElement('textarea');
        textarea.value = command;
        textarea.setAttribute('readonly', '');
        textarea.style.position = 'fixed';
        textarea.style.opacity = '0';
        document.body.appendChild(textarea);
        textarea.select();
        document.execCommand('copy');
        textarea.remove();
      }
      copiedCommandId = script.id;
      setTimeout(() => {
        if (copiedCommandId === script.id) copiedCommandId = '';
      }, 1400);
    } catch (e) {
      console.warn('复制命令失败:', e);
    }
  }

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
	      formData.append('params', JSON.stringify(normalizeUploadParams()));
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
    uploadParamsText = '';
    uploadParams = [];
  }

  function handleFileSelect(e) {
    const file = e.target.files[0];
    if (file) {
      uploadFile = file;
      if (!uploadId) uploadId = file.name.split('.')[0].toLowerCase().replace(/[^a-z0-9_-]+/g, '-').replace(/^-+|-+$/g, '');
      if (!uploadTitle) uploadTitle = file.name.replace(/\.[^.]+$/, '');
    }
  }

  async function handleEditFileSelect(e) {
    const file = e.target.files[0];
    if (!file) return;
    editFile = file;
    editError = null;
    try {
      editContent = await file.text();
    } catch (_) {
      editError = '已选择脚本文件；该文件不是可预览文本，保存时仍会按原始文件更新。';
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
    editParamsText = JSON.stringify(script.metadata?.params || [], null, 2);
    editContent = '';
    editFile = null;
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
    editFile = null;
    editParamsText = '';
  }

  function parseParamsText(textValue) {
    const text = textValue.trim();
    if (!text) return [];
    const parsed = JSON.parse(text);
    if (!Array.isArray(parsed)) throw new Error('参数定义必须是 JSON 数组');
    return parsed.map((item, index) => {
      if (!item || typeof item !== 'object' || Array.isArray(item)) {
        throw new Error(`第 ${index + 1} 个参数必须是对象`);
      }
      const name = String(item.name || '').trim();
      if (!/^[A-Za-z_][A-Za-z0-9_-]{0,63}$/.test(name)) {
        throw new Error(`第 ${index + 1} 个参数 name 不合法，只能使用字母、数字、下划线、短横线，且必须以字母或下划线开头`);
      }
      const type = String(item.type || 'string').trim();
      if (!['string', 'number', 'boolean', 'select'].includes(type)) {
        throw new Error(`第 ${index + 1} 个参数 type 只能是 string/number/boolean/select`);
      }
      return {
        name,
        description: String(item.description || ''),
        type,
        default: item.default === undefined || item.default === null ? null : String(item.default),
        required: Boolean(item.required),
      };
    });
  }

  function parseEditParams() {
    return parseParamsText(editParamsText);
  }

  function formatEditParams() {
    try {
      editParamsText = JSON.stringify(parseEditParams(), null, 2);
      editError = null;
    } catch (e) {
      editError = e.message;
    }
  }

  function appendParamTemplate(textValue) {
    const params = parseParamsText(textValue);
    const used = new Set(params.map(p => p.name));
    let name = 'target';
    let index = 1;
    while (used.has(name)) {
      index += 1;
      name = `target_${index}`;
    }
    params.push({
      name,
      description: '目标服务或路径',
      type: 'string',
      default: '',
      required: false,
    });
    return JSON.stringify(params, null, 2);
  }

  function formatUploadParams() {
    try {
      uploadParamsText = JSON.stringify(parseParamsText(uploadParamsText), null, 2);
      uploadError = null;
    } catch (e) {
      uploadError = e.message;
    }
  }

  function insertUploadParamTemplate() {
    try {
      uploadParamsText = appendParamTemplate(uploadParamsText);
      uploadError = null;
    } catch (e) {
      uploadParamsText = JSON.stringify([{
        name: 'target',
        description: '目标服务或路径',
        type: 'string',
        default: '',
        required: false,
      }], null, 2);
      uploadError = null;
    }
  }

  function clearUploadParams() {
    uploadParams = [];
    uploadError = null;
  }

  function newUploadParam() {
    return { name: '', description: '', type: 'string', default: '', required: false };
  }

  function addUploadParam() {
    uploadParams = [...uploadParams, newUploadParam()];
  }

  function updateUploadParam(index, key, value) {
    uploadParams = uploadParams.map((param, i) => i === index ? { ...param, [key]: value } : param);
  }

  function removeUploadParam(index) {
    uploadParams = uploadParams.filter((_, i) => i !== index);
  }

  function normalizeUploadParams() {
    return uploadParams
      .map((item, index) => ({ ...item, name: String(item.name || '').trim(), description: String(item.description || '').trim() }))
      .filter(item => item.name || item.description || item.default)
      .map((item, index) => {
        const name = item.name || `param_${index + 1}`;
        return {
          name,
          description: item.description || name,
          type: item.type || 'string',
          default: item.default === undefined || item.default === null || item.default === '' ? null : String(item.default),
          required: Boolean(item.required),
        };
      });
  }

  function insertParamTemplate() {
    try {
      editParamsText = appendParamTemplate(editParamsText);
      editError = null;
    } catch (e) {
      editParamsText = JSON.stringify([{
        name: 'target',
        description: '目标服务或路径',
        type: 'string',
        default: '',
        required: false,
      }], null, 2);
      editError = null;
    }
  }

  function clearEditParams() {
    editParamsText = '[]';
    editError = null;
  }

  async function updateScript() {
    if (!editingScript) return;
    editLoading = true;
    editError = null;
    try {
      const params = parseEditParams();
      if (editFile) {
        const fileForm = new FormData();
        fileForm.append('file', editFile);
        const fileRes = await fetch('/api/scripts/' + encodeURIComponent(editingScript.id) + '/file', {
          method: 'PUT',
          body: fileForm,
        });
        if (!fileRes.ok) {
          const d = await fileRes.json().catch(() => ({}));
          throw new Error(d.error || (fileRes.status === 403 ? '内置脚本不可更新文件' : '脚本文件更新失败: ' + fileRes.status));
        }
      }
      const payload = {
        name: editName,
        description: editDescription,
        feature: editFeature,
        category: editCategory,
        author: editAuthor,
        version: editVersion,
        params,
      };
      if (!editFile) payload.content = editContent;
      const r = await fetch('/api/scripts/' + encodeURIComponent(editingScript.id), {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
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
    confirmDeleteScript = script;
  }

  async function confirmDeleteSelectedScript() {
    const script = confirmDeleteScript;
    if (!script?.user_managed) return;
    deleteLoading = true;
    try {
      const r = await fetch('/api/scripts/' + encodeURIComponent(script.id), { method: 'DELETE' });
      if (r.ok) {
        confirmDeleteScript = null;
        await load();
      } else {
        uploadError = r.status === 403 ? '内置脚本不可删除' : '删除失败: ' + r.status;
      }
    } catch (e) { uploadError = '删除失败: ' + e.message; }
    deleteLoading = false;
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
      s.description?.toLowerCase().includes(q) ||
      (s.metadata?.params || []).some(p =>
        p.name?.toLowerCase().includes(q) ||
        p.description?.toLowerCase().includes(q) ||
        p.type?.toLowerCase().includes(q)
      );
  }

  let numberedScripts = $derived.by(() => scripts.map((script, index) => {
    const number = index + 1;
    return {
      ...script,
      number,
      numberLabel: String(number).padStart(2, '0'),
    };
  }));

  let filteredCats = $derived.by(() => {
    const q = categorySearch.trim().toLowerCase();
    return cats.filter(c => (!q || c.toLowerCase().includes(q)) && categoryCount(c) > 0);
  });

  function selectCategory(value) {
    cat = value;
    showCategoryMenu = false;
    categorySearch = '';
  }

  function toggleCategoryMenu() {
    showCategoryMenu = !showCategoryMenu;
    if (showCategoryMenu) {
      requestAnimationFrame(() => categorySearchInput?.focus());
    }
  }

  let sortedScripts = $derived.by(() => {
    const q = search.trim().toLowerCase();
    let arr = numberedScripts.filter(s => {
      const matchesCategory = !cat || s.category === cat;
      const matchesSearch = scriptMatchesSearch(s, q);
      const matchesSource = scriptInVisibleSource(s);
      return matchesCategory && matchesSearch && matchesSource;
    });
    if (showFavOnly) arr = arr.filter(s => favorites.has(s.id));
    if (showFailedOnly) arr = arr.filter(s => lastExecState(s.id).className === 'fail');
    if (showParamOnly) arr = arr.filter(s => paramCount(s) > 0);
    if (sort === 'number') arr.sort((a, b) => a.number - b.number);
    else if (sort === 'name') arr.sort((a, b) => a.name.localeCompare(b.name, 'zh-CN'));
    else if (sort === 'type') arr.sort((a, b) => scriptType(a).localeCompare(scriptType(b), 'zh-CN') || a.name.localeCompare(b.name, 'zh-CN'));
    else if (sort === 'category') arr.sort((a, b) => a.category.localeCompare(b.category, 'zh-CN') || a.name.localeCompare(b.name, 'zh-CN'));
    else if (sort === 'version') arr.sort((a, b) => versionText(a).localeCompare(versionText(b), 'zh-CN') || a.name.localeCompare(b.name, 'zh-CN'));
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

  function onGlobalKeydown(event) {
    const tag = event.target?.tagName?.toLowerCase();
    if (event.key === '/' && !event.ctrlKey && !event.metaKey && tag !== 'input' && tag !== 'textarea' && tag !== 'select') {
      event.preventDefault();
      headerSearchInput?.focus();
    }
  }

  onMount(() => {
    loadFavorites();
    load();
    window.addEventListener('keydown', onGlobalKeydown);
    return () => window.removeEventListener('keydown', onGlobalKeydown);
  });
</script>

<div class="scripts-page">
  <div class="page-header">
    <div class="header-left">
      <div>
        <h2 class="page-title">维护脚本中心</h2>
        <p class="page-subtitle">共 {scripts.length} 个脚本，当前显示 {sortedScripts.length} 个 · {visibleScopeText()}</p>
      </div>
      <div class="summary-pills">
        <span class="summary-pill">分类 {cats.length}</span>
        <span class="summary-pill">执行 {totalExecutions}</span>
        <span class="summary-pill">成功率 {successRate}</span>
        <button class="summary-pill summary-filter" class:active={showFailedOnly} onclick={() => showFailedOnly = !showFailedOnly}>
          最近失败 {failedExecCount()}
        </button>
      </div>
    </div>
    <div class="header-right">
      <div class="header-search search-wrap">
        <svg class="search-icon" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="11" cy="11" r="8"/>
          <path d="m21 21-4.35-4.35" stroke-linecap="round"/>
        </svg>
        <input
          bind:this={headerSearchInput}
          class="search-input"
          value={search}
          oninput={onSearch}
          placeholder="搜索脚本、ID、功能..."
          autocomplete="off"
          spellcheck="false"
          aria-label="搜索脚本" />
        {#if !search}
          <span class="search-shortcut">/</span>
        {/if}
        {#if search}
          <button class="search-clear" onclick={() => search = ''} aria-label="清空搜索">
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M18 6 6 18M6 6l12 12" stroke-linecap="round"/>
            </svg>
          </button>
        {/if}
      </div>
      <button class="upload-btn" onclick={() => { showUpload = true; uploadError = null; }}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="17 8 12 3 7 8"/><line x1="12" y1="3" x2="12" y2="15"/></svg>
        上传脚本
      </button>
      <button class="upload-btn filter-toggle-btn" onclick={() => showFilters = !showFilters}>
        筛选{cat || search || showFavOnly || showFailedOnly || showSystemScripts ? ' *' : ''} {showFilters ? '↑' : '↓'}
      </button>
      <label class="builtin-checkbox-toggle" class:active={showSystemScripts} title="默认显示；关闭后仅显示用户脚本">
        <input type="checkbox" bind:checked={showSystemScripts} />
        <span>显示内置脚本</span>
        <em>{showSystemScripts ? '默认显示' : '已隐藏'}</em>
      </label>
      <select class="sort-select" bind:value={sort} aria-label="排序方式">
        <option value="number">默认排序</option>
        <option value="name">按名称</option>
        <option value="type">按脚本类型</option>
        <option value="category">按分类</option>
        <option value="version">按版本</option>
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
    <div class="category-select-filter">
      <button class="category-trigger" class:active={Boolean(cat)} onclick={toggleCategoryMenu} type="button">
        <span>{cat || '全部类别'}</span>
        <em>{cat ? categoryCount(cat) : visibleSourceCount()}</em>
        <b>{showCategoryMenu ? '↑' : '↓'}</b>
      </button>
      {#if showCategoryMenu}
        <div class="category-menu" role="listbox" tabindex="-1" onclick={(event) => event.stopPropagation()} onkeydown={(event) => {
          if (event.key === 'Escape') showCategoryMenu = false;
          if (event.key === 'Enter' && filteredCats[0]) selectCategory(filteredCats[0]);
        }}>
          <input class="category-search" bind:this={categorySearchInput} bind:value={categorySearch} placeholder="搜索类别..." aria-label="搜索脚本类别" />
          <button class="category-option" class:active={!cat} onclick={() => selectCategory('')} type="button">
            <span>全部类别</span>
            <em>{visibleSourceCount()}</em>
          </button>
          {#each filteredCats as c}
            <button class="category-option" class:active={cat === c} onclick={() => selectCategory(c)} type="button">
              <span>{c}</span>
              <em>{categoryCount(c)}</em>
            </button>
          {:else}
            <div class="category-empty">没有匹配类别</div>
          {/each}
        </div>
      {/if}
    </div>
    <div class="category-chips" aria-label="分类筛选">
      <button class="chip" class:active={!cat} onclick={() => cat = ''}>
        <span>当前范围全部</span>
        <span class="chip-count">{visibleSourceCount()}</span>
      </button>
      <button class="chip fav-chip" class:active={showFavOnly} onclick={() => showFavOnly = !showFavOnly}>
        <span>收藏</span>
        <span class="chip-count">{favorites.size}</span>
      </button>
      <button class="chip failed-chip" class:active={showFailedOnly} onclick={() => showFailedOnly = !showFailedOnly}>
        <span>最近失败</span>
        <span class="chip-count">{failedExecCount()}</span>
      </button>
      <button class="chip param-chip" class:active={showParamOnly} onclick={() => showParamOnly = !showParamOnly}>
        <span>可配置</span>
        <span class="chip-count">{configurableCount()}</span>
      </button>
      {#each filteredCats as c}
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
          <div class="card-inner">
            <div class="card-top">
              <div class="card-icon" style="background:linear-gradient(135deg,{getCatColors(s.category)[0]}, {getCatColors(s.category)[1]})">
                <span>{getIcon(s.category)}</span>
              </div>
              <div class="card-head-main">
                <div class="card-kicker">
                  <span class="script-number">#{s.numberLabel}</span>
                  <span class="card-category">{s.category}</span>
                  <span class="source-pill {sourceClass(s)}">{sourceLabel(s)}</span>
                  {#if s.metadata?.version}
                    <span class="card-version">v{s.metadata.version}</span>
                  {/if}
                  {#if paramCount(s) > 0}
                    <span class="param-badge">参数 {paramCount(s)}</span>
                  {/if}
                </div>
                {#if search.trim()}
                  <h3 class="card-title">{@html highlightMatch(s.name, search)}</h3>
                {:else}
                  <h3 class="card-title">{s.name}</h3>
                {/if}
              </div>
            </div>

            <div class="card-actions">
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
                onclick={(e) => { e.preventDefault(); e.stopPropagation(); openRunParams(s); }}
                aria-label="{paramCount(s) > 0 ? '带参数执行' : '快速执行'} {s.name}"
                title="{paramCount(s) > 0 ? '带参数执行' : '快速执行'} {s.name}">
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
            </div>

            <div class="card-body">
              {#if execCount(s.id) > 0}
                <span class="card-badge inline-badge" title="最近执行 {execCount(s.id)} 次">
                  <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                    <polygon points="6 3 20 12 6 21 6 3" fill="currentColor"/>
                  </svg>
                  {execCount(s.id)}
                </span>
              {/if}
              {#if search.trim()}
                {#if s.feature}
                  <p class="card-feature">{@html highlightMatch(s.feature, search)}</p>
                {/if}
                <p class="card-desc">{@html highlightMatch(s.description, search)}</p>
              {:else}
                {#if s.feature}
                  <p class="card-feature">{s.feature}</p>
                {/if}
                <p class="card-desc">{s.description}</p>
              {/if}
            </div>
            <div class="card-footer">
              <div class="card-command">
                <button
                  class="row-copy-btn card-copy-cli"
                  onclick={(e) => { e.preventDefault(); e.stopPropagation(); copyCommand(s); }}
                  aria-label="复制执行命令 {s.name}"
                  title={cliCommand(s)}>
                  {copiedCommandId === s.id ? '已复制' : '复制 CLI'}
                </button>
              </div>
              <div class="card-foot-meta">
                {#if s.metadata?.author}
                  <span class="card-author">{s.metadata.author}</span>
                {/if}
                {#if s.metadata?.example}
                  <span class="card-example-inline">示例 {s.metadata.example}</span>
                {/if}
              </div>
            </div>
          </div>
        </a>
      {/each}
    </div>
  {:else}
    <div class="scripts-list table-scroll" aria-label="维护脚本列表，可横向拖动查看全部列">
      <div class="scripts-list-head" aria-hidden="true">
        <span>序号</span>
        <span>名称</span>
        <span>功能</span>
        <span>类型</span>
        <span>来源</span>
        <span>类别</span>
        <span>标识</span>
        <span>状态</span>
        <span>次数</span>
        <span>时间</span>
        <span>CLI</span>
        <span>操作</span>
      </div>
      {#each sortedScripts as s, i (s.id)}
        <a href="#/script/{s.id}" class="script-row" style="animation-delay:{Math.min(i * 30, 500)}ms">
          <span class="row-number">#{s.numberLabel}</span>
          <div class="row-name">
            {#if search.trim()}
              <h3 class="row-title">{@html highlightMatch(s.name, search)}</h3>
            {:else}
              <h3 class="row-title">{s.name}</h3>
            {/if}
          </div>
          <div class="row-feature">
            <span class="row-desc">
              {#if search.trim()}
                {@html highlightMatch(s.feature || s.description, search)}
              {:else}
                {s.feature || s.description}
              {/if}
            </span>
          </div>
          <div class="row-type">
            <span class="type-pill">{scriptType(s)}</span>
          </div>
          <div class="row-source">
            <span class="source-pill {sourceClass(s)}">{sourceLabel(s)}</span>
          </div>
          <div class="row-category">
            <span class="cat-pill" style="background:{getCatColors(s.category)[0]}1a;color:{getCatColors(s.category)[0]};border-color:{getCatColors(s.category)[0]}33">{s.category}</span>
          </div>
          <div class="row-identity-cell">
            <span class="row-id">{s.id}</span>
            <span class="row-version-mini">{versionText(s)}</span>
          </div>
          <div class="row-status-cell">
            <span class="last-state {lastExecState(s.id).className}">{lastExecState(s.id).label}</span>
          </div>
          <div class="row-exec-count">{execCount(s.id)}</div>
          <div class="row-time-cell"><span class="last-time" title={lastExecTitle(s.id)}>{lastExecText(s.id)}</span></div>
          <div class="row-command">
            <button
              class="row-copy-btn"
              onclick={(e) => { e.preventDefault(); e.stopPropagation(); copyCommand(s); }}
              aria-label="复制执行命令 {s.name}"
              title={cliCommand(s)}>
              {copiedCommandId === s.id ? '已复制' : '复制 CLI'}
            </button>
          </div>
          <div class="row-actions">
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
              onclick={(e) => { e.preventDefault(); e.stopPropagation(); openRunParams(s); }}
              aria-label="{paramCount(s) > 0 ? '带参数执行' : '执行'} {s.name} 并查看实时返回"
              title="{paramCount(s) > 0 ? '带参数执行并查看实时返回' : '执行并查看实时返回'}">
              <svg width="11" height="11" viewBox="0 0 24 24" fill="currentColor">
                <path d="M8 5v14l11-7z"/>
              </svg>
              <span>{paramCount(s) > 0 ? '参数执行' : '执行'}</span>
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
          </div>
        </a>
      {/each}
    </div>
  {/if}
</div>

{#if showUpload}
  <div class="modal-overlay locked-overlay" role="presentation">
    <div class="modal upload-script-modal" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="dialog" aria-modal="true" tabindex="-1">
      <div class="modal-header">
        <h3>上传脚本</h3>
        <button class="modal-close" onclick={closeUpload}>✕</button>
      </div>
      <div class="modal-body">
        <div class="upload-area">
          <input type="file" id="script-file" accept={executableAccept} onchange={handleFileSelect} class="file-input" />
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
            <div class="form-field full upload-param-builder">
              <span class="field-title-row">
                执行参数
                <button type="button" class="inline-mini-btn" onclick={addUploadParam}>添加参数</button>
                {#if uploadParams.length > 0}
                  <button type="button" class="inline-mini-btn danger" onclick={clearUploadParams}>清空</button>
                {/if}
              </span>
              {#if uploadParams.length === 0}
                <div class="param-empty">无参数脚本可直接上传；需要参数时点击“添加参数”。</div>
              {:else}
                <div class="upload-param-list">
                  {#each uploadParams as param, index}
                    <div class="upload-param-row">
                      <input value={param.name} placeholder="参数名" oninput={(e) => updateUploadParam(index, 'name', e.currentTarget.value)} />
                      <input value={param.description} placeholder="说明" oninput={(e) => updateUploadParam(index, 'description', e.currentTarget.value)} />
                      <select value={param.type} onchange={(e) => updateUploadParam(index, 'type', e.currentTarget.value)}>
                        <option value="string">文本</option>
                        <option value="number">数字</option>
                        <option value="boolean">开关</option>
                        <option value="select">选项</option>
                      </select>
                      <input value={param.default || ''} placeholder="默认值" oninput={(e) => updateUploadParam(index, 'default', e.currentTarget.value)} />
                      <label class="param-required">
                        <input type="checkbox" checked={param.required} onchange={(e) => updateUploadParam(index, 'required', e.currentTarget.checked)} />
                        必填
                      </label>
                      <button type="button" class="inline-mini-btn danger" onclick={() => removeUploadParam(index)}>删除</button>
                    </div>
                  {/each}
                </div>
              {/if}
              <small class="field-hint">参数会在脚本详情页自动生成表单，并转换为 CLI 的 --param KEY=VALUE。</small>
            </div>
          </div>
        {/if}
        {#if uploadError}
          <p class="upload-error">{uploadError}</p>
        {/if}
        <div class="upload-actions">
          <button class="submit-btn" onclick={uploadScript} disabled={!uploadFile || uploadLoading}>
            {uploadLoading ? '上传中...' : '上传'}
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}

{#if editingScript}
  <div class="modal-overlay locked-overlay" role="presentation">
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
          <div class="edit-file-row">
            <input type="file" id="edit-script-file" accept={executableAccept} onchange={handleEditFileSelect} class="file-input" />
            <label for="edit-script-file" class="edit-file-label">
              {editFile ? `已选择：${editFile.name}` : '重新传入脚本文件'}
            </label>
            <small>支持 sh / perl / python 等可执行脚本；保存后会替换原文件并刷新脚本类型。</small>
          </div>
          <textarea bind:value={editContent} rows="14" class="code-editor" spellcheck="false"></textarea>
        </label>
        <label class="form-field full">
          <span class="field-title-row">
            参数定义 JSON
            <button type="button" class="inline-mini-btn" onclick={formatEditParams}>格式化</button>
            <button type="button" class="inline-mini-btn" onclick={insertParamTemplate}>插入示例</button>
            <button type="button" class="inline-mini-btn danger" onclick={clearEditParams}>清空</button>
          </span>
          <textarea
            bind:value={editParamsText}
            rows="6"
            class="params-json-editor"
            spellcheck="false"
            placeholder={paramDefinitionPlaceholder}></textarea>
          <small class="field-hint">支持 string / number / boolean / select。保存后脚本详情页会自动生成参数表单、命令预览、历史复现参数。</small>
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

{#if runParamScript}
  <div class="modal-overlay locked-overlay" role="presentation">
    <div class="modal run-param-modal" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="dialog" aria-modal="true" tabindex="-1">
      <div class="modal-header">
        <h3>带参数执行 - {runParamScript.name}</h3>
        <button class="modal-close" onclick={closeRunParams}>✕</button>
      </div>
      <div class="modal-body">
        <div class="run-param-summary">
          <div>
            <span>脚本标识</span>
            <strong>{runParamScript.id}</strong>
          </div>
          <div>
            <span>脚本类型</span>
            <strong>{scriptType(runParamScript)}</strong>
          </div>
          <div>
            <span>参数数量</span>
            <strong>{runParamDefs(runParamScript).length}</strong>
          </div>
        </div>

        <div class="run-mode-tabs" aria-label="参数传递方式">
          <button class:active={runParamMode === 'named'} onclick={() => runParamMode = 'named'} type="button">命名参数</button>
          <button class:active={runParamMode === 'positional'} onclick={() => runParamMode = 'positional'} type="button">位置参数</button>
        </div>

        <div class="run-param-grid">
          {#each runParamDefs(runParamScript) as p}
            <label class="run-param-field" class:required={p.required}>
              <span>
                <code>{p.name}</code>
                <em>{paramType(p)}</em>
                {#if p.required}<b>必填</b>{/if}
              </span>
              {#if p.description}
                <small>{p.description}</small>
              {/if}
              {#if paramType(p) === 'boolean'}
                <select value={runParamValues[p.name] ?? 'false'} onchange={(e) => updateRunParam(p.name, e.currentTarget.value)}>
                  <option value="true">true</option>
                  <option value="false">false</option>
                </select>
              {:else if paramType(p) === 'number'}
                <input type="number" value={runParamValues[p.name] ?? ''} placeholder={p.default ?? ''} oninput={(e) => updateRunParam(p.name, e.currentTarget.value)} />
              {:else}
                <input type="text" value={runParamValues[p.name] ?? ''} placeholder={p.default ?? ''} oninput={(e) => updateRunParam(p.name, e.currentTarget.value)} />
              {/if}
            </label>
          {/each}
        </div>

        <div class="run-param-command">
          <span>执行命令</span>
          <code>{buildListRunCommand()}</code>
        </div>

        {#if runParamError}
          <p class="upload-error">{runParamError}</p>
        {/if}
        <div class="upload-actions">
          <button class="cancel-btn" onclick={closeRunParams}>取消</button>
          <button class="submit-btn" onclick={confirmRunParams}>确认执行</button>
        </div>
      </div>
    </div>
  </div>
{/if}

<ConfirmDialog
  open={Boolean(confirmDeleteScript)}
  title="删除用户脚本"
  message={`确认删除脚本「${confirmDeleteScript?.name || ''}」？该操作只允许删除用户脚本，内置脚本会被系统保护。`}
  detail={confirmDeleteScript ? `ID: ${confirmDeleteScript.id}\n分类: ${confirmDeleteScript.category}\n命令: dm run ${confirmDeleteScript.id}` : ''}
  confirmText="删除脚本"
  loading={deleteLoading}
  onCancel={() => confirmDeleteScript = null}
  onConfirm={confirmDeleteSelectedScript}
/>

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

  .builtin-checkbox-toggle {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    min-height: 34px;
    padding: 0 10px;
    border: 1px solid rgba(34, 211, 238, .18);
    border-radius: 8px;
    background: var(--bg-secondary);
    color: var(--text-secondary);
    font-size: 12px;
    font-weight: 800;
    white-space: nowrap;
    user-select: none;
  }

  .builtin-checkbox-toggle input {
    width: 15px;
    height: 15px;
    margin: 0;
    accent-color: #22d3ee;
  }

  .builtin-checkbox-toggle em {
    padding: 2px 6px;
    border-radius: 999px;
    background: rgba(148, 163, 184, .1);
    color: var(--text-tertiary);
    font-style: normal;
    font-size: 10px;
    font-weight: 900;
  }

  .builtin-checkbox-toggle:has(input:checked) {
    border-color: rgba(52, 211, 153, .38);
    background: linear-gradient(135deg, rgba(16, 185, 129, .14), rgba(34, 211, 238, .1));
    color: #a7f3d0;
  }

  .builtin-checkbox-toggle:has(input:checked) em {
    background: rgba(16, 185, 129, .16);
    color: #a7f3d0;
  }

  .modal-overlay { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.6); z-index: 100; display: flex; align-items: center; justify-content: center; }
  .modal { width: 400px; max-width: 90vw; max-height: 86vh; background: var(--bg-card); border: 1px solid var(--border-primary); border-radius: 14px; overflow: hidden; display: flex; flex-direction: column; }
  .upload-script-modal { width: min(860px, 94vw); max-height: 90vh; }
  .script-edit-modal { width: min(860px, 92vw); }
  .run-param-modal { width: min(720px, 94vw); max-height: 88vh; }
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
  .params-json-editor { font-family: var(--theme-font-family-mono); line-height: 1.5; white-space: pre; overflow: auto; min-height: 116px; resize: vertical; }
  .field-title-row { display: flex; align-items: center; flex-wrap: wrap; gap: 6px; }
  .inline-mini-btn { min-height: 23px; padding: 0 7px; border-radius: 6px; border: 1px solid var(--border-primary); background: var(--bg-secondary); color: var(--text-secondary); font-size: 10px; font-weight: 800; cursor: pointer; }
  .inline-mini-btn:hover { border-color: var(--border-focus); color: var(--accent-primary); background: var(--accent-primary-light); }
  .inline-mini-btn.danger { color: #fb7185; border-color: rgba(251, 113, 133, .24); }
  .inline-mini-btn.danger:hover { background: rgba(251, 113, 133, .1); border-color: rgba(251, 113, 133, .38); }
  .field-hint { margin-top: -2px; color: var(--text-tertiary); font-size: 11px; line-height: 1.45; }
  .upload-area { margin-bottom: 16px; }
  .file-input { display: none; }
  .file-label { display: flex; flex-direction: column; align-items: center; gap: 8px; padding: 24px; border: 2px dashed var(--border-primary); border-radius: 10px; cursor: pointer; transition: all 0.2s; color: var(--text-secondary); }
  .file-label:hover { border-color: var(--accent-primary); color: var(--accent-primary); }
  .upload-error { color: #ef4444; font-size: 12px; margin-bottom: 12px; }
  .run-param-summary {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 8px;
    margin-bottom: 12px;
  }
  .run-param-summary > div {
    min-width: 0;
    padding: 10px 11px;
    border: 1px solid rgba(34, 211, 238, .14);
    border-radius: 10px;
    background: rgba(15, 23, 42, .54);
  }
  .run-param-summary span {
    display: block;
    color: var(--text-tertiary);
    font-size: 10px;
    font-weight: 800;
  }
  .run-param-summary strong {
    display: block;
    min-width: 0;
    margin-top: 4px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-primary);
    font-size: 13px;
  }
  .run-mode-tabs {
    display: inline-flex;
    gap: 4px;
    padding: 3px;
    margin-bottom: 12px;
    border: 1px solid var(--border-primary);
    border-radius: 10px;
    background: var(--bg-secondary);
  }
  .run-mode-tabs button {
    min-height: 28px;
    padding: 0 12px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--text-secondary);
    font-size: 12px;
    font-weight: 800;
  }
  .run-mode-tabs button.active {
    background: rgba(34, 211, 238, .14);
    color: #67e8f9;
  }
  .run-param-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
  }
  .run-param-field {
    min-width: 0;
    display: grid;
    gap: 6px;
    padding: 10px;
    border: 1px solid var(--border-primary);
    border-radius: 10px;
    background: rgba(2, 6, 23, .28);
  }
  .run-param-field.required {
    border-color: rgba(251, 113, 133, .28);
  }
  .run-param-field > span {
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .run-param-field code {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: #bfdbfe;
    font-family: var(--theme-font-family-mono);
    font-size: 12px;
  }
  .run-param-field em,
  .run-param-field b {
    flex: 0 0 auto;
    padding: 2px 6px;
    border-radius: 999px;
    background: rgba(34, 211, 238, .1);
    color: #67e8f9;
    font-style: normal;
    font-size: 10px;
  }
  .run-param-field b {
    background: rgba(251, 113, 133, .12);
    color: #fda4af;
  }
  .run-param-field small {
    min-height: 16px;
    color: var(--text-tertiary);
    font-size: 11px;
    line-height: 1.35;
  }
  .run-param-field input,
  .run-param-field select {
    width: 100%;
    min-width: 0;
    min-height: 34px;
    box-sizing: border-box;
    border-radius: 8px;
    border: 1px solid var(--border-primary);
    background: var(--bg-input);
    color: var(--text-primary);
    padding: 0 10px;
    outline: none;
  }
  .run-param-field input:focus,
  .run-param-field select:focus {
    border-color: var(--border-focus);
    box-shadow: 0 0 0 3px var(--accent-primary-light);
  }
  .run-param-command {
    min-width: 0;
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 10px;
    align-items: center;
    margin-top: 12px;
    padding: 10px;
    border-radius: 10px;
    border: 1px solid rgba(34, 211, 238, .14);
    background: rgba(2, 6, 23, .42);
  }
  .run-param-command span {
    color: var(--text-tertiary);
    font-size: 11px;
    font-weight: 800;
  }
  .run-param-command code {
    min-width: 0;
    overflow: auto;
    white-space: nowrap;
    color: #a7f3d0;
    font-family: var(--theme-font-family-mono);
    font-size: 12px;
  }
  .upload-actions { display: flex; gap: 10px; justify-content: flex-end; }
  .advanced-toggle { width: 100%; display: flex; align-items: center; justify-content: space-between; margin: 0 0 12px; padding: 9px 12px; border: 1px solid var(--border-primary); border-radius: 8px; background: var(--bg-secondary); color: var(--text-secondary); cursor: pointer; font-size: 12px; }
  .advanced-toggle:hover { border-color: var(--accent-primary); color: var(--accent-primary); }
  .advanced-panel { margin-bottom: 12px; padding: 12px; border: 1px solid var(--border-secondary); border-radius: 10px; background: var(--bg-secondary); }
  .cancel-btn { padding: 8px 16px; background: var(--bg-secondary); border: 1px solid var(--border-primary); border-radius: 8px; color: var(--text-secondary); font-size: 13px; cursor: pointer; }
  .submit-btn { padding: 8px 16px; background: var(--accent-primary); color: white; border: none; border-radius: 8px; font-size: 13px; font-weight: 600; cursor: pointer; }
  .submit-btn:disabled { opacity: 0.5; cursor: not-allowed; }

  .upload-param-builder {
    padding: 12px;
    border: 1px solid var(--border-secondary);
    border-radius: 10px;
    background: rgba(15, 23, 42, .34);
  }

  .param-empty {
    padding: 10px 12px;
    border: 1px dashed var(--border-secondary);
    border-radius: 8px;
    color: var(--text-tertiary);
    font-size: 12px;
  }

  .upload-param-list {
    display: grid;
    gap: 8px;
  }

  .upload-param-row {
    display: grid;
    grid-template-columns: minmax(120px, 1fr) minmax(180px, 1.3fr) 102px minmax(120px, 1fr) 72px 56px;
    gap: 8px;
    align-items: center;
    min-width: 0;
  }

  .upload-param-row input,
  .upload-param-row select {
    width: 100%;
    min-width: 0;
    box-sizing: border-box;
    height: 34px;
    padding: 0 9px;
    border-radius: 7px;
    border: 1px solid var(--border-primary);
    background: var(--bg-input);
    color: var(--text-primary);
    font-size: 12px;
    outline: none;
  }

  .upload-param-row input:focus,
  .upload-param-row select:focus {
    border-color: var(--border-focus);
    box-shadow: 0 0 0 3px var(--accent-primary-light);
  }

  .param-required {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    color: var(--text-secondary);
    font-size: 11px;
    white-space: nowrap;
  }

  .param-required input {
    width: 14px;
    height: 14px;
    min-width: 14px;
    padding: 0;
  }

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

  .failed-chip {
    border-color: rgba(251, 113, 133, 0.16) !important;
  }
  .failed-chip.active {
    background: rgba(251, 113, 133, 0.1) !important;
    border-color: rgba(251, 113, 133, 0.34) !important;
    color: #fb7185 !important;
  }
  .param-chip {
    border-color: rgba(52, 211, 153, 0.16) !important;
  }
  .param-chip.active {
    background: rgba(52, 211, 153, 0.1) !important;
    border-color: rgba(52, 211, 153, 0.34) !important;
    color: #34d399 !important;
  }
  .param-chip.active .chip-count {
    background: rgba(52, 211, 153, 0.14) !important;
    color: #34d399 !important;
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

  .param-badge {
    display: inline-flex;
    align-items: center;
    height: 18px;
    padding: 0 7px;
    border-radius: 999px;
    border: 1px solid rgba(52, 211, 153, 0.24);
    background: rgba(52, 211, 153, 0.09);
    color: #34d399;
    font-size: 10px;
    font-weight: 800;
    line-height: 1;
    white-space: nowrap;
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

  .category-select-filter {
    position: relative;
    width: min(360px, 100%);
  }

  .category-trigger {
    width: 100%;
    min-height: 34px;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto auto;
    gap: 8px;
    align-items: center;
    padding: 0 10px;
    border: 1px solid rgba(34, 211, 238, .20);
    border-radius: 8px;
    background: linear-gradient(135deg, rgba(15, 23, 42, .72), rgba(8, 47, 73, .32));
    color: var(--text-secondary);
    text-align: left;
    font-size: 12px;
    font-weight: 800;
    cursor: default;
  }

  .category-trigger.active,
  .category-trigger:hover {
    border-color: rgba(34, 211, 238, .42);
    color: #67e8f9;
  }

  .category-trigger span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .category-trigger em {
    min-width: 28px;
    height: 20px;
    display: inline-grid;
    place-items: center;
    border-radius: 999px;
    background: rgba(34, 211, 238, .10);
    color: #67e8f9;
    font-family: var(--theme-font-family-mono);
    font-size: 10px;
    font-style: normal;
  }

  .category-trigger b {
    color: var(--text-tertiary);
    font-size: 11px;
  }

  .category-menu {
    position: absolute;
    left: 0;
    top: calc(100% + 6px);
    z-index: 60;
    width: min(420px, calc(100vw - 40px));
    max-height: 340px;
    overflow: auto;
    padding: 8px;
    border: 1px solid rgba(34, 211, 238, .24);
    border-radius: 12px;
    background: linear-gradient(145deg, rgba(2, 6, 23, .98), rgba(8, 24, 34, .97));
    box-shadow: 0 20px 58px rgba(0, 0, 0, .42), inset 0 0 22px rgba(34, 211, 238, .04);
  }

  .category-search {
    width: 100%;
    height: 34px;
    box-sizing: border-box;
    margin-bottom: 8px;
    padding: 0 10px;
    border: 1px solid rgba(34, 211, 238, .20);
    border-radius: 8px;
    background: var(--bg-input);
    color: var(--text-primary);
    outline: none;
    cursor: text;
  }

  .category-search:focus {
    border-color: rgba(34, 211, 238, .48);
    box-shadow: 0 0 0 3px rgba(34, 211, 238, .10);
  }

  .category-option {
    width: 100%;
    min-height: 32px;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 8px;
    align-items: center;
    margin-bottom: 4px;
    padding: 0 9px;
    border: 1px solid transparent;
    border-radius: 8px;
    background: transparent;
    color: var(--text-secondary);
    text-align: left;
    cursor: default;
  }

  .category-option:hover,
  .category-option.active {
    border-color: rgba(34, 211, 238, .25);
    background: rgba(34, 211, 238, .08);
    color: #e0f2fe;
  }

  .category-option span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12px;
  }

  .category-option em {
    color: var(--text-tertiary);
    font-family: var(--theme-font-family-mono);
    font-size: 10px;
    font-style: normal;
  }

  .category-empty {
    padding: 20px 8px;
    color: var(--text-tertiary);
    font-size: 12px;
    text-align: center;
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

  .search-shortcut {
    position: absolute;
    right: 10px;
    top: 50%;
    transform: translateY(-50%);
    display: inline-grid;
    place-items: center;
    min-width: 18px;
    height: 18px;
    border-radius: 5px;
    border: 1px solid var(--border-primary);
    background: var(--bg-secondary);
    color: var(--text-tertiary);
    font-family: var(--theme-font-family-mono);
    font-size: 10px;
    line-height: 1;
    pointer-events: none;
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

  .scripts-list-head {
    display: none;
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

  .row-command {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }

  .row-copy-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    height: 24px;
    min-width: 42px;
    padding: 0 7px;
    border-radius: 7px;
    border: 1px solid var(--border-secondary);
    background: var(--bg-secondary);
    color: var(--text-tertiary);
    font-size: 10px;
    font-weight: 800;
    cursor: pointer;
    transition: all .16s ease;
  }

  .row-copy-btn:hover {
    border-color: var(--border-focus);
    background: var(--accent-primary-light);
    color: var(--accent-primary);
  }

  .row-last {
    display: grid;
    gap: 2px;
    min-width: 0;
  }

  .last-state {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: fit-content;
    max-width: 100%;
    height: 20px;
    padding: 0 7px;
    border-radius: 999px;
    border: 1px solid rgba(148, 163, 184, .18);
    background: rgba(148, 163, 184, .08);
    color: var(--text-secondary);
    font-size: 10px;
    font-weight: 900;
    line-height: 1;
    white-space: nowrap;
  }

  .last-state.ok {
    color: #34d399;
    border-color: rgba(52, 211, 153, .28);
    background: rgba(52, 211, 153, .1);
  }

  .last-state.fail {
    color: #fb7185;
    border-color: rgba(251, 113, 133, .28);
    background: rgba(251, 113, 133, .1);
  }

  .last-state.running {
    color: #67e8f9;
    border-color: rgba(103, 232, 249, .28);
    background: rgba(103, 232, 249, .1);
  }

  .last-time {
    min-width: 0;
    width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-align: center;
    color: var(--text-tertiary);
    font-family: var(--theme-font-family-mono);
    font-size: 10px;
  }

  .row-actions {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 5px;
    min-width: 0;
    flex-wrap: nowrap;
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

  .summary-filter {
    cursor: pointer;
    font-weight: 800;
    transition: all .16s ease;
  }

  .summary-filter:hover,
  .summary-filter.active {
    border-color: rgba(251, 113, 133, .34);
    background: rgba(251, 113, 133, .1);
    color: #fb7185;
  }

  .header-right {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-left: auto;
  }
  .header-search {
    width: min(360px, 32vw);
    flex: 0 1 360px;
  }
  .header-search .search-input {
    height: 34px;
    padding-top: 0;
    padding-bottom: 0;
    font-size: 12px;
    border-radius: 9px;
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
    padding: 10px;
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
  .card-version {
    color: var(--text-secondary);
  }

  .card-example,
  .row-copy-btn {
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

  .card-badge {
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

  .scripts-list {
    gap: 5px;
    overflow-x: auto;
    overflow-y: visible;
    overscroll-behavior-x: contain;
    padding-bottom: 4px;
    --script-list-columns: 58px minmax(160px, .72fr) minmax(240px, 1fr) 86px 92px 120px 146px 90px 72px 132px 92px minmax(210px, 230px);
    --script-list-min-width: 1490px;
  }

  .scripts-list-head {
    display: grid;
    grid-template-columns: var(--script-list-columns);
    gap: 10px;
    align-items: center;
    min-width: var(--script-list-min-width);
    padding: 0 12px 5px;
    color: var(--text-tertiary);
    font-size: 10px;
    font-weight: 900;
    letter-spacing: .08em;
    text-transform: uppercase;
    text-align: center;
  }

  .script-row {
    display: grid;
    grid-template-columns: var(--script-list-columns);
    gap: 10px;
    min-width: var(--script-list-min-width);
    min-height: 54px;
    padding: 8px 12px;
    border-radius: 9px;
  }

  .scripts-list-head span,
  .script-row > span,
  .script-row > div {
    min-width: 0;
    text-align: center;
  }

  .row-name,
  .row-feature {
    text-align: left;
  }

  .script-row:hover {
    border-color: rgba(34,211,238,.32);
    background: linear-gradient(90deg, rgba(34,211,238,.055), var(--bg-card));
  }

  .scripts-list-head span:last-child,
  .row-actions {
    position: sticky;
    right: 0;
    z-index: 3;
    padding-left: 8px;
    background: linear-gradient(90deg, rgba(15, 23, 42, 0), var(--bg-card) 20%, var(--bg-card));
    box-shadow: -10px 0 16px rgba(2, 6, 23, 0.18);
  }

  .scripts-list-head span:last-child {
    z-index: 4;
    text-align: right;
    background: linear-gradient(90deg, rgba(15, 23, 42, 0), var(--bg-primary) 20%, var(--bg-primary));
  }

  .script-row:hover .row-actions {
    background: linear-gradient(90deg, rgba(34,211,238,0), color-mix(in srgb, var(--bg-card) 82%, rgba(34,211,238,.16)) 22%, color-mix(in srgb, var(--bg-card) 82%, rgba(34,211,238,.16)));
  }

  .row-number {
    min-width: 40px;
    height: 24px;
  }

  .row-title {
    max-width: 100%;
    margin: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 13px;
  }

  .row-name,
  .row-feature {
    min-width: 0;
    display: flex;
    align-items: center;
  }

  .row-desc {
    max-width: 100%;
    display: block;
    font-size: 11px;
  }

  .row-category {
    min-width: 0;
  }

  .row-type,
  .row-identity-cell,
  .row-exec-count,
  .row-last,
  .row-command {
    min-width: 0;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .row-last {
    flex-direction: column;
    gap: 2px;
  }

  .type-pill,
  .row-identity-cell,
  .row-exec-count {
    font-family: var(--theme-font-family-mono);
    font-size: 11px;
    font-weight: 800;
    color: var(--text-secondary);
    font-variant-numeric: tabular-nums;
  }

  .type-pill {
    min-width: 66px;
    padding: 4px 8px;
    border-radius: 999px;
    border: 1px solid rgba(34, 211, 238, .22);
    background: rgba(34, 211, 238, .08);
    color: #67e8f9;
  }

  .row-identity-cell {
    flex-direction: column;
    gap: 2px;
    color: #a7f3d0;
  }

  .row-identity-cell .row-id,
  .row-version-mini {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .row-version-mini {
    color: var(--text-tertiary);
    font-size: 10px;
    font-weight: 700;
  }

  .row-exec-count {
    color: #facc15;
  }

  .cat-pill {
    display: block;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    text-align: center;
    padding: 3px 8px;
    font-size: 10px;
  }

  .row-copy-btn {
    height: 24px;
    min-width: 38px;
    padding: 0 6px;
  }

  .row-run-btn {
    width: auto;
    min-width: 54px;
    height: 28px;
    gap: 4px;
    margin-left: 0;
    background: linear-gradient(135deg, rgba(20,184,166,.22), rgba(34,211,238,.14));
    padding: 0 9px;
    font-size: 11px;
    font-weight: 800;
  }

  .row-fav-btn {
    width: 28px;
    height: 28px;
  }

  .row-edit-btn,
  .row-delete-btn {
    height: 26px;
    min-width: 40px;
    padding: 0 7px;
    font-size: 10px;
  }

  .row-arrow {
    display: none;
  }

  .modal-overlay {
    background: rgba(15, 18, 24, 0.55);
    backdrop-filter: blur(4px);
  }

  .modal {
    border-radius: 10px;
    box-shadow: 0 20px 60px rgba(15, 23, 42, 0.22);
  }

  .scripts-grid {
    grid-template-columns: repeat(auto-fill, minmax(330px, 1fr));
    gap: 16px;
  }

  .script-card {
    min-height: 250px;
    padding: 0;
  }

  .card-inner {
    display: grid;
    grid-template-columns: 70px minmax(0, 1fr);
    grid-template-areas:
      "icon header"
      "icon title"
      "body body"
      "example example"
      "footer footer";
    gap: 10px 14px;
    height: 100%;
    padding: 16px;
  }

  .card-header {
    grid-area: header;
    display: grid;
    grid-template-columns: minmax(0, 1fr) repeat(4, auto);
    align-items: center;
    gap: 6px;
    margin: 0;
    min-width: 0;
  }

  .card-icon {
    grid-area: icon;
    width: 58px;
    height: 58px;
    align-self: start;
    font-size: 13px;
    font-weight: 900;
    letter-spacing: 0;
  }

  .script-number {
    position: absolute;
    top: 12px;
    left: 12px;
    min-width: 40px;
    height: 22px;
    z-index: 2;
    background: rgba(2, 6, 23, 0.72);
    color: #e2e8f0;
    border-color: rgba(255, 255, 255, 0.12);
  }

  .card-meta {
    min-width: 0;
    margin-right: 0;
  }

  .card-fav-btn,
  .card-run-btn,
  .card-edit-btn,
  .card-delete-btn {
    width: 28px;
    height: 28px;
    padding: 0;
  }

  .card-edit-btn,
  .card-delete-btn {
    font-size: 10px;
  }

  .card-badge {
    position: absolute;
    right: 12px;
    bottom: 12px;
  }

  .card-title {
    grid-area: title;
    margin: 0;
    align-self: end;
    line-height: 1.35;
  }

  .card-feature,
  .card-desc {
    grid-column: 1 / -1;
  }

  .card-feature {
    margin: 0;
  }

  .card-desc {
    margin: 0;
    min-height: 38px;
  }

  .card-example {
    grid-area: example;
    margin: 0;
  }

  .card-footer {
    grid-area: footer;
    padding-right: 54px;
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
    .header-right { width: 100%; flex-wrap: wrap; }
    .header-search { width: 100%; flex-basis: 100%; }
    .sort-select { flex: 1; }
    .script-card { padding: 0; }
    .card-inner { grid-template-columns: 58px minmax(0, 1fr); padding: 14px; }
    .card-icon { width: 48px; height: 48px; font-size: 11px; }
    .card-header { grid-template-columns: minmax(0, 1fr) repeat(2, auto); }
    .card-edit-btn, .card-delete-btn { display: none; }
    .script-row { padding: 8px 12px; }
    .row-edit-btn, .row-delete-btn { display: none; }
  }

  .scripts-grid {
    grid-template-columns: repeat(auto-fill, minmax(360px, 1fr));
    gap: 14px;
  }

  .script-card {
    min-height: 0;
    padding: 0;
    overflow: hidden;
    border-radius: 16px;
    background:
      linear-gradient(180deg, rgba(255,255,255,0.025), transparent 46%),
      var(--bg-card);
  }

  .script-card::before {
    content: '';
    position: absolute;
    inset: 0;
    border-top: 1px solid rgba(255,255,255,0.06);
    pointer-events: none;
  }

  .card-inner {
    display: grid;
    grid-template-columns: 1fr;
    grid-template-areas: none;
    grid-auto-flow: row;
    gap: 0;
    min-height: 230px;
    padding: 0;
  }

  .card-top,
  .card-actions,
  .card-body,
  .card-footer {
    grid-area: auto;
    grid-column: 1 / -1;
    width: auto;
  }

  .card-top {
    display: grid;
    grid-template-columns: 58px minmax(0, 1fr);
    gap: 14px;
    align-items: start;
    padding: 16px 16px 12px;
    border-bottom: 1px solid var(--border-secondary);
  }

  .card-icon {
    grid-area: auto;
    width: 58px;
    height: 58px;
    border-radius: 15px;
    align-self: start;
    font-size: 13px;
    font-weight: 900;
    letter-spacing: .02em;
    box-shadow: inset 0 1px 0 rgba(255,255,255,.16), 0 12px 26px rgba(0,0,0,.18);
  }

  .card-head-main {
    min-width: 0;
    display: grid;
    gap: 8px;
  }

  .card-kicker {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
    min-width: 0;
  }

  .script-number {
    position: static;
    min-width: 44px;
    height: 23px;
    z-index: auto;
    background: rgba(34,211,238,.1);
    color: var(--accent-primary);
    border-color: rgba(34,211,238,.22);
  }

  .card-title {
    grid-area: auto;
    margin: 0;
    max-width: 100%;
    color: var(--text-primary);
    font-size: 17px;
    font-weight: 800;
    line-height: 1.25;
    letter-spacing: -.01em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .card-category,
  .source-pill,
  .card-version {
    height: 23px;
    display: inline-flex;
    align-items: center;
  }

  .card-actions {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 10px 16px;
    border-bottom: 1px solid var(--border-secondary);
    background: color-mix(in srgb, var(--bg-secondary) 68%, transparent);
  }

  .card-actions .card-fav-btn,
  .card-actions .card-run-btn,
  .card-actions .card-edit-btn,
  .card-actions .card-delete-btn {
    position: static;
    width: auto;
    min-width: 34px;
    height: 30px;
    padding: 0 10px;
    opacity: 1;
    border-radius: 9px;
    font-size: 11px;
  }

  .card-actions .card-run-btn {
    min-width: 42px;
    margin-left: 0;
  }

  .card-body {
    padding: 14px 16px 12px;
    min-height: 94px;
  }

  .card-feature {
    grid-area: auto;
    grid-column: auto;
    margin: 0 0 7px;
    color: var(--accent-primary);
    font-size: 12px;
    font-weight: 800;
  }

  .card-desc {
    grid-area: auto;
    grid-column: auto;
    margin: 0;
    min-height: 0;
    color: var(--text-secondary);
    font-size: 12px;
    line-height: 1.65;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .inline-badge {
    position: static;
    width: fit-content;
    margin: 0 0 8px;
  }

  .card-footer {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 8px;
    margin-top: auto;
    padding: 12px 16px 14px;
    border-top: 1px solid var(--border-secondary);
    background: rgba(2, 6, 23, 0.12);
  }

  .card-command {
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .card-foot-meta {
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-tertiary);
    font-size: 10px;
  }

  .card-author,
  .card-example-inline {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .card-example {
    display: none;
  }

  .card-badge {
    position: static;
  }

  /* Compact density override: keep actions visible but remove oversized card mass. */
  .scripts-grid {
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 10px;
  }
  .card-inner {
    min-height: 164px;
  }
  .card-top {
    grid-template-columns: 42px minmax(0, 1fr);
    gap: 10px;
    padding: 11px 12px 8px;
  }
  .card-icon {
    width: 42px;
    height: 42px;
    border-radius: 11px;
    font-size: 11px;
  }
  .card-kicker {
    gap: 4px;
  }
  .script-number,
  .card-category,
  .source-pill,
  .card-version {
    height: 20px;
    font-size: 9px;
  }
  .script-number {
    min-width: 38px;
  }
  .card-title {
    font-size: 14px;
  }
  .card-actions {
    padding: 7px 12px;
    gap: 5px;
  }
  .card-actions .card-fav-btn,
  .card-actions .card-run-btn,
  .card-actions .card-edit-btn,
  .card-actions .card-delete-btn {
    min-width: 28px;
    height: 25px;
    padding: 0 7px;
  }
  .card-body {
    min-height: 38px;
    padding: 8px 12px 7px;
  }
  .card-feature {
    margin-bottom: 4px;
    font-size: 11px;
  }
  .card-desc {
    font-size: 11px;
    line-height: 1.45;
    -webkit-line-clamp: 1;
  }
  .card-footer {
    display: none;
  }
  .card-foot-meta {
    font-size: 9px;
  }

  .edit-file-row {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 8px 10px;
    align-items: center;
    margin-bottom: 8px;
    padding: 8px;
    border: 1px solid var(--border-secondary);
    border-radius: 9px;
    background: var(--bg-secondary);
  }

  .edit-file-label {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 30px;
    padding: 0 12px;
    border-radius: 8px;
    border: 1px solid rgba(34, 211, 238, .24);
    background: rgba(34, 211, 238, .08);
    color: var(--accent-primary);
    font-size: 12px;
    font-weight: 900;
    white-space: nowrap;
    cursor: default;
  }

  .edit-file-row small {
    min-width: 0;
    color: var(--text-tertiary);
    font-size: 11px;
    line-height: 1.35;
  }

  .builtin-checkbox-toggle.active {
    border-color: rgba(52, 211, 153, .38);
    background: linear-gradient(135deg, rgba(16, 185, 129, .14), rgba(34, 211, 238, .1));
    color: #a7f3d0;
  }

  .builtin-checkbox-toggle.active em {
    background: rgba(16, 185, 129, .16);
    color: #a7f3d0;
  }

  .scripts-list {
    --script-list-columns: 42px 124px minmax(140px, 1fr) 56px 72px 72px 98px 60px 42px 142px 52px 130px;
    --script-list-min-width: 1090px;
    gap: 4px;
    padding-bottom: 6px;
    overflow-x: auto;
  }

  .scripts-list-head,
  .script-row {
    display: grid;
    grid-template-columns: var(--script-list-columns);
    gap: 6px;
    min-width: var(--script-list-min-width);
  }

  .scripts-list-head {
    height: 28px;
    padding: 0 10px;
    border: 1px solid var(--border-primary);
    border-radius: 8px;
    background: var(--bg-secondary);
    color: var(--text-secondary);
    letter-spacing: 0;
    text-transform: none;
  }

  .script-row {
    min-height: 42px;
    padding: 5px 10px;
    align-items: stretch;
    border-radius: 8px;
  }

  .scripts-list-head > span,
  .script-row > span,
  .script-row > div {
    display: flex;
    align-items: center;
    justify-content: center;
    min-width: 0;
    height: 100%;
    overflow: hidden;
    text-align: center;
  }

  .scripts-list-head > span {
    font-size: 10px;
    font-weight: 900;
  }

  .row-name,
  .row-feature,
  .scripts-list-head > span:nth-child(2),
  .scripts-list-head > span:nth-child(3) {
    justify-content: flex-start !important;
    text-align: left !important;
  }

  .row-title {
    margin: 0;
    font-size: 12px;
    font-weight: 800;
  }

  .row-desc {
    font-size: 10px;
  }

  .row-title,
  .row-desc,
  .row-id,
  .row-version-mini,
  .last-time {
    min-width: 0;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .row-status-cell,
  .row-source,
  .row-exec-count,
  .row-time-cell {
    justify-content: center !important;
    text-align: center !important;
    font-family: var(--theme-font-family-mono);
    font-variant-numeric: tabular-nums;
  }

  .last-state {
    width: min(100%, 54px);
    height: 18px;
    padding: 0 4px;
    font-size: 9px;
  }

  .row-time-cell .last-time {
    width: 100%;
    text-align: center;
    font-size: 9px;
  }

  .type-pill,
  .cat-pill,
  .row-copy-btn,
  .row-run-btn,
  .row-edit-btn,
  .row-delete-btn {
    height: 22px;
    font-size: 9px;
  }

  .type-pill {
    min-width: 50px;
    padding: 0 6px;
  }

  .cat-pill {
    padding: 0 6px;
  }

  .row-identity-cell {
    display: grid !important;
    align-content: center;
    justify-items: center;
    gap: 1px;
    font-family: var(--theme-font-family-mono);
  }

  .row-id {
    font-size: 10px;
  }

  .row-version-mini {
    font-size: 9px;
  }

  .row-actions {
    position: static !important;
    right: auto !important;
    z-index: auto !important;
    justify-content: center;
    gap: 3px;
    min-width: 0;
    padding-left: 0;
    background: transparent !important;
    box-shadow: none !important;
  }

  .scripts-list-head span:last-child {
    position: static !important;
    right: auto !important;
    z-index: auto !important;
    background: transparent !important;
    box-shadow: none !important;
  }

  .row-copy-btn {
    min-width: 42px;
    padding: 0 4px;
  }

  .row-run-btn {
    width: auto;
    min-width: 38px;
    padding: 0 4px;
    margin-left: 0;
    gap: 3px;
  }

  .row-fav-btn {
    width: 20px;
    height: 22px;
    font-size: 11px;
  }

  .row-edit-btn,
  .row-delete-btn {
    min-width: 28px;
    padding: 0 3px;
  }

  .row-actions button {
    flex: 0 0 auto;
  }

  .upload-script-modal {
    width: min(860px, 94vw) !important;
    max-height: 90vh;
  }

  .upload-script-modal .modal-body,
  .upload-script-modal .advanced-panel,
  .upload-script-modal .upload-param-builder,
  .upload-script-modal .upload-param-list {
    min-width: 0;
    max-width: 100%;
    box-sizing: border-box;
  }

  .upload-script-modal .field-title-row {
    min-width: 0;
    row-gap: 6px;
  }

  .upload-param-list {
    overflow: visible;
  }

  .upload-param-row {
    display: grid !important;
    grid-template-columns: minmax(112px, 1fr) minmax(168px, 1.25fr) 96px minmax(112px, 1fr) 68px 54px !important;
    gap: 8px;
    align-items: center;
    width: 100%;
    min-width: 0;
    max-width: 100%;
    box-sizing: border-box;
  }

  .upload-param-row input,
  .upload-param-row select,
  .upload-param-row button {
    min-width: 0;
    max-width: 100%;
    box-sizing: border-box;
  }

  .upload-param-row .param-required {
    min-width: 0;
    overflow: hidden;
  }

  .upload-param-row .inline-mini-btn {
    width: 54px;
    padding: 0 4px;
  }

  @media (max-width: 860px) {
    .upload-script-modal {
      width: min(620px, 94vw) !important;
    }

    .upload-param-row {
      grid-template-columns: minmax(0, 1fr) minmax(0, 1fr) !important;
    }

    .upload-param-row .param-required,
    .upload-param-row .inline-mini-btn {
      min-height: 32px;
      width: 100%;
      justify-content: center;
    }
  }

  @media (max-width: 520px) {
    .upload-script-modal {
      width: 94vw !important;
    }

    .upload-script-modal .modal-body {
      padding: 14px;
    }

    .upload-script-modal .advanced-panel {
      padding: 10px;
    }

    .upload-param-row {
      grid-template-columns: minmax(0, 1fr) !important;
    }
  }

  @media (max-width: 768px) {
    .scripts-grid {
      grid-template-columns: 1fr;
    }
    .card-top {
      grid-template-columns: 50px minmax(0, 1fr);
    }
    .card-icon {
      width: 50px;
      height: 50px;
    }
    .card-actions {
      flex-wrap: wrap;
    }
  }
</style>
