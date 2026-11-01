// agx Desktop GUI Controller (Tauri v2)

const invoke = (window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke)
  ? window.__TAURI__.core.invoke
  : async (cmd, args) => {
      throw new Error('Open the desktop app to access your workspace.');
    };

// State
let accounts = [];
let activeAccountName = null;
let cleanerCategories = [];

// DOM Elements
const navTabs = document.querySelectorAll('.nav-tab');
const tabContents = document.querySelectorAll('.tab-content');
const toastContainer = document.getElementById('toast-container');

// Initialize
document.addEventListener('DOMContentLoaded', () => {
  const connected = Boolean(window.__TAURI__?.core?.invoke);
  document.getElementById('connection-status').textContent = connected ? 'Desktop workspace' : 'Browser preview';
  setupTabs();
  setupModals();
  setupEventHandlers();
  setupUpdater();

  // Load initial data
  loadAccountsData();
  startTokenRefresh();

});

// GitHub checks and downloads run outside the window thread.
let updaterState = 'check';
function setupUpdater() {
  const button = document.getElementById('btn-check-update');
  button.addEventListener('click', () => runUpdater(false));
  if (window.__TAURI__?.core?.invoke) setTimeout(() => runUpdater(true), 3000);
}
async function runUpdater(automatic = false) {
  const button = document.getElementById('btn-check-update');
  const status = document.getElementById('update-status');
  if (button.disabled) return;
  button.disabled = true;
  try {
    if (updaterState === 'restart') {
      await invoke('restart_after_update');
      return;
    }
    if (updaterState === 'install') {
      button.textContent = 'Downloading…';
      status.textContent = 'Downloading and verifying update';
      const version = await invoke('install_update');
      updaterState = 'restart';
      button.textContent = 'Restart agx';
      status.textContent = `${version} installed`;
      return;
    }
    button.textContent = 'Checking…';
    const info = await invoke('check_update');
    document.getElementById('app-version').textContent = `v${info.current_version}`;
    status.textContent = info.message;
    updaterState = info.installed ? 'restart' : info.installable ? 'install' : 'check';
    button.textContent = updaterState === 'restart' ? 'Restart agx' : updaterState === 'install' ? `Update to ${info.latest_version}` : 'Check for updates';
  } catch (error) {
    status.textContent = automatic ? 'Could not check for updates' : String(error);
    button.textContent = updaterState === 'install' ? 'Retry update' : updaterState === 'restart' ? 'Restart agx' : 'Check for updates';
  } finally {
    button.disabled = false;
  }
}

// Refresh while the desktop window is open; schedule after completion to avoid overlap.
let lastRefreshFailure = '';
async function startTokenRefresh() {
  if (!window.__TAURI__?.core?.invoke) return;
  try {
    const report = await invoke('refresh_due_tokens');
    if (report.updated.length) {
      quotaRequests.clear();
      await loadAccountsData();
    }
    const failed = report.failed.join(', ');
    if (failed && failed !== lastRefreshFailure) {
      showToast(`Could not refresh token: ${failed}. Check connection or sign in again.`, 'error');
    }
    lastRefreshFailure = failed;
  } catch (err) {
    console.error('Automatic token refresh failed:', err);
  } finally {
    setTimeout(startTokenRefresh, lastRefreshFailure ? 300000 : 60000);
  }
}

// Start expensive scans only when their section is first opened.
const sectionLoads = new Map();
function loadSection(id) {
  const loader = { patcher: loadTargetsData, cleaner: loadCleanerData, doctor: loadDoctorData }[id];
  if (loader && !sectionLoads.has(id)) {
    sectionLoads.set(id, loader());
  }
}

// Share in-flight results between the overview and profile card. Limit network work.
const quotaRequests = new Map();
const quotaQueue = [];
let runningQuotas = 0;
function pumpQuotas() {
  while (runningQuotas < 2 && quotaQueue.length) {
    const job = quotaQueue.shift();
    runningQuotas++;
    invoke('get_account_quota', { name: job.name })
      .then(job.resolve, job.reject)
      .finally(() => { runningQuotas--; pumpQuotas(); });
  }
}
function requestQuota(name) {
  if (quotaRequests.has(name)) return quotaRequests.get(name);
  const request = new Promise((resolve, reject) => {
    quotaQueue.push({ name, resolve, reject });
  });
  quotaRequests.set(name, request);
  request.then(() => {
    // Short-lived cache avoids repeated token refreshes when rerendering profiles.
    setTimeout(() => { if (quotaRequests.get(name) === request) quotaRequests.delete(name); }, 30000);
  }, () => { if (quotaRequests.get(name) === request) quotaRequests.delete(name); });
  pumpQuotas();
  return request;
}

// Toast notification helper
function showToast(message, type = 'info') {
  const toast = document.createElement('div');
  toast.className = `toast ${type}`;
  toast.textContent = message;
  toastContainer.appendChild(toast);

  setTimeout(() => {
    toast.style.opacity = '0';
    toast.style.transform = 'translateX(20px)';
    setTimeout(() => toast.remove(), 300);
  }, 3500);
}

// Navigation Tabs
function setupTabs() {
  navTabs.forEach(tab => {
    tab.addEventListener('click', () => {
      const targetId = tab.dataset.tab;
      navTabs.forEach(t => t.classList.remove('active'));
      tabContents.forEach(c => c.classList.remove('active'));

      tab.classList.add('active');
      navTabs.forEach(t => t.setAttribute('aria-current', t === tab ? 'page' : 'false'));
      const content = document.getElementById(`tab-${targetId}`);
      if (content) content.classList.add('active');
      loadSection(targetId);
    });
  });
}

// Global Event Handlers
function setupEventHandlers() {
  // Auto-Best button
  const autoBestBtn = document.getElementById('btn-auto-best');
  if (autoBestBtn) {
    autoBestBtn.addEventListener('click', async () => {
      autoBestBtn.disabled = true;
      try {
        const best = await invoke('auto_best_cmd');
        showToast(`Auto-switched to best profile: ${best}`, 'success');
        await loadAccountsData();
      } catch (err) {
        showToast(`Auto-best failed: ${err}`, 'error');
      } finally {
        autoBestBtn.disabled = false;
      }
    });
  }

  // Refresh Quotas button
  const refreshBtn = document.getElementById('btn-refresh-quotas');
  if (refreshBtn) {
    refreshBtn.addEventListener('click', async () => {
      refreshBtn.disabled = true;
      try {
        quotaRequests.clear();
        await loadAccountsData();
        showToast('Live quotas refreshed', 'success');
      } catch (err) {
        showToast(`Failed to refresh quotas: ${err}`, 'error');
      } finally {
        refreshBtn.disabled = false;
      }
    });
  }

  // Binary Gates: Patch All / Restore All
  const patchAllBtn = document.getElementById('btn-patch-all');
  if (patchAllBtn) {
    patchAllBtn.addEventListener('click', async () => {
      patchAllBtn.disabled = true;
      try {
        const msg = await invoke('apply_patch_cmd', { target: 'all' });
        showToast(msg || 'All targets patched', 'success');
        await loadTargetsData();
      } catch (err) {
        showToast(`Patch all error: ${err}`, 'error');
      } finally {
        patchAllBtn.disabled = false;
      }
    });
  }

  const restoreAllBtn = document.getElementById('btn-restore-all');
  if (restoreAllBtn) {
    restoreAllBtn.addEventListener('click', async () => {
      restoreAllBtn.disabled = true;
      try {
        const msg = await invoke('restore_patch_cmd', { target: 'all' });
        showToast(msg || 'All targets restored', 'success');
        await loadTargetsData();
      } catch (err) {
        showToast(`Restore all error: ${err}`, 'error');
      } finally {
        restoreAllBtn.disabled = false;
      }
    });
  }

  // Purge Selected Cleaner
  const purgeBtn = document.getElementById('btn-purge-selected');
  if (purgeBtn) {
    purgeBtn.addEventListener('click', async () => {
      const checkedBoxes = document.querySelectorAll('.clean-checkbox:checked');
      const ids = Array.from(checkedBoxes).map(cb => cb.dataset.id);
      if (ids.length === 0) {
        showToast('No clean categories selected', 'error');
        return;
      }

      purgeBtn.disabled = true;
      try {
        const res = await invoke('purge_clean_categories', { ids });
        showToast(res || 'Selected caches purged successfully', 'success');
        await loadCleanerData();
      } catch (err) {
        showToast(`Purge failed: ${err}`, 'error');
      } finally {
        purgeBtn.disabled = false;
      }
    });
  }

  // Doctor Re-check
  const docRecheckBtn = document.getElementById('btn-re-doctor');
  if (docRecheckBtn) {
    docRecheckBtn.addEventListener('click', async () => {
      docRecheckBtn.disabled = true;
      try {
        await loadDoctorData();
        showToast('Diagnostics complete', 'success');
      } catch (err) {
        showToast(`Doctor check failed: ${err}`, 'error');
      } finally {
        docRecheckBtn.disabled = false;
      }
    });
  }
}

// Modal handling
function setupModals() {
  const modal = document.getElementById('modal-add-account');
  const openBtn = document.getElementById('btn-open-add-modal');
  const closeBtn = document.getElementById('modal-close-btn');
  const cancelBtn = document.getElementById('modal-cancel-btn');
  const submitBtn = document.getElementById('modal-submit-btn');

  const openModal = () => {
    document.getElementById('input-account-name').value = '';
    document.getElementById('input-account-json').value = '';
    modal.classList.add('show');
    document.getElementById('input-account-name').focus();
  };

  const closeModal = () => { modal.classList.remove('show'); openBtn?.focus(); };
  modal.addEventListener('click', e => { if (e.target === modal) closeModal(); });
  document.addEventListener('keydown', e => {
    if (!modal.classList.contains('show')) return;
    if (e.key === 'Escape') closeModal();
    if (e.key === 'Tab') {
      const items = [...modal.querySelectorAll('button,input,textarea')].filter(el => !el.disabled);
      const first = items[0], last = items[items.length - 1];
      if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last.focus(); }
      else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first.focus(); }
    }
  });

  if (openBtn) openBtn.addEventListener('click', openModal);
  if (closeBtn) closeBtn.addEventListener('click', closeModal);
  if (cancelBtn) cancelBtn.addEventListener('click', closeModal);

  if (submitBtn) {
    submitBtn.addEventListener('click', async () => {
      const name = document.getElementById('input-account-name').value.trim();
      const tokenJson = document.getElementById('input-account-json').value.trim();

      if (!name) {
        showToast('Profile name is required', 'error');
        return;
      }

      submitBtn.disabled = true;
      try {
        const res = await invoke('save_account_cmd', { name, tokenJson });
        showToast(res || `Account '${name}' saved`, 'success');
        closeModal();
        await loadAccountsData();
      } catch (err) {
        showToast(`Failed to save account: ${err}`, 'error');
      } finally {
        submitBtn.disabled = false;
      }
    });
  }
}

// -----------------------------------------------------------------------------
// 1. ACCOUNTS & QUOTA MANAGEMENT
// -----------------------------------------------------------------------------

async function loadAccountsData() {
  const container = document.getElementById('accounts-cards-grid');
  try {
    const list = await invoke('get_accounts');
    accounts = list || [];

    // Identify active
    const active = accounts.find(a => a.is_active);
    activeAccountName = active ? active.name : (accounts[0] ? accounts[0].name : null);

    // Update Header Pill
    const headerPill = document.getElementById('header-active-name');
    if (headerPill) headerPill.textContent = activeAccountName || 'None';

    // Update Hero Card
    const heroName = document.getElementById('hero-account-name');
    const heroMeta = document.getElementById('hero-account-meta');
    if (heroName) heroName.textContent = activeAccountName || 'No Active Profile';
    if (heroMeta && active) {
      heroMeta.textContent = `Token ${active.expiry} ${active.email ? '• ' + active.email : ''}`;
    }

    // Render Accounts Grid
    renderAccountsGrid();

    // Fetch and render live quotas for the active account
    if (activeAccountName) {
      await Promise.all([loadActiveQuotas(activeAccountName), loadMiniQuota(activeAccountName)]);
    } else {
      ['gemini-5h', 'gemini-wk', 'claude-5h', 'claude-wk'].forEach(id => updateGauge(id, null));
    }
    await Promise.all(accounts.filter(a => a.name !== activeAccountName).map(a => loadMiniQuota(a.name)));
    // Quota loading may have renewed a token; update displayed expiry from disk.
    const latest = await invoke('get_accounts');
    if (latest) {
      const current = latest.find(a => a.name === activeAccountName);
      if (current && heroMeta) heroMeta.textContent = `Token ${current.expiry} ${current.email ? '• ' + current.email : ''}`;
      container.querySelectorAll('.account-card').forEach(card => {
        const account = latest.find(a => a.name === card.dataset.name);
        const expiry = card.querySelector('.account-expiry');
        if (account && expiry) expiry.textContent = account.expiry;
      });
    }
  } catch (err) {
    document.getElementById('hero-account-name').textContent = 'Workspace unavailable';
    document.getElementById('hero-account-meta').textContent = 'Open the desktop app to view your accounts.';
    ['gemini-5h', 'gemini-wk', 'claude-5h', 'claude-wk'].forEach(id => updateGauge(id, null));
    container.innerHTML = `<div class="skeleton-card" style="color:var(--red-neon)">Failed to load accounts: ${err}</div>`;
  }
}

function renderAccountsGrid() {
  const container = document.getElementById('accounts-cards-grid');
  if (!container) return;

  if (accounts.length === 0) {
    container.innerHTML = `<div class="skeleton-card">No saved accounts found. Click "Add Account" to import one.</div>`;
    return;
  }

  container.innerHTML = accounts.map(acct => `
    <div class="account-card ${acct.is_active ? 'is-active-card' : ''}" data-name="${acct.name}">
      <div class="account-card-header">
        <div class="account-name-group">
          <span class="account-title">${acct.name}</span>
          ${acct.is_active ? '<span class="active-tag">ACTIVE</span>' : ''}
        </div>
        <span class="account-expiry">${acct.expiry}</span>
      </div>

      <div class="account-quotas-mini" id="mini-quota-${acct.name}">
        <div class="quota-mini-row">
          <span class="quota-mini-label">Claude & GPT 5h:</span>
          <span class="quota-mini-val" id="mini-val-claude-${acct.name}">loading...</span>
        </div>
        <div class="quota-mini-row">
          <span class="quota-mini-label">Gemini Models 5h:</span>
          <span class="quota-mini-val" id="mini-val-gemini-${acct.name}">loading...</span>
        </div>
      </div>

      <div class="account-card-actions">
        ${!acct.is_active ? `
          <button class="btn-primary-ghost btn-small btn-switch" data-name="${acct.name}">
            Switch
          </button>
        ` : `
          <button class="btn-secondary btn-small" disabled>
            Currently Active
          </button>
        `}
        <button class="btn-secondary btn-small btn-delete" data-name="${acct.name}" title="Delete account">
          <svg class="btn-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" aria-hidden="true"><path d="M3 6h18M9 6V4h6v2M5 6l1 14h12l1-14M10 10v6M14 10v6"/></svg>
        </button>
      </div>
    </div>
  `).join('');

  // Attach switch button listeners
  container.querySelectorAll('.btn-switch').forEach(btn => {
    btn.addEventListener('click', async (e) => {
      e.stopPropagation();
      const name = btn.dataset.name;
      btn.disabled = true;
      btn.textContent = 'Switching...';
      try {
        await invoke('switch_account_cmd', { name });
        showToast(`Switched active profile to ${name}`, 'success');
        await loadAccountsData();
      } catch (err) {
        showToast(`Failed to switch: ${err}`, 'error');
        btn.disabled = false;
        btn.textContent = 'Switch';
      }
    });
  });

  // Attach delete button listeners
  container.querySelectorAll('.btn-delete').forEach(btn => {
    btn.addEventListener('click', async (e) => {
      e.stopPropagation();
      const name = btn.dataset.name;
      if (!confirm(`Delete profile '${name}'? This cannot be undone.`)) return;

      try {
        await invoke('delete_account_cmd', { name });
        showToast(`Profile '${name}' deleted`, 'success');
        await loadAccountsData();
      } catch (err) {
        showToast(`Failed to delete: ${err}`, 'error');
      }
    });
  });


}

async function loadMiniQuota(name) {
  try {
    const q = await requestQuota(name);
    const claudeEl = document.getElementById(`mini-val-claude-${name}`);
    const geminiEl = document.getElementById(`mini-val-gemini-${name}`);

    if (claudeEl && q.claude_5h) {
      claudeEl.textContent = q.claude_5h.has_data ? (q.claude_5h.disabled ? 'Disabled' : `${q.claude_5h.pct}% (${q.claude_5h.reset_str})`) : '—';
      claudeEl.title = q.claude_5h.reset_str;
      claudeEl.style.color = getQuotaColor(q.claude_5h.pct);
    }
    if (geminiEl && q.gemini_5h) {
      geminiEl.textContent = q.gemini_5h.has_data ? (q.gemini_5h.disabled ? 'Disabled' : `${q.gemini_5h.pct}% (${q.gemini_5h.reset_str})`) : '—';
      geminiEl.title = q.gemini_5h.reset_str;
      geminiEl.style.color = getQuotaColor(q.gemini_5h.pct);
    }
  } catch (err) {
    const claudeEl = document.getElementById(`mini-val-claude-${name}`);
    const geminiEl = document.getElementById(`mini-val-gemini-${name}`);
    if (claudeEl) claudeEl.textContent = 'error';
    if (geminiEl) geminiEl.textContent = 'error';
  }
}

async function loadActiveQuotas(name) {
  try {
    const quotas = await requestQuota(name);
    if (activeAccountName !== name) return;
    updateGauge('gemini-5h', quotas.gemini_5h);
    updateGauge('gemini-wk', quotas.gemini_wk);
    updateGauge('claude-5h', quotas.claude_5h);
    updateGauge('claude-wk', quotas.claude_wk);
  } catch (err) {
    console.error('Failed to load active quotas:', err);
    showToast(`Quota fetch warning: ${err}`, 'error');
  }
}

function updateGauge(gaugeId, metric) {
  const pctEl = document.getElementById(`pct-${gaugeId}`);
  const resetEl = document.getElementById(`reset-${gaugeId}`);
  const ringEl = document.getElementById(`ring-${gaugeId}`);

  if (!metric || !metric.has_data) {
    if (pctEl) pctEl.textContent = '—';
    if (resetEl) resetEl.textContent = 'no data';
    if (ringEl) ringEl.style.strokeDashoffset = '314.159';
    return;
  }

  if (metric.disabled) {
    if (pctEl) pctEl.textContent = '0%';
    if (resetEl) resetEl.textContent = 'disabled';
    if (ringEl) {
      ringEl.style.stroke = 'var(--red-neon)';
      ringEl.style.strokeDashoffset = '314.159';
    }
    return;
  }

  const pct = Math.max(0, Math.min(100, metric.pct));
  if (pctEl) pctEl.textContent = `${pct}%`;
  if (resetEl) resetEl.textContent = metric.reset_str;

  if (ringEl) {
    const circumference = 314.159;
    ringEl.style.strokeDashoffset = circumference - (pct / 100) * circumference;
    ringEl.style.stroke = getQuotaColor(pct);
  }
}

function getQuotaColor(pct) {
  if (pct > 50) return 'var(--green-neon)';
  if (pct > 20) return 'var(--amber-neon)';
  return 'var(--red-neon)';
}

// -----------------------------------------------------------------------------
// 2. BINARY GATES (PATCHER)
// -----------------------------------------------------------------------------

async function loadTargetsData() {
  const container = document.getElementById('targets-list-container');
  try {
    const targets = await invoke('get_patch_targets');
    if (!targets || targets.length === 0) {
      container.innerHTML = `<div class="skeleton-card">No modifiable binary targets detected.</div>`;
      return;
    }

    container.innerHTML = targets.map(t => `
      <div class="target-card" data-name="${t.name}">
        <div class="target-info">
          <div class="target-header">
            <span class="target-name">${t.name}</span>
            <span class="target-badge ${t.state}">${formatTargetState(t.state)}</span>
          </div>
          <div class="target-desc">${t.description}</div>
          <div class="target-path">${t.path || 'Binary path not detected'}</div>
        </div>
        <div class="target-actions">
          <button class="btn-accent btn-small btn-target-patch" data-target="${t.name}">
            Patch Gate
          </button>
          <button class="btn-secondary btn-small btn-target-restore" data-target="${t.name}" ${!t.has_backup ? 'disabled title="No backup file found"' : ''}>
            Restore
          </button>
        </div>
      </div>
    `).join('');

    // Attach listeners
    container.querySelectorAll('.btn-target-patch').forEach(btn => {
      btn.addEventListener('click', async () => {
        const target = btn.dataset.target;
        btn.disabled = true;
        try {
          const res = await invoke('apply_patch_cmd', { target });
          showToast(res || `Target '${target}' patched`, 'success');
          await loadTargetsData();
        } catch (err) {
          showToast(`Patch failed: ${err}`, 'error');
          btn.disabled = false;
        }
      });
    });

    container.querySelectorAll('.btn-target-restore').forEach(btn => {
      btn.addEventListener('click', async () => {
        const target = btn.dataset.target;
        btn.disabled = true;
        try {
          const res = await invoke('restore_patch_cmd', { target });
          showToast(res || `Target '${target}' restored`, 'success');
          await loadTargetsData();
        } catch (err) {
          showToast(`Restore failed: ${err}`, 'error');
          btn.disabled = false;
        }
      });
    });
  } catch (err) {
    container.innerHTML = `<div class="skeleton-card" style="color:var(--red-neon)">Failed to scan targets: ${err}</div>`;
  }
}

function formatTargetState(state) {
  switch (state) {
    case 'patched': return 'PATCHED';
    case 'pristine': return 'PRISTINE';
    case 'not_found': return 'NOT FOUND';
    default: return 'UNKNOWN';
  }
}

// -----------------------------------------------------------------------------
// 3. VACUUM CLEANER
// -----------------------------------------------------------------------------

async function loadCleanerData() {
  const container = document.getElementById('cleaner-categories-container');
  const totalSizeEl = document.getElementById('cleaner-total-size');

  try {
    const cats = await invoke('get_clean_categories');
    cleanerCategories = cats || [];

    const totalBytes = cleanerCategories.reduce((acc, c) => acc + c.bytes, 0);
    if (totalSizeEl) {
      totalSizeEl.textContent = formatBytes(totalBytes);
    }

    if (cleanerCategories.length === 0) {
      container.innerHTML = `<div class="skeleton-card">Cache directories clean. No junk detected.</div>`;
      return;
    }

    container.innerHTML = cleanerCategories.map(c => `
      <div class="clean-category-card">
        <input type="checkbox" class="clean-checkbox" data-id="${c.id}" ${c.bytes > 0 ? 'checked' : 'disabled'} />
        <div class="clean-cat-info">
          <div class="clean-cat-title">${c.title}</div>
          <div class="clean-cat-desc">${c.description} (${c.paths_count} path${c.paths_count !== 1 ? 's' : ''})</div>
        </div>
        <div class="clean-cat-size">${c.size_formatted}</div>
      </div>
    `).join('');
  } catch (err) {
    container.innerHTML = `<div class="skeleton-card" style="color:var(--red-neon)">Failed to scan cleaner: ${err}</div>`;
  }
}

function formatBytes(bytes) {
  const mb = bytes / (1024 * 1024);
  if (mb >= 1024) {
    return `${(mb / 1024).toFixed(2)} GB`;
  }
  return `${mb.toFixed(1)} MB`;
}

// -----------------------------------------------------------------------------
// 4. SYSTEM DOCTOR
// -----------------------------------------------------------------------------

async function loadDoctorData() {
  const container = document.getElementById('doctor-items-container');
  const okEl = document.getElementById('doc-ok-count');
  const warnEl = document.getElementById('doc-warn-count');
  const failEl = document.getElementById('doc-fail-count');

  try {
    const report = await invoke('get_doctor_report');
    if (!report) return;

    if (okEl) okEl.textContent = report.ok_count;
    if (warnEl) warnEl.textContent = report.warn_count;
    if (failEl) failEl.textContent = report.fail_count;

    if (!report.items || report.items.length === 0) {
      container.innerHTML = `<div class="skeleton-card">No diagnostics available.</div>`;
      return;
    }

    container.innerHTML = report.items.map(item => `
      <div class="doctor-item-card">
        <div class="doc-status-icon">${getDoctorIcon(item.status)}</div>
        <div class="doc-item-body">
          <div class="doc-item-title">${item.title}</div>
          <div class="doc-item-detail">${item.detail}</div>
        </div>
      </div>
    `).join('');
  } catch (err) {
    container.innerHTML = `<div class="skeleton-card" style="color:var(--red-neon)">Doctor scan error: ${err}</div>`;
  }
}

function getDoctorIcon(status) {
  switch (status) {
    case 'ok':
      return `<svg viewBox="0 0 24 24" fill="none" stroke="var(--green-neon)" stroke-width="2.5"><polyline points="20 6 9 17 4 12"/></svg>`;
    case 'warn':
      return `<svg viewBox="0 0 24 24" fill="none" stroke="var(--amber-neon)" stroke-width="2.5"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>`;
    case 'fail':
      return `<svg viewBox="0 0 24 24" fill="none" stroke="var(--red-neon)" stroke-width="2.5"><circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/></svg>`;
    default:
      return `<svg viewBox="0 0 24 24" fill="none" stroke="var(--text-dim)" stroke-width="2.5"><circle cx="12" cy="12" r="10"/></svg>`;
  }
}
