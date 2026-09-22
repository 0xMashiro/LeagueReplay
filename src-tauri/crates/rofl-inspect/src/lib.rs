// Adapted from leaguedirectorX / League Replay Studio rofl-inspect.
// Copyright (c) 2026 League Replay Studio Contributors. MIT license: ../LICENSE.
// Standalone container inspection only; optional match-summary projection omitted.
#![deny(unreachable_pub)]
#![forbid(unsafe_code)]

use std::io::{self, Cursor, Read, Seek, SeekFrom};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

const SIGNATURE_LEN: usize = 6;
const LEGACY_HEADER_FIELDS_OFFSET: u64 = 262;
const LEGACY_MIN_HEADER_LEN: u64 = 288;
const LEGACY_PAYLOAD_PREFIX_LEN: u64 = 34;
const ROFL2_VERSION_LENGTH_OFFSET: u64 = 14;
const ROFL2_VERSION_OFFSET: u64 = 15;

/// Increment when an inspector change should re-analyze persisted summaries.
pub const PARSER_VERSION: u16 = 3;
const METADATA_FOOTER_LEN: u64 = 4;
const DEFAULT_MAX_METADATA_BYTES: usize = 8 * 1024 * 1024;

/// Resource limits applied before allocating data declared by an untrusted replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InspectOptions {
    pub max_metadata_bytes: usize,
}

impl Default for InspectOptions {
    fn default() -> Self {
        Self {
            max_metadata_bytes: DEFAULT_MAX_METADATA_BYTES,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ReplayGeneration {
    Legacy,
    Rofl2,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayInspection {
    pub generation: ReplayGeneration,
    pub game_version: String,
    pub metadata: ReplayMetadata,
    pub raw_metadata: Map<String, Value>,
    pub layout: ReplayLayout,
    pub legacy_payload: Option<LegacyPayloadSummary>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayMetadata {
    pub game_length_ms: u64,
    pub last_game_chunk_id: u32,
    pub last_keyframe_id: u32,
    pub participants: Vec<Map<String, Value>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyPayloadSummary {
    pub game_id: u64,
    pub game_length_ms: u32,
    pub keyframe_count: u32,
    pub chunk_count: u32,
    pub end_startup_chunk_id: u32,
    pub start_game_chunk_id: u32,
    pub keyframe_interval_ms: u32,
    pub encryption_key_length: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ByteRange {
    pub offset: u64,
    pub length: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayLayout {
    pub file_size: u64,
    pub header: ByteRange,
    pub metadata: ByteRange,
    pub payload_header: Option<ByteRange>,
    pub payload: ByteRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectErrorKind {
    Io,
    Truncated,
    InvalidSignature,
    UnsupportedGeneration,
    InvalidLayout,
    MetadataTooLarge,
    InvalidGameVersion,
    InvalidMetadata,
}

#[derive(Debug, Error)]
pub enum InspectError {
    #[error("replay input/output failed")]
    Io(#[source] io::Error),
    #[error("replay is truncated while reading {section}")]
    Truncated { section: &'static str },
    #[error("input does not have a RIOT replay signature")]
    InvalidSignature,
    #[error("RIOT replay generation {major}.{minor} is not supported")]
    UnsupportedGeneration { major: u8, minor: u8 },
    #[error("replay has an invalid {section} layout")]
    InvalidLayout { section: &'static str },
    #[error("replay metadata length {declared} exceeds limit {limit}")]
    MetadataTooLarge { declared: u64, limit: usize },
    #[error("replay game version is missing or invalid")]
    InvalidGameVersion,
    #[error("replay metadata JSON is invalid")]
    InvalidMetadataJson(#[source] serde_json::Error),
    #[error("replay participant metadata JSON is invalid")]
    InvalidParticipantJson(#[source] serde_json::Error),
}

impl InspectError {
    #[must_use]
    pub const fn kind(&self) -> InspectErrorKind {
        match self {
            Self::Io(_) => InspectErrorKind::Io,
            Self::Truncated { .. } => InspectErrorKind::Truncated,
            Self::InvalidSignature => InspectErrorKind::InvalidSignature,
            Self::UnsupportedGeneration { .. } => InspectErrorKind::UnsupportedGeneration,
            Self::InvalidLayout { .. } => InspectErrorKind::InvalidLayout,
            Self::MetadataTooLarge { .. } => InspectErrorKind::MetadataTooLarge,
            Self::InvalidGameVersion => InspectErrorKind::InvalidGameVersion,
            Self::InvalidMetadataJson(_) | Self::InvalidParticipantJson(_) => {
                InspectErrorKind::InvalidMetadata
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMetadata {
    game_length: u64,
    #[serde(default)]
    game_version: Option<String>,
    last_game_chunk_id: u32,
    #[serde(rename = "lastKeyFrameId")]
    last_keyframe_id: u32,
    stats_json: String,
}

struct ParsedMetadata {
    game_version: Option<String>,
    metadata: ReplayMetadata,
    raw_metadata: Map<String, Value>,
}

/// Inspects a seekable replay without decoding or decrypting its payload.
pub fn inspect<R: Read + Seek>(
    reader: &mut R,
    options: &InspectOptions,
) -> Result<ReplayInspection, InspectError> {
    let file_len = reader.seek(SeekFrom::End(0)).map_err(InspectError::Io)?;
    let signature = read_array_at::<SIGNATURE_LEN, _>(reader, 0, "signature")?;
    if &signature[..4] != b"RIOT" {
        return Err(InspectError::InvalidSignature);
    }

    match (signature[4], signature[5]) {
        (0, 0) => inspect_legacy(reader, file_len, options),
        (2, 0) => inspect_rofl2(reader, file_len, options),
        (major, minor) => Err(InspectError::UnsupportedGeneration { major, minor }),
    }
}

/// Convenience wrapper for callers that already hold the complete replay in memory.
pub fn inspect_bytes(
    bytes: &[u8],
    options: &InspectOptions,
) -> Result<ReplayInspection, InspectError> {
    inspect(&mut Cursor::new(bytes), options)
}

fn inspect_legacy<R: Read + Seek>(
    reader: &mut R,
    file_len: u64,
    options: &InspectOptions,
) -> Result<ReplayInspection, InspectError> {
    let fields = read_array_at::<26, _>(
        reader,
        LEGACY_HEADER_FIELDS_OFFSET,
        "legacy container header",
    )?;
    let header_len = u64::from(u16_at(&fields, 0));
    let declared_file_len = u64::from(u32_at(&fields, 2));
    let metadata_offset = u64::from(u32_at(&fields, 6));
    let metadata_len = u64::from(u32_at(&fields, 10));
    let payload_header_offset = u64::from(u32_at(&fields, 14));
    let payload_header_len = u64::from(u32_at(&fields, 18));
    let payload_offset = u64::from(u32_at(&fields, 22));

    if header_len < LEGACY_MIN_HEADER_LEN || declared_file_len != file_len {
        return Err(InspectError::InvalidLayout {
            section: "legacy container",
        });
    }
    let metadata_end = checked_range(metadata_offset, metadata_len, file_len, "metadata")?;
    let payload_header_end = checked_range(
        payload_header_offset,
        payload_header_len,
        file_len,
        "legacy payload header",
    )?;
    if metadata_offset < header_len
        || metadata_end > payload_header_offset
        || payload_header_len < LEGACY_PAYLOAD_PREFIX_LEN
        || payload_header_end > payload_offset
        || payload_offset > file_len
    {
        return Err(InspectError::InvalidLayout {
            section: "legacy container",
        });
    }

    let parsed = read_metadata(reader, metadata_offset, metadata_len, options)?;
    let game_version = parsed
        .game_version
        .filter(|version| valid_game_version(version))
        .ok_or(InspectError::InvalidGameVersion)?;
    let payload_prefix =
        read_array_at::<34, _>(reader, payload_header_offset, "legacy payload header")?;
    let encryption_key_len = u64::from(u16_at(&payload_prefix, 32));
    if LEGACY_PAYLOAD_PREFIX_LEN
        .checked_add(encryption_key_len)
        .is_none_or(|required| required > payload_header_len)
    {
        return Err(InspectError::InvalidLayout {
            section: "legacy payload header",
        });
    }

    Ok(ReplayInspection {
        generation: ReplayGeneration::Legacy,
        game_version,
        metadata: parsed.metadata,
        raw_metadata: parsed.raw_metadata,
        layout: ReplayLayout {
            file_size: file_len,
            header: ByteRange {
                offset: 0,
                length: header_len,
            },
            metadata: ByteRange {
                offset: metadata_offset,
                length: metadata_len,
            },
            payload_header: Some(ByteRange {
                offset: payload_header_offset,
                length: payload_header_len,
            }),
            payload: ByteRange {
                offset: payload_offset,
                length: file_len - payload_offset,
            },
        },
        legacy_payload: Some(LegacyPayloadSummary {
            game_id: u64_at(&payload_prefix, 0),
            game_length_ms: u32_at(&payload_prefix, 8),
            keyframe_count: u32_at(&payload_prefix, 12),
            chunk_count: u32_at(&payload_prefix, 16),
            end_startup_chunk_id: u32_at(&payload_prefix, 20),
            start_game_chunk_id: u32_at(&payload_prefix, 24),
            keyframe_interval_ms: u32_at(&payload_prefix, 28),
            encryption_key_length: u16_at(&payload_prefix, 32),
        }),
    })
}

fn inspect_rofl2<R: Read + Seek>(
    reader: &mut R,
    file_len: u64,
    options: &InspectOptions,
) -> Result<ReplayInspection, InspectError> {
    if file_len < ROFL2_VERSION_OFFSET + METADATA_FOOTER_LEN {
        return Err(InspectError::Truncated {
            section: "ROFL2 header",
        });
    }
    let version_len = usize::from(
        read_array_at::<1, _>(
            reader,
            ROFL2_VERSION_LENGTH_OFFSET,
            "ROFL2 game version length",
        )?[0],
    );
    let content_offset = ROFL2_VERSION_OFFSET + version_len as u64;
    if content_offset + METADATA_FOOTER_LEN > file_len {
        return Err(InspectError::InvalidLayout {
            section: "ROFL2 header",
        });
    }
    let mut version_bytes = vec![0; version_len];
    read_exact_at(
        reader,
        ROFL2_VERSION_OFFSET,
        &mut version_bytes,
        "ROFL2 game version",
    )?;
    let version =
        std::str::from_utf8(&version_bytes).map_err(|_| InspectError::InvalidGameVersion)?;
    if !valid_game_version(version) {
        return Err(InspectError::InvalidGameVersion);
    }

    let footer = read_array_at::<4, _>(
        reader,
        file_len - METADATA_FOOTER_LEN,
        "ROFL2 metadata footer",
    )?;
    let metadata_len = u64::from(u32::from_le_bytes(footer));
    enforce_metadata_limit(metadata_len, options)?;
    let metadata_offset = file_len
        .checked_sub(METADATA_FOOTER_LEN)
        .and_then(|end| end.checked_sub(metadata_len))
        .ok_or(InspectError::InvalidLayout {
            section: "ROFL2 metadata",
        })?;
    if metadata_offset < content_offset {
        return Err(InspectError::InvalidLayout {
            section: "ROFL2 metadata",
        });
    }
    let parsed = read_metadata(reader, metadata_offset, metadata_len, options)?;

    Ok(ReplayInspection {
        generation: ReplayGeneration::Rofl2,
        game_version: version.to_owned(),
        metadata: parsed.metadata,
        raw_metadata: parsed.raw_metadata,
        layout: ReplayLayout {
            file_size: file_len,
            header: ByteRange {
                offset: 0,
                length: content_offset,
            },
            metadata: ByteRange {
                offset: metadata_offset,
                length: metadata_len,
            },
            payload_header: None,
            payload: ByteRange {
                offset: content_offset,
                length: metadata_offset - content_offset,
            },
        },
        legacy_payload: None,
    })
}

fn read_metadata<R: Read + Seek>(
    reader: &mut R,
    offset: u64,
    len: u64,
    options: &InspectOptions,
) -> Result<ParsedMetadata, InspectError> {
    enforce_metadata_limit(len, options)?;
    let len = usize::try_from(len).map_err(|_| InspectError::MetadataTooLarge {
        declared: len,
        limit: options.max_metadata_bytes,
    })?;
    let mut bytes = vec![0; len];
    read_exact_at(reader, offset, &mut bytes, "metadata")?;
    let raw_metadata: Map<String, Value> =
        serde_json::from_slice(&bytes).map_err(InspectError::InvalidMetadataJson)?;
    let raw: RawMetadata = serde_json::from_value(Value::Object(raw_metadata.clone()))
        .map_err(InspectError::InvalidMetadataJson)?;
    let participants =
        serde_json::from_str(&raw.stats_json).map_err(InspectError::InvalidParticipantJson)?;
    Ok(ParsedMetadata {
        game_version: raw.game_version,
        metadata: ReplayMetadata {
            game_length_ms: raw.game_length,
            last_game_chunk_id: raw.last_game_chunk_id,
            last_keyframe_id: raw.last_keyframe_id,
            participants,
        },
        raw_metadata,
    })
}

fn enforce_metadata_limit(len: u64, options: &InspectOptions) -> Result<(), InspectError> {
    if len > options.max_metadata_bytes as u64 {
        return Err(InspectError::MetadataTooLarge {
            declared: len,
            limit: options.max_metadata_bytes,
        });
    }
    Ok(())
}

fn valid_game_version(version: &str) -> bool {
    !version.is_empty() && version.bytes().all(|byte| byte.is_ascii_graphic())
}

fn checked_range(
    offset: u64,
    len: u64,
    file_len: u64,
    section: &'static str,
) -> Result<u64, InspectError> {
    let end = offset
        .checked_add(len)
        .ok_or(InspectError::InvalidLayout { section })?;
    if end > file_len {
        return Err(InspectError::InvalidLayout { section });
    }
    Ok(end)
}

fn read_array_at<const N: usize, R: Read + Seek>(
    reader: &mut R,
    offset: u64,
    section: &'static str,
) -> Result<[u8; N], InspectError> {
    let mut bytes = [0; N];
    read_exact_at(reader, offset, &mut bytes, section)?;
    Ok(bytes)
}

fn read_exact_at<R: Read + Seek>(
    reader: &mut R,
    offset: u64,
    bytes: &mut [u8],
    section: &'static str,
) -> Result<(), InspectError> {
    reader
        .seek(SeekFrom::Start(offset))
        .map_err(InspectError::Io)?;
    match reader.read_exact(bytes) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
            Err(InspectError::Truncated { section })
        }
        Err(error) => Err(InspectError::Io(error)),
    }
}

fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
        bytes[offset + 4],
        bytes[offset + 5],
        bytes[offset + 6],
        bytes[offset + 7],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    const METADATA: &[u8] = br#"{"gameLength":1234,"gameVersion":"14.8.1","lastGameChunkId":7,"lastKeyFrameId":3,"statsJson":"[{\"NAME\":\"player\"} ]"}"#;

    #[test]
    fn inspects_rofl2_using_the_declared_game_version_length() {
        let version = b"15.8.675.0448";
        let bytes = rofl2(version, METADATA);
        let inspection = inspect_bytes(&bytes, &InspectOptions::default()).expect("valid ROFL2");

        assert_eq!(inspection.generation, ReplayGeneration::Rofl2);
        assert_eq!(inspection.game_version, "15.8.675.0448");
        assert_eq!(inspection.metadata.last_game_chunk_id, 7);
        assert_eq!(inspection.metadata.participants.len(), 1);
        assert_eq!(
            inspection.raw_metadata.get("gameLength"),
            Some(&1234.into())
        );
        assert_eq!(
            inspection.layout.header.length,
            ROFL2_VERSION_OFFSET + version.len() as u64
        );
        assert_eq!(inspection.layout.metadata.length, METADATA.len() as u64);
        assert_eq!(inspection.legacy_payload, None);
    }

    #[test]
    fn inspects_legacy_layout_without_exposing_the_encryption_key() {
        let bytes = legacy(METADATA);
        let inspection = inspect_bytes(&bytes, &InspectOptions::default()).expect("valid legacy");

        assert_eq!(inspection.generation, ReplayGeneration::Legacy);
        assert_eq!(inspection.game_version, "14.8.1");
        let payload = inspection.legacy_payload.expect("payload summary");
        assert_eq!(payload.game_id, 42);
        assert_eq!(payload.encryption_key_length, 4);
        assert_eq!(inspection.layout.header.length, LEGACY_MIN_HEADER_LEN);
        assert_eq!(inspection.layout.metadata.length, METADATA.len() as u64);
    }

    #[test]
    fn rejects_unknown_generations_explicitly() {
        let error = inspect_bytes(b"RIOT\x03\x00", &InspectOptions::default())
            .expect_err("unknown generation");

        assert_eq!(error.kind(), InspectErrorKind::UnsupportedGeneration);
    }

    #[test]
    fn rejects_declared_metadata_before_allocating_it() {
        let mut bytes = rofl2(b"16.13.1", METADATA);
        let footer = bytes.len() - 4;
        bytes[footer..].copy_from_slice(&u32::MAX.to_le_bytes());
        let error = inspect_bytes(
            &bytes,
            &InspectOptions {
                max_metadata_bytes: 1024,
            },
        )
        .expect_err("oversized declaration");

        assert_eq!(error.kind(), InspectErrorKind::MetadataTooLarge);
    }

    #[test]
    fn rejects_a_rofl2_version_length_that_overlaps_the_footer() {
        let mut bytes = rofl2(b"16.13.1", METADATA);
        bytes[ROFL2_VERSION_LENGTH_OFFSET as usize] = u8::MAX;

        let error = inspect_bytes(&bytes, &InspectOptions::default())
            .expect_err("version length must stay within the container");

        assert_eq!(error.kind(), InspectErrorKind::InvalidLayout);
    }

    fn rofl2(version: &[u8], metadata: &[u8]) -> Vec<u8> {
        let version_len = u8::try_from(version.len()).expect("bounded test version");
        let content_offset = ROFL2_VERSION_OFFSET as usize + version.len();
        let mut bytes = vec![0; content_offset];
        bytes[..6].copy_from_slice(b"RIOT\x02\x00");
        bytes[ROFL2_VERSION_LENGTH_OFFSET as usize] = version_len;
        bytes[ROFL2_VERSION_OFFSET as usize..content_offset].copy_from_slice(version);
        bytes.extend_from_slice(metadata);
        bytes.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
        bytes
    }

    fn legacy(metadata: &[u8]) -> Vec<u8> {
        let metadata_offset = LEGACY_MIN_HEADER_LEN as usize;
        let payload_header_offset = metadata_offset + metadata.len();
        let payload_header_len = LEGACY_PAYLOAD_PREFIX_LEN as usize + 4;
        let payload_offset = payload_header_offset + payload_header_len;
        let mut bytes = vec![0; payload_offset + 1];
        let file_len = bytes.len() as u32;
        bytes[..6].copy_from_slice(b"RIOT\0\0");
        let fields = LEGACY_HEADER_FIELDS_OFFSET as usize;
        bytes[fields..fields + 2].copy_from_slice(&(LEGACY_MIN_HEADER_LEN as u16).to_le_bytes());
        bytes[fields + 2..fields + 6].copy_from_slice(&file_len.to_le_bytes());
        bytes[fields + 6..fields + 10].copy_from_slice(&(metadata_offset as u32).to_le_bytes());
        bytes[fields + 10..fields + 14].copy_from_slice(&(metadata.len() as u32).to_le_bytes());
        bytes[fields + 14..fields + 18]
            .copy_from_slice(&(payload_header_offset as u32).to_le_bytes());
        bytes[fields + 18..fields + 22].copy_from_slice(&(payload_header_len as u32).to_le_bytes());
        bytes[fields + 22..fields + 26].copy_from_slice(&(payload_offset as u32).to_le_bytes());
        bytes[metadata_offset..payload_header_offset].copy_from_slice(metadata);
        bytes[payload_header_offset..payload_header_offset + 8]
            .copy_from_slice(&42_u64.to_le_bytes());
        bytes[payload_header_offset + 8..payload_header_offset + 12]
            .copy_from_slice(&1234_u32.to_le_bytes());
        bytes[payload_header_offset + 32..payload_header_offset + 34]
            .copy_from_slice(&4_u16.to_le_bytes());
        bytes
    }
}
