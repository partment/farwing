//! Current official behavior baseline. Capability negotiation remains explicit.
use std::sync::{OnceLock, RwLock};

pub(crate) const CLIENT: &str = "4.48.1.5606";
pub(crate) const SDK: &str = "V4.14.2";
pub(crate) const CAPTURE_SETTING_LEVEL: u32 = 6;
pub(crate) const CAPTURE_SETTING_RPC_MIN_LEVEL: u32 = 1;
pub(crate) const CUSTOM_BITRATE_MIN_LEVEL: u32 = 4;

fn active_client() -> &'static RwLock<String> {
    static ACTIVE: OnceLock<RwLock<String>> = OnceLock::new();
    ACTIVE.get_or_init(|| RwLock::new(CLIENT.to_owned()))
}

/// Effective official client version sent on the wire (`X-Param-VN` / `X-Param-VC` and
/// `ConnectOptions`). Starts at [`CLIENT`] and only moves forward when a newer official Windows
/// build is seen.
pub(crate) fn client() -> String {
    active_client()
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

/// Adopt `candidate` if it is a valid 4-part version strictly newer than the active one.
pub(crate) fn update_client(candidate: &str) -> bool {
    let mut guard = active_client()
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if !is_newer_version(&guard, candidate) {
        return false;
    }
    guard.clear();
    guard.push_str(candidate);
    true
}

/// Extract a 4-part Windows client version (e.g. `4.48.4.6020`) from the official installer
/// redirect `Location` URL. Rejects anything older than the compiled baseline [`CLIENT`].
pub(crate) fn parse_release_redirect(location: &str) -> Option<String> {
    let without_query = location.split(['?', '#']).next()?;
    let file_name = without_query.rsplit('/').next()?;
    let stem = file_name
        .strip_suffix(".exe")
        .or_else(|| file_name.strip_suffix(".EXE"))
        .unwrap_or(file_name);
    let baseline = parse_four_part(CLIENT)?;
    stem.split(['_', '-']).find_map(|token| {
        let parsed = parse_four_part(token)?;
        (parsed >= baseline).then(|| token.to_owned())
    })
}

fn is_newer_version(current: &str, candidate: &str) -> bool {
    let Some(next) = parse_four_part(candidate) else {
        return false;
    };
    parse_four_part(current).is_none_or(|cur| next > cur)
}

fn parse_four_part(text: &str) -> Option<[u32; 4]> {
    let mut parts = [0u32; 4];
    let mut count = 0;
    for piece in text.split('.') {
        if count == 4 || piece.is_empty() || !piece.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        parts[count] = piece.parse().ok()?;
        count += 1;
    }
    (count == 4).then_some(parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_official_release_redirect_version() {
        let url = "https://a56.gdl.netease.com/UURemote_Setup_4.48.4.6020_1124140237_gwqd.exe?key1=abc&n=uuyc_4.48.4.exe";
        assert_eq!(parse_release_redirect(url).as_deref(), Some("4.48.4.6020"));
        assert_eq!(
            parse_release_redirect("https://1.2.3.4/UURemote_Setup_4.41.2.2602_gwqd.exe"),
            None
        );
        assert_eq!(
            parse_release_redirect("https://a56.gdl.netease.com/uuyc_4.48.4.exe"),
            None
        );
    }

    #[test]
    fn version_updates_are_strictly_monotonic() {
        assert!(is_newer_version("4.48.1.5606", "4.48.4.6020"));
        assert!(is_newer_version("4.48.4.6020", "4.49.0.1"));
        assert!(!is_newer_version("4.48.4.6020", "4.48.4.6020"));
        assert!(!is_newer_version("4.48.4.6020", "4.48.1.5606"));
        assert!(!is_newer_version("4.48.1.5606", "4.48.4"));
        assert!(!is_newer_version("4.48.1.5606", "4.48.4.6020\r\n"));
    }
}
