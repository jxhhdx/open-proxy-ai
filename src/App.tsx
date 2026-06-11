import { useState, useEffect, useCallback } from "react";
import Header from "./components/Header";
import ApiKeys from "./components/ApiKeys";
import ModelPool from "./components/ModelPool";
import AddProviderDialog from "./components/AddProviderDialog";
import Toast from "./components/Toast";
import { getStatus, getModelPool } from "./hooks/useTauri";
import type { AppStatus, ModelPoolEntry } from "./types";

export default function App() {
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [pool, setPool] = useState<ModelPoolEntry[]>([]);
  const [results, setResults] = useState<Record<string, any>>({});
  const [toast, setToast] = useState<string>("");
  const [showAdd, setShowAdd] = useState(false);
  const [loading, setLoading] = useState(true);

  const showToast = useCallback((msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(""), 2000);
  }, []);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const [s, p] = await Promise.all([getStatus(), getModelPool()]);
      setStatus(s);
      setPool(p.entries);
    } catch (e: any) {
      showToast("Error: " + e);
    }
    setLoading(false);
  }, [showToast]);

  useEffect(() => { refresh(); }, []);

  return (
    <div className="max-w-2xl mx-auto px-6 py-5">
      <Header status={status} loading={loading} onRefresh={refresh} />

      <ApiKeys keys={status?.keys || []} />

      <ModelPool
        entries={pool}
        results={results}
        setResults={setResults}
        onRefresh={refresh}
        showToast={showToast}
        onAddClick={() => setShowAdd(true)}
      />

      {showAdd && (
        <AddProviderDialog
          onClose={() => setShowAdd(false)}
          onAdded={() => { setShowAdd(false); refresh(); }}
          showToast={showToast}
        />
      )}

      <Toast message={toast} />
    </div>
  );
}
