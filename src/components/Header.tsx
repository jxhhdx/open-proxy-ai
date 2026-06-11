import type { AppStatus } from "../types";

export default function Header({
  status,
  loading,
  onRefresh,
}: {
  status: AppStatus | null;
  loading: boolean;
  onRefresh: () => void;
}) {
  const dotClass = loading
    ? "bg-orange-400 animate-pulse"
    : status?.running
    ? "bg-green-400 shadow-[0_0_8px_rgba(74,222,128,0.3)]"
    : "bg-red-400";

  return (
    <header className="flex justify-between items-start pb-4 mb-6 border-b border-[#2a2d3e]">
      <div className="flex items-center gap-3">
        <div className="text-[#6c8cff] flex">
          <svg width="28" height="28" viewBox="0 0 28 28" fill="none">
            <rect width="28" height="28" rx="6" fill="currentColor" opacity="0.15" />
            <path d="M8 14l4-4 4 4-4 4-4-4z" fill="currentColor" opacity="0.6" />
            <path d="M20 14l-4 4-4-4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
          </svg>
        </div>
        <div>
          <h1 className="text-xl font-semibold tracking-tight">OpenCode Free Proxy</h1>
          <div className="flex items-center gap-2 text-sm text-muted mt-0.5">
            <span className={`w-2 h-2 rounded-full ${dotClass}`} />
            <span>
              {loading
                ? "Loading..."
                : status?.running
                ? `Running on port ${status.port}`
                : "Stopped"}
            </span>
            <span className="text-[#2a2d3e]">·</span>
            <span
              onClick={() => copyText("http://localhost:6446")}
              className="inline-flex items-center gap-1.5 px-2 py-0.5 rounded bg-[#1e2030] border border-border cursor-pointer hover:border-[#6c8cff] hover:bg-[#6c8cff]/10 transition-all text-xs group"
            >
              <code className="text-[#6c8cff]">http://localhost:6446</code>
              <span className="text-[10px] opacity-0 group-hover:opacity-100 transition-opacity">Copy</span>
            </span>
          </div>
        </div>
      </div>
      <button
        onClick={onRefresh}
        className="inline-flex items-center gap-1.5 px-3.5 py-1.5 rounded-md bg-[#1e2030] text-muted border border-border text-xs font-medium hover:bg-border hover:text-white transition-all active:scale-95 cursor-pointer"
      >
        <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
          <path d="M2 8a6 6 0 0111.2-3M14 8a6 6 0 01-11.2 3" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
          <path d="M14 2v4h-4M2 14v-4h4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
        </svg>
        Refresh
      </button>
    </header>
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
