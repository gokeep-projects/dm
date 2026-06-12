<script>
  import { onMount, tick } from 'svelte';
  import { fade } from 'svelte/transition';
  import './theme.css';
  import { t, getLang } from './i18n.js';
  import Dashboard from './routes/Dashboard.svelte';

  const routeLoaders = {
    dashboard: () => Promise.resolve({ default: Dashboard }),
    checks: () => import('./routes/CheckList.svelte'),
    check: () => import('./routes/CheckResult.svelte'),
    'check-import': () => import('./routes/CheckImportReport.svelte'),
    scripts: () => import('./routes/Scripts.svelte'),
    script: () => import('./routes/ScriptDetail.svelte'),
    knowledge: () => import('./routes/Docs.svelte'),
    'maintenance-detail': () => import('./routes/Docs.svelte'),
    'doc-detail': () => import('./routes/Docs.svelte'),
    services: () => import('./routes/ServiceManage.svelte'),
    traffic: () => import('./routes/TrafficAnalysis.svelte'),
    'java-analyzer': () => import('./routes/JavaAnalyzer.svelte'),
    alerts: () => import('./routes/Alerts.svelte'),
    rules: () => import('./routes/RuleEngine.svelte'),
    settings: () => import('./routes/Settings.svelte'),
    help: () => import('./routes/Help.svelte'),
  };

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
  let lang = $state('zh');
  let alertCount = $state(0);
  // 连接状态: 由 /api/ping 心跳驱动
  let connectStatus = $state('unknown'); // unknown | connected | disconnected
  // 状态指示器只反映"本平台与后端的连通性", 告警走独立铃铛
  const systemStatus = $derived(
    connectStatus === 'disconnected' ? 'disconnected' :
    connectStatus === 'unknown' ? 'unknown' :
    'ok'
  );
  let lastHeartbeatAt = $state(0);
  let heartbeatCount = $state(0);
  let heartbeatTimer = null;
  let heartbeatToken = 0;
  const HEARTBEAT_TIMEOUT_MS = 6000;
  const HEARTBEAT_INTERVAL_MS = 3000;

  let showAlerts = $state(false);
  let alerts = $state([]);
  let alertsLoading = $state(false);
  let ActiveRoute = $state(Dashboard);
  let activeRouteKey = $state('dashboard');
  let routeLoading = $state(false);
  let routeLoadSeq = 0;

  function navigate(target) {
    location.hash = '#/' + target;
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

  /// 一次心跳: 先 ping (验证后端可达), 再拉告警统计 (决定 status 颜色)
  /// 任何一次成功都触发一次"闪动"动画, 失败则标红
  async function sendHeartbeat() {
    const token = ++heartbeatToken;
    try {
      const pingCtrl = new AbortController();
      const pingTimer = setTimeout(() => pingCtrl.abort(), HEARTBEAT_TIMEOUT_MS);
      const r = await fetch('/api/ping?ts=' + Date.now(), {
        cache: 'no-store',
        signal: pingCtrl.signal,
      });
      clearTimeout(pingTimer);
      if (token !== heartbeatToken) return;
      if (!r.ok) {
        connectStatus = 'disconnected';
        return;
      }
      // ping 成功 -> 触发闪动 + 标记连接
      if (token === heartbeatToken) connectStatus = 'connected';
      lastHeartbeatAt = Date.now();
      heartbeatCount += 1;
      // 同时拉告警统计, 不阻塞视觉反馈
      try {
        const ar = await fetch('/api/alerts?ts=' + Date.now(), { cache: 'no-store' });
        if (token === heartbeatToken && ar.ok) {
          const d = await ar.json();
          const items = Array.isArray(d.alerts) ? d.alerts : [];
          const hasError = items.some(a => a.level === 'error');
          const hasWarn = items.some(a => a.level === 'warn');
        }
      } catch (_) {}
    } catch (_) {
      if (token === heartbeatToken) connectStatus = 'disconnected';
    }
  }

  function startHeartbeat() {
    if (heartbeatTimer) return;
    sendHeartbeat();
    heartbeatTimer = setInterval(sendHeartbeat, HEARTBEAT_INTERVAL_MS);
  }

  function stopHeartbeat() {
    if (heartbeatTimer) {
      clearInterval(heartbeatTimer);
      heartbeatTimer = null;
    }
  }

  function statusTitle() {
    const conn = connectStatus === 'connected' ? '已连接'
              : connectStatus === 'disconnected' ? '后端不可达'
              : '正在连接';
    return `服务运行状态 · 心跳 ${heartbeatCount} 次 · ${conn}`;
  }

  function statusLabel() {
    if (connectStatus === 'disconnected') return '后端断开';
    if (connectStatus === 'unknown') return '连接中';
    return '运行正常';
  }

  function compactCount(value) {
    const n = Number(value || 0);
    return n > 500 ? '500+' : String(n);
  }

  function normalizedRouteKey(value) {
    return routeLoaders[value] ? value : 'dashboard';
  }

  let activeRouteProps = $derived.by(() => {
    if (activeRouteKey === 'script') return { id: sid, autorun };
    if (activeRouteKey === 'check') return { id: sid };
    if (activeRouteKey === 'doc-detail') return { detailId: sid };
    return {};
  });

  $effect(() => {
    const key = normalizedRouteKey(route);
    const seq = ++routeLoadSeq;
    routeLoading = key !== activeRouteKey;
    routeLoaders[key]().then((module) => {
      if (seq !== routeLoadSeq) return;
      ActiveRoute = module.default;
      activeRouteKey = key;
      routeLoading = false;
    }).catch((error) => {
      console.error('加载页面失败:', error);
      if (seq !== routeLoadSeq) return;
      ActiveRoute = Dashboard;
      activeRouteKey = 'dashboard';
      routeLoading = false;
    });
  });

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

  function attachTableDragScroll() {
    let active = null;
    const selector = '.table-scroll, .table-wrap, .item-table, .result-table, .proc-table, .process-table';
    const interactive = 'button, a, input, textarea, select, summary, details';
    const onPointerDown = (event) => {
      if (event.button !== 0 || event.target.closest(interactive)) return;
      const el = event.target.closest(selector);
      if (!el || el.scrollWidth <= el.clientWidth) return;
      active = {
        el,
        x: event.clientX,
        left: el.scrollLeft,
        moved: false,
      };
      el.classList.add('drag-scrolling');
    };
    const onPointerMove = (event) => {
      if (!active) return;
      const delta = event.clientX - active.x;
      if (Math.abs(delta) > 3) active.moved = true;
      active.el.scrollLeft = active.left - delta;
      if (active.moved) event.preventDefault();
    };
    const stop = () => {
      if (!active) return;
      active.el.classList.remove('drag-scrolling');
      active = null;
    };
    window.addEventListener('pointerdown', onPointerDown);
    window.addEventListener('pointermove', onPointerMove, { passive: false });
    window.addEventListener('pointerup', stop);
    window.addEventListener('pointercancel', stop);
    return () => {
      window.removeEventListener('pointerdown', onPointerDown);
      window.removeEventListener('pointermove', onPointerMove);
      window.removeEventListener('pointerup', stop);
      window.removeEventListener('pointercancel', stop);
    };
  }

  onMount(() => {
    startHeartbeat();
    document.documentElement.setAttribute('data-theme', 'dark');

    const savedLang = localStorage.getItem('dm-lang') || 'zh';
    lang = savedLang;
    if (window.innerWidth < 820) sidebar = false;

    const h = location.hash.slice(1) || '/dashboard';
    parse(h);
    window.onhashchange = () => parse(location.hash.slice(1) || '/dashboard');
    window.addEventListener('keydown', onKeyDown);
    window.addEventListener('dm-alerts-refresh', loadAlerts);
    loadAlerts();
    const detachTableDragScroll = attachTableDragScroll();
    const alertTimer = setInterval(loadAlerts, 5000);
    return () => {
      clearInterval(alertTimer);
      stopHeartbeat();
      detachTableDragScroll();
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
    { id: 'traffic', label: '流量分析', icon: 'traffic' },
    { id: 'java-analyzer', label: '堆栈分析', icon: 'java' },
    { id: 'scripts', label: t('nav.scripts', lang), icon: 'play' },
    { id: 'knowledge', label: '维护文档', icon: 'book' },
    { id: 'alerts', label: '系统告警', icon: 'bell' },
    { id: 'rules', label: '规则引擎', icon: 'rules' },
    { id: 'help', label: t('nav.help', lang), icon: 'help' },
    { id: 'settings', label: '系统设置', icon: 'settings' },
  ]);

  const pageTitle = $derived.by(() => {
    if (route === 'script') return '脚本详情';
    if (route === 'check') return '检查结果';
    if (route === 'check-import') return '导入报告';
    if (route === 'maintenance-detail') return '维护文档';
    if (route === 'doc-detail') return '文档详情';
    return (nav.find(n => n.id === route) || nav[0]).label;
  });

  function iconPath(name) {
    const paths = {
      dashboard: 'M4 6.5 12 3l8 3.5v7L12 21l-8-7.5v-7Zm4 1.4 4 1.8 4-1.8M8 13.7l4 2.9 4-2.9M12 9.7v6.9',
      search: 'M5 11.5a6.5 6.5 0 1 1 11.1 4.6L21 21M8.5 11.5h7M12 8v7M10 3.7 12 2l2 1.7',
      tool: 'M6 19 19 6M8 21H4v-4l9.5-9.5m3.2-3.2 1.6-1.6 2.7 2.7-1.6 1.6M14 19h6M17 16v6M4 8h6M7 5v6',
      traffic: 'M3 12h3l2-7 4 14 3-9 2 5h4M5 5h4m10 0h-4M5 19h4m10 0h-4M9 5v3m6-3v3M9 16v3m6-3v3',
      java: 'M7 18c3 1.8 7 1.8 10 0M8 21c2.6 1 5.4 1 8 0M10 4c2 1.8 2 3.2 0 5s-2 3.2 0 5M14 3c2.4 2.2 2.4 4 0 6.2M5 14h14M6 11h12M5 7h5m4 0h5',
      play: 'M6 5.5 19 12 6 18.5v-13Zm3.5 4.3v4.4L14 12 9.5 9.8ZM4 4h3M4 20h3M17 4h3M17 20h3',
      book: 'M5 4h12a2 2 0 0 1 2 2v15H7a3 3 0 0 1-3-3V6a2 2 0 0 1 2-2Zm2 0v14m3-9h5m-5 4h4',
      bell: 'M18 9a6 6 0 0 0-12 0c0 5.2-2.2 7.1-3 8h18c-.8-.9-3-2.8-3-8ZM10 21h4M8 4l-1.5-2M16 4l1.5-2',
      rules: 'M4 5h10M4 12h8M4 19h7M17 5l2 2 3-4M17 12l2 2 3-4M16 19l2 2 4-6M8 5v14',
      settings: 'M12 14.8A2.8 2.8 0 1 0 12 9.2a2.8 2.8 0 0 0 0 5.6ZM19.6 13.5v-3l-2.2-.5a6.5 6.5 0 0 0-.8-1.8l1.1-2-2.1-2.1-2 1.1a6.5 6.5 0 0 0-1.8-.8L11.3 2h-3l-.5 2.2a6.5 6.5 0 0 0-1.8.8l-2-1.1-2.1 2.1 1.1 2a6.5 6.5 0 0 0-.8 1.8l-2.2.5v3l2.2.5c.2.7.4 1.3.8 1.8l-1.1 2 2.1 2.1 2-1.1c.6.3 1.2.6 1.8.8l.5 2.2h3l.5-2.2c.7-.2 1.3-.4 1.8-.8l2 1.1 2.1-2.1-1.1-2c.3-.6.6-1.2.8-1.8l2.2-.5Z',
      help: 'M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18Zm0-5h.01M9.7 9.2a2.5 2.5 0 1 1 4.5 1.5c-.8.9-2.2 1.2-2.2 3'
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
      <a class="logo animate-float" href="#/dashboard" aria-label="返回仪表盘首页" title="返回仪表盘首页">
        <svg viewBox="0 0 48 48" aria-hidden="true">
          <defs>
            <linearGradient id="dm-logo-line" x1="6" y1="6" x2="42" y2="42">
              <stop offset="0" stop-color="#67e8f9" />
              <stop offset=".52" stop-color="#2dd4bf" />
              <stop offset="1" stop-color="#a78bfa" />
            </linearGradient>
          </defs>
          <path class="logo-ring" d="M24 4 42 14v20L24 44 6 34V14L24 4Z" />
          <path class="logo-core" d="M16 17.5h8.5c5.1 0 8.5 2.8 8.5 6.5s-3.4 6.5-8.5 6.5H16v-13Zm5.2 3.7v5.6h3.2c2.1 0 3.4-1.1 3.4-2.8s-1.3-2.8-3.4-2.8h-3.2Z" />
          <path class="logo-circuit" d="M12 12h8M28 12h8M12 36h8M28 36h8M24 4v8M24 36v8" />
        </svg>
      </a>
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
        <div
          class="sys-status"
          class:ok={systemStatus === 'ok'}
          class:warn={false}
          class:err={false}
          class:unknown={systemStatus === 'unknown'}
          class:disconnected={systemStatus === 'disconnected'}
          class:pulse={lastHeartbeatAt > 0}
          style="--beat:{heartbeatCount}"
          title={statusTitle()}
          role="status"
          aria-live="polite"
          tabindex="0">
          <!--
            {#key heartbeatCount} 让 .sys-dot / .sys-beat 在每次心跳时重新挂载,
            强制 CSS animation 重新播放. 仅靠 .pulse class 持续存在无法重播动画.
          -->
          {#key heartbeatCount}
            <span class="sys-dot"></span>
            <span class="sys-beat" aria-hidden="true"></span>
          {/key}
          <span class="sys-label">{statusLabel()}</span>
        </div>
        <button class="alert-bell" onclick={() => { showAlerts = !showAlerts; loadAlerts(); }} title="系统告警">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9"/><path d="M13.73 21a2 2 0 0 1-3.46 0"/></svg>
          {#if alertCount > 0}
            <span class="alert-badge">{compactCount(alertCount)}</span>
          {/if}
        </button>
      </div>
    </header>
    <div class="content-area">
      {#if !transitioning}
        <div class="route-shell" class:dashboard-route={normalizedRouteKey(route) === 'dashboard'} in:fade={{ duration: 200, delay: 50 }}>
          {#if routeLoading}
            <div class="route-loading" aria-live="polite">
              <span></span>
              <em>加载页面</em>
            </div>
          {:else}
            <ActiveRoute {...activeRouteProps} />
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
            <span class="alerts-stat stat-error">{compactCount(alerts.filter(a => a.level === 'error').length)} 错误</span>
            <span class="alerts-stat stat-warn">{compactCount(alerts.filter(a => a.level === 'warn').length)} 警告</span>
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
    background:
      radial-gradient(circle at 50% 42%, rgba(103, 232, 249, 0.22), transparent 58%),
      linear-gradient(145deg, rgba(2, 6, 23, 0.96), rgba(8, 47, 73, 0.78));
    display: flex;
    align-items: center;
    justify-content: center;
    color: #fff;
    box-shadow: 0 0 0 1px rgba(125, 211, 252, 0.26), 0 0 30px rgba(34, 211, 238, 0.24), inset 0 0 24px rgba(45, 212, 191, 0.08);
    position: relative;
    overflow: hidden;
  }

  .logo::after {
    content: '';
    position: absolute;
    inset: 7px;
    border-radius: 7px;
    border: 1px solid rgba(94, 234, 212, 0.22);
    filter: drop-shadow(0 0 8px rgba(45, 212, 191, 0.35));
  }

  .logo svg {
    width: 34px;
    height: 34px;
    position: relative;
    z-index: 1;
  }

  .logo-ring,
  .logo-circuit {
    fill: none;
    stroke: url(#dm-logo-line);
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .logo-core {
    fill: url(#dm-logo-line);
    filter: drop-shadow(0 0 8px rgba(103, 232, 249, 0.46));
  }

  .logo-circuit {
    stroke-width: 1.5;
    opacity: 0.9;
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
    border: 1px solid transparent;
    transition: background 0.16s ease, color 0.16s ease, border-color 0.16s ease, transform 0.16s ease, box-shadow 0.16s ease;
    position: relative;
    overflow: hidden;
  }

  .nav-item:hover {
    background: linear-gradient(135deg, rgba(34, 211, 238, 0.09), rgba(99, 102, 241, 0.08));
    color: #f1f5f9;
    border-color: rgba(34, 211, 238, 0.16);
    transform: translateX(2px);
    box-shadow: inset 0 0 20px rgba(34, 211, 238, 0.04);
  }

  .nav-item.active {
    background: linear-gradient(135deg, rgba(20, 184, 166, 0.92), rgba(14, 116, 144, 0.94));
    color: #ffffff;
    border-color: rgba(125, 211, 252, 0.26);
    box-shadow: 0 10px 24px rgba(13, 148, 136, 0.18), inset 0 0 20px rgba(255, 255, 255, 0.05);
  }

  .nav-icon {
    width: 28px;
    height: 28px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    position: relative;
    border-radius: 8px;
    background: rgba(15, 23, 42, 0.56);
    box-shadow: inset 0 0 0 1px rgba(148, 163, 184, 0.1);
  }

  .nav-icon::before {
    content: '';
    position: absolute;
    inset: 4px;
    border-radius: inherit;
    background: radial-gradient(circle, rgba(34, 211, 238, 0.22), transparent 68%);
    opacity: 0;
    transition: opacity 0.16s ease;
  }

  .nav-icon svg {
    width: 18px;
    height: 18px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
    position: relative;
    z-index: 1;
    filter: drop-shadow(0 0 8px rgba(34, 211, 238, 0.12));
  }

  .nav-item:hover .nav-icon::before,
  .nav-item.active .nav-icon::before {
    opacity: 1;
  }

  .nav-item.active .nav-icon {
    background: rgba(2, 6, 23, 0.34);
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.16), 0 0 20px rgba(45, 212, 191, 0.18);
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

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.5; }
  }

  .main-content {
    flex: 1;
    min-width: 0;
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


  
  /* === 服务运行状态指示器 === */
  .sys-status {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    padding: 0 10px;
    border-radius: 999px;
    background: rgba(15, 23, 42, 0.55);
    border: 1px solid rgba(148, 163, 184, 0.18);
    color: #94a3b8;
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    user-select: none;
    transition: all 0.2s;
  }
  .sys-status:hover { background: rgba(15, 23, 42, 0.8); border-color: rgba(34, 211, 238, 0.32); }
  .sys-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #64748b;
    box-shadow: 0 0 0 0 currentColor;
    transition: all 0.2s;
  }
  .sys-status.ok .sys-dot { background: #22c55e; box-shadow: 0 0 6px rgba(34,197,94,0.55); }
  .sys-status.ok .sys-label { color: #86efac; }
  .sys-status.warn .sys-dot { background: #f59e0b; box-shadow: 0 0 6px rgba(245,158,11,0.55); }
  .sys-status.warn .sys-label { color: #fcd34d; }
  .sys-status.err .sys-dot { background: #ef4444; box-shadow: 0 0 8px rgba(239,68,68,0.7); }
  .sys-status.err .sys-label { color: #fca5a5; }

  /* 每次 .pulse 重新挂载, animation 重新播放, 实现"心跳一次闪一下" */
  .sys-status .sys-dot { transition: box-shadow 0.2s; }
  .sys-status.pulse .sys-dot {
    animation: sysDotBeat 0.45s ease-out;
  }
  /* 在 dot 外层再画一圈环, 每次心跳扩出去一次 */
  .sys-status { position: relative; overflow: visible; }
  .sys-status .sys-beat {
    position: absolute;
    left: 10px;       /* 对齐 .sys-dot (padding-left 10px) */
    top: 50%;
    width: 8px; height: 8px;
    border-radius: 50%;
    transform: translate(-50%, -50%);
    pointer-events: none;
    opacity: 0;
  }
  .sys-status.pulse .sys-beat {
    animation: sysBeatRing 0.7s ease-out;
  }
  .sys-status.ok .sys-beat { background: rgba(34, 197, 94, 0.45); }
  .sys-status.warn .sys-beat { background: rgba(245, 158, 11, 0.45); }
  .sys-status.err .sys-beat { background: rgba(239, 68, 68, 0.55); }
  .sys-status.unknown .sys-beat { background: rgba(148, 163, 184, 0.45); }

  @keyframes sysDotBeat {
    0%   { transform: scale(1); filter: brightness(1); }
    35%  { transform: scale(1.6); filter: brightness(1.5); }
    100% { transform: scale(1); filter: brightness(1); }
  }
  @keyframes sysBeatRing {
    0%   { transform: translate(-50%, -50%) scale(1); opacity: 0.6; }
    100% { transform: translate(-50%, -50%) scale(4.5); opacity: 0; }
  }

  /* unknown (未连接) 状态: dot 慢闪, 提示正在尝试 */
  /* disconnected 状态: 红色 + 慢闪 (不依赖 .err 的脉冲, 因为 .err 已是 error 告警) */
  .sys-status.disconnected {
    border-color: rgba(239, 68, 68, 0.45);
    color: #fca5a5;
    background: rgba(239, 68, 68, 0.08);
    animation: sysDiscoBg 1.2s ease-in-out infinite;
  }
  .sys-status.disconnected .sys-dot {
    background: #ef4444;
    box-shadow: 0 0 8px rgba(239, 68, 68, 0.7);
  }
  @keyframes sysDiscoBg {
    0%, 100% { background: rgba(239, 68, 68, 0.04); }
    50%      { background: rgba(239, 68, 68, 0.18); }
  }

  .sys-status.unknown .sys-dot {
    animation: sysDotUnknown 1.4s ease-in-out infinite;
  }
  @keyframes sysDotUnknown {
    0%, 100% { opacity: 0.4; }
    50%      { opacity: 1; }
  }

  /* err (有 error 级告警但连接正常): 暗红色稳定 */
  .sys-status.err {
    border-color: rgba(239, 68, 68, 0.45);
    color: #fca5a5;
    background: rgba(239, 68, 68, 0.05);
  }
  .sys-status.err .sys-dot {
    background: #ef4444;
    box-shadow: 0 0 6px rgba(239, 68, 68, 0.55);
  }

  /* warn (有 warning 级告警但连接正常): 暗黄色 */
  .sys-status.warn {
    border-color: rgba(245, 158, 11, 0.45);
    color: #fcd34d;
  }
  .sys-status.warn .sys-dot {
    background: #f59e0b;
    box-shadow: 0 0 6px rgba(245, 158, 11, 0.55);
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
    min-height: 0;
    overflow: auto;
    width: 100%;
    min-width: 0;
    box-sizing: border-box;
    padding: clamp(14px, 1.2vw, 26px);
    background: var(--bg-primary);
  }

  .route-shell {
    width: 100%;
    min-width: 0;
    min-height: 0;
  }

  .route-shell.dashboard-route {
    height: 100%;
    overflow: hidden;
  }

  .route-loading {
    min-height: min(360px, 70vh);
    display: grid;
    place-items: center;
    align-content: center;
    gap: 10px;
    color: var(--text-tertiary);
    font-size: 12px;
    font-weight: 800;
  }

  .route-loading span {
    width: 34px;
    height: 34px;
    border-radius: 50%;
    border: 2px solid rgba(34, 211, 238, 0.18);
    border-top-color: #22d3ee;
    box-shadow: 0 0 26px rgba(34, 211, 238, 0.18);
    animation: routeSpin 0.8s linear infinite;
  }

  .route-loading em {
    font-style: normal;
  }

  @keyframes routeSpin {
    to { transform: rotate(360deg); }
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
