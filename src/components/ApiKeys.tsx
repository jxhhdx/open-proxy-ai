import type { ApiKeyEntry } from "../types";

export default function ApiKeys({ keys }: { keys: ApiKeyEntry[] }) {
  return (
    <section className="bg-[#181a22] border border-[#2a2d3e] rounded-xl p-4 mb-4">
      <div className="flex items-center justify-between mb-3">
        <h2 className="text-sm font-semibold flex items-center gap-2">
          <svg width="16" height="16" viewBox="0 0 18 18" fill="none">
            <rect x="3" y="7" width="12" height="4" rx="1.5" stroke="currentColor" strokeWidth="1.2" />
            <circle cx="9" cy="9" r="1.5" fill="currentColor" />
          </svg>
          API Keys
        </h2>
        <span className="text-[11px] px-2 py-0.5 rounded-full bg-[#1e2030] text-muted">{keys.length}</span>
      </div>
      <div className="flex flex-col gap-2">
        {keys.map((k) => (
          <div
            key={k.name}
            className="flex items-center justify-between px-3 py-2.5 rounded-md bg-[#1e2030] border border-[#2a2d3e]"
          >
            <div className="flex flex-col gap-0.5 min-w-0">
              <span className="text-[11px] font-semibold text-muted uppercase tracking-wide">
                {k.name}
              </span>
              <span className="text-sm text-white font-mono break-all">{k.key}</span>
            </div>
            <button
              onClick={() => copyText(k.key)}
              className="text-base p-1 rounded hover:bg-white/10 transition-colors cursor-pointer"
            >
              Copy
            </button>
          </div>
        ))}
      </div>
    </section>
  );
}

async function copyText(text: string) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    const ta = document.createElement("textarea");
    ta.value = text;
    document.body.appendChild(ta);
    ta.select();
    document.execCommand("copy");
    ta.remove();
  }
}
