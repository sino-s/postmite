const methodClasses: Record<string, string> = {
  GET: "method-color-get border-emerald-300 bg-emerald-50 text-emerald-800",
  POST: "method-color-post border-sky-300 bg-sky-50 text-sky-800",
  PUT: "method-color-put border-amber-300 bg-amber-50 text-amber-900",
  PATCH: "method-color-patch border-violet-300 bg-violet-50 text-violet-800",
  DELETE: "method-color-delete border-red-300 bg-red-50 text-red-800",
  HEAD: "method-color-head border-slate-300 bg-slate-50 text-slate-800",
  OPTIONS: "method-color-options border-cyan-300 bg-cyan-50 text-cyan-800",
};

export function methodColorClass(method: string) {
  return methodClasses[method.toUpperCase()] ?? "method-color-other border-zinc-300 bg-zinc-50 text-zinc-800";
}
