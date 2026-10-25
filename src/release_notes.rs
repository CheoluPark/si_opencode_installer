use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseNotes {
    pub schema: u32,
    pub version: u64,
    pub title: String,
    pub summary: String,
    pub items: Vec<ReleaseNote>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseNote {
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

impl ReleaseNotes {
    pub fn parse(bytes: &[u8], version: u64) -> Result<Self, String> {
        let notes: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if notes.schema != 1
            || notes.version != version
            || notes.title.trim().is_empty()
            || notes.items.len() > 20
        {
            return Err("Invalid release notes metadata".into());
        }
        for item in &notes.items {
            if item.title.trim().is_empty() || item.body.len() > 20_000 {
                return Err("Invalid release note item".into());
            }
            if let Some(image) = &item.image {
                let ok = [".png", ".jpg", ".jpeg", ".webp"]
                    .iter()
                    .any(|ext| image.to_ascii_lowercase().ends_with(ext));
                if !image.starts_with("release-notes/") || !ok {
                    return Err("Invalid release note image".into());
                }
            }
            if let Some(url) = &item.url {
                let parsed = reqwest::Url::parse(url).map_err(|_| "Invalid release note URL")?;
                if parsed.scheme() != "https" || parsed.host_str() != Some("file.satreci.com") {
                    return Err("Release note URL must use file.satreci.com HTTPS".into());
                }
            }
        }
        Ok(notes)
    }
}

#[cfg(test)]
mod tests {
    use super::ReleaseNotes;

    #[test]
    fn accepts_signed_release_note_contract() {
        let notes = ReleaseNotes::parse(br#"{"schema":1,"version":7,"title":"Update","summary":"Ready","items":[{"title":"KB","body":"Search","image":"release-notes/kb.png","url":"https://file.satreci.com/help/kb"}]}"#, 7).unwrap();
        assert_eq!(notes.items.len(), 1);
    }

    #[test]
    fn rejects_untrusted_links_and_images() {
        assert!(ReleaseNotes::parse(br#"{"schema":1,"version":7,"title":"x","summary":"x","items":[{"title":"x","body":"x","url":"https://evil.example/help"}]}"#, 7).is_err());
        assert!(ReleaseNotes::parse(br#"{"schema":1,"version":7,"title":"x","summary":"x","items":[{"title":"x","body":"x","image":"release-notes/x.svg"}]}"#, 7).is_err());
    }
}
