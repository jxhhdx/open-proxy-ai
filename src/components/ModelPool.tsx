import { useState, useCallback, useEffect } from "react";
import { DndContext, closestCenter, PointerSensor, useSensor, useSensors, type DragEndEvent } from "@dnd-kit/core";
import { SortableContext, verticalListSortingStrategy, useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { reorderPool, togglePoolEntry, removePoolEntry, runSpeedTest, importToTool, getStatus } from "../hooks/useTauri";
import { useI18n } from "../i18n/context";

const C = { accent: "#6c8cff", surface: "var(--surface)", surface2: "var(--surface2)", border: "var(--border)", text: "var(--text)", muted: "var(--muted)", red: "#f87171", orange: "#fb923c" };
const btn: React.CSSProperties = { padding: "5px 12px", borderRadius: 6, fontSize: 12, fontWeight: 500, cursor: "pointer", border: "none" };

function Row({ entry, result, onToggle, onRemove, onEdit, onImport, onTest, activeModelId, activeAt }: any) {
  const { t } = useI18n();
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id: entry.id });
  const isOpen = entry.provider_type === "opencode";
  const isActive = entry.id === activeModelId && activeAt != null && (Date.now() - activeAt) < 10000;
  return (
    <div ref={setNodeRef} style={{ display: "flex", alignItems: "center", gap: 10, padding: "10px 14px", borderRadius: 8, background: isDragging ? C.surface : C.surface2, border: `1px solid ${isDragging ? C.accent : C.border}`, transform: CSS.Transform.toString(transform), transition, opacity: isDragging ? 0.85 : entry.enabled ? 1 : 0.5, marginBottom: 6 }}>
      <span {...attributes} {...listeners} style={{ color: C.muted, cursor: "grab", fontSize: 16, lineHeight: 1, userSelect: "none" }}>⋮⋮</span>
      <button onClick={() => onToggle(entry.id)} style={{ flexShrink: 0, width: 32, height: 18, borderRadius: 9, position: "relative", border: "none", cursor: "pointer", background: entry.enabled ? C.accent : C.border, transition: "background 0.2s" }}>
        <div style={{ position: "absolute", top: 2, width: 14, height: 14, borderRadius: "50%", background: "white", left: entry.enabled ? 16 : 2, transition: "left 0.2s", boxShadow: "0 1px 3px rgba(0,0,0,0.3)" }} />
      </button>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 6, marginBottom: 3 }}>
          <span style={{ width: 8, height: 8, borderRadius: "50%", flexShrink: 0, background: isActive ? "#4ade80" : "#6b7280", boxShadow: isActive ? "0 0 6px rgba(74,222,128,0.5)" : "none" }} />
          <span style={{ fontSize: 13, fontWeight: 600, color: C.text }}>{entry.name}</span>
          <span style={{ fontSize: 10, fontWeight: 600, padding: "1px 6px", borderRadius: 4, background: isOpen ? "rgba(108,140,255,0.15)" : "rgba(251,146,60,0.15)", color: isOpen ? C.accent : C.orange }}>{isOpen ? t.pool.opencode : t.pool.custom}</span>
          <span style={{ fontSize: 10, color: C.muted }}>#{entry.priority}</span>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 12, fontSize: 11, color: C.muted }}>
          {result ? result.success ? (
            <><span>⏱ <strong style={{ color: C.text }}>{result.latency_ms}ms</strong></span><span>⚡ <strong style={{ color: C.text }}>{result.tokens_per_sec.toFixed(1)}</strong> tok/s</span></>
          ) : <span style={{ color: C.red }}>✕ {result.error || "Failed"}</span> : <span>— {t.pool.notTested}</span>}
        </div>
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: 4, flexShrink: 0 }}>
        {!isOpen && <button onClick={() => onEdit(entry)} style={{ ...btn, background: "transparent", color: C.muted, border: `1px solid ${C.border}`, fontSize: 11, padding: "5px 8px" }}>✎</button>}
        {!isOpen && <button onClick={() => onRemove(entry.id)} style={{ ...btn, background: "transparent", color: C.red, border: `1px solid ${C.border}`, fontSize: 11, padding: "5px 8px" }}>✕</button>}
      </div>
    </div>
  );
}

export default function ModelPool({ entries, results, setResults, onRefresh, showToast, onAddClick, onEdit, activeModelId, activeAt }: any) {
  const { t } = useI18n();
  const [testing, setTesting] = useState(false);
  const [showPoolImp, setShowPoolImp] = useState(false);
  const [tick, setTick] = useState(0);
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 8 } }));

  // Re-render every 2s so the active indicator (10s timeout) updates in real-time
  useEffect(() => {
    const id = setInterval(() => setTick(t => t + 1), 2000);
    return () => clearInterval(id);
  }, []);

  const handleDragEnd = useCallback(async (e: DragEndEvent) => {
    const { active, over } = e;
    if (!over || active.id === over.id) return;
    const ids = entries.slice().sort((a: any, b: any) => a.priority - b.priority).map((x: any) => x.id);
    const oi = ids.indexOf(active.id), ni = ids.indexOf(over.id);
    if (oi < 0 || ni < 0) return;
    ids.splice(ni, 0, ids.splice(oi, 1)[0]);
    try { await reorderPool(ids); onRefresh(); showToast(t.pool.orderSaved); } catch (err: any) { showToast("Error: " + err); }
  }, [entries, onRefresh, showToast]);

  const handleToggle = useCallback(async (id: string) => { try { await togglePoolEntry(id); onRefresh(); } catch (err: any) { showToast("Error: " + err); } }, [onRefresh, showToast]);
  const handleRemove = useCallback(async (id: string) => { try { await removePoolEntry(id); onRefresh(); } catch (err: any) { showToast("Error: " + err); } }, [onRefresh, showToast]);

  const handleBatchTest = useCallback(async () => {
    setTesting(true); setResults({});
    const sorted = entries.slice().sort((a: any, b: any) => a.priority - b.priority);
    // Fire all speed tests concurrently, update results as each finishes
    await Promise.allSettled(sorted.map((e: any) =>
      runSpeedTest(e.name).then(r => setResults((p: any) => ({ ...p, [e.name]: r })))
    ));
    setTesting(false); showToast(t.pool.testComplete);
  }, [entries, setResults, showToast]);

  const handleTest = useCallback(async (name: string) => { try { const r = await runSpeedTest(name); setResults((p: any) => ({ ...p, [name]: r })); } catch {} }, [setResults]);
  const handleImport = useCallback(async (name: string, tool: string) => {
    try { const status: any = await getStatus(); const key = status.keys[0]?.key; if (!key) { showToast(t.pool.noApiKey); return; } showToast(await importToTool({ model: name, model_name: name, api_key: key, tool })); } catch (err: any) { showToast("Error: " + err); }
  }, [showToast]);
  const handlePoolImport = useCallback(async (tool: string) => {
    try { const status: any = await getStatus(); const key = status.keys[0]?.key; if (!key) { showToast(t.pool.noApiKey); return; } showToast(await importToTool({ model: "ModelPool", model_name: "ModelPool", api_key: key, tool })); setShowPoolImp(false); } catch (err: any) { showToast("Error: " + err); }
  }, [showToast]);

  const sorted = entries.slice().sort((a: any, b: any) => a.priority - b.priority);
  return (
    <div style={{ background: C.surface, border: `1px solid ${C.border}`, borderRadius: 12, padding: 20, marginBottom: 16 }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 16 }}>
        <h2 style={{ fontSize: 14, fontWeight: 600, display: "flex", alignItems: "center", gap: 8, color: C.text }}>{t.pool.title}</h2>
        <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <button onClick={onAddClick} style={{ ...btn, background: C.surface2, color: C.text, border: `1px solid ${C.border}` }}>{t.pool.add}</button>
          <button onClick={handleBatchTest} disabled={testing} style={{ ...btn, background: C.accent, color: "white", opacity: testing ? 0.5 : 1 }}>{testing ? t.pool.testing : t.pool.batchTest}</button>
          <div style={{ position: "relative" }}>
            <button onClick={() => setShowPoolImp(!showPoolImp)} style={{ ...btn, background: C.surface2, color: C.text, border: `1px solid ${C.border}` }}>{t.pool.importPool}</button>
            {showPoolImp && <>
              <div style={{ position: "fixed", inset: 0, zIndex: 10 }} onClick={() => setShowPoolImp(false)} />
              <div style={{ position: "absolute", right: 0, top: "100%", marginTop: 4, zIndex: 20, background: C.surface, border: `1px solid ${C.border}`, borderRadius: 8, padding: 4, minWidth: 140, boxShadow: "0 8px 24px rgba(0,0,0,0.4)" }}>
                {[["claude", t.import.claude], ["codex", t.import.codex], ["ccswitch", t.import.ccswitch]].map(([k, label]) => (
                  <button key={k} onClick={() => handlePoolImport(k)} style={{ display: "block", width: "100%", textAlign: "left", padding: "6px 12px", fontSize: 12, color: C.text, cursor: "pointer", background: "none", border: "none", borderRadius: 4 }}>{label}</button>
                ))}
              </div>
            </>}
          </div>
          <span style={{ fontSize: 11, padding: "2px 8px", borderRadius: 6, background: C.surface2, color: C.muted }}>{entries.length}</span>
        </div>
      </div>
      {sorted.length === 0 ? (
        <div style={{ textAlign: "center", padding: 32, color: C.muted, fontSize: 13 }}>
          <div style={{ fontSize: 32, marginBottom: 8, opacity: 0.3 }}>+</div>
          {t.pool.noModels}<div style={{ marginTop: 4 }}>{t.pool.noModelsHint}</div>
        </div>
      ) : (
        <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
          <SortableContext items={sorted.map((e: any) => e.id)} strategy={verticalListSortingStrategy}>
            {sorted.map((e: any) => <Row key={e.id} entry={e} result={results[e.name]} onToggle={handleToggle} onRemove={handleRemove} onEdit={onEdit} onImport={handleImport} onTest={handleTest} activeModelId={activeModelId} activeAt={activeAt} />)}
          </SortableContext>
        </DndContext>
      )}
    </div>
  );
}
