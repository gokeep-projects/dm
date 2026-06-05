<script>
  import { onMount } from 'svelte';

  let config = $state({
    port: 3399,
    bind: '0.0.0.0',
    allow_remote: true,
    log_dir: '',
    data_dir: '',
    scripts_dir: '',
    theme: 'dark',
    language: 'zh',
    about: {
      version: '0.1.0',
      author: 'xuning',
      email: 'gokeeps@qq.com',
      license: 'MIT',
    },
  });
  let loading = $state(true);
  let saving = $state(false);
  let saveMessage = $state('');
  let saveError = $state('');

  async function load() {
    loading = true;
    try {
      const r = await fetch('/api/config');
      if (r.ok) {
        const d = await r.json();
        config = { ...config, ...d, allow_remote: d.allow_remote ?? d.bind !== '127.0.0.1' };
      }
    } catch (e) { console.warn('加载配置失败:', e); }
    loading = false;
  }

  async function save() {
    saving = true;
    saveMessage = '';
    saveError = '';
    try {
      const r = await fetch('/api/config', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ ...config, bind: config.allow_remote ? '0.0.0.0' : '127.0.0.1' }),
      });
      if (r.ok) {
        saveMessage = '配置已保存';
        setTimeout(() => saveMessage = '', 3000);
      } else {
        saveError = '保存失败';
      }
    } catch (e) { saveError = e.message; }
    saving = false;
  }

  function resetToDefault() {
    if (!confirm('确定恢复默认配置？')) return;
    config = {
      port: 3399,
      bind: '0.0.0.0',
      allow_remote: true,
      log_dir: '',
      data_dir: '',
      scripts_dir: '',
      theme: 'dark',
      language: 'zh',
      about: config.about,
    };
  }

  onMount(load);
</script>

<div class="settings-page">
  <div class="page-header">
    <div class="header-left">
      <span class="settings-desc">系统配置与偏好设置</span>
    </div>
    <div class="header-right">
      {#if saveMessage}
        <span class="save-msg success">{saveMessage}</span>
      {/if}
      {#if saveError}
        <span class="save-msg error">{saveError}</span>
      {/if}
      <button class="reset-btn" onclick={resetToDefault}>恢复默认</button>
      <button class="save-btn" onclick={save} disabled={saving}>
        {saving ? '保存中...' : '保存配置'}
      </button>
    </div>
  </div>

  {#if loading}
    <div class="loading">
      <div class="loading-spinner"></div>
      <span>加载中...</span>
    </div>
  {:else}
    <div class="settings-grid">
      <div class="settings-section">
        <h3>🌐 服务配置</h3>
        <div class="setting-rows">
          <div class="setting-row">
            <div class="setting-info">
              <span class="setting-label">监听端口</span>
              <span class="setting-desc">Web 服务监听端口号</span>
            </div>
            <input type="number" bind:value={config.port} min="1" max="65535" class="setting-input" />
          </div>
          <div class="setting-row">
            <div class="setting-info">
              <span class="setting-label">远程访问</span>
              <span class="setting-desc">{config.allow_remote ? '监听 0.0.0.0，允许局域网访问' : '仅监听 127.0.0.1，只允许本机访问'}</span>
            </div>
            <div class="segmented">
              <button class:active={config.allow_remote} onclick={() => { config.allow_remote = true; config.bind = '0.0.0.0'; }}>允许远程</button>
              <button class:active={!config.allow_remote} onclick={() => { config.allow_remote = false; config.bind = '127.0.0.1'; }}>仅本机</button>
            </div>
          </div>
        </div>
      </div>

      <div class="settings-section">
        <h3>📁 目录配置</h3>
        <div class="setting-rows">
          <div class="setting-row">
            <div class="setting-info">
              <span class="setting-label">日志目录</span>
              <span class="setting-desc">日志文件存储路径</span>
            </div>
            <input type="text" bind:value={config.log_dir} class="setting-input" placeholder="~/.dm/logs" />
          </div>
          <div class="setting-row">
            <div class="setting-info">
              <span class="setting-label">数据目录</span>
              <span class="setting-desc">数据库、规则覆盖和持久化趋势数据目录，保存后自动迁移已有数据</span>
            </div>
            <input type="text" bind:value={config.data_dir} class="setting-input" placeholder="~/.dm/data" />
          </div>
          <div class="setting-row">
            <div class="setting-info">
              <span class="setting-label">脚本目录</span>
              <span class="setting-desc">脚本文件存储路径</span>
            </div>
            <input type="text" bind:value={config.scripts_dir} class="setting-input" placeholder="~/.dm/scripts" />
          </div>
        </div>
      </div>

      <div class="settings-section">
        <h3>🎨 界面设置</h3>
        <div class="setting-rows">
          <div class="setting-row">
            <div class="setting-info">
              <span class="setting-label">主题</span>
              <span class="setting-desc">界面主题颜色</span>
            </div>
            <select bind:value={config.theme} class="setting-select">
              <option value="light">明亮</option>
              <option value="dark">暗黑</option>
            </select>
          </div>
          <div class="setting-row">
            <div class="setting-info">
              <span class="setting-label">语言</span>
              <span class="setting-desc">界面显示语言</span>
            </div>
            <select bind:value={config.language} class="setting-select">
              <option value="zh">中文</option>
              <option value="en">English</option>
            </select>
          </div>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .settings-page { max-width: 1000px; margin: 0 auto; }
  .page-header { display: flex; align-items: center; justify-content: space-between; gap: 16px; margin-bottom: 24px; }
  .header-left { display: flex; align-items: baseline; gap: 10px; }
  .settings-desc { font-size: 14px; color: var(--text-secondary); }
  .header-right { display: flex; align-items: center; gap: 10px; }
  .save-msg { font-size: 12px; padding: 4px 10px; border-radius: 6px; }
  .save-msg.success { background: rgba(16, 185, 129, 0.1); color: #10b981; }
  .save-msg.error { background: rgba(239, 68, 68, 0.1); color: #ef4444; }
  .reset-btn { padding: 8px 14px; background: var(--bg-secondary); border: 1px solid var(--border-primary); border-radius: 8px; color: var(--text-secondary); font-size: 13px; cursor: pointer; }
  .reset-btn:hover { background: var(--bg-hover); }
  .save-btn { padding: 8px 16px; background: var(--accent-primary); color: white; border: none; border-radius: 8px; font-size: 13px; font-weight: 600; cursor: pointer; }
  .save-btn:hover { background: var(--accent-primary-hover); }
  .save-btn:disabled { opacity: 0.5; cursor: not-allowed; }

  .settings-grid { display: flex; flex-direction: column; gap: 20px; }
  .settings-section { background: var(--bg-card); border: 1px solid var(--border-primary); border-radius: 14px; padding: 20px; }
  .settings-section h3 { font-size: 15px; font-weight: 700; color: var(--text-primary); margin: 0 0 16px; }
  .setting-rows { display: flex; flex-direction: column; gap: 14px; }
  .setting-row { display: flex; align-items: center; justify-content: space-between; gap: 20px; }
  .setting-info { flex: 1; }
  .setting-label { display: block; font-size: 14px; font-weight: 600; color: var(--text-primary); margin-bottom: 2px; }
  .setting-desc { font-size: 12px; color: var(--text-tertiary); }
  .setting-input { width: 200px; padding: 8px 12px; background: var(--bg-input); border: 1px solid var(--border-primary); border-radius: 8px; font-size: 13px; color: var(--text-primary); outline: none; }
  .setting-input:focus { border-color: var(--border-focus); }
  .setting-select { width: 200px; padding: 8px 12px; background: var(--bg-input); border: 1px solid var(--border-primary); border-radius: 8px; font-size: 13px; color: var(--text-primary); outline: none; cursor: pointer; }
  .segmented { display: inline-flex; gap: 4px; padding: 4px; background: var(--bg-secondary); border: 1px solid var(--border-primary); border-radius: 8px; }
  .segmented button { min-height: 30px; padding: 0 12px; border: none; border-radius: 6px; background: transparent; color: var(--text-secondary); font-size: 12px; cursor: pointer; }
  .segmented button.active { background: var(--accent-primary); color: #fff; }

  .loading { display: flex; align-items: center; justify-content: center; gap: 10px; padding: 60px 0; color: var(--text-secondary); font-size: 14px; }
  .loading-spinner { width: 24px; height: 24px; border: 3px solid var(--border-primary); border-top-color: var(--accent-primary); border-radius: 50%; animation: spin 0.8s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }

  @media (max-width: 768px) {
    .page-header { flex-direction: column; align-items: stretch; }
    .header-right { flex-wrap: wrap; }
    .setting-row { flex-direction: column; align-items: stretch; gap: 8px; }
    .setting-input, .setting-select { width: 100%; }
  }
</style>
