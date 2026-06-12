import type { AppStatus } from "../types";

export default function Header({ status, loading, onRefresh }: {
  status: AppStatus | null;
  loading: boolean;
  onRefresh: () => void;
}) {
  const online = status?.running && !loading;
  return (
    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", paddingBottom: 16, marginBottom: 20, borderBottom: "1px solid var(--border)" }}>
      <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
        {/* Logo */}
        <div style={{
          width: 36, height: 36, borderRadius: 8,
          background: "linear-gradient(135deg, #6c8cff, #4a6adf)",
          display: "flex", alignItems: "center", justifyContent: "center",
          color: "white", fontWeight: 700, fontSize: 16,
        }}>
          P
        </div>
        <div>
          <div style={{ fontSize: 16, fontWeight: 600, color: "var(--text)", letterSpacing: -0.3 }}>
            OpenCode Free Proxy
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: 8, marginTop: 2 }}>
            <span style={{
              width: 8, height: 8, borderRadius: "50%",
              background: loading ? "#fb923c" : online ? "#4ade80" : "#f87171",
              boxShadow: online ? "0 0 8px rgba(74,222,128,0.4)" : "none",
            }} />
            <span style={{ fontSize: 12, color: "var(--muted)" }}>
              {loading ? "Starting..." : online ? `Running on :${status!.port}` : "Stopped"}
            </span>
            <span style={{ color: "var(--border)", fontSize: 10 }}>|</span>
            <span
              onClick={() => copy("http://localhost:6446")}
              style={{
                fontSize: 11, color: "var(--accent)", cursor: "pointer",
                padding: "1px 6px", borderRadius: 4,
                background: "rgba(108,140,255,0.08)", border: "1px solid rgba(108,140,255,0.15)",
              }}>
              http://localhost:6446
            </span>
          </div>
        </div>
      </div>

      <button onClick={onRefresh}
        style={{
          display: "inline-flex", alignItems: "center", gap: 6,
          padding: "6px 14px", borderRadius: 6,
          background: "var(--surface2)", color: "var(--muted)",
          border: "1px solid var(--border)", fontSize: 12, fontWeight: 500,
          cursor: "pointer", transition: "all 0.15s",
        }}>
        <svg width="12" height="12" viewBox="0 0 16 16" fill="none">
          <path d="M2 8a6 6 0 0111.2-3M14 8a6 6 0 01-11.2 3" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
          <path d="M14 2v4h-4M2 14v-4h4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
        </svg>
        Refresh
      </button>
    </div>
  );
}

async function copy(t: string) {
  try { await navigator.clipboard.writeText(t); } catch { const ta = document.createElement("textarea"); ta.value = t; document.body.appendChild(ta); ta.select(); document.execCommand("copy"); ta.remove(); }
}
