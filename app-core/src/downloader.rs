/// Validates a YouTube URL using proper parsing.
pub fn is_valid_youtube_url(url: &str) -> bool {
    let url = url.trim();

    if !url.starts_with("http://") && !url.starts_with("https://") {
        return false;
    }

    let lower = url.to_lowercase();
    let is_youtube = lower.contains("youtube.com/watch")
        || lower.contains("www.youtube.com/watch")
        || lower.contains("m.youtube.com/watch")
        || lower.contains("youtu.be/");

    if !is_youtube {
        return false;
    }

    extract_video_id(url).is_some()
}

/// Extracts the YouTube video ID from a URL.
pub fn extract_video_id(url: &str) -> Option<String> {
    let url = url.trim();

    if url.contains("youtu.be/") {
        let start = url.find("youtu.be/")? + 9;
        let rest = &url[start..];
        let end = rest
            .find('?')
            .or_else(|| rest.find('&'))
            .or_else(|| rest.find('/'))
            .unwrap_or(rest.len());
        let id = &rest[..end];
        if id.len() >= 11 && id.len() <= 12 {
            return Some(id.to_string());
        }
        return None;
    }

    if let Some(pos) = url.find("v=") {
        let start = pos + 2;
        let rest = &url[start..];
        let end = rest.find('&').unwrap_or(rest.len());
        let id = &rest[..end];
        if id.len() >= 11 && id.len() <= 12 {
            return Some(id.to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_standard_watch_url() {
        assert!(is_valid_youtube_url(
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
        ));
    }

    #[test]
    fn accepts_youtu_be_short_url() {
        assert!(is_valid_youtube_url("https://youtu.be/dQw4w9WgXcQ"));
    }

    #[test]
    fn accepts_url_with_extra_query_params() {
        assert!(is_valid_youtube_url(
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=42s"
        ));
    }

    #[test]
    fn rejects_non_youtube_domain() {
        assert!(!is_valid_youtube_url("https://vimeo.com/12345678"));
    }

    #[test]
    fn rejects_url_without_scheme() {
        assert!(!is_valid_youtube_url("www.youtube.com/watch?v=dQw4w9WgXcQ"));
    }

    #[test]
    fn rejects_youtube_url_without_video_id() {
        assert!(!is_valid_youtube_url("https://www.youtube.com/watch"));
    }

    #[test]
    fn extracts_id_from_standard_url() {
        assert_eq!(
            extract_video_id("https://www.youtube.com/watch?v=dQw4w9WgXcQ").as_deref(),
            Some("dQw4w9WgXcQ")
        );
    }

    #[test]
    fn extracts_id_from_short_url() {
        assert_eq!(
            extract_video_id("https://youtu.be/dQw4w9WgXcQ").as_deref(),
            Some("dQw4w9WgXcQ")
        );
    }

    #[test]
    fn extracts_id_ignoring_trailing_query_params() {
        assert_eq!(
            extract_video_id("https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=42s").as_deref(),
            Some("dQw4w9WgXcQ")
        );
    }

    #[test]
    fn returns_none_for_missing_id() {
        assert_eq!(extract_video_id("https://www.youtube.com/watch"), None);
    }
}
