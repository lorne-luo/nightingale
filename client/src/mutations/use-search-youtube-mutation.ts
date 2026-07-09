import { searchYoutubeSongs } from "@/bridge/downloader";
import { YOUTUBE_SEARCH } from "@/queries/keys";
import { useMutation } from "@tanstack/react-query";
import { toast } from "sonner";

export const useSearchYoutubeMutation = () => {
  return useMutation({
    mutationKey: YOUTUBE_SEARCH,
    mutationFn: (query: string) => searchYoutubeSongs(query),
    onError: (error: Error) => {
      toast.error(`Error searching YouTube: ${error.message}`);
    },
  });
};
