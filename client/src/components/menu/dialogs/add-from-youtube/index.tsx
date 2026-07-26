import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useDialogNav } from "@/hooks/navigation/use-dialog-nav";
import { useDialog } from "@/hooks/use-dialog";
import { useDownloadYoutubeMutation } from "@/mutations/use-download-youtube-mutation";
import { useSearchYoutubeMutation } from "@/mutations/use-search-youtube-mutation";
import type { YoutubeSearchResult } from "@/types/YoutubeSearchResult";
import { cn } from "@/lib/utils";
import { useRef, useState } from "react";
import { ARIA_DISABLED_CLASS, ringFor } from "../edit-lyrics/parts";
import { SearchResults } from "./search-results";

type AddYoutubeTab = "search" | "paste";

const isLikelyYoutubeUrl = (value: string): boolean => {
  try {
    const url = new URL(value.trim());
    if (url.protocol !== "https:") {
      return false;
    }

    const host = url.hostname.replace(/^www\./, "");
    const validId = (id: string): boolean => /^[A-Za-z0-9_-]{11}$/.test(id);
    if (host === "youtu.be") {
      const id = url.pathname.slice(1).split(/[/?#]/)[0];
      return validId(id);
    }

    if (host === "youtube.com" || host === "m.youtube.com" || host === "music.youtube.com") {
      return url.pathname === "/watch" && validId(url.searchParams.get("v") ?? "");
    }

    return false;
  } catch {
    return false;
  }
};

// Segment 0 is always the tabs row (2 slots: Search / Paste URL — kept as a
// single 2-slot segment so left/right switches tabs, matching the
// edit-lyrics header-row pattern). Every other focusable gets its own
// single-slot segment, since — unlike edit-lyrics's one-candidate carousel —
// each search result is an independent row with its own Download button.
function navStops(tab: AddYoutubeTab, resultCount: number): number[] {
  if (tab === "paste") {
    // tabs, url input, download button, cancel
    return [2, 1, 1, 1];
  }
  // tabs, [query input, search button], one segment per result, cancel
  return [2, 2, ...Array.from({ length: resultCount }, () => 1), 1];
}

export const AddFromYoutubeDialog = () => {
  const { mode, close } = useDialog();
  const open = mode === "add-from-youtube";

  const containerRef = useRef<HTMLDivElement>(null);
  const [tab, setTab] = useState<AddYoutubeTab>("search");
  const [query, setQuery] = useState("");
  const [pastedUrl, setPastedUrl] = useState("");

  const search = useSearchYoutubeMutation();
  const download = useDownloadYoutubeMutation();

  const startDownload = (url: string) => {
    download.mutate(url, { onSuccess: close });
  };

  const results = search.data ?? [];
  const resultCount = tab === "search" && search.isPending ? 0 : results.length;
  const stops = navStops(tab, resultCount);
  const footerSegment = stops.length - 1;
  const resultsStartSegment = 2;

  const { isFocused, focusSegment } = useDialogNav({
    open,
    itemCount: stops.reduce((sum, n) => sum + n, 0),
    stops,
    onBack: close,
    containerRef,
    onAction: (segment, slot, action) => {
      // Radix TabsTrigger doesn't respond to the hook's default click() —
      // same workaround as edit-lyrics/index.tsx.
      if (segment === 0 && action.confirm && slot < 2) {
        setTab(slot === 0 ? "search" : "paste");
        return true;
      }
      return false;
    },
  });

  if (!open) {
    return null;
  }

  return (
    <Dialog open={open} onOpenChange={(next) => !next && close()}>
      <DialogContent className="flex h-[70vh] flex-col sm:max-w-lg">
        <div ref={containerRef} className="contents">
          <DialogHeader>
            <DialogTitle>Add from YouTube</DialogTitle>
            <DialogDescription>
              Search for a song or paste a YouTube link. The video is downloaded and added to your
              library.
            </DialogDescription>
          </DialogHeader>

          <Tabs
            value={tab}
            onValueChange={(v) => setTab(v as AddYoutubeTab)}
            className="flex min-h-0 flex-1 flex-col"
          >
            <TabsList>
              <TabsTrigger
                value="search"
                onPointerDown={() => focusSegment(0, 0)}
                className={ringFor(isFocused(0, 0))}
              >
                Search
              </TabsTrigger>
              <TabsTrigger
                value="paste"
                onPointerDown={() => focusSegment(0, 1)}
                className={ringFor(isFocused(0, 1))}
              >
                Paste URL
              </TabsTrigger>
            </TabsList>

            <TabsContent value="search" className="mt-3 flex min-h-0 flex-1 flex-col gap-3">
              <div className="flex gap-2">
                <Input
                  placeholder="Song title or artist…"
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                  className={ringFor(isFocused(1, 0))}
                />
                <Button
                  aria-disabled={query.trim().length === 0 || search.isPending}
                  onClick={() => {
                    if (query.trim().length === 0 || search.isPending) return;
                    search.mutate(query.trim());
                  }}
                  className={cn(ARIA_DISABLED_CLASS, ringFor(isFocused(1, 1)))}
                >
                  Search
                </Button>
              </div>
              <SearchResults
                results={results}
                isSearching={search.isPending}
                isDownloading={download.isPending}
                onSelect={(result: YoutubeSearchResult) =>
                  startDownload(`https://www.youtube.com/watch?v=${result.id}`)
                }
                isFocused={(index) => isFocused(resultsStartSegment + index, 0)}
              />
            </TabsContent>

            <TabsContent value="paste" className="mt-3 flex flex-col gap-3">
              <Input
                placeholder="https://www.youtube.com/watch?v=…"
                value={pastedUrl}
                onChange={(e) => setPastedUrl(e.target.value)}
                className={ringFor(isFocused(1, 0))}
              />
              <Button
                aria-disabled={!isLikelyYoutubeUrl(pastedUrl) || download.isPending}
                onClick={() => {
                  if (!isLikelyYoutubeUrl(pastedUrl) || download.isPending) return;
                  startDownload(pastedUrl.trim());
                }}
                className={cn(ARIA_DISABLED_CLASS, ringFor(isFocused(2, 0)))}
              >
                {download.isPending ? "Downloading…" : "Download"}
              </Button>
            </TabsContent>
          </Tabs>

          <DialogFooter>
            <Button
              variant="outline"
              onClick={close}
              className={ringFor(isFocused(footerSegment, 0))}
            >
              Cancel
            </Button>
          </DialogFooter>
        </div>
      </DialogContent>
    </Dialog>
  );
};
