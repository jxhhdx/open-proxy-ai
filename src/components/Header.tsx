
import type { AppStatus } from "../types";
import { useI18n } from "../i18n/context";

export default function Header({ status, loading, onRefresh, onSettings }: {
  status: AppStatus | null;
  loading: boolean;
  onRefresh: () => void;
  onSettings: () => void;
}) {
  const { t } = useI18n();
  const online = status?.running && !loading;
  return (
    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", paddingBottom: 16, marginBottom: 20, borderBottom: "1px solid var(--border)" }}>
      <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
        <div style={{ width: 36, height: 36, borderRadius: 8, background: "linear-gradient(135deg, #6c8cff, #4a6adf)", display: "flex", alignItems: "center", justifyContent: "center", color: "white", fontWeight: 700, fontSize: 16 }}>P</div>
        <div>
          <div style={{ fontSize: 16, fontWeight: 600, color: "var(--text)", letterSpacing: -0.3 }}>{t.header.title}</div>
          <div style={{ display: "flex", alignItems: "center", gap: 8, marginTop: 2 }}>
            <span style={{ width: 8, height: 8, borderRadius: "50%", background: loading ? "#fb923c" : online ? "#4ade80" : "#f87171", boxShadow: online ? "0 0 8px rgba(74,222,128,0.4)" : "none" }} />
            <span style={{ fontSize: 12, color: "var(--muted)" }}>{loading ? t.header.starting : online ? `${t.header.running} :${status!.port}` : t.header.stopped}</span>
            <span style={{ color: "var(--border)", fontSize: 10 }}>|</span>
            <span onClick={() => copy(`http://localhost:${status!.port}`)} style={{ fontSize: 11, color: "var(--accent)", cursor: "pointer", padding: "1px 6px", borderRadius: 4, background: "rgba(108,140,255,0.08)", border: "1px solid rgba(108,140,255,0.15)" }}>http://localhost:{status!.port}</span>
          </div>
        </div>
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <button onClick={onSettings} style={{ display: "flex", alignItems: "center", justifyContent: "center", width: 32, height: 32, borderRadius: 6, background: "transparent", color: "var(--muted)", border: "none", cursor: "pointer", fontSize: 16 }} title="Settings">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>
        </button>
        <button onClick={onRefresh} style={{ display: "inline-flex", alignItems: "center", gap: 6, padding: "6px 14px", borderRadius: 6, background: "var(--surface2)", color: "var(--muted)", border: "1px solid var(--border)", fontSize: 12, fontWeight: 500, cursor: "pointer" }}>
          <svg width="12" height="12" viewBox="0 0 16 16" fill="none"><path d="M2 8a6 6 0 0111.2-3M14 8a6 6 0 01-11.2 3" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" /><path d="M14 2v4h-4M2 14v-4h4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" /></svg>
          {t.header.refresh}
        </button>
      </div>
    </div>
  );
}

async function copy(t: string) {
  try { await navigator.clipboard.writeText(t); } catch { const ta = document.createElement("textarea"); ta.value = t; document.body.appendChild(ta); ta.select(); document.execCommand("copy"); ta.remove(); }
}
