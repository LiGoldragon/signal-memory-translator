use signal_memory_translator::{
    ARCHIVE_VERSION, PROTOCOL_VERSION, RootRecord, RootRecordError, RootRecordSurface,
    VocabularyEncodedId, VocabularyRoot,
};

#[test]
fn production_roots_and_versions_have_explicit_values() {
    assert_eq!(VocabularyRoot::Universal.tag(), 1);
    assert_eq!(VocabularyRoot::Rust.tag(), 2);
    assert_eq!(PROTOCOL_VERSION, 1);
    assert_eq!(ARCHIVE_VERSION, 1);
    assert_eq!(
        VocabularyRoot::try_from(0),
        Err(RootRecordError::UnknownRoot { found: 0 })
    );
}

#[test]
fn wire_and_archive_records_round_trip_both_roots() {
    for root in [VocabularyRoot::Universal, VocabularyRoot::Rust] {
        let wire = RootRecord::encode_wire(root);
        assert_eq!(RootRecord::decode_wire(&wire), Ok(root));

        let archive = RootRecord::encode_archive(root);
        assert_eq!(RootRecord::decode_archive(&archive), Ok(root));
        assert_ne!(&wire[..4], &archive[..4]);
    }
}

#[test]
fn wrong_contract_is_rejected_before_version_or_root() {
    let mut wire = RootRecord::encode_wire(VocabularyRoot::Universal);
    wire[..4].copy_from_slice(b"OLD!");
    wire[4..6].copy_from_slice(&99_u16.to_le_bytes());
    wire[6] = 99;
    assert_eq!(
        RootRecord::decode_wire(&wire),
        Err(RootRecordError::WrongContract {
            surface: RootRecordSurface::Wire,
        })
    );

    let mut archive = RootRecord::encode_archive(VocabularyRoot::Rust);
    archive[..4].copy_from_slice(b"OLD!");
    archive[4..6].copy_from_slice(&99_u16.to_le_bytes());
    archive[6] = 99;
    assert_eq!(
        RootRecord::decode_archive(&archive),
        Err(RootRecordError::WrongContract {
            surface: RootRecordSurface::Archive,
        })
    );
}

#[test]
fn unsupported_versions_and_root_tags_are_rejected_for_each_surface() {
    let mut wire = RootRecord::encode_wire(VocabularyRoot::Universal);
    wire[4..6].copy_from_slice(&99_u16.to_le_bytes());
    assert_eq!(
        RootRecord::decode_wire(&wire),
        Err(RootRecordError::UnsupportedVersion {
            surface: RootRecordSurface::Wire,
            found: 99,
        })
    );

    let mut archive = RootRecord::encode_archive(VocabularyRoot::Universal);
    archive[4..6].copy_from_slice(&99_u16.to_le_bytes());
    assert_eq!(
        RootRecord::decode_archive(&archive),
        Err(RootRecordError::UnsupportedVersion {
            surface: RootRecordSurface::Archive,
            found: 99,
        })
    );

    let mut wire = RootRecord::encode_wire(VocabularyRoot::Universal);
    wire[6] = 99;
    assert_eq!(
        RootRecord::decode_wire(&wire),
        Err(RootRecordError::UnknownRoot { found: 99 })
    );

    let mut archive = RootRecord::encode_archive(VocabularyRoot::Universal);
    archive[6] = 99;
    assert_eq!(
        RootRecord::decode_archive(&archive),
        Err(RootRecordError::UnknownRoot { found: 99 })
    );
}

#[test]
fn malformed_lengths_are_rejected_and_legacy_archive_metadata_is_not_guessed() {
    for encoded in [
        RootRecord::encode_wire(VocabularyRoot::Universal).to_vec(),
        RootRecord::encode_archive(VocabularyRoot::Rust).to_vec(),
    ] {
        let short = &encoded[..encoded.len() - 1];
        let mut long = encoded.clone();
        long.push(0);
        for malformed in [short, long.as_slice()] {
            assert_eq!(
                RootRecord::decode_wire(malformed),
                Err(RootRecordError::InvalidLength {
                    expected: 7,
                    found: malformed.len(),
                })
            );
            assert_eq!(
                RootRecord::decode_archive(malformed),
                Err(RootRecordError::InvalidLength {
                    expected: 7,
                    found: malformed.len(),
                })
            );
        }
    }

    let mut legacy = RootRecord::encode_archive(VocabularyRoot::Universal);
    legacy[4..6].copy_from_slice(&0_u16.to_le_bytes());
    assert_eq!(
        RootRecord::decode_archive(&legacy),
        Err(RootRecordError::UnsupportedVersion {
            surface: RootRecordSurface::Archive,
            found: 0,
        })
    );
}

#[test]
fn vocabulary_encoded_id_is_the_opaque_name_table_archive_type() {
    let bytes = [0xA5; 16];
    let encoded: VocabularyEncodedId = name_table::EncodedName::from_archive_bytes(bytes);
    assert_eq!(encoded.archive_bytes(), bytes);
}
