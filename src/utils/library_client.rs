//! Talks to the sound pack library at `LIBRARY_BASE_URL`: looks a pack up and
//! downloads its zip, checked against the checksum the library publishes.

use crate::state::soundpack::SoundpackType;
use crate::utils::constants::{ APP_NAME, APP_VERSION, LIBRARY_BASE_URL };
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

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client
        ::builder()
        .user_agent(format!("{}/{}", APP_NAME, APP_VERSION))
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| format!("Could not start the download: {}", e))
}

/// Look a pack up in the library.
pub async fn fetch_pack(id: &str) -> Result<LibraryPack, String> {
    let url = format!("{}/api/sound-packs/{}", LIBRARY_BASE_URL, id);
    let response = client()?
        .get(&url)
        .send().await
        .map_err(|_| "Could not reach the sound pack library. Check your connection and try again.".to_string())?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err("This sound pack is not in the library any more.".to_string());
    }
    if !response.status().is_success() {
        return Err(format!("The sound pack library answered with an error ({}).", response.status()));
    }
    let pack = response
        .json::<PackResponse>().await
        .map_err(|_| "The sound pack library sent something this app cannot read.".to_string())?.pack;
    check_pack(id, pack)
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

    let path = std::env::temp_dir().join(format!("mechvibes-{}-r{}.zip", pack.id, pack.revision));
    let result = async {
        let mut file = std::fs::File
            ::create(&path)
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
}
