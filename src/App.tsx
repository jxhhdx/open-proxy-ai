import { useState, useEffect, useCallback } from "react";
import { I18nProvider } from "./i18n/context";
import { ThemeProvider } from "./i18n/theme";
import Header from "./components/Header";
import ApiKeys from "./components/ApiKeys";
import ModelPool from "./components/ModelPool";
import AddProviderDialog from "./components/AddProviderDialog";
import Toast from "./components/Toast";
import Settings from "./components/Settings";
import { getStatus, getModelPool } from "./hooks/useTauri";
import type { AppStatus, ModelPoolEntry, PoolStatus } from "./types";

function AppInner() {
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [pool, setPool] = useState<ModelPoolEntry[]>([]);
  const [activeModelId, setActiveModelId] = useState<string | null>(null);
  const [activeAt, setActiveAt] = useState<number | null>(null);
  const [results, setResults] = useState<Record<string, any>>({});
  const [toast, setToast] = useState("");
  const [showAdd, setShowAdd] = useState(false);
  const [editingEntry, setEditingEntry] = useState<ModelPoolEntry | null>(null);
  const [showSettings, setShowSettings] = useState(false);
  const [loading, setLoading] = useState(true);

  const showToast = useCallback((msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(""), 2000);
  }, []);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const [s, p] = await Promise.all([getStatus(), getModelPool()]) as [AppStatus, PoolStatus];
      setStatus(s);
      setPool(p.entries);
      setActiveModelId(p.active_model_id ?? null);
      setActiveAt(p.active_at ? Number(p.active_at) : null);
    } catch {}
    setLoading(false);
  }, []);

  useEffect(() => { refresh(); }, []);

  // Poll pool status every 500ms to catch active_model_id changes
  useEffect(() => {
    const id = setInterval(refresh, 500);
    return () => clearInterval(id);
  }, [refresh]);

  return (
    <div style={{ maxWidth: 720, margin: "0 auto", padding: "20px 24px" }}>
      <Header status={status} loading={loading} onRefresh={refresh} onSettings={() => setShowSettings(true)} />
      <ApiKeys keys={status?.keys || []} showToast={showToast} />
      <ModelPool entries={pool} activeModelId={activeModelId} activeAt={activeAt} results={results} setResults={setResults} onRefresh={refresh} showToast={showToast} onAddClick={() => setShowAdd(true)} onEdit={(e: ModelPoolEntry) => setEditingEntry(e)} />
      {(showAdd || editingEntry) && <AddProviderDialog entry={editingEntry} onClose={() => { setShowAdd(false); setEditingEntry(null); }} onAdded={() => { setShowAdd(false); setEditingEntry(null); refresh(); }} showToast={showToast} />}
      {showSettings && <Settings onClose={() => setShowSettings(false)} />}
      <Toast message={toast} />
    </div>
  );
}

export default function App() {
  return <I18nProvider><ThemeProvider><AppInner /></ThemeProvider></I18nProvider>;
}
