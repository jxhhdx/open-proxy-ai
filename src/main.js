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
  if (!el) {
    el = document.createElement('div');
    el.id = 'toast';
    el.className = 'toast';
    document.body.appendChild(el);
  }
  el.textContent = msg;
  el.classList.add('visible');
  setTimeout(() => el.classList.remove('visible'), 2000);
}

async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
    showToast('✓ 已复制到剪贴板');
  } catch {
    // Fallback
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
  statusDot.className = 'status-dot loading';
  statusText.textContent = '加载中...';

  try {
    const status = await invoke('get_status');
    updateStatusUI(status);
    const models = await loadModels();
    // 自动批量测速所有模型
    if (models && models.length > 0) {
      showToast(`⏳ 正在批量测速 ${models.length} 个模型...`);
      await batchTestAllModels(models);
      showToast('✅ 批量测速完成');
    }
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

  // Keys
  keyCount.textContent = status.keys.length;
  const keyList = document.getElementById('keyList');
  keyList.innerHTML = status.keys.map(k => `
    <div class="key-item">
      <div class="key-info">
        <span class="key-name">${escapeHtml(k.name)}</span>
        <span class="key-value">${escapeHtml(k.key)}</span>
      </div>
      <div class="key-actions">
        <button class="btn-ico" onclick="copyText('${escapeHtml(k.key)}')" title="复制 Key">📋</button>
      </div>
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
      `<div class="model-empty">加载失败: ${e}</div>`;
    return null;
  }
}

// ── Batch Speed Test ──────────────────────────
async function batchTestAllModels(models) {
  for (const m of models) {
    await testModel(m.id);
  }
}

function renderModels(models) {
  const container = document.getElementById('modelList');
  if (!models.length) {
    container.innerHTML = `<div class="model-empty">暂无模型</div>`;
    return;
  }

  container.innerHTML = models.map(m => `
    <div class="model-item" id="model-${escapeHtml(m.id)}">
      <div class="model-left">
        <div class="model-name">
          ${escapeHtml(m.id)}
          ${m.builtin
            ? `<span class="tag">内置</span>`
            : `<span class="tag custom">自定义</span>`}
        </div>
        <div class="model-results" id="results-${escapeHtml(m.id)}">
          <span class="model-metric">点击右侧按钮测速</span>
        </div>
      </div>
      <div style="display:flex;gap:4px;">
        <button class="btn btn-primary" onclick="testModel('${escapeHtml(m.id)}')" id="testbtn-${escapeHtml(m.id)}">
          ⚡ 测速
        </button>
        ${!m.builtin ? `<button class="btn btn-danger" onclick="removeCustomModel('${escapeHtml(m.id)}')">✕</button>` : ''}
      </div>
    </div>
  `).join('');
}

// ── Speed Test ─────────────────────────────────
async function testModel(modelId) {
  if (testingModels.has(modelId)) return;

  testingModels.add(modelId);
  const item = document.getElementById('model-' + modelId);
  const btn = document.getElementById('testbtn-' + modelId);
  const results = document.getElementById('results-' + modelId);
  if (item) item.classList.add('testing');
  if (btn) btn.disabled = true;
  if (results) results.innerHTML = `<span class="model-metric">⏳ 测试中...</span>`;

  try {
    const result = await invoke('run_speed_test_cmd', { req: { model: modelId } });
    displayTestResult(modelId, result);
  } catch (e) {
    const results = document.getElementById('results-' + modelId);
    if (results) {
      results.innerHTML = `<span class="model-error">${escapeHtml(String(e))}</span>`;
    }
  } finally {
    testingModels.delete(modelId);
    if (item) item.classList.remove('testing');
    if (btn) { btn.disabled = false; }
  }
}

function displayTestResult(modelId, result) {
  const results = document.getElementById('results-' + modelId);
  if (!results) return;

  if (result.success) {
    const tokensPerSec = result.tokens_per_sec.toFixed(1);
    const latency = result.latency_ms;
    const preview = escapeHtml(result.response_preview);
    results.innerHTML = `
      <span class="model-metric">⏱ <strong>${latency}ms</strong> 延迟</span>
      <span class="model-metric">⚡ <strong>${tokensPerSec}</strong> tok/s</span>
      <span class="model-metric">📝 <strong>${result.total_tokens}</strong> tokens</span>
      <span class="model-metric" title="${preview}">"${preview.substring(0, 30)}..."</span>
    `;
  } else {
    results.innerHTML = `<span class="model-error">❌ ${escapeHtml(result.error || '测试失败')}</span>`;
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
