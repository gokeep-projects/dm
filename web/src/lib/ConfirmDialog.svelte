<script>
  let {
    open = false,
    title = '确认操作',
    message = '',
    detail = '',
    confirmText = '确认',
    cancelText = '取消',
    tone = 'danger',
    loading = false,
    onConfirm = () => {},
    onCancel = () => {},
  } = $props();
</script>

{#if open}
  <div class="confirm-backdrop" onclick={onCancel} role="presentation">
    <div class="confirm-card {tone}" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()} role="dialog" aria-modal="true" tabindex="-1">
      <div class="confirm-orb"></div>
      <div class="confirm-head">
        <span class="confirm-kicker">{tone === 'danger' ? 'DANGER ZONE' : 'CONFIRM'}</span>
        <h3>{title}</h3>
      </div>
      <p>{message}</p>
      {#if detail}
        <pre>{detail}</pre>
      {/if}
      <div class="confirm-actions">
        <button class="cancel" onclick={onCancel} disabled={loading}>{cancelText}</button>
        <button class="confirm" onclick={onConfirm} disabled={loading}>
          {loading ? '处理中...' : confirmText}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .confirm-backdrop {
    position: fixed;
    inset: 0;
    z-index: 300;
    display: grid;
    place-items: center;
    padding: 18px;
    background:
      radial-gradient(circle at 18% 12%, rgba(248, 113, 113, .24), transparent 30%),
      radial-gradient(circle at 80% 0%, rgba(34, 211, 238, .16), transparent 34%),
      rgba(0, 0, 0, .68);
    backdrop-filter: blur(10px);
  }
  .confirm-card {
    position: relative;
    width: min(460px, calc(100vw - 36px));
    overflow: hidden;
    padding: 20px;
    border-radius: 18px;
    border: 1px solid rgba(248, 113, 113, .34);
    background: linear-gradient(145deg, rgba(15, 23, 42, .98), rgba(27, 36, 54, .96));
    box-shadow: 0 28px 88px rgba(0, 0, 0, .45), inset 0 0 40px rgba(248, 113, 113, .05);
  }
  .confirm-card.safe {
    border-color: rgba(34, 211, 238, .32);
    box-shadow: 0 28px 88px rgba(0, 0, 0, .45), inset 0 0 40px rgba(34, 211, 238, .05);
  }
  .confirm-orb {
    position: absolute;
    right: -58px;
    top: -70px;
    width: 180px;
    height: 180px;
    border-radius: 50%;
    background: radial-gradient(circle, rgba(248, 113, 113, .32), transparent 68%);
    filter: blur(2px);
    pointer-events: none;
  }
  .confirm-head { position: relative; margin-bottom: 10px; }
  .confirm-kicker {
    display: inline-flex;
    margin-bottom: 7px;
    color: #fb7185;
    font-size: 10px;
    font-weight: 900;
    letter-spacing: .14em;
  }
  .confirm-card.safe .confirm-kicker { color: #67e8f9; }
  h3 { margin: 0; color: #f8fafc; font-size: 19px; letter-spacing: -.03em; }
  p { margin: 0 0 12px; color: #cbd5e1; line-height: 1.65; font-size: 13px; }
  pre {
    max-height: 140px;
    overflow: auto;
    margin: 0 0 14px;
    padding: 10px;
    border-radius: 10px;
    border: 1px solid rgba(148, 163, 184, .14);
    background: rgba(2, 6, 23, .62);
    color: #e2e8f0;
    font-size: 12px;
    white-space: pre-wrap;
    word-break: break-word;
  }
  .confirm-actions { display: flex; justify-content: flex-end; gap: 9px; }
  button {
    min-height: 34px;
    padding: 0 14px;
    border-radius: 10px;
    font-size: 13px;
    font-weight: 900;
    cursor: pointer;
  }
  button:disabled { opacity: .55; cursor: not-allowed; }
  .cancel {
    border: 1px solid rgba(148, 163, 184, .18);
    background: rgba(15, 23, 42, .7);
    color: #cbd5e1;
  }
  .confirm {
    border: 0;
    background: linear-gradient(135deg, #fb7185, #f97316);
    color: #fff7ed;
    box-shadow: 0 12px 28px rgba(248, 113, 113, .22);
  }
  .safe .confirm {
    background: linear-gradient(135deg, #22d3ee, #34d399);
    color: #042f2e;
    box-shadow: 0 12px 28px rgba(34, 211, 238, .18);
  }
</style>
