import type { YoutubeSearchResult } from "@/types/YoutubeSearchResult";
import { invoke } from "./runtime";

export const searchYoutubeSongs = async (query: string): Promise<YoutubeSearchResult[]> => {
  return await invoke<YoutubeSearchResult[]>("search_youtube_songs", { query });
};

export const downloadYoutubeSong = async (url: string): Promise<void> => {
  return await invoke<void>("download_youtube_song", { url });
};
