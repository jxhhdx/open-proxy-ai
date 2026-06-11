import { useState } from "react";
import { upsertPoolEntry } from "../hooks/useTauri";

export default function AddProviderDialog({
  onClose,
  onAdded,
  showToast,
}: {
  onClose: () => void;
  onAdded: () => void;
  showToast: (msg: string) => void;
}) {
  const [name, setName] = useState("");
  const [modelName, setModelName] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [format, setFormat] = useState("openai");

  const handleSubmit = async () => {
    if (!name.trim()) {
      showToast("Enter a name");
      return;
    }
    try {
      await upsertPoolEntry({
        id: null,
        name: name.trim(),
        base_url: baseUrl.trim(),
        api_key: apiKey.trim(),
        model_name: modelName.trim() || name.trim(),
        priority: 999,
        enabled: true,
        builtin: false,
        provider_type: baseUrl.trim() ? "custom" : "opencode",
        api_format: format,
      });
      showToast("Added: " + name);
      onAdded();
    } catch (e: any) {
      showToast("Error: " + e);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="bg-[#181a22] border border-[#2a2d3e] rounded-xl p-5 w-full max-w-sm mx-4 shadow-2xl">
        <h3 className="text-sm font-semibold mb-4">Add Provider</h3>
        <div className="flex flex-col gap-3">
          <input
            placeholder="Name *"
            value={name}
            onChange={(e) => setName(e.target.value)}
            autoFocus
            className="w-full px-3 py-2 rounded-md bg-[#1e2030] border border-[#2a2d3e] text-sm text-white placeholder-muted outline-none focus:border-[#6c8cff] transition-colors"
          />
          <input
            placeholder="Model name *"
            value={modelName}
            onChange={(e) => setModelName(e.target.value)}
            className="w-full px-3 py-2 rounded-md bg-[#1e2030] border border-[#2a2d3e] text-sm text-white placeholder-muted outline-none focus:border-[#6c8cff] transition-colors"
          />
          <input
            placeholder="API URL (empty = use OpenCode free)"
            value={baseUrl}
            onChange={(e) => setBaseUrl(e.target.value)}
            className="w-full px-3 py-2 rounded-md bg-[#1e2030] border border-[#2a2d3e] text-sm text-white placeholder-muted outline-none focus:border-[#6c8cff] transition-colors"
          />
          <input
            type="password"
            placeholder="API Key (optional)"
            value={apiKey}
            onChange={(e) => setApiKey(e.target.value)}
            className="w-full px-3 py-2 rounded-md bg-[#1e2030] border border-[#2a2d3e] text-sm text-white placeholder-muted outline-none focus:border-[#6c8cff] transition-colors"
          />
          <div className="flex gap-2">
            <label
              className={`flex-1 flex items-center gap-2 px-3 py-2 rounded-md border cursor-pointer text-sm text-white transition-colors ${
                format === "openai"
                  ? "border-[#6c8cff] bg-[#1e2030]"
                  : "border-[#2a2d3e] bg-[#1e2030]"
              }`}
            >
              <input
                type="radio"
                name="apiFormat"
                value="openai"
                checked={format === "openai"}
                onChange={() => setFormat("openai")}
                className="accent-[#6c8cff]"
              />
              OpenAI
            </label>
            <label
              className={`flex-1 flex items-center gap-2 px-3 py-2 rounded-md border cursor-pointer text-sm text-white transition-colors ${
                format === "anthropic"
                  ? "border-[#6c8cff] bg-[#1e2030]"
                  : "border-[#2a2d3e] bg-[#1e2030]"
              }`}
            >
              <input
                type="radio"
                name="apiFormat"
                value="anthropic"
                checked={format === "anthropic"}
                onChange={() => setFormat("anthropic")}
                className="accent-[#6c8cff]"
              />
              Anthropic
            </label>
          </div>
        </div>
        <div className="flex justify-end gap-2 mt-4">
          <button
            onClick={onClose}
            className="px-3.5 py-2 rounded-md bg-[#1e2030] text-muted text-xs font-medium border border-border hover:bg-border transition-all cursor-pointer"
          >
            Cancel
          </button>
          <button
            onClick={handleSubmit}
            className="px-3.5 py-2 rounded-md bg-[#6c8cff] text-white text-xs font-medium hover:bg-[#4a6adf] transition-all cursor-pointer"
          >
            Add
          </button>
        </div>
      </div>
    </div>
  );
}
