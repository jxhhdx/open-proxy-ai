// ═══════════════════════════════════════════════
// OpenCode Free Proxy - Dashboard
// ═══════════════════════════════════════════════

const { invoke, listen } = window.__TAURI__.core;

// ── State ──────────────────────────────────────
let testingModels = new Set();
let isRefreshing = false;

// ── Init ───────────────────────────────────────
document.addEventListener('DOMContentLoaded', () => {
  refreshStatus();
});

// ── Toast ──────────────────────────────────────
function showToast(msg) {
  let el = document.getElementById('toast');
  if (!el) return;
  el.textContent = msg;
  el.classList.remove('opacity-0');
  el.classList.add('opacity-100');
  setTimeout(() => {
    el.classList.remove('opacity-100');
    el.classList.add('opacity-0');
  }, 2000);
}

async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
    showToast('✓ 已复制到剪贴板');
  } catch {
    const ta = document.createElement('textarea');
    ta.value = text;
    document.body.appendChild(ta);
    ta.select();
    document.execCommand('copy');
    ta.remove();
    showToast('✓ 已复制到剪贴板');
  }
}

// ── Refresh Status ─────────────────────────────
async function refreshStatus() {
  if (isRefreshing) return;
  isRefreshing = true;

  const refreshBtn = document.getElementById('refreshBtn');
  const statusDot = document.getElementById('statusDot');
  const statusText = document.getElementById('statusText');

  if (refreshBtn) refreshBtn.disabled = true;
  statusDot.className = 'status-dot';
  statusText.textContent = '加载中...';

  try {
    const status = await invoke('get_status');
    updateStatusUI(status);
    await loadModels();
  } catch (e) {
    statusDot.className = 'status-dot offline';
    statusText.textContent = '连接失败: ' + e;
  } finally {
    isRefreshing = false;
    if (refreshBtn) refreshBtn.disabled = false;
  }
}

function updateStatusUI(status) {
  const dot = document.getElementById('statusDot');
  const text = document.getElementById('statusText');
  const keyCount = document.getElementById('keyCount');

  if (status.running) {
    dot.className = 'status-dot online';
    text.textContent = '运行中 · 端口 ' + status.port;
  } else {
    dot.className = 'status-dot offline';
    text.textContent = '已停止';
  }

  keyCount.textContent = status.keys.length;
  const keyList = document.getElementById('keyList');
  keyList.innerHTML = status.keys.map(k => `
    <div class="flex items-center justify-between px-3 py-2.5 rounded-md bg-surface2 border border-border">
      <div class="flex flex-col gap-0.5 min-w-0">
        <span class="text-[11px] font-semibold text-muted uppercase tracking-wide">${escapeHtml(k.name)}</span>
        <span class="text-sm text-white font-mono break-all">${escapeHtml(k.key)}</span>
      </div>
      <button onclick="copyText('${escapeHtml(k.key)}')" class="flex-shrink-0 text-base p-1 rounded hover:bg-white/10 transition-colors cursor-pointer" title="复制 Key">📋</button>
    </div>
  `).join('');
}

// ── Models ─────────────────────────────────────
async function loadModels() {
  try {
    const models = await invoke('get_models');
    document.getElementById('modelCount').textContent = models.length;
    renderModels(models);
    return models;
  } catch (e) {
    document.getElementById('modelList').innerHTML =
      `<div class="text-center py-5 text-muted text-sm">加载失败: ${e}</div>`;
    return null;
  }
}

function renderModels(models) {
  const container = document.getElementById('modelList');
  if (!models.length) {
    container.innerHTML = `<div class="text-center py-5 text-muted text-sm">暂无模型</div>`;
    return;
  }

  container.innerHTML = models.map(m => `
    <div class="flex items-center justify-between px-3 py-2.5 rounded-md bg-surface2 border border-border">
      <div class="flex flex-col gap-0.5 min-w-0">
        <div class="flex items-center gap-1.5">
          <span class="text-sm font-medium text-white">${escapeHtml(m.id)}</span>
          ${m.builtin
            ? `<span class="text-[10px] px-1.5 py-0.5 rounded bg-[#6c8cff] text-white font-semibold leading-none">内置</span>`
            : `<span class="text-[10px] px-1.5 py-0.5 rounded bg-[#fb923c] text-white font-semibold leading-none">自定义</span>`}
        </div>
        <div class="model-results flex items-center gap-3 text-xs text-muted" id="results-${escapeHtml(m.id)}">
          <span>等待测速</span>
        </div>
      </div>
      ${!m.builtin
        ? `<button onclick="removeCustomModel('${escapeHtml(m.id)}')" class="flex-shrink-0 px-2 py-1 rounded text-xs text-red-400 bg-red-400/10 hover:bg-red-400/20 transition-all cursor-pointer">✕</button>`
        : ''}
    </div>
  `).join('');
}

// ── Batch Speed Test ──────────────────────────
async function batchTestAll() {
  if (isRefreshing) return;
  isRefreshing = true;

  const btn = document.getElementById('batchTestBtn');
  if (btn) btn.disabled = true;

  try {
    const models = await loadModels();
    if (models && models.length > 0) {
      showToast(`⏳ 一键测速 ${models.length} 个模型...`);
      for (const m of models) {
        await testModel(m.id);
      }
      showToast('✅ 测速完成');
    } else {
      showToast('暂无模型可测速');
    }
  } catch (e) {
    showToast('❌ 测速出错: ' + e);
  } finally {
    isRefreshing = false;
    if (btn) btn.disabled = false;
  }
}

async function testModel(modelId) {
  if (testingModels.has(modelId)) return;

  testingModels.add(modelId);
  const results = document.getElementById('results-' + modelId);
  if (results) results.innerHTML = `<span class="text-xs text-muted">⏳ 测试中...</span>`;

  try {
    const result = await invoke('run_speed_test_cmd', { req: { model: modelId } });
    displayTestResult(modelId, result);
  } catch (e) {
    const results = document.getElementById('results-' + modelId);
    if (results) {
      results.innerHTML = `<span class="text-xs text-red-400">❌ ${escapeHtml(String(e))}</span>`;
    }
  } finally {
    testingModels.delete(modelId);
  }
}

function displayTestResult(modelId, result) {
  const results = document.getElementById('results-' + modelId);
  if (!results) return;

  if (result.success) {
    const tokensPerSec = result.tokens_per_sec.toFixed(1);
    const latency = result.latency_ms;
    const preview = escapeHtml(result.response_preview.substring(0, 30));
    results.innerHTML = `
      <span>⏱ <strong class="text-white">${latency}ms</strong></span>
      <span>⚡ <strong class="text-white">${tokensPerSec}</strong> tok/s</span>
      <span class="hidden sm:inline">📝 <strong class="text-white">${result.total_tokens}</strong> tokens</span>
      <span class="hidden sm:inline" title="${preview}">"${preview}..."</span>
    `;
  } else {
    results.innerHTML = `<span class="text-red-400">❌ ${escapeHtml(result.error || '测试失败')}</span>`;
  }
}

// Listen for speed test completion (from Rust streaming)
try {
  listen('speed-test-complete', (event) => {
    const r = event.payload;
    displayTestResult(r.model, r);
  });
} catch(e) { /* Tauri API not available in browser */ }

// ── Custom Models ──────────────────────────────
async function addCustomModel() {
  const input = document.getElementById('customModelInput');
  const name = input.value.trim();
  if (!name) { showToast('请输入模型名称'); return; }

  try {
    const updated = await invoke('add_custom_model', { req: { name } });
    input.value = '';
    showToast('✓ 已添加模型: ' + name);
    await loadModels();
  } catch (e) {
    showToast('❌ ' + e);
  }
}

async function removeCustomModel(name) {
  try {
    await invoke('remove_custom_model', { req: { name } });
    showToast('已移除: ' + name);
    await loadModels();
  } catch (e) {
    showToast('❌ ' + e);
  }
}

// ── Utilities ──────────────────────────────────
function escapeHtml(str) {
  if (!str) return '';
  return String(str)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}
