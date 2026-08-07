//! Typed wire and archive contract for the SEMA naming translator.
//!
//! This crate binds the production root address spaces to the
//! [`name_table`] substrate and publishes the production vocabulary
//! root enumeration.  Runtime actors, embedded database ownership,
//! authorization decisions, and the authority wire protocol remain in
//! `sema-translator`.
//!
//! The transactional name-table protocol surface was removed when
//! name-table 0.4.0 deleted the generic stateful table infrastructure
//! (primary-eyr.1).  The surviving contract is the root enumeration,
//! the root-record codec, and the opaque `VocabularyEncodedId` alias.
//!
// psyche-grasp: slightly-reviewed (2026-08-07)

/// Current authority protocol version.
pub const PROTOCOL_VERSION: u16 = 1;

/// Current translator archive version.
pub const ARCHIVE_VERSION: u16 = 1;

const ROOT_RECORD_LENGTH: usize = 7;
const WIRE_CONTRACT: [u8; 4] = *b"SSTW";
const ARCHIVE_CONTRACT: [u8; 4] = *b"SSTA";

/// The production naming address spaces.
///
/// These explicit discriminants are part of the wire and archive contract.
/// Declaration order must never determine their representation.
#[repr(u8)]
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum VocabularyRoot {
    /// Builtin priors and authored modules.
    Universal = 1,
    /// Rust-owned vocabulary.
    Rust = 2,
}

impl VocabularyRoot {
    /// The stable byte carried by wire and archive records.
    pub const fn tag(self) -> u8 {
        self as u8
    }
}

impl TryFrom<u8> for VocabularyRoot {
    type Error = RootRecordError;

    fn try_from(tag: u8) -> Result<Self, Self::Error> {
        match tag {
            1 => Ok(Self::Universal),
            2 => Ok(Self::Rust),
            found => Err(RootRecordError::UnknownRoot { found }),
        }
    }
}

/// One production durable encoded name.
///
/// This is a thin alias: the production train uses the opaque
/// authority-issued `EncodedName` without root discrimination at the
/// type level.
pub type VocabularyEncodedId = name_table::EncodedName;

/// Which stable root-record surface was decoded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RootRecordSurface {
    /// A local-socket protocol root record.
    Wire,
    /// A persisted translator archive root record.
    Archive,
}

/// Typed refusal at the root wire/archive boundary.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RootRecordError {
    /// The record is not the exact fixed size.
    #[error("root record has length {found}; expected {expected}")]
    InvalidLength { expected: usize, found: usize },

    /// The record belongs to another contract.
    #[error("root record has the wrong contract marker")]
    WrongContract { surface: RootRecordSurface },

    /// The record selects an unsupported explicit version.
    #[error("root record version {found} is unsupported")]
    UnsupportedVersion {
        surface: RootRecordSurface,
        found: u16,
    },

    /// The record carries no known production root tag.
    #[error("root record tag {found} is unknown")]
    UnknownRoot { found: u8 },
}

/// Fixed, explicit codec for root records.
pub struct RootRecord;

impl RootRecord {
    /// Encode one root for the local-socket protocol.
    pub const fn encode_wire(root: VocabularyRoot) -> [u8; ROOT_RECORD_LENGTH] {
        Self::encode(WIRE_CONTRACT, PROTOCOL_VERSION, root)
    }

    /// Decode one local-socket root record.
    pub fn decode_wire(bytes: &[u8]) -> Result<VocabularyRoot, RootRecordError> {
        Self::decode(
            bytes,
            RootRecordSurface::Wire,
            WIRE_CONTRACT,
            PROTOCOL_VERSION,
        )
    }

    /// Encode one root for persisted translator metadata.
    pub const fn encode_archive(root: VocabularyRoot) -> [u8; ROOT_RECORD_LENGTH] {
        Self::encode(ARCHIVE_CONTRACT, ARCHIVE_VERSION, root)
    }

    /// Decode one persisted translator root record.
    pub fn decode_archive(bytes: &[u8]) -> Result<VocabularyRoot, RootRecordError> {
        Self::decode(
            bytes,
            RootRecordSurface::Archive,
            ARCHIVE_CONTRACT,
            ARCHIVE_VERSION,
        )
    }

    const fn encode(
        contract: [u8; 4],
        version: u16,
        root: VocabularyRoot,
    ) -> [u8; ROOT_RECORD_LENGTH] {
        let version = version.to_le_bytes();
        [
            contract[0],
            contract[1],
            contract[2],
            contract[3],
            version[0],
            version[1],
            root.tag(),
        ]
    }

    fn decode(
        bytes: &[u8],
        surface: RootRecordSurface,
        contract: [u8; 4],
        supported_version: u16,
    ) -> Result<VocabularyRoot, RootRecordError> {
        let record: &[u8; ROOT_RECORD_LENGTH] =
            bytes
                .try_into()
                .map_err(|_| RootRecordError::InvalidLength {
                    expected: ROOT_RECORD_LENGTH,
                    found: bytes.len(),
                })?;

        if record[..4] != contract {
            return Err(RootRecordError::WrongContract { surface });
        }

        let version = u16::from_le_bytes([record[4], record[5]]);
        if version != supported_version {
            return Err(RootRecordError::UnsupportedVersion {
                surface,
                found: version,
            });
        }

        VocabularyRoot::try_from(record[6])
    }
}
