import { downloadYoutubeSong } from "@/bridge/downloader";
import { ANALYSIS_QUEUE, MENU, SONGS, SONGS_META } from "@/queries/keys";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

export const useDownloadYoutubeMutation = () => {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (url: string) => downloadYoutubeSong(url),
    onSuccess: () => {
      toast.info("Downloading song from YouTube…");
      queryClient.invalidateQueries({ queryKey: MENU });
      queryClient.invalidateQueries({ queryKey: SONGS });
      queryClient.invalidateQueries({ queryKey: SONGS_META });
      queryClient.invalidateQueries({ queryKey: ANALYSIS_QUEUE });
    },
    onError: (error: Error) => {
      toast.error(`Error downloading from YouTube: ${error.message}`);
    },
  });
};
