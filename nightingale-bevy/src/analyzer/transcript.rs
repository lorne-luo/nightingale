use serde::{Deserialize, Serialize};

fn default_source() -> String {
    "generated".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transcript {
    pub language: String,
    pub segments: Vec<Segment>,
    #[serde(default = "default_source")]
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    pub text: String,
    pub start: f64,
    pub end: f64,
    #[serde(default)]
    pub words: Vec<Word>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Word {
    pub word: String,
    pub start: f64,
    pub end: f64,
    #[serde(default)]
    pub score: Option<f64>,
    #[serde(default)]
    pub estimated: bool,
}

impl Transcript {
    pub fn load(path: &std::path::Path) -> Result<Self, crate::error::NightingaleError> {
        let data = std::fs::read_to_string(path)?;
        let transcript: Transcript = serde_json::from_str(&data)?;
        Ok(transcript)
    }

    /// Split long segments for better display.
    /// If words are available, split by word count.
    /// If words are empty (e.g., from Groq), split by duration threshold.
    pub fn split_long_segments(&mut self, max_words: usize) {
        let mut new_segments = Vec::new();
        const MAX_SEGMENT_DURATION_SECS: f64 = 8.0;

        for seg in &self.segments {
            // If we have words, use the word-based splitting
            if !seg.words.is_empty() {
                if seg.words.len() <= max_words {
                    new_segments.push(seg.clone());
                    continue;
                }
                for chunk in seg.words.chunks(max_words) {
                    let text = chunk
                        .iter()
                        .map(|w| w.word.as_str())
                        .collect::<Vec<_>>()
                        .join(" ");
                    new_segments.push(Segment {
                        text,
                        start: chunk.first().unwrap().start,
                        end: chunk.last().unwrap().end,
                        words: chunk.to_vec(),
                    });
                }
            } else {
                // No words available - split by duration if segment is too long
                let duration = seg.end - seg.start;
                if duration <= MAX_SEGMENT_DURATION_SECS {
                    new_segments.push(seg.clone());
                } else {
                    // Split long segments by time into roughly equal parts
                    let num_parts = (duration / MAX_SEGMENT_DURATION_SECS).ceil() as usize;
                    let part_duration = duration / num_parts as f64;

                    // Try to split on sentence boundaries if possible
                    let sub_texts = Self::split_text_by_punctuation(&seg.text, num_parts);

                    for (i, text) in sub_texts.into_iter().enumerate() {
                        let start = seg.start + (i as f64) * part_duration;
                        let end = if i == num_parts - 1 {
                            seg.end
                        } else {
                            seg.start + ((i + 1) as f64) * part_duration
                        };
                        new_segments.push(Segment {
                            text: text.trim().to_string(),
                            start,
                            end,
                            words: vec![],
                        });
                    }
                }
            }
        }
        self.segments = new_segments;
    }

    /// Split text into parts, preferring sentence boundaries
    fn split_text_by_punctuation(text: &str, num_parts: usize) -> Vec<String> {
        if num_parts <= 1 {
            return vec![text.to_string()];
        }

        // Find all sentence-ending positions
        let sentence_enders = ['.', '!', '?', '。', '！', '？', '\n'];
        let mut split_points: Vec<usize> = text
            .char_indices()
            .filter(|(_, c)| sentence_enders.contains(c))
            .map(|(i, _)| i + 1)
            .collect();

        // If not enough sentence boundaries, use character count
        if split_points.len() < num_parts - 1 {
            let chars_per_part = text.chars().count() / num_parts;
            split_points = (1..num_parts)
                .map(|i| {
                    let target = i * chars_per_part;
                    text.char_indices()
                        .nth(target)
                        .map(|(i, _)| i)
                        .unwrap_or(text.len())
                })
                .collect();
        }

        // Sort and take the best split points
        split_points.sort();
        split_points.truncate(num_parts - 1);

        // Split the text at the chosen points
        let mut parts = Vec::new();
        let mut last_pos = 0;
        for pos in split_points {
            if pos > last_pos && pos < text.len() {
                parts.push(text[last_pos..pos].trim().to_string());
                last_pos = pos;
            }
        }
        if last_pos < text.len() {
            parts.push(text[last_pos..].trim().to_string());
        }

        // If we didn't get enough parts, return the original text
        if parts.is_empty() {
            vec![text.to_string()]
        } else {
            parts
        }
    }
}
