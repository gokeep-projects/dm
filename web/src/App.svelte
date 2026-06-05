<script>
  import { onMount, tick } from 'svelte';
  import { fade } from 'svelte/transition';
  import './theme.css';
  import { t, getLang } from './i18n.js';
  import Dashboard from './routes/Dashboard.svelte';
  import Scripts from './routes/Scripts.svelte';
  import ScriptDetail from './routes/ScriptDetail.svelte';
  import CheckList from './routes/CheckList.svelte';
  import CheckResult from './routes/CheckResult.svelte';
  import CheckImportReport from './routes/CheckImportReport.svelte';
  import Knowledge from './routes/Knowledge.svelte';
  import ServiceManage from './routes/ServiceManage.svelte';
  import Help from './routes/Help.svelte';
  import Alerts from './routes/Alerts.svelte';
  import Settings from './routes/Settings.svelte';
  import RuleEngine from './routes/RuleEngine.svelte';

  let route = $state('dashboard');
  let sid = $state('');
  let autorun = $state(false);
  let sidebar = $state(true);
  let transitioning = $state(false);
  let showShortcuts = $state(false);
  let lastKey = $state('');
  let lastKeyTime = $state(0);
  let showPalette = $state(false);
  let paletteQuery = $state('');
  let paletteScripts = $state([]);
  let paletteIdx = $state(0);
  let paletteInput = $state(null);
  let theme = $state('dark');
  let lang = $state('zh');
  let alertCount = $state(0);
  let showAlerts = $state(false);
  let showAbout = $state(false);
  let alerts = $state([]);
  let alertsLoading = $state(false);

  function navigate(target) {
    location.hash = '#/' + target;
  }

  function toggleTheme() {
    theme = theme === 'light' ? 'dark' : 'light';
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem('dm-theme', theme);
  }

  function toggleLang() {
    lang = lang === 'zh' ? 'en' : 'zh';
    localStorage.setItem('dm-lang', lang);
  }

  async function loadAlerts() {
    if (alertsLoading) return;
    alertsLoading = true;
    try {
      const r = await fetch('/api/alerts?ts=' + Date.now(), { cache: 'no-store' });
      if (r.ok) {
        const d = await r.json();
        alerts = d.alerts || [];
        alertCount = d.total || 0;
      }
    } catch (e) { console.warn('加载告警失败:', e); }
    finally { alertsLoading = false; }
  }

  function alertLevelLabel(level) {
    if (level === 'error') return 'FAIL';
    if (level === 'warn') return 'WARN';
    return 'INFO';
  }

  function alertTarget(alert) {
    return alert.service_name || (alert.pid ? `PID ${alert.pid}` : alert.title);
  }

  function firstEvidence(alert) {
    const ev = alert.evidence || [];
    return ev.length ? ev[0] : '';
  }

  async function openPalette() {
    if (paletteScripts.length === 0) {
      try {
        const r = await fetch('/api/scripts');
        if (r.ok) { const d = await r.json(); paletteScripts = d.scripts || []; }
      } catch (_) {}
    }
    paletteQuery = '';
    paletteIdx = 0;
    showPalette = true;
    await tick();
    paletteInput?.focus();
  }

  function closePalette() {
    showPalette = false;
    paletteQuery = '';
  }

  let paletteFiltered = $derived.by(() => {
    const q = paletteQuery.trim().toLowerCase();
    if (!q) return paletteScripts;
    return paletteScripts.filter(s =>
      s.name.toLowerCase().includes(q) ||
      s.id.toLowerCase().includes(q) ||
      (s.feature || '').toLowerCase().includes(q) ||
      (s.description || '').toLowerCase().includes(q) ||
      s.category.toLowerCase().includes(q)
    );
  });

  function gotoScript(s, autorunFlag = false) {
    if (!s) return;
    closePalette();
    location.hash = '#/script/' + s.id + (autorunFlag ? '/run' : '');
  }

  function onKeyDown(e) {
    if (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA' || e.target.isContentEditable) return;

    if (e.key === 'Escape') {
      if (showPalette) { closePalette(); return; }
      if (showShortcuts) { showShortcuts = false; return; }
      return;
    }

    if (showPalette) {
      if (e.key === 'ArrowDown') { e.preventDefault(); paletteIdx = Math.min(paletteIdx + 1, paletteFiltered.length - 1); return; }
      if (e.key === 'ArrowUp') { e.preventDefault(); paletteIdx = Math.max(paletteIdx - 1, 0); return; }
      if (e.key === 'Enter') { e.preventDefault(); gotoScript(paletteFiltered[paletteIdx], e.shiftKey); return; }
      return;
    }

    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') { e.preventDefault(); openPalette(); return; }

    if (e.metaKey || e.ctrlKey || e.altKey) return;

    const now = Date.now();
    if (now - lastKeyTime > 800) lastKey = '';
    lastKey = e.key;
    lastKeyTime = now;

    if (lastKey === 'g') {
      const handler = (e2) => {
        if (e2.key === 'd') navigate('dashboard');
        else if (e2.key === 's') navigate('scripts');
        else if (e2.key === 'h') navigate('help');
        window.removeEventListener('keydown', handler, true);
      };
      window.addEventListener('keydown', handler, true);
      setTimeout(() => window.removeEventListener('keydown', handler, true), 800);
      return;
    }

    if (e.key === '?') { e.preventDefault(); showShortcuts = true; return; }
    if (e.key === '[') { e.preventDefault(); sidebar = !sidebar; return; }
  }

  onMount(() => {
    const savedTheme = localStorage.getItem('dm-theme') || 'dark';
    theme = savedTheme;
    document.documentElement.setAttribute('data-theme', theme);

    const savedLang = localStorage.getItem('dm-lang') || 'zh';
    lang = savedLang;
    if (window.innerWidth < 820) sidebar = false;

    const h = location.hash.slice(1) || '/dashboard';
    parse(h);
    window.onhashchange = () => parse(location.hash.slice(1) || '/dashboard');
    window.addEventListener('keydown', onKeyDown);
    window.addEventListener('dm-alerts-refresh', loadAlerts);
    loadAlerts();
    const alertTimer = setInterval(loadAlerts, 5000);
    return () => {
      clearInterval(alertTimer);
      window.removeEventListener('dm-alerts-refresh', loadAlerts);
      window.removeEventListener('keydown', onKeyDown);
    };
  });

  function parse(h) {
    transitioning = true;
    setTimeout(() => {
      if (h.startsWith('/script/')) {
        const parts = h.replace('/script/', '').split('/');
        sid = parts[0];
        autorun = parts.includes('run');
        route = 'script';
      } else if (h.startsWith('/check/')) {
        sid = h.replace('/check/', '');
        route = 'check';
      } else if (h.startsWith('/check-import')) {
        sid = '';
        route = 'check-import';
      } else if (h.startsWith('/maintenance/')) {
        sid = h.replace('/maintenance/', '');
        route = 'maintenance-detail';
      } else if (h.startsWith('/doc/')) {
        sid = h.replace('/doc/', '');
        route = 'doc-detail';
      } else {
        autorun = false;
        route = h.replace('/', '');
      }
      setTimeout(() => transitioning = false, 50);
    }, 150);
  }

  const nav = $derived([
    { id: 'dashboard', label: t('nav.dashboard', lang), icon: 'dashboard' },
    { id: 'checks', label: t('nav.checks', lang), icon: 'search' },
    { id: 'services', label: t('nav.services', lang), icon: 'tool' },
    { id: 'scripts', label: t('nav.scripts', lang), icon: 'play' },
    { id: 'knowledge', label: '维护管理', icon: 'book' },
    { id: 'alerts', label: '系统告警', icon: 'bell' },
    { id: 'rules', label: '规则引擎', icon: 'rules' },
    { id: 'settings', label: '系统设置', icon: 'settings' },
    { id: 'help', label: t('nav.help', lang), icon: 'help' },
  ]);

  const pageTitle = $derived.by(() => {
    if (route === 'script') return '脚本详情';
    if (route === 'check') return '检查结果';
    if (route === 'check-import') return '导入报告';
    if (route === 'maintenance-detail') return '维护记录';
    if (route === 'doc-detail') return '文档详情';
    return (nav.find(n => n.id === route) || nav[0]).label;
  });

  function iconPath(name) {
    const paths = {
      dashboard: 'M3 13h8V3H3v10Zm10 8h8V3h-8v18ZM3 21h8v-6H3v6Z',
      search: 'M11 19a8 8 0 1 1 5.3-2l4.3 4.3',
      tool: 'M14.7 6.3a4 4 0 0 0-5 5L4 17v3h3l5.7-5.7a4 4 0 0 0 5-5l-2.5 2.5-3-3 2.5-2.5Z',
      play: 'M8 5v14l11-7-11-7Z',
      book: 'M4 5.5A2.5 2.5 0 0 1 6.5 3H20v16H7a3 3 0 0 0-3 3V5.5Zm0 0V22',
      bell: 'M18 8a6 6 0 0 0-12 0c0 7-3 8-3 8h18s-3-1-3-8Zm-5 12a2 2 0 0 1-2 0',
      rules: 'M4 6h16M4 12h16M4 18h7M8 6v12M16 6v6M14 16l2 2 4-5',
      settings: 'M12 15.5A3.5 3.5 0 1 0 12 8a3.5 3.5 0 0 0 0 7.5ZM19 12a7 7 0 0 0-.1-1.2l2-1.5-2-3.5-2.4 1a7.3 7.3 0 0 0-2-1.2L14.2 3h-4.4l-.4 2.6a7.3 7.3 0 0 0-2 1.2l-2.4-1-2 3.5 2 1.5A7 7 0 0 0 5 12c0 .4 0 .8.1 1.2l-2 1.5 2 3.5 2.4-1a7.3 7.3 0 0 0 2 1.2l.4 2.6h4.4l.4-2.6a7.3 7.3 0 0 0 2-1.2l2.4 1 2-3.5-2-1.5c.1-.4.1-.8.1-1.2Z',
      help: 'M9.1 9a3 3 0 1 1 5.8 1c-.5 1.2-1.9 1.7-2.5 2.7-.3.4-.4.8-.4 1.3M12 18h.01'
    };
    return paths[name] || paths.dashboard;
  }

  const shortcuts = [
    { keys: ['g', 'd'], desc: '跳转到仪表盘' },
    { keys: ['g', 's'], desc: '跳转到脚本中心' },
    { keys: ['g', 'h'], desc: '跳转到使用指南' },
    { keys: ['Ctrl', 'K'], desc: '打开命令面板（搜索脚本）' },
    { keys: ['['], desc: '折叠/展开侧边栏' },
    { keys: ['?'], desc: '显示快捷键帮助' },
    { keys: ['Esc'], desc: '关闭弹窗' },
  ];
</script>

<div class="app-shell">
  <aside class="sidebar" class:closed={!sidebar}>
    <div class="brand">
      <div class="logo animate-float">DM</div>
      <div>
        <div class="brand-name">DM 平台</div>
        <div class="brand-ver">运维工具 v2.0</div>
      </div>
    </div>
    <nav class="nav-list">
      {#each nav as item}
        <a href="#/{item.id}" class="nav-item" class:active={route === item.id}>
          <span class="nav-icon">
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path d={iconPath(item.icon)} />
            </svg>
          </span>
          <span>{item.label}</span>
          {#if route === item.id}
            <span class="nav-indicator"></span>
          {/if}
        </a>
      {/each}
    </nav>
    <div class="sidebar-bottom">
      <button class="about-link" onclick={() => showAbout = true}>关于 DM</button>
    </div>
  </aside>

  <main class="main-content">
    <header class="top-bar">
      <button class="menu-toggle" onclick={() => sidebar = !sidebar} aria-label="切换侧边栏">
        <svg width="20" height="20" viewBox="0 0 20 20" fill="none">
          <path d="M3 5h14M3 10h14M3 15h14" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
        </svg>
      </button>
      <h1 class="page-title">{pageTitle}</h1>
      <div class="top-bar-right">
        <button class="theme-toggle" onclick={toggleTheme} title={theme === 'light' ? '切换到暗黑模式' : '切换到明亮模式'}>
          {#if theme === 'light'}
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/></svg>
          {:else}
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="5"/><line x1="12" y1="1" x2="12" y2="3"/><line x1="12" y1="21" x2="12" y2="23"/><line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/><line x1="1" y1="12" x2="3" y2="12"/><line x1="21" y1="12" x2="23" y2="12"/><line x1="4.22" y1="19.78" x2="5.64" y2="18.36"/><line x1="18.36" y1="5.64" x2="19.78" y2="4.22"/></svg>
          {/if}
        </button>
        <button class="lang-toggle" onclick={toggleLang} title={lang === 'zh' ? 'Switch to English' : '切换到中文'}>
          {lang === 'zh' ? 'EN' : '中'}
        </button>
        <button class="alert-bell" onclick={() => { showAlerts = !showAlerts; loadAlerts(); }} title="系统告警">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9"/><path d="M13.73 21a2 2 0 0 1-3.46 0"/></svg>
          {#if alertCount > 0}
            <span class="alert-badge">{alertCount}</span>
          {/if}
        </button>
      </div>
    </header>
    <div class="content-area">
      {#if !transitioning}
        <div in:fade={{ duration: 200, delay: 50 }}>
          {#if route === 'dashboard'}
            <Dashboard />
          {:else if route === 'checks'}
            <CheckList />
          {:else if route === 'check'}
            <CheckResult id={sid} />
          {:else if route === 'check-import'}
            <CheckImportReport />
          {:else if route === 'scripts'}
            <Scripts />
          {:else if route === 'script'}
            <ScriptDetail id={sid} {autorun} />
          {:else if route === 'knowledge'}
            <Knowledge />
          {:else if route === 'maintenance-detail'}
            <Knowledge detailId={sid} detailType="record" />
          {:else if route === 'doc-detail'}
            <Knowledge detailId={sid} detailType="doc" />
          {:else if route === 'services'}
            <ServiceManage />
          {:else if route === 'alerts'}
            <Alerts />
          {:else if route === 'rules'}
            <RuleEngine />
          {:else if route === 'settings'}
            <Settings />
          {:else if route === 'help'}
            <Help />
          {:else}
            <Dashboard />
          {/if}
        </div>
      {/if}
    </div>
  </main>
</div>

{#if showAlerts}
  <div class="alerts-overlay" onclick={() => showAlerts = false} role="presentation">
    <div class="alerts-panel" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="dialog" aria-modal="true" tabindex="-1">
      <div class="alerts-header">
        <h2 class="alerts-title">系统告警</h2>
        <div class="alerts-stats">
          {#if alertCount > 0}
            <span class="alerts-stat stat-error">{alerts.filter(a => a.level === 'error').length} 错误</span>
            <span class="alerts-stat stat-warn">{alerts.filter(a => a.level === 'warn').length} 警告</span>
          {:else}
            <span class="alerts-stat stat-ok">无告警</span>
          {/if}
        </div>
        <button class="alerts-close" onclick={() => showAlerts = false}>✕</button>
      </div>
      <div class="alerts-body">
        {#if alerts.length === 0}
          <div class="alerts-empty">
            <span>当前无活跃告警</span>
          </div>
        {:else}
          {#each alerts as alert}
            <a href="#/alerts" class="alert-item" class:alert-error={alert.level === 'error'} class:alert-warn={alert.level === 'warn'} onclick={() => showAlerts = false}>
              <div class="alert-icon">{alertLevelLabel(alert.level)}</div>
              <div class="alert-info">
                <div class="alert-title">{alertTarget(alert)}</div>
                <div class="alert-message">{alert.summary || alert.message}</div>
                <div class="alert-meta">
                  <span class="alert-type">{alert.pid || alert.log_path || alert.type}</span>
                  <span class="alert-time">{alert.last_seen || alert.timestamp}</span>
                </div>
                {#if alert.rule_id || alert.log_path || firstEvidence(alert)}
                  <div class="alert-extra">
                    {#if alert.rule_id}<span>规则 {alert.rule_id}</span>{/if}
                    {#if alert.log_path}<span>日志 {alert.log_path}</span>{/if}
                    {#if firstEvidence(alert)}<span>{firstEvidence(alert)}</span>{/if}
                  </div>
                {/if}
                {#if alert.handling}
                  <div class="alert-handling">{alert.handling}</div>
                {/if}
              </div>
            </a>
          {/each}
        {/if}
      </div>
      <div class="alerts-footer">
        <button class="alerts-refresh" onclick={loadAlerts}>刷新</button>
        <a href="#/alerts" class="alerts-view-all" onclick={() => showAlerts = false}>查看全部</a>
      </div>
    </div>
  </div>
{/if}

{#if showAbout}
  <div class="about-overlay" onclick={() => showAbout = false} role="presentation">
    <div class="about-modal" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="dialog" aria-modal="true" tabindex="-1">
      <div class="about-head">
        <div>
          <div class="about-mark">DM</div>
          <h2>关于 DM</h2>
        </div>
        <button class="about-close" onclick={() => showAbout = false}>✕</button>
      </div>
      <div class="about-grid">
        <div><span>版本</span><strong>v0.1.0</strong></div>
        <div><span>作者</span><strong>xuning</strong></div>
        <div><span>邮箱</span><strong>gokeeps@qq.com</strong></div>
        <div><span>版权</span><strong>MIT</strong></div>
      </div>
    </div>
  </div>
{/if}

{#if showShortcuts}
  <div class="shortcut-overlay" onclick={() => showShortcuts = false} role="presentation">
    <div class="shortcut-modal" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="dialog" aria-modal="true" aria-labelledby="sc-title" tabindex="-1">
      <div class="shortcut-head">
        <h2 id="sc-title">键盘快捷键</h2>
        <button class="shortcut-close" onclick={() => showShortcuts = false} aria-label="关闭">✕</button>
      </div>
      <div class="shortcut-list">
        {#each shortcuts as sc}
          <div class="shortcut-row">
            <div class="shortcut-keys">
              {#each sc.keys as k, i}
                {#if i > 0}<span class="key-sep">+</span>{/if}
                <kbd>{k}</kbd>
              {/each}
            </div>
            <div class="shortcut-desc">{sc.desc}</div>
          </div>
        {/each}
      </div>
      <div class="shortcut-foot">按 <kbd>Esc</kbd> 关闭</div>
    </div>
  </div>
{/if}

{#if showPalette}
  <div class="palette-overlay" onclick={closePalette} role="presentation">
    <div class="palette-modal" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="dialog" aria-modal="true" aria-labelledby="pal-title" tabindex="-1">
      <div class="palette-search">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="11" cy="11" r="8"/>
          <path d="m21 21-4.35-4.35" stroke-linecap="round"/>
        </svg>
        <input
          bind:this={paletteInput}
          bind:value={paletteQuery}
          placeholder="搜索脚本...（名称、ID、分类、描述）"
          class="palette-input"
          aria-label="搜索脚本"
          autocomplete="off"
          spellcheck="false" />
        <span class="palette-hint"><kbd>↑↓</kbd> 选择 <kbd>Enter</kbd> 打开 <kbd>⇧Enter</kbd> 快速执行</span>
      </div>
      <div class="palette-results">
        {#if paletteFiltered.length === 0}
          <div class="palette-empty">没有匹配 "{paletteQuery}" 的脚本</div>
        {:else}
          {#each paletteFiltered as s, i (s.id)}
            <button
              class="palette-item"
              class:active={i === paletteIdx}
              onclick={() => gotoScript(s, false)}
              onmouseenter={() => paletteIdx = i}>
              <span class="palette-item-name">{s.name}</span>
              <span class="palette-item-id">{s.id}</span>
              <span class="palette-item-cat">{s.category}</span>
              {#if s.feature}<span class="palette-item-feat">{s.feature}</span>{/if}
            </button>
          {/each}
        {/if}
      </div>
      <div class="palette-foot">
        <span>{paletteFiltered.length} 个匹配</span>
        <span>按 <kbd>Esc</kbd> 关闭</span>
      </div>
    </div>
  </div>
{/if}

<style>
  .app-shell {
    display: flex;
    height: 100vh;
    background: var(--bg-primary);
    color: var(--text-primary);
  }

  .sidebar {
    width: 248px;
    min-width: 248px;
    background: #141821;
    border-right: 1px solid #252b36;
    display: flex;
    flex-direction: column;
    transition: width 0.22s ease, min-width 0.22s ease;
    z-index: 20;
  }

  .sidebar.closed {
    width: 0;
    min-width: 0;
    overflow: hidden;
    border-right: none;
  }

  .brand {
    padding: 18px 18px;
    border-bottom: 1px solid #252b36;
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .logo {
    width: 42px;
    height: 42px;
    border-radius: 8px;
    background: #0f766e;
    display: flex;
    align-items: center;
    justify-content: center;
    font-weight: 800;
    font-size: 15px;
    color: #fff;
  }

  .brand-name {
    font-weight: 700;
    font-size: 15px;
    color: #f8fafc;
    letter-spacing: 0;
  }

  .brand-ver {
    font-size: 13px;
    color: #8b95a7;
    margin-top: 2px;
  }

  .nav-list {
    flex: 1;
    padding: 12px 10px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border-radius: 8px;
    font-size: 14px;
    font-weight: 500;
    color: #a7b0c0;
    text-decoration: none;
    transition: background 0.16s ease, color 0.16s ease;
    position: relative;
    overflow: hidden;
  }

  .nav-item:hover {
    background: #1d2430;
    color: #f1f5f9;
  }

  .nav-item.active {
    background: #0f766e;
    color: #ffffff;
  }

  .nav-icon {
    width: 20px;
    height: 20px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .nav-icon svg {
    width: 18px;
    height: 18px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .nav-indicator {
    position: absolute;
    left: 0;
    top: 50%;
    transform: translateY(-50%);
    width: 3px;
    height: 20px;
    background: #5eead4;
    border-radius: 0 3px 3px 0;
  }

  .sidebar-bottom {
    border-top: 1px solid #252b36;
    padding: 10px 12px 14px;
  }

  .about-link {
    width: 100%;
    padding: 8px 10px;
    border: 1px solid rgba(94, 234, 212, 0.18);
    border-radius: 8px;
    background: rgba(15, 118, 110, 0.08);
    color: #99f6e4;
    font-size: 12px;
    font-weight: 700;
    text-align: left;
    cursor: pointer;
  }

  .about-link:hover {
    background: rgba(15, 118, 110, 0.18);
    border-color: rgba(94, 234, 212, 0.38);
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.5; }
  }

  .main-content {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .top-bar {
    height: 58px;
    min-height: 58px;
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 0 24px;
    background: var(--bg-card);
    border-bottom: 1px solid var(--border-primary);
  }

  .menu-toggle {
    background: none;
    border: none;
    color: var(--text-secondary);
    cursor: pointer;
    padding: 8px;
    border-radius: 8px;
    transition: all 0.2s;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .menu-toggle:hover {
    color: var(--text-primary);
    background: var(--bg-hover);
  }

  .page-title {
    font-size: 20px;
    font-weight: 600;
    color: var(--text-primary);
    letter-spacing: 0;
  }

  .top-bar-right {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .theme-toggle, .lang-toggle {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 34px;
    height: 34px;
    border: 1px solid var(--border-primary);
    background: var(--bg-card);
    color: var(--text-secondary);
    border-radius: 8px;
    cursor: pointer;
    transition: all var(--transition-fast);
  }

  .theme-toggle:hover, .lang-toggle:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
    border-color: var(--border-focus);
  }

  .lang-toggle {
    font-size: 12px;
    font-weight: 600;
  }

  .alert-bell {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 34px;
    height: 34px;
    border: 1px solid var(--border-primary);
    background: var(--bg-card);
    color: var(--text-secondary);
    border-radius: 8px;
    cursor: pointer;
    transition: all var(--transition-fast);
  }

  .alert-bell:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
    border-color: var(--border-focus);
  }

  .alert-badge {
    position: absolute;
    top: -4px;
    right: -4px;
    min-width: 16px;
    height: 16px;
    padding: 0 4px;
    background: #ef4444;
    color: white;
    font-size: 10px;
    font-weight: 700;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    animation: pulse 2s ease-in-out infinite;
  }

  @keyframes pulse {
    0%, 100% { transform: scale(1); }
    50% { transform: scale(1.1); }
  }

  .content-area {
    flex: 1;
    overflow: auto;
    padding: clamp(14px, 1.6vw, 28px);
    background: var(--bg-primary);
  }

  .about-overlay {
    position: fixed;
    inset: 0;
    z-index: 130;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 18px;
    background: rgba(2, 6, 23, 0.66);
    backdrop-filter: blur(10px);
  }

  .about-modal {
    width: min(420px, 100%);
    overflow: hidden;
    border-radius: 14px;
    border: 1px solid rgba(94, 234, 212, 0.22);
    background: linear-gradient(180deg, rgba(15, 23, 42, 0.98), rgba(8, 13, 24, 0.98));
    box-shadow: 0 28px 80px rgba(0, 0, 0, 0.46);
  }

  .about-head {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 16px;
    padding: 18px 18px 14px;
    border-bottom: 1px solid rgba(148, 163, 184, 0.14);
  }

  .about-mark {
    display: inline-grid;
    place-items: center;
    width: 38px;
    height: 38px;
    margin-bottom: 10px;
    border-radius: 9px;
    background: #0f766e;
    color: #fff;
    font-size: 13px;
    font-weight: 900;
    box-shadow: 0 0 30px rgba(45, 212, 191, 0.16);
  }

  .about-head h2 {
    margin: 0;
    color: #f8fafc;
    font-size: 18px;
    letter-spacing: 0;
  }

  .about-close {
    border: none;
    border-radius: 8px;
    background: rgba(148, 163, 184, 0.08);
    color: #94a3b8;
    cursor: pointer;
    padding: 7px 10px;
  }

  .about-close:hover {
    color: #f8fafc;
    background: rgba(148, 163, 184, 0.16);
  }

  .about-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
    padding: 16px 18px 18px;
  }

  .about-grid div {
    min-width: 0;
    padding: 12px;
    border-radius: 10px;
    border: 1px solid rgba(148, 163, 184, 0.14);
    background: rgba(15, 23, 42, 0.64);
  }

  .about-grid span {
    display: block;
    margin-bottom: 5px;
    color: #94a3b8;
    font-size: 12px;
  }

  .about-grid strong {
    display: block;
    color: #e2e8f0;
    font-family: var(--theme-font-family-mono);
    font-size: 13px;
    overflow-wrap: anywhere;
  }

  @media (max-width: 820px) {
    .sidebar {
      position: fixed;
      inset: 0 auto 0 0;
      height: 100vh;
      box-shadow: 16px 0 40px rgba(15, 23, 42, 0.28);
    }

    .sidebar.closed {
      transform: translateX(-100%);
      width: 248px;
      min-width: 248px;
      border-right: 1px solid #252b36;
    }

    .top-bar {
      padding: 0 12px;
      gap: 10px;
    }

    .page-title {
      font-size: 18px;
      max-width: 42vw;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }

    .top-bar-right {
      gap: 6px;
    }

    .content-area {
      padding: 14px;
    }
  }

  .shortcut-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.65);
    backdrop-filter: blur(8px);
    z-index: 100;
    display: flex;
    align-items: center;
    justify-content: center;
    animation: scFadeIn 0.18s ease-out;
  }
  @keyframes scFadeIn { from { opacity: 0; } to { opacity: 1; } }

  .shortcut-modal {
    width: 420px;
    max-width: 90vw;
    background: rgba(20, 22, 30, 0.95);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 16px;
    padding: 20px 24px;
    box-shadow: 0 24px 80px rgba(0, 0, 0, 0.6);
    animation: scSlideUp 0.22s cubic-bezier(0.4, 0, 0.2, 1);
  }
  @keyframes scSlideUp { from { opacity: 0; transform: translateY(12px) scale(0.97); } to { opacity: 1; transform: translateY(0) scale(1); } }

  .shortcut-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 16px;
    padding-bottom: 12px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  }
  .shortcut-head h2 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
    color: #f1f5f9;
    letter-spacing: -0.2px;
  }
  .shortcut-close {
    background: none;
    border: none;
    color: #6b7280;
    font-size: 16px;
    cursor: pointer;
    padding: 4px 8px;
    border-radius: 6px;
    transition: all 0.15s;
  }
  .shortcut-close:hover { background: rgba(255, 255, 255, 0.06); color: #c9d1d9; }

  .shortcut-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .shortcut-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 10px;
    border-radius: 8px;
    transition: background 0.15s;
  }
  .shortcut-row:hover { background: rgba(255, 255, 255, 0.03); }
  .shortcut-keys { display: flex; align-items: center; gap: 4px; }
  .shortcut-keys kbd {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 24px;
    height: 24px;
    padding: 0 6px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-bottom-width: 2px;
    border-radius: 6px;
    font-family: var(--theme-font-family-mono);
    font-size: 11px;
    color: #c9d1d9;
    font-weight: 500;
  }
  .key-sep { color: #4b5563; font-size: 10px; margin: 0 2px; }
  .shortcut-desc { font-size: 12px; color: #94a3b8; }
  .shortcut-foot {
    margin-top: 14px;
    padding-top: 12px;
    border-top: 1px solid rgba(255, 255, 255, 0.06);
    text-align: center;
    font-size: 11px;
    color: #6b7280;
  }
  .shortcut-foot kbd {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 22px;
    height: 20px;
    padding: 0 5px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-bottom-width: 2px;
    border-radius: 5px;
    font-family: var(--theme-font-family-mono);
    font-size: 10px;
    color: #94a3b8;
    margin: 0 2px;
  }

  .palette-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.65);
    backdrop-filter: blur(8px);
    z-index: 110;
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 12vh;
    animation: scFadeIn 0.15s ease-out;
  }

  .palette-modal {
    width: 560px;
    max-width: 92vw;
    max-height: 70vh;
    background: rgba(20, 22, 30, 0.97);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 14px;
    box-shadow: 0 24px 80px rgba(0, 0, 0, 0.6);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    animation: scSlideUp 0.18s cubic-bezier(0.4, 0, 0.2, 1);
  }

  .palette-search {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 14px 16px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    color: #6b7280;
  }
  .palette-input {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    color: #f1f5f9;
    font-size: 14px;
    font-family: var(--theme-font-family-base);
    min-width: 0;
  }
  .palette-input::placeholder { color: #4b5563; }
  .palette-hint {
    font-size: 10px;
    color: #4b5563;
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
  }
  .palette-hint kbd {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 18px;
    height: 18px;
    padding: 0 4px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 4px;
    font-family: var(--theme-font-family-mono);
    font-size: 9px;
    color: #94a3b8;
    margin: 0 1px;
  }

  .palette-results {
    flex: 1;
    overflow-y: auto;
    min-height: 80px;
    max-height: 50vh;
  }
  .palette-empty {
    padding: 32px 16px;
    text-align: center;
    color: #6b7280;
    font-size: 13px;
  }
  .palette-item {
    display: grid;
    grid-template-columns: 1fr auto;
    grid-template-rows: auto auto;
    gap: 2px 10px;
    width: 100%;
    padding: 10px 16px;
    border: none;
    background: transparent;
    text-align: left;
    cursor: pointer;
    transition: background 0.1s;
    border-bottom: 1px solid rgba(255, 255, 255, 0.03);
  }
  .palette-item:last-child { border-bottom: none; }
  .palette-item.active { background: rgba(34, 211, 238, 0.08); }
  .palette-item:hover { background: rgba(34, 211, 238, 0.05); }
  .palette-item-name {
    font-size: 13px;
    font-weight: 600;
    color: #f1f5f9;
    grid-column: 1;
    grid-row: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .palette-item-id {
    font-family: var(--theme-font-family-mono);
    font-size: 10px;
    color: #6b7280;
    grid-column: 2;
    grid-row: 1;
    align-self: center;
  }
  .palette-item-cat {
    font-size: 10px;
    color: #94a3b8;
    background: rgba(34, 211, 238, 0.08);
    border: 1px solid rgba(34, 211, 238, 0.15);
    padding: 1px 6px;
    border-radius: 4px;
    font-weight: 500;
    grid-column: 1;
    grid-row: 2;
    justify-self: start;
  }
  .palette-item-feat {
    font-size: 10px;
    color: #6b7280;
    grid-column: 1 / -1;
    grid-row: 3;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    margin-top: 1px;
  }

  .palette-foot {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 8px 16px;
    border-top: 1px solid rgba(255, 255, 255, 0.06);
    font-size: 11px;
    color: #6b7280;
  }

  .alerts-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    backdrop-filter: blur(4px);
    z-index: 120;
    display: flex;
    align-items: flex-start;
    justify-content: flex-end;
    padding: 80px 20px 20px 20px;
  }

  .alerts-panel {
    width: 400px;
    max-height: calc(100vh - 100px);
    background: var(--bg-card);
    border: 1px solid var(--border-primary);
    border-radius: 16px;
    box-shadow: var(--shadow-lg);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    animation: slideDown 0.2s ease-out;
  }

  @keyframes slideDown {
    from { opacity: 0; transform: translateY(-10px); }
    to { opacity: 1; transform: translateY(0); }
  }

  .alerts-header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 16px 20px;
    border-bottom: 1px solid var(--border-primary);
  }

  .alerts-title {
    font-size: 16px;
    font-weight: 700;
    color: var(--text-primary);
    margin: 0;
  }

  .alerts-stats {
    display: flex;
    gap: 8px;
    margin-left: auto;
  }

  .alerts-stat {
    font-size: 11px;
    padding: 2px 8px;
    border-radius: 6px;
    font-weight: 600;
  }

  .stat-error { background: rgba(239, 68, 68, 0.1); color: #ef4444; }
  .stat-warn { background: rgba(245, 158, 11, 0.1); color: #f59e0b; }
  .stat-ok { background: rgba(16, 185, 129, 0.1); color: #10b981; }

  .alerts-close {
    background: none;
    border: none;
    color: var(--text-tertiary);
    font-size: 18px;
    cursor: pointer;
    padding: 4px 8px;
    border-radius: 6px;
  }

  .alerts-close:hover { background: var(--bg-hover); }

  .alerts-body {
    flex: 1;
    overflow-y: auto;
    padding: 12px;
  }

  .alerts-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 40px 0;
    color: var(--text-secondary);
  }

  .alert-item {
    display: flex;
    gap: 12px;
    padding: 12px;
    border-radius: 10px;
    margin-bottom: 8px;
    transition: all 0.15s;
    color: inherit;
    text-decoration: none;
  }

  .alert-item:hover { background: var(--bg-hover); }

  .alert-error { background: rgba(239, 68, 68, 0.05); border: 1px solid rgba(239, 68, 68, 0.15); }
  .alert-warn { background: rgba(245, 158, 11, 0.05); border: 1px solid rgba(245, 158, 11, 0.15); }

  .alert-icon {
    flex-shrink: 0;
    width: 44px;
    min-width: 44px;
    height: 32px;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 10px;
    font-weight: 800;
    font-family: var(--theme-font-family-mono);
  }

  .alert-error .alert-icon { background: rgba(239, 68, 68, 0.1); color: #ef4444; }
  .alert-warn .alert-icon { background: rgba(245, 158, 11, 0.1); color: #f59e0b; }

  .alert-info { flex: 1; min-width: 0; }

  .alert-title {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
    margin-bottom: 2px;
  }

  .alert-message {
    font-size: 12px;
    color: var(--text-secondary);
    margin-bottom: 4px;
  }

  .alert-meta {
    display: flex;
    gap: 8px;
    font-size: 10px;
    color: var(--text-tertiary);
  }

  .alert-type {
    background: var(--bg-tertiary);
    padding: 1px 6px;
    border-radius: 4px;
  }

  .alert-extra {
    display: flex;
    flex-direction: column;
    gap: 3px;
    margin-top: 5px;
    color: var(--text-tertiary);
    font-size: 10px;
    line-height: 1.35;
    overflow-wrap: anywhere;
  }

  .alert-handling {
    margin-top: 6px;
    color: var(--text-secondary);
    font-size: 11px;
    line-height: 1.4;
  }

  .alerts-footer {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 16px;
    border-top: 1px solid var(--border-primary);
  }

  .alerts-refresh {
    padding: 6px 12px;
    background: var(--bg-secondary);
    border: 1px solid var(--border-primary);
    border-radius: 6px;
    color: var(--text-secondary);
    font-size: 12px;
    cursor: pointer;
  }

  .alerts-refresh:hover { background: var(--bg-hover); }

  .alerts-view-all {
    font-size: 12px;
    color: var(--accent-primary);
    text-decoration: none;
  }

  .alerts-view-all:hover { text-decoration: underline; }
</style>
