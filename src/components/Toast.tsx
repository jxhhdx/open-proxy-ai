export default function Toast({ message }: { message: string }) {
  if (!message) return null;
  return (
    <div className="fixed bottom-6 left-1/2 -translate-x-1/2 bg-[#1e2030] border border-[#2a2d3e] px-5 py-2.5 rounded-md text-sm text-white z-50 transition-opacity duration-250">
      {message}
    </div>
  );
}
