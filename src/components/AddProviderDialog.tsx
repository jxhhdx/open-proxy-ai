import { useState } from "react";
import { upsertPoolEntry } from "../hooks/useTauri";
import { useI18n } from "../i18n/context";
import type { ModelPoolEntry } from "../types";

export default function AddProviderDialog({ entry, onClose, onAdded, showToast }: {
  entry?: ModelPoolEntry | null; onClose: () => void; onAdded: () => void; showToast: (msg: string) => void;
}) {
  const { t } = useI18n();
  const isEdit = !!entry;
  const [name, setName] = useState(entry?.name || "");
  const [modelName, setModelName] = useState(entry?.model_name || "");
  const [baseUrl, setBaseUrl] = useState(entry?.base_url || "");
  const [apiKey, setApiKey] = useState(entry?.api_key || "");
  const [format, setFormat] = useState(entry?.api_format || "openai");
  const S: React.CSSProperties = { width: "100%", padding: "8px 12px", borderRadius: 6, background: "var(--surface2)", border: "1px solid var(--border)", fontSize: 13, color: "var(--text)", outline: "none" };

  const handleSubmit = async () => {
    if (!name.trim()) { showToast("Enter a name"); return; }
    try {
      await upsertPoolEntry({ id: isEdit ? entry!.id : null, name: name.trim(), base_url: baseUrl.trim(), api_key: apiKey.trim(), model_name: modelName.trim() || name.trim(), priority: 999, enabled: true, builtin: false, provider_type: baseUrl.trim() ? "custom" : "opencode", api_format: format });
      showToast((isEdit ? t.pool.edited : t.pool.added) + ": " + name); onAdded();
    } catch (e: any) { showToast("Error: " + e); }
  };

  return (
    <div style={{ position: "fixed", inset: 0, zIndex: 50, display: "flex", alignItems: "center", justifyContent: "center", background: "rgba(0,0,0,0.5)" }}>
      <div style={{ background: "var(--surface)", border: "1px solid var(--border)", borderRadius: 12, padding: 20, width: "100%", maxWidth: "24rem", margin: "0 16px" }}>
        <h3 style={{ fontSize: 14, fontWeight: 600, marginBottom: 16 }}>{isEdit ? t.dialog.editProvider : t.dialog.addProvider}</h3>
        <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
          <input placeholder={t.dialog.name} value={name} onChange={e => setName(e.target.value)} autoFocus style={S} />
          <input placeholder={t.dialog.modelName} value={modelName} onChange={e => setModelName(e.target.value)} style={S} />
          <input placeholder={t.dialog.apiUrl} value={baseUrl} onChange={e => setBaseUrl(e.target.value)} style={S} />
          <input type="password" placeholder={t.dialog.apiKey} value={apiKey} onChange={e => setApiKey(e.target.value)} style={S} />
          <div style={{ display: "flex", gap: 8 }}>
            {["openai", "anthropic"].map(f => (
              <label key={f} style={{ flex: 1, display: "flex", alignItems: "center", gap: 8, padding: "8px 12px", borderRadius: 6, border: "1px solid" + (format === f ? " var(--accent)" : " var(--border)"), background: "var(--surface2)", cursor: "pointer", fontSize: 13, color: "var(--text)" }}>
                <input type="radio" checked={format === f} onChange={() => setFormat(f)} style={{ accentColor: "var(--accent)" }} />
                {f === "openai" ? t.dialog.openai : t.dialog.anthropic}
              </label>
            ))}
          </div>
        </div>
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8, marginTop: 16 }}>
          <button onClick={onClose} style={{ padding: "8px 14px", borderRadius: 6, background: "var(--surface2)", color: "var(--muted)", fontSize: 12, fontWeight: 500, border: "1px solid var(--border)", cursor: "pointer" }}>{t.dialog.cancel}</button>
          <button onClick={handleSubmit} style={{ padding: "8px 14px", borderRadius: 6, background: "var(--accent)", color: "white", fontSize: 12, fontWeight: 500, border: "none", cursor: "pointer" }}>{isEdit ? t.dialog.save : t.dialog.add}</button>
        </div>
      </div>
    </div>
  );
}
