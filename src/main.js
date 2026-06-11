// ═══════════════════════════════════════════════
// OpenCode Free Proxy - Dashboard
// ═══════════════════════════════════════════════

const { invoke, listen } = window.__TAURI__.core;

// ── State ──────────────────────────────────────
let testingModels = new Set();
let isRefreshing = false;
let resultsCache = {};

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
    showToast('✓ 已复制');
  } catch {
    const ta = document.createElement('textarea');
    ta.value = text;
    document.body.appendChild(ta);
    ta.select();
    document.execCommand('copy');
    ta.remove();
    showToast('✓ 已复制');
  }
}

// ── Refresh Status ─────────────────────────────
async function refreshStatus() {
  if (isRefreshing) return;
  isRefreshing = true;

  const btn = document.getElementById('refreshBtn');
  const dot = document.getElementById('statusDot');
  const text = document.getElementById('statusText');
  if (btn) btn.disabled = true;
  dot.className = 'status-dot';
  text.textContent = '加载中...';

  try {
    const status = await invoke('get_status');
    updateStatusUI(status);
    await loadPool();
  } catch (e) {
    dot.className = 'status-dot offline';
    text.textContent = '连接失败: ' + e;
  } finally {
    isRefreshing = false;
    if (btn) btn.disabled = false;
  }
}

function updateStatusUI(status) {
  const dot = document.getElementById('statusDot');
  const text = document.getElementById('statusText');
  if (status.running) {
    dot.className = 'status-dot online';
    text.textContent = '运行中 · 端口 ' + status.port;
  } else {
    dot.className = 'status-dot offline';
    text.textContent = '已停止';
  }

  document.getElementById('keyCount').textContent = status.keys.length;
  document.getElementById('keyList').innerHTML = status.keys.map(k => `
    <div class="flex items-center justify-between px-3 py-2.5 rounded-md bg-surface2 border border-border">
      <div class="flex flex-col gap-0.5 min-w-0">
        <span class="text-[11px] font-semibold text-muted uppercase">${escapeHtml(k.name)}</span>
        <span class="text-sm text-white font-mono break-all">${escapeHtml(k.key)}</span>
      </div>
      <button onclick="copyText('${escapeHtml(k.key)}')" class="flex-shrink-0 text-base p-1 rounded hover:bg-white/10 transition-colors cursor-pointer" title="复制">📋</button>
    </div>
  `).join('');
}

// ── Pool Mode Toggle ──────────────────────────
async function togglePoolMode() {
  const input = document.getElementById('poolModeInput');
  const newMode = input.checked;
  try {
    await invoke('set_pool_mode', { req: { pool_mode: newMode } });
    updatePoolToggleUI(newMode);
    showToast(newMode ? '🔀 号池模式已开启' : '🔀 号池模式已关闭');
  } catch (e) {
    input.checked = !newMode;
    showToast('❌ ' + e);
  }
}

function updatePoolToggleUI(enabled) {
  const toggle = document.getElementById('poolToggle');
  const dot = document.getElementById('poolToggleDot');
  const label = document.getElementById('poolModeLabel');
  if (enabled) {
    toggle.classList.remove('bg-[#2a2d3e]');
    toggle.classList.add('bg-[#6c8cff]');
    dot.classList.remove('bg-muted', 'left-0.5');
    dot.classList.add('bg-white', 'left-[18px]');
    label.textContent = '号池开';
  } else {
    toggle.classList.add('bg-[#2a2d3e]');
    toggle.classList.remove('bg-[#6c8cff]');
    dot.classList.add('bg-muted', 'left-0.5');
    dot.classList.remove('bg-white', 'left-[18px]');
    label.textContent = '号池关';
  }
}

// ── Add Provider Dialog ──────────────────────
function showAddDialog() {
  document.getElementById('addDialog').classList.remove('hidden');
  document.getElementById('dlgName').value = '';
  document.getElementById('dlgModel').value = '';
  document.getElementById('dlgUrl').value = '';
  document.getElementById('dlgKey').value = '';
  setTimeout(() => document.getElementById('dlgName').focus(), 100);
}

function hideAddDialog() {
  document.getElementById('addDialog').classList.add('hidden');
}

async function confirmAddProvider() {
  const name = document.getElementById('dlgName').value.trim();
  const model_name = document.getElementById('dlgModel').value.trim() || name;
  const base_url = document.getElementById('dlgUrl').value.trim();
  const api_key = document.getElementById('dlgKey').value.trim();
  if (!name) { showToast('请输入名称'); return; }

  try {
    await invoke('upsert_pool_entry', {
      req: {
        id: null, name, base_url, api_key, model_name,
        priority: 999, enabled: true, builtin: false,
        provider_type: base_url ? 'custom' : 'opencode',
      }
    });
    hideAddDialog();
    showToast('✓ 已添加: ' + name);
    await loadPool();
  } catch (e) { showToast('❌ ' + e); }
}

// ── Model Pool ─────────────────────────────────
async function loadPool() {
  try {
    const pool = await invoke('get_model_pool');
    document.getElementById('modelCount').textContent = pool.entries.length;
    document.getElementById('poolModeInput').checked = pool.pool_mode;
    updatePoolToggleUI(pool.pool_mode);
    renderPool(pool.entries);
    return pool.entries;
  } catch (e) {
    document.getElementById('modelList').innerHTML =
      `<div class="text-center py-5 text-muted text-sm">加载失败: ${e}</div>`;
    return null;
  }
}

function renderPool(entries) {
  const container = document.getElementById('modelList');
  if (!entries || !entries.length) {
    container.innerHTML = `<div class="text-center py-5 text-muted text-sm">暂无模型，点「+ 添加」添加</div>`;
    return;
  }

  const sorted = [...entries].sort((a, b) => a.priority - b.priority);

  container.innerHTML = sorted.map(e => {
    const isOpenCode = e.provider_type === 'opencode';
    return `
    <div class="flex items-center justify-between px-3 py-2.5 rounded-md bg-surface2 border border-border ${!e.enabled ? 'opacity-50' : ''}">
      <div class="flex items-center gap-2 min-w-0">
        <button onclick="toggleEntry('${escapeHtml(e.id)}')" class="flex-shrink-0 w-7 h-4 rounded-full transition-colors ${e.enabled ? 'bg-[#6c8cff]' : 'bg-[#2a2d3e]'} relative cursor-pointer">
          <div class="absolute top-0.5 w-3 h-3 rounded-full bg-white transition-all ${e.enabled ? 'left-[14px]' : 'left-0.5'}"></div>
        </button>
        <div class="flex flex-col gap-0.5 min-w-0">
          <div class="flex items-center gap-1.5">
            <span class="text-sm font-medium text-white">${escapeHtml(e.name)}</span>
            ${isOpenCode
              ? `<span class="text-[10px] px-1.5 py-0.5 rounded bg-[#6c8cff] text-white font-semibold">免费</span>`
              : `<span class="text-[10px] px-1.5 py-0.5 rounded bg-[#fb923c] text-white font-semibold">自定义</span>`}
            <span class="text-[10px] text-muted">#${e.priority}</span>
          </div>
          <div class="model-results flex items-center gap-3 text-xs text-muted" id="results-${escapeHtml(e.name)}">
            ${resultsCache[e.name] ? '' : '<span>等待测速</span>'}
          </div>
        </div>
      </div>
      <div class="flex items-center gap-1 flex-shrink-0">
        <div class="relative" id="import-menu-${escapeHtml(e.name)}" data-model-name="${escapeHtml(e.model_name || e.name)}">
          <button onclick="toggleImportMenu('${escapeHtml(e.name)}')" class="px-2 py-1 rounded text-xs text-white bg-[#2a2d3e] hover:bg-[#3a3d4e] transition-all cursor-pointer">导入</button>
          <div id="import-dropdown-${escapeHtml(e.name)}" class="hidden absolute right-0 top-full mt-1 z-10 bg-surface2 border border-border rounded-lg shadow-xl py-1 min-w-[120px]">
            <button onclick="importModel('${escapeHtml(e.name)}', 'claude')" class="block w-full text-left px-3 py-1.5 text-xs text-white hover:bg-white/10 cursor-pointer">🤖 Claude</button>
            <button onclick="importModel('${escapeHtml(e.name)}', 'codex')" class="block w-full text-left px-3 py-1.5 text-xs text-white hover:bg-white/10 cursor-pointer">△ Codex</button>
            <button onclick="importModel('${escapeHtml(e.name)}', 'ccswitch')" class="block w-full text-left px-3 py-1.5 text-xs text-white hover:bg-white/10 cursor-pointer">🔄 CCSwitch</button>
          </div>
        </div>
        ${!isOpenCode
          ? `<button onclick="removeProvider('${escapeHtml(e.id)}')" class="flex-shrink-0 px-2 py-1 rounded text-xs text-red-400 bg-red-400/10 hover:bg-red-400/20 cursor-pointer">✕</button>`
          : ''}
      </div>
    </div>`;
  }).join('');

  for (const [name, result] of Object.entries(resultsCache)) {
    displayTestResult(name, result);
  }
}

// ── Pool Management ───────────────────────────
async function toggleEntry(id) {
  try { await invoke('toggle_pool_entry', { id }); await loadPool(); }
  catch (e) { showToast('❌ ' + e); }
}

async function removeProvider(id) {
  try { await invoke('remove_pool_entry', { id }); showToast('已移除'); await loadPool(); }
  catch (e) { showToast('❌ ' + e); }
}

// ── Batch Speed Test ──────────────────────────
async function batchTestAll() {
  if (isRefreshing) return;
  isRefreshing = true;

  const btn = document.getElementById('batchTestBtn');
  if (btn) btn.disabled = true;

  try {
    resultsCache = {};
    const entries = await loadPool();
    if (entries && entries.length > 0) {
      showToast(`⏳ 测速 ${entries.length} 个模型...`);
      for (const e of entries) await testModel(e.name);
      showToast('✅ 测速完成');
    } else { showToast('暂无模型'); }
  } catch (e) { showToast('❌ ' + e); }
  finally {
    isRefreshing = false;
    if (btn) btn.disabled = false;
  }
}

async function testModel(name) {
  if (testingModels.has(name)) return;
  testingModels.add(name);
  const el = document.getElementById('results-' + name);
  if (el) el.innerHTML = '<span class="text-xs text-muted">⏳ 测试中...</span>';
  try {
    const result = await invoke('run_speed_test_cmd', { req: { model: name } });
    displayTestResult(name, result);
  } catch (e) {
    const el = document.getElementById('results-' + name);
    if (el) el.innerHTML = `<span class="text-xs text-red-400">❌ ${escapeHtml(String(e))}</span>`;
  } finally { testingModels.delete(name); }
}

function displayTestResult(name, result) {
  resultsCache[name] = result;
  const el = document.getElementById('results-' + name);
  if (!el) return;
  if (result.success) {
    el.innerHTML = `
      <span>⏱ <strong class="text-white">${result.latency_ms}ms</strong></span>
      <span>⚡ <strong class="text-white">${result.tokens_per_sec.toFixed(1)}</strong> tok/s</span>
      <span class="hidden sm:inline">📝 ${result.total_tokens} tok</span>`;
  } else {
    el.innerHTML = `<span class="text-red-400">❌ ${escapeHtml(result.error || '失败')}</span>`;
  }
}

try { listen('speed-test-complete', (e) => displayTestResult(e.payload.model, e.payload)); } catch {}

// ── Import to Tool ──────────────────────────
function toggleImportMenu(name) {
  document.querySelectorAll('[id^="import-dropdown-"]').forEach(el => el.classList.add('hidden'));
  const d = document.getElementById('import-dropdown-' + name);
  if (d) d.classList.toggle('hidden');
}
document.addEventListener('click', (e) => {
  if (!e.target.closest('[id^="import-menu-"]'))
    document.querySelectorAll('[id^="import-dropdown-"]').forEach(el => el.classList.add('hidden'));
});
async function importModel(name, tool) {
  document.querySelectorAll('[id^="import-dropdown-"]').forEach(el => el.classList.add('hidden'));
  try {
    const status = await invoke('get_status');
    const key = status.keys[0]?.key;
    if (!key) { showToast('❌ 无可用 Key'); return; }

    const menu = document.getElementById('import-menu-' + name);
    const modelName = menu ? menu.dataset.modelName || name : name;
    const r = await invoke('import_to_tool', { req: { model: name, model_name: modelName, api_key: key, tool } });
    showToast(r);
  } catch (e) { showToast('❌ ' + e); }
}

// ── Utilities ──────────────────────────────────
function escapeHtml(str) {
  if (!str) return '';
  return str.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}
