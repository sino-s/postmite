import type { ReactNode } from "react";

export function IconButton({
  children,
  label,
  onClick,
}: {
  children: ReactNode;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      aria-label={label}
      className="inline-flex h-7 w-7 items-center justify-center rounded-md text-slate-600 hover:bg-white focus-visible:outline focus-visible:outline-2 focus-visible:outline-sky-500"
      onClick={onClick}
      title={label}
      type="button"
    >
      {children}
    </button>
  );
}
