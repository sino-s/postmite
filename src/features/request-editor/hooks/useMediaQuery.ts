import { useEffect, useState } from "react";

export function useMediaQuery(query: string, defaultMatches: boolean) {
  const [matches, setMatches] = useState(() =>
    typeof window === "undefined" ? defaultMatches : window.matchMedia(query).matches,
  );

  useEffect(() => {
    const media = window.matchMedia(query);
    setMatches(media.matches);
    const onChange = (event: MediaQueryListEvent) => setMatches(event.matches);
    media.addEventListener("change", onChange);
    return () => media.removeEventListener("change", onChange);
  }, [query]);

  return matches;
}
