import { useState, useCallback } from "react";
import {
  DndContext,
  closestCenter,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core";
import {
  SortableContext,
  sortableKeyboardCoordinates,
  verticalListSortingStrategy,
  useSortable,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import type { ModelPoolEntry, SpeedTestResult } from "../types";
import {
  reorderPool,
  togglePoolEntry,
  removePoolEntry,
  runSpeedTest,
  importToTool,
} from "../hooks/useTauri";

function SortableRow({
  entry,
  result,
  onToggle,
  onRemove,
  onTest,
  onImport,
}: {
  entry: ModelPoolEntry;
  result: SpeedTestResult | undefined;
  onToggle: (id: string) => void;
  onRemove: (id: string) => void;
  onTest: (name: string) => void;
  onImport: (name: string, tool: string) => void;
}) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: entry.id,
  });
  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.4 : entry.enabled ? 1 : 0.5,
  };
  const [showImp, setShowImp] = useState(false);
  const isOpen = entry.provider_type === "opencode";

  return (
    <div
      ref={setNodeRef}
      style={style}
      className="flex items-center px-3 py-2 rounded-md bg-[#1e2030] border border-[#2a2d3e]"
    >
      <span
        {...attributes}
        {...listeners}
        className="text-muted hover:text-white cursor-grab active:cursor-grabbing text-sm mr-2 select-none"
      >
        ⠿
      </span>

      <button
        onClick={() => onToggle(entry.id)}
        className="flex-shrink-0 w-7 h-4 rounded-full relative mr-2"
        style={{ background: entry.enabled ? "#6c8cff" : "#2a2d3e" }}
      >
        <div
          className="absolute top-0.5 w-3 h-3 rounded-full bg-white transition-all"
          style={{ left: entry.enabled ? "14px" : "2px" }}
        />
      </button>

      <div className="flex flex-col gap-0.5 min-w-0 flex-1">
        <div className="flex items-center gap-1.5">
          <span className="text-sm font-medium text-white">{entry.name}</span>
          <span
            className="text-[10px] px-1.5 py-0.5 rounded"
            style={{ background: isOpen ? "#6c8cff" : "#fb923c", color: "white" }}
          >
            {isOpen ? "Free" : "Custom"}
          </span>
          <span className="text-[10px] text-muted">#{entry.priority}</span>
        </div>
        <div className="flex items-center gap-3 text-xs text-muted">
          {result ? (
            result.success ? (
              <>
                <span>
                  Latency: <strong className="text-white">{result.latency_ms}ms</strong>
                </span>
                <span>
                  Speed:{" "}
                  <strong className="text-white">{result.tokens_per_sec.toFixed(1)}</strong> tok/s
                </span>
              </>
            ) : (
              <span className="text-red-400">Failed: {result.error}</span>
            )
          ) : (
            <span>No data</span>
          )}
        </div>
      </div>

      <div className="flex items-center gap-1 flex-shrink-0 relative">
        <div className="relative">
          <button
            onClick={() => setShowImp(!showImp)}
            className="px-2 py-1 rounded text-xs text-white bg-[#2a2d3e] hover:bg-[#3a3d4e] transition-all cursor-pointer"
          >
            Import
          </button>
          {showImp && (
            <>
              <div
                className="fixed inset-0 z-10"
                onClick={() => setShowImp(false)}
              />
              <div className="absolute right-0 top-full mt-1 z-20 bg-[#1e2030] border border-[#2a2d3e] rounded-lg shadow-xl py-1 min-w-[120px]">
                <button
                  onClick={() => { setShowImp(false); onImport(entry.name, "claude"); }}
                  className="block w-full text-left px-3 py-1.5 text-xs text-white hover:bg-white/10 cursor-pointer bg-transparent border-none"
                >
                  Claude
                </button>
                <button
                  onClick={() => { setShowImp(false); onImport(entry.name, "codex"); }}
                  className="block w-full text-left px-3 py-1.5 text-xs text-white hover:bg-white/10 cursor-pointer bg-transparent border-none"
                >
                  Codex
                </button>
                <button
                  onClick={() => { setShowImp(false); onImport(entry.name, "ccswitch"); }}
                  className="block w-full text-left px-3 py-1.5 text-xs text-white hover:bg-white/10 cursor-pointer bg-transparent border-none"
                >
                  CCSwitch
                </button>
              </div>
            </>
          )}
        </div>
        {!isOpen && (
          <button
            onClick={() => onRemove(entry.id)}
            className="px-2 py-1 rounded text-xs text-red-400 bg-red-400/10 hover:bg-red-400/20 transition-all cursor-pointer border-none"
          >
            X
          </button>
        )}
      </div>
    </div>
  );
}

export default function ModelPool({
  entries,
  results,
  setResults,
  onRefresh,
  showToast,
  onAddClick,
}: {
  entries: ModelPoolEntry[];
  results: Record<string, any>;
  setResults: (r: Record<string, any>) => void;
  onRefresh: () => void;
  showToast: (msg: string) => void;
  onAddClick: () => void;
}) {
  const [testing, setTesting] = useState(false);

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 8 } })
  );

  const handleDragEnd = useCallback(
    async (event: DragEndEvent) => {
      const { active, over } = event;
      if (!over || active.id === over.id) return;
      const ids = entries
        .slice()
        .sort((a, b) => a.priority - b.priority)
        .map((e) => e.id);
      const oldIdx = ids.indexOf(active.id as string);
      const newIdx = ids.indexOf(over.id as string);
      if (oldIdx < 0 || newIdx < 0) return;
      ids.splice(newIdx, 0, ids.splice(oldIdx, 1)[0]);
      try {
        await reorderPool(ids);
        onRefresh();
      } catch (e: any) {
        showToast("Error: " + e);
      }
    },
    [entries, onRefresh, showToast]
  );

  const handleToggle = useCallback(
    async (id: string) => {
      try {
        await togglePoolEntry(id);
        onRefresh();
      } catch (e: any) {
        showToast("Error: " + e);
      }
    },
    [onRefresh, showToast]
  );

  const handleRemove = useCallback(
    async (id: string) => {
      try {
        await removePoolEntry(id);
        onRefresh();
      } catch (e: any) {
        showToast("Error: " + e);
      }
    },
    [onRefresh, showToast]
  );

  const handleTest = useCallback(
    async (name: string) => {
      try {
        const r = await runSpeedTest(name);
        setResults({ ...results, [name]: r });
      } catch (e: any) {
        showToast("Error: " + e);
      }
    },
    [results, setResults, showToast]
  );

  const handleBatchTest = useCallback(async () => {
    setTesting(true);
    setResults({});
    const sorted = entries.slice().sort((a, b) => a.priority - b.priority);
    for (const e of sorted) {
      try {
        const r = await runSpeedTest(e.name);
        setResults((prev: any) => ({ ...prev, [e.name]: r }));
      } catch {}
    }
    setTesting(false);
    showToast("Batch test complete");
  }, [entries, setResults, showToast]);

  const handleImport = useCallback(
    async (name: string, tool: string) => {
      try {
        const { getStatus } = await import("../hooks/useTauri");
        const status: any = await getStatus();
        const key = status.keys[0]?.key;
        if (!key) {
          showToast("No API key");
          return;
        }
        const r = await importToTool({
          model: name,
          model_name: name,
          api_key: key,
          tool,
        });
        showToast(r);
      } catch (e: any) {
        showToast("Error: " + e);
      }
    },
    [showToast]
  );

  const handlePoolImport = useCallback(
    async (tool: string) => {
      try {
        const { getStatus } = await import("../hooks/useTauri");
        const status: any = await getStatus();
        const key = status.keys[0]?.key;
        if (!key) {
          showToast("No API key");
          return;
        }
        const r = await importToTool({
          model: "ModelPool",
          model_name: "",
          api_key: key,
          tool,
        });
        showToast(r);
      } catch (e: any) {
        showToast("Error: " + e);
      }
    },
    [showToast]
  );

  const sorted = entries.slice().sort((a, b) => a.priority - b.priority);

  return (
    <section className="bg-[#181a22] border border-[#2a2d3e] rounded-xl p-4 mb-4">
      <div className="flex items-center justify-between mb-3">
        <h2 className="text-sm font-semibold flex items-center gap-2">
          <svg width="16" height="16" viewBox="0 0 18 18" fill="none">
            <path d="M9 2v14M2 9h14" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
            <circle cx="9" cy="9" r="6" stroke="currentColor" strokeWidth="1.2" />
          </svg>
          Model Pool
        </h2>
        <div className="flex items-center gap-2">
          <button
            onClick={onAddClick}
            className="inline-flex items-center gap-1 px-3 py-1.5 rounded-md bg-[#1e2030] text-white border border-border text-xs font-medium hover:bg-border transition-all cursor-pointer"
          >
            + Add
          </button>
          <button
            onClick={handleBatchTest}
            disabled={testing}
            className="inline-flex items-center gap-1 px-3 py-1.5 rounded-md bg-[#6c8cff] text-white text-xs font-medium hover:bg-[#4a6adf] transition-all active:scale-95 cursor-pointer disabled:opacity-40"
          >
            {testing ? "Testing..." : "Speed Test"}
          </button>
          <div className="relative">
            <button
              onClick={() => {
                const d = document.getElementById("pool-import-dd");
                if (d) d.classList.toggle("hidden");
              }}
              className="px-2.5 py-1.5 rounded-md text-xs text-white bg-[#2a2d3e] hover:bg-[#3a3d4e] transition-all cursor-pointer"
            >
              Import Pool
            </button>
            <div
              id="pool-import-dd"
              className="hidden absolute right-0 top-full mt-1 z-30 bg-[#1e2030] border border-[#2a2d3e] rounded-lg shadow-xl py-1 min-w-[120px]"
            >
              <button
                onClick={() => handlePoolImport("claude")}
                className="block w-full text-left px-3 py-1.5 text-xs text-white hover:bg-white/10 cursor-pointer bg-transparent border-none"
              >
                Claude
              </button>
              <button
                onClick={() => handlePoolImport("codex")}
                className="block w-full text-left px-3 py-1.5 text-xs text-white hover:bg-white/10 cursor-pointer bg-transparent border-none"
              >
                Codex
              </button>
              <button
                onClick={() => handlePoolImport("ccswitch")}
                className="block w-full text-left px-3 py-1.5 text-xs text-white hover:bg-white/10 cursor-pointer bg-transparent border-none"
              >
                CCSwitch
              </button>
            </div>
          </div>
          <span className="text-[11px] px-2 py-0.5 rounded-full bg-[#1e2030] text-muted">
            {entries.length}
          </span>
        </div>
      </div>

      {sorted.length === 0 ? (
        <div className="text-center py-5 text-muted text-sm">No models. Click + Add</div>
      ) : (
        <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
          <SortableContext items={sorted.map((e) => e.id)} strategy={verticalListSortingStrategy}>
            <div className="flex flex-col gap-1.5">
              {sorted.map((e) => (
                <SortableRow
                  key={e.id}
                  entry={e}
                  result={results[e.name]}
                  onToggle={handleToggle}
                  onRemove={handleRemove}
                  onTest={handleTest}
                  onImport={handleImport}
                />
              ))}
            </div>
          </SortableContext>
        </DndContext>
      )}
    </section>
  );
}
