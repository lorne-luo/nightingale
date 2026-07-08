import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { YoutubeSearchResult } from "@/types/YoutubeSearchResult";
import { ARIA_DISABLED_CLASS, ringFor } from "../edit-lyrics/parts";

const formatDuration = (seconds: number): string => {
  const total = Math.round(seconds);
  const mins = Math.floor(total / 60);
  const secs = total % 60;
  return `${mins}:${secs.toString().padStart(2, "0")}`;
};

interface SearchResultsProps {
  results: YoutubeSearchResult[];
  isSearching: boolean;
  isDownloading: boolean;
  onSelect: (result: YoutubeSearchResult) => void;
  isFocused: (index: number) => boolean;
}

export const SearchResults = ({
  results,
  isSearching,
  isDownloading,
  onSelect,
  isFocused,
}: SearchResultsProps) => {
  if (isSearching) {
    return <p className="text-muted-foreground">Searching YouTube…</p>;
  }

  if (results.length === 0) {
    return <p className="text-muted-foreground">No results yet. Try searching above.</p>;
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto">
      {results.map((result, index) => (
        <div
          key={result.id}
          className="flex items-center gap-3 rounded-md border border-border bg-card p-2"
        >
          <div className="flex min-w-0 flex-1 flex-col">
            <span className="truncate text-sm font-medium">{result.title}</span>
            <span className="truncate text-xs text-muted-foreground">
              {result.channel} • {formatDuration(result.duration_secs)}
            </span>
          </div>
          <Button
            size="xs"
            variant="default"
            aria-disabled={isDownloading}
            onClick={() => {
              if (isDownloading) return;
              onSelect(result);
            }}
            className={cn(ARIA_DISABLED_CLASS, ringFor(isFocused(index)))}
          >
            Download
          </Button>
        </div>
      ))}
    </div>
  );
};
