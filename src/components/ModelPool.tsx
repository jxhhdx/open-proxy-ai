import { useState, useCallback } from "react";
import {
  DndContext, closestCenter, PointerSensor, useSensor, useSensors, type DragEndEvent,
} from "@dnd-kit/core";
import {
  SortableContext, verticalListSortingStrategy, useSortable,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import type { ModelPoolEntry } from "../types";
import {
  reorderPool, togglePoolEntry, removePoolEntry, runSpeedTest, importToTool, getStatus,
} from "../hooks/useTauri";

// ── Shared styles ──────────────────────────────────────────
const colors = {
  accent: "#6c8cff",
  accentHover: "#4a6adf",
  surface: "#181a22",
  surface2: "#1e2030",
  border: "#2a2d3e",
  text: "#e1e3eb",
  muted: "#8b8fa3",
  red: "#f87171",
  green: "#4ade80",
  orange: "#fb923c",
};

const baseBtn: React.CSSProperties = {
  padding: "5px 12px", borderRadius: 6, fontSize: 12, fontWeight: 500,
  cursor: "pointer", border: "none", transition: "all 0.15s",
};

// ── Sortable Row ───────────────────────────────────────────
function SortableRow({ entry, result, onToggle, onRemove, onImport, onTest }: any) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id: entry.id });
  const [showImp, setShowImp] = useState(false);
  const isOpen = entry.provider_type === "opencode";

  const style: React.CSSProperties = {
    display: "flex", alignItems: "center", gap: 10,
    padding: "10px 14px", borderRadius: 8,
    background: isDragging ? "var(--surface)" : "var(--surface2)",
    border: `1px solid ${isDragging ? colors.accent : colors.border}`,
    transform: CSS.Transform.toString(transform),
    transition: transition || undefined,
    opacity: isDragging ? 0.85 : entry.enabled ? 1 : 0.5,
    marginBottom: 6,
  };

  return (
    <div ref={setNodeRef} style={style}>
      {/* Drag handle */}
      <span {...attributes} {...listeners}
        style={{ color: colors.muted, cursor: "grab", fontSize: 16, lineHeight: 1, padding: "2px 0", userSelect: "none" }}>
        ⋮⋮
      </span>

      {/* Toggle */}
      <button onClick={() => onToggle(entry.id)}
        style={{
          flexShrink: 0, width: 32, height: 18, borderRadius: 9, position: "relative",
          border: "none", cursor: "pointer", transition: "background 0.2s",
          background: entry.enabled ? colors.accent : colors.border,
        }}>
        <div style={{
          position: "absolute", top: 2, width: 14, height: 14, borderRadius: "50%",
          background: "white", transition: "left 0.2s",
          boxShadow: "0 1px 3px rgba(0,0,0,0.3)",
          left: entry.enabled ? 16 : 2,
        }} />
      </button>

      {/* Info */}
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 6, marginBottom: 3 }}>
          <span style={{ fontSize: 13, fontWeight: 600, color: colors.text }}>{entry.name}</span>
          <span style={{
            fontSize: 10, fontWeight: 600, padding: "1px 6px", borderRadius: 4,
            background: isOpen ? "rgba(108,140,255,0.15)" : "rgba(251,146,60,0.15)",
            color: isOpen ? colors.accent : colors.orange,
          }}>
            {isOpen ? "OpenCode" : "Custom"}
          </span>
          <span style={{ fontSize: 10, color: colors.muted }}>#{entry.priority}</span>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 12, fontSize: 11, color: colors.muted }}>
          {result ? result.success ? (
            <>
              <span>⏱ <strong style={{ color: colors.text }}>{result.latency_ms}ms</strong></span>
              <span>⚡ <strong style={{ color: colors.text }}>{result.tokens_per_sec.toFixed(1)}</strong> tok/s</span>
            </>
          ) : (
            <span style={{ color: colors.red }}>✕ {result.error || "Failed"}</span>
          ) : (
            <span style={{ color: colors.muted }}>— Not tested</span>
          )}
        </div>
      </div>

      {/* Actions */}
      <div style={{ display: "flex", alignItems: "center", gap: 4, flexShrink: 0 }}>
        <button onClick={() => onTest(entry.name)}
          style={{ ...baseBtn, background: "transparent", color: colors.muted, border: `1px solid ${colors.border}`, fontSize: 11 }}>
          Test
        </button>

        <div style={{ position: "relative" }}>
          <button onClick={() => setShowImp(!showImp)}
            style={{ ...baseBtn, background: "transparent", color: colors.muted, border: `1px solid ${colors.border}`, fontSize: 11 }}>
            Import
          </button>
          {showImp && (
            <>
              <div style={{ position: "fixed", inset: 0, zIndex: 10 }} onClick={() => setShowImp(false)} />
              <div style={{
                position: "absolute", right: 0, top: "100%", marginTop: 4, zIndex: 20,
                background: colors.surface, border: `1px solid ${colors.border}`,
                borderRadius: 8, padding: 4, minWidth: 130, boxShadow: "0 8px 24px rgba(0,0,0,0.4)",
              }}>
                {["claude", "codex", "ccswitch"].map((t) => (
                  <button key={t} onClick={() => { setShowImp(false); onImport(entry.name, t); }}
                    style={{
                      display: "block", width: "100%", textAlign: "left", padding: "6px 12px", fontSize: 12,
                      color: colors.text, cursor: "pointer", background: "none", border: "none", borderRadius: 4,
                    }}>
                    {t === "claude" ? "🤖 Claude Code" : t === "codex" ? "△ Codex" : "🔄 CCSwitch"}
                  </button>
                ))}
              </div>
            </>
          )}
        </div>

        {!isOpen && (
          <button onClick={() => onRemove(entry.id)}
            style={{ ...baseBtn, background: "transparent", color: colors.red, border: `1px solid ${colors.border}`, fontSize: 11, padding: "5px 8px" }}>
            ✕
          </button>
        )}
      </div>
    </div>
  );
}

// ── Main Component ─────────────────────────────────────────
export default function ModelPool({ entries, results, setResults, onRefresh, showToast, onAddClick }: any) {
  const [testing, setTesting] = useState(false);
  const [showPoolImp, setShowPoolImp] = useState(false);
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 8 } }));

  const handleDragEnd = useCallback(async (e: DragEndEvent) => {
    const { active, over } = e;
    if (!over || active.id === over.id) return;
    const ids = entries.slice().sort((a: any, b: any) => a.priority - b.priority).map((x: any) => x.id);
    const oi = ids.indexOf(active.id), ni = ids.indexOf(over.id);
    if (oi < 0 || ni < 0) return;
    ids.splice(ni, 0, ids.splice(oi, 1)[0]);
    try { await reorderPool(ids); onRefresh(); } catch (err: any) { showToast("Error: " + err); }
  }, [entries, onRefresh, showToast]);

  const handleToggle = useCallback(async (id: string) => {
    try { await togglePoolEntry(id); onRefresh(); } catch (err: any) { showToast("Error: " + err); }
  }, [onRefresh, showToast]);

  const handleRemove = useCallback(async (id: string) => {
    try { await removePoolEntry(id); onRefresh(); } catch (err: any) { showToast("Error: " + err); }
  }, [onRefresh, showToast]);

  const handleBatchTest = useCallback(async () => {
    setTesting(true);
    setResults({});
    const sorted = entries.slice().sort((a: any, b: any) => a.priority - b.priority);
    for (const e of sorted) {
      try { const r = await runSpeedTest(e.name); setResults((p: any) => ({ ...p, [e.name]: r })); } catch {}
    }
    setTesting(false);
    showToast("Test complete");
  }, [entries, setResults, showToast]);

  const handleTest = useCallback(async (name: string) => {
    try { const r = await runSpeedTest(name); setResults((p: any) => ({ ...p, [name]: r })); } catch {}
  }, [setResults]);

  const handleImport = useCallback(async (name: string, tool: string) => {
    try {
      const status: any = await getStatus();
      const key = status.keys[0]?.key;
      if (!key) { showToast("No API key"); return; }
      showToast(await importToTool({ model: name, model_name: name, api_key: key, tool }));
    } catch (err: any) { showToast("Error: " + err); }
  }, [showToast]);

  const handlePoolImport = useCallback(async (tool: string) => {
    try {
      const status: any = await getStatus();
      const key = status.keys[0]?.key;
      if (!key) { showToast("No API key"); return; }
      showToast(await importToTool({ model: "ModelPool", model_name: "", api_key: key, tool }));
      setShowPoolImp(false);
    } catch (err: any) { showToast("Error: " + err); }
  }, [showToast]);

  const sorted = entries.slice().sort((a: any, b: any) => a.priority - b.priority);

  return (
    <div style={{ background: colors.surface, border: `1px solid ${colors.border}`, borderRadius: 12, padding: 20, marginBottom: 16 }}>
      {/* Header */}
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 16 }}>
        <h2 style={{ fontSize: 14, fontWeight: 600, display: "flex", alignItems: "center", gap: 8, color: colors.text }}>
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
            <rect x="2" y="2" width="12" height="12" rx="3" stroke={colors.accent} strokeWidth="1.2" fill="none" />
            <path d="M6 8l2 2 3-4" stroke={colors.accent} strokeWidth="1.2" strokeLinecap="round" />
          </svg>
          Model Pool
        </h2>
        <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <button onClick={onAddClick}
            style={{ ...baseBtn, background: colors.surface2, color: colors.text, border: `1px solid ${colors.border}` }}>
            + Add
          </button>
          <button onClick={handleBatchTest} disabled={testing}
            style={{ ...baseBtn, background: colors.accent, color: "white", opacity: testing ? 0.5 : 1 }}>
            {testing ? "Testing..." : "Batch Test"}
          </button>

          <div style={{ position: "relative" }}>
            <button onClick={() => setShowPoolImp(!showPoolImp)}
              style={{ ...baseBtn, background: colors.surface2, color: colors.text, border: `1px solid ${colors.border}` }}>
              Import Pool
            </button>
            {showPoolImp && (
              <>
                <div style={{ position: "fixed", inset: 0, zIndex: 10 }} onClick={() => setShowPoolImp(false)} />
                <div style={{
                  position: "absolute", right: 0, top: "100%", marginTop: 4, zIndex: 20,
                  background: colors.surface, border: `1px solid ${colors.border}`,
                  borderRadius: 8, padding: 4, minWidth: 140, boxShadow: "0 8px 24px rgba(0,0,0,0.4)",
                }}>
                  {["claude", "codex", "ccswitch"].map((t) => (
                    <button key={t} onClick={() => handlePoolImport(t)}
                      style={{
                        display: "block", width: "100%", textAlign: "left", padding: "6px 12px",
                        fontSize: 12, color: colors.text, cursor: "pointer",
                        background: "none", border: "none", borderRadius: 4,
                      }}>
                      {t === "claude" ? "🤖 Claude Code" : t === "codex" ? "△ Codex" : "🔄 CCSwitch"}
                    </button>
                  ))}
                </div>
              </>
            )}
          </div>

          <span style={{ fontSize: 11, padding: "2px 8px", borderRadius: 6, background: colors.surface2, color: colors.muted }}>
            {entries.length}
          </span>
        </div>
      </div>

      {/* Empty state */}
      {sorted.length === 0 ? (
        <div style={{ textAlign: "center", padding: 32, color: colors.muted, fontSize: 13 }}>
          <div style={{ fontSize: 32, marginBottom: 8, opacity: 0.3 }}>+</div>
          No providers configured yet.
          <div style={{ marginTop: 4 }}>Click <strong style={{ color: colors.text }}>+ Add</strong> to add one.</div>
        </div>
      ) : (
        /* Drag and drop list */
        <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
          <SortableContext items={sorted.map((e: any) => e.id)} strategy={verticalListSortingStrategy}>
            {sorted.map((e: any) => (
              <SortableRow
                key={e.id}
                entry={e}
                result={results[e.name]}
                onToggle={handleToggle}
                onRemove={handleRemove}
                onImport={handleImport}
                onTest={handleTest}
              />
            ))}
          </SortableContext>
        </DndContext>
      )}
    </div>
  );
}
