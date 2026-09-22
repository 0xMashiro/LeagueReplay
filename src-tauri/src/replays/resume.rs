use reqwest::{Response, StatusCode};
use std::path::Path;

pub struct Resume {
    pub offset: u64,
    pub validator: String,
}
pub fn load(part: &Path, meta: &Path) -> Option<Resume> {
    let offset = std::fs::metadata(part).ok()?.len();
    let validator = std::fs::read_to_string(meta).ok()?;
    (offset > 0 && valid_validator(&validator)).then_some(Resume { offset, validator })
}
fn valid_validator(value: &str) -> bool {
    value.starts_with('"')
        && value.ends_with('"')
        && value.len() <= 1024
        && !value.contains(['\r', '\n'])
}
pub fn save_validator(response: &Response, path: &Path) -> Result<(), String> {
    if let Some(value) = response
        .headers()
        .get("etag")
        .and_then(|v| v.to_str().ok())
        .filter(|v| valid_validator(v))
    {
        super::storage::atomic_write(path, value.as_bytes())
    } else {
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err("replay.storageError".into()),
        }
    }
}
pub fn response(
    response: &Response,
    resume: Option<&Resume>,
) -> Result<(u64, Option<u64>), String> {
    if response.status() == StatusCode::OK {
        return Ok((0, response.content_length()));
    }
    if response.status() != StatusCode::PARTIAL_CONTENT {
        return Err("replay.incomplete".into());
    }
    let resume = resume.ok_or("replay.incomplete")?;
    let range = response
        .headers()
        .get("content-range")
        .and_then(|v| v.to_str().ok())
        .ok_or("replay.incomplete")?;
    let total = parse_range(range, resume.offset)?;
    if response.headers().get("etag").and_then(|v| v.to_str().ok())
        != Some(resume.validator.as_str())
    {
        return Err("replay.incomplete".into());
    }
    Ok((resume.offset, Some(total)))
}
fn parse_range(value: &str, offset: u64) -> Result<u64, String> {
    let parse = || {
        let (range, total) = value.strip_prefix("bytes ")?.split_once('/')?;
        let (start, end) = range.split_once('-')?;
        let (start, end, total) = (
            start.parse::<u64>().ok()?,
            end.parse::<u64>().ok()?,
            total.parse::<u64>().ok()?,
        );
        (start == offset && start <= end && end.checked_add(1) == Some(total)).then_some(total)
    };
    parse().ok_or("replay.incomplete".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_wrong_ranges_and_weak_validators() {
        assert_eq!(parse_range("bytes 40-99/100", 40).unwrap(), 100);
        for range in [
            "bytes 0-99/100",
            "bytes 40-60/100",
            "bytes 40-100/100",
            "bytes 40-99/*",
        ] {
            assert!(parse_range(range, 40).is_err());
        }
        assert!(!valid_validator("W/\"abc\""));
        assert!(valid_validator("\"abc\""));
    }
}
