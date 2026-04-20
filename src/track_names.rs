pub(crate) const MAX_TRACK_NAME_LEN: usize = 48;

pub(crate) fn default_track_name(track_index: usize) -> String {
    format!("Track {}", track_index + 1)
}

pub(crate) fn normalized_track_name_override(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    Some(trimmed.chars().take(MAX_TRACK_NAME_LEN).collect())
}

pub(crate) fn effective_track_name(
    track_index: usize,
    midi_name: Option<&str>,
    user_override: Option<&str>,
) -> String {
    if let Some(name) = user_override.map(str::trim).filter(|name| !name.is_empty()) {
        return name.to_string();
    }

    if let Some(name) = midi_name.map(str::trim).filter(|name| !name.is_empty()) {
        return name.to_string();
    }

    default_track_name(track_index)
}

pub(crate) fn ellipsize_middle(value: &str, max_len: usize) -> String {
    let len = value.chars().count();
    if len <= max_len {
        return value.to_string();
    }

    if max_len <= 1 {
        return "…".to_string();
    }

    let head_len = (max_len - 1) / 2;
    let tail_len = max_len - 1 - head_len;
    let head: String = value.chars().take(head_len).collect();
    let tail: String = value
        .chars()
        .rev()
        .take(tail_len)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!("{head}…{tail}")
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_TRACK_NAME_LEN, default_track_name, effective_track_name, ellipsize_middle,
        normalized_track_name_override,
    };

    #[test]
    fn effective_name_priority_is_default_then_midi_then_user() {
        assert_eq!(effective_track_name(2, None, None), "Track 3");
        assert_eq!(effective_track_name(2, Some("Flute"), None), "Flute");
        assert_eq!(
            effective_track_name(2, Some("Flute"), Some("Lead Flute")),
            "Lead Flute"
        );
    }

    #[test]
    fn override_is_trimmed_and_limited() {
        assert_eq!(normalized_track_name_override("   "), None);
        let value = "x".repeat(MAX_TRACK_NAME_LEN + 8);
        let normalized = normalized_track_name_override(&value).expect("override should exist");
        assert_eq!(normalized.chars().count(), MAX_TRACK_NAME_LEN);
    }

    #[test]
    fn middle_ellipsis_keeps_start_and_end() {
        assert_eq!(ellipsize_middle("Contrabass Section", 10), "Cont…ction");
        assert_eq!(ellipsize_middle("Violin", 10), "Violin");
        assert_eq!(default_track_name(0), "Track 1");
    }
}
