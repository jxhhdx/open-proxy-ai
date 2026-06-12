import type { ApiKeyEntry } from "../types";
import { useI18n } from "../i18n/context";

export default function ApiKeys({ keys }: { keys: ApiKeyEntry[] }) {
  const { t } = useI18n();
  return (
    <div style={{ background: "var(--surface)", border: "1px solid var(--border)", borderRadius: 12, padding: 16, marginBottom: 16 }}>
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 12 }}>
        <h2 style={{ fontSize: 14, fontWeight: 600, display: "flex", alignItems: "center", gap: 8 }}>{t.keys.title}</h2>
        <span style={{ fontSize: 11, padding: "2px 8px", borderRadius: 20, background: "var(--surface2)", color: "var(--muted)" }}>{keys.length}</span>
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
        {keys.map((k) => (
          <div key={k.name} style={{ display: "flex", alignItems: "center", justifyContent: "space-between", padding: "10px 12px", borderRadius: 6, background: "var(--surface2)", border: "1px solid var(--border)" }}>
            <div style={{ display: "flex", flexDirection: "column", gap: 2, minWidth: 0 }}>
              <span style={{ fontSize: 11, fontWeight: 600, color: "var(--muted)", textTransform: "uppercase", letterSpacing: 0.5 }}>{k.name}</span>
              <span style={{ fontSize: 13, color: "var(--text)", fontFamily: '"SF Mono","Cascadia Code",monospace', wordBreak: "break-all" }}>{k.key}</span>
            </div>
            <button onClick={() => copy(k.key)} style={{ fontSize: 13, padding: 4, borderRadius: 4, cursor: "pointer", background: "none", border: "none", color: "var(--muted)" }}>Copy</button>
          </div>
        ))}
      </div>
    </div>
  );
}

async function copy(t: string) {
  try { await navigator.clipboard.writeText(t); } catch { const ta = document.createElement("textarea"); ta.value = t; document.body.appendChild(ta); ta.select(); document.execCommand("copy"); ta.remove(); }
}
