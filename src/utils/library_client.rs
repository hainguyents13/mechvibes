//! Talks to the sound pack and theme libraries at `LIBRARY_BASE_URL`: looks a
//! pack or theme up and downloads its file, checked against the checksum the
//! library publishes.

use crate::state::soundpack::SoundpackType;
use crate::utils::constants::{ APP_NAME, APP_VERSION, LIBRARY_BASE_URL };
use crate::utils::theme_share;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::io::Write;
use std::path::PathBuf;

/// Larger than any pack the library takes (5 MB for uploads), so it only
/// stops a download that would never end.
const MAX_PACK_BYTES: u64 = 50 * 1024 * 1024;

/// What the install dialog shows before anything is downloaded.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryPack {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub author: Option<String>,
    pub category: String,
    pub revision: u64,
    pub checksum: String,
    pub zip_size: u64,
}

impl LibraryPack {
    /// Where the pack installs. The library only has these two categories.
    pub fn soundpack_type(&self) -> Option<SoundpackType> {
        match self.category.as_str() {
            "keyboard" => Some(SoundpackType::Keyboard),
            "mouse" => Some(SoundpackType::Mouse),
            _ => None,
        }
    }
}

#[derive(Deserialize)]
struct PackResponse {
    pack: LibraryPack,
}

/// What the theme dialog shows before anything is downloaded.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryTheme {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub author: Option<String>,
    pub colors_label: String,
    pub has_logo: bool,
    pub has_background: bool,
    pub size: u64,
    pub checksum: String,
}

#[derive(Deserialize)]
struct ThemeResponse {
    theme: LibraryTheme,
}

pub(crate) fn client() -> Result<reqwest::Client, String> {
    reqwest::Client
        ::builder()
        .user_agent(format!("{}/{}", APP_NAME, APP_VERSION))
        // Per step rather than for the whole request, so a slow connection
        // still finishes a large pack as long as data keeps arriving.
        .connect_timeout(std::time::Duration::from_secs(15))
        .read_timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("Could not start the download: {}", e))
}

/// GET `path` from the library and read the details it answers with. `what`
/// names the thing for the messages: "sound pack" or "theme".
async fn fetch_details<T: DeserializeOwned>(path: &str, what: &str) -> Result<T, String> {
    let url = format!("{}{}", LIBRARY_BASE_URL, path);
    let response = client()?
        .get(&url)
        .send().await
        .map_err(|_| format!("Could not reach the {} library. Check your connection and try again.", what))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(format!("This {} is not in the library any more.", what));
    }
    if !response.status().is_success() {
        return Err(format!("The {} library answered with an error ({}).", what, response.status()));
    }
    response
        .json::<T>().await
        .map_err(|_| format!("The {} library sent something this app cannot read.", what))
}

/// Look a pack up in the library.
pub async fn fetch_pack(id: &str) -> Result<LibraryPack, String> {
    let response: PackResponse = fetch_details(&format!("/api/sound-packs/{}", id), "sound pack").await?;
    check_pack(id, response.pack)
}

/// Look a theme up in the library.
pub async fn fetch_theme(id: &str) -> Result<LibraryTheme, String> {
    let response: ThemeResponse = fetch_details(&format!("/api/themes/{}", id), "theme").await?;
    check_theme(id, response.theme)
}

/// The details have to describe the theme that was asked for, at a size a
/// theme file can have.
fn check_theme(id: &str, theme: LibraryTheme) -> Result<LibraryTheme, String> {
    if theme.id != id {
        return Err("The theme library sent a different theme than the one asked for.".to_string());
    }
    if theme.size > theme_share::MAX_FILE_BYTES {
        return Err("This theme is too large to install.".to_string());
    }
    Ok(theme)
}

/// Download the theme's file into memory and check it against the published
/// checksum. Opening it as a theme is left to `theme_share`.
pub async fn download_theme(theme: &LibraryTheme) -> Result<Vec<u8>, String> {
    use sha2::{ Digest, Sha256 };

    let url = format!("{}/api/themes/{}/download", LIBRARY_BASE_URL, theme.id);
    let mut response = client()?
        .get(&url)
        .send().await
        .map_err(|_| "Could not download the theme. Check your connection and try again.".to_string())?;
    if !response.status().is_success() {
        return Err(format!("Could not download the theme ({}).", response.status()));
    }
    let mut bytes = Vec::new();
    while
        let Some(chunk) = response
            .chunk().await
            .map_err(|_| "The download was interrupted. Try again.".to_string())?
    {
        bytes.extend_from_slice(&chunk);
        if (bytes.len() as u64) > theme_share::MAX_FILE_BYTES {
            return Err("This theme is too large to install.".to_string());
        }
    }
    let actual = crate::utils::update_installer::hex_encode(&Sha256::digest(&bytes));
    if actual != theme.checksum.trim().to_lowercase() {
        return Err("The downloaded theme is damaged. Try again.".to_string());
    }
    Ok(bytes)
}

/// The details have to describe the pack that was asked for, in a form this
/// app can install.
fn check_pack(id: &str, pack: LibraryPack) -> Result<LibraryPack, String> {
    if pack.id != id {
        return Err("The sound pack library sent a different pack than the one asked for.".to_string());
    }
    if pack.soundpack_type().is_none() {
        return Err(format!("This app cannot install a '{}' sound pack.", pack.category));
    }
    if pack.zip_size > MAX_PACK_BYTES {
        return Err("This sound pack is too large to install.".to_string());
    }
    Ok(pack)
}

/// Download the pack's zip to a temporary file and check it against the
/// published checksum. The file is removed again if anything is wrong.
pub async fn download_pack(pack: &LibraryPack) -> Result<PathBuf, String> {
    let url = format!("{}/api/sound-packs/{}/download", LIBRARY_BASE_URL, pack.id);
    let mut response = client()?
        .get(&url)
        .send().await
        .map_err(|_| "Could not download the sound pack. Check your connection and try again.".to_string())?;
    if !response.status().is_success() {
        return Err(format!("Could not download the sound pack ({}).", response.status()));
    }

    let dir = crate::state::paths::data::library_downloads_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("Could not save the download: {}", e))?;
    let path = dir.join(format!("{}-{}.zip", pack.id, uuid::Uuid::new_v4()));
    let result = async {
        let mut file = std::fs::File
            ::options()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| format!("Could not save the download: {}", e))?;
        let mut written: u64 = 0;
        while
            let Some(chunk) = response
                .chunk().await
                .map_err(|_| "The download was interrupted. Try again.".to_string())?
        {
            written += chunk.len() as u64;
            if written > MAX_PACK_BYTES {
                return Err("This sound pack is too large to install.".to_string());
            }
            file.write_all(&chunk).map_err(|e| format!("Could not save the download: {}", e))?;
        }
        drop(file);
        crate::utils::update_installer
            ::verify_file_sha256(&path, &pack.checksum)
            .map_err(|_| "The downloaded sound pack is damaged. Try again.".to_string())
    }.await;

    match result {
        Ok(()) => Ok(path),
        Err(e) => {
            let _ = std::fs::remove_file(&path);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "mv-8ac44d88301f4699b771df00587a9246";

    /// The shape `GET /api/sound-packs/:id` answers with, trimmed from a real
    /// response.
    const RESPONSE: &str =
        r#"{"pack":{"id":"mv-8ac44d88301f4699b771df00587a9246","slug":"asmr-programming-soundpack","name":"ASMR Programming Soundpack","category":"keyboard","author":"luv.sora","revision":1,"checksum":"6624a1a4f6e699d78e72e7c58ac2ca114909563dc8e8472a5506e6acc40493c1","zipSize":428304,"downloads":19}}"#;

    fn pack() -> LibraryPack {
        serde_json::from_str::<PackResponse>(RESPONSE).expect("real response").pack
    }

    #[test]
    fn a_real_response_reads_into_the_dialogs_details() {
        let pack = check_pack(ID, pack()).expect("valid pack");
        assert_eq!(pack.name, "ASMR Programming Soundpack");
        assert_eq!(pack.author.as_deref(), Some("luv.sora"));
        assert_eq!(pack.revision, 1);
        assert_eq!(pack.zip_size, 428304);
        assert_eq!(pack.soundpack_type(), Some(SoundpackType::Keyboard));
    }

    #[test]
    fn details_for_another_pack_or_category_are_refused() {
        assert!(check_pack("mv-00000000000000000000000000000000", pack()).is_err());

        let mut odd = pack();
        odd.category = "ambient".to_string();
        assert!(check_pack(ID, odd).is_err());

        let mut huge = pack();
        huge.zip_size = MAX_PACK_BYTES + 1;
        assert!(check_pack(ID, huge).is_err());
    }

    const THEME_ID: &str = "th-0123456789abcdef0123456789abcdef";

    /// The shape `GET /api/themes/:id` answers with, from the library's API doc.
    const THEME_RESPONSE: &str =
        r##"{"theme":{"id":"th-0123456789abcdef0123456789abcdef","slug":"night-drive","name":"Night drive","description":"Dark and pink.","author":"Hai","owner":{"name":"Hai","image":null,"providers":["github"]},"builtInTheme":null,"colorsLabel":"Custom colors","displayColors":["#101010"],"hasLogo":true,"hasBackground":false,"size":812345,"checksum":"9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08","previewUrl":null,"downloads":42,"createdAt":"2026-10-31T08:00:00.000Z"}}"##;

    fn theme() -> LibraryTheme {
        serde_json::from_str::<ThemeResponse>(THEME_RESPONSE).expect("documented response").theme
    }

    #[test]
    fn a_theme_response_reads_into_the_dialogs_details() {
        let theme = check_theme(THEME_ID, theme()).expect("valid theme");
        assert_eq!(theme.name, "Night drive");
        assert_eq!(theme.author.as_deref(), Some("Hai"));
        assert_eq!(theme.colors_label, "Custom colors");
        assert!(theme.has_logo && !theme.has_background);
        assert_eq!(theme.size, 812345);
    }

    #[test]
    fn details_for_another_or_oversized_theme_are_refused() {
        assert!(check_theme("th-00000000000000000000000000000000", theme()).is_err());

        let mut huge = theme();
        huge.size = theme_share::MAX_FILE_BYTES + 1;
        assert!(check_theme(THEME_ID, huge).is_err());
    }
}
