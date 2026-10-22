use name_table::{
    Declaration, LocalEncodedId, ModuleDeclaration, Name, OperationKey, RenameRequest,
    RootDeclaration, SealRequest, TableAddress, TableMutability,
};
use signal_memory_translator::{
    ARCHIVE_VERSION, PROTOCOL_VERSION, RootRecord, RootRecordError, RootRecordSurface,
    TablePolicyError, VocabularyNameTable, VocabularyRoot, module_declaration,
    rust_vocabulary_release, universal_declarations, validate_table_policy,
};

fn key(byte: u8) -> OperationKey {
    OperationKey::new([byte; 32])
}

fn member(spelling: &str) -> Declaration {
    Declaration::Member(Name::new(spelling))
}

#[test]
fn root_discriminants_and_versions_are_explicit() {
    assert_eq!(VocabularyRoot::Universal.tag(), 1);
    assert_eq!(VocabularyRoot::Rust.tag(), 2);
    assert_eq!(PROTOCOL_VERSION, 1);
    assert_eq!(ARCHIVE_VERSION, 1);
}

#[test]
fn wire_and_archive_records_round_trip_both_roots() {
    for root in [VocabularyRoot::Universal, VocabularyRoot::Rust] {
        let wire = RootRecord::encode_wire(root);
        assert_eq!(RootRecord::decode_wire(&wire), Ok(root));

        let archive = RootRecord::encode_archive(root);
        assert_eq!(RootRecord::decode_archive(&archive), Ok(root));
        assert_ne!(wire[..4], archive[..4]);
    }
}

#[test]
fn record_contract_version_and_root_are_refused_in_that_order() {
    let mut wrong_contract = RootRecord::encode_wire(VocabularyRoot::Universal);
    wrong_contract[..4].copy_from_slice(b"OLD!");
    wrong_contract[4..6].copy_from_slice(&99_u16.to_le_bytes());
    wrong_contract[6] = 99;
    assert_eq!(
        RootRecord::decode_wire(&wrong_contract),
        Err(RootRecordError::WrongContract {
            surface: RootRecordSurface::Wire,
        })
    );

    let mut wrong_version = RootRecord::encode_archive(VocabularyRoot::Universal);
    wrong_version[4..6].copy_from_slice(&99_u16.to_le_bytes());
    wrong_version[6] = 99;
    assert_eq!(
        RootRecord::decode_archive(&wrong_version),
        Err(RootRecordError::UnsupportedVersion {
            surface: RootRecordSurface::Archive,
            found: 99,
        })
    );

    let mut unknown_root = RootRecord::encode_wire(VocabularyRoot::Universal);
    unknown_root[6] = 99;
    assert_eq!(
        RootRecord::decode_wire(&unknown_root),
        Err(RootRecordError::UnknownRoot { found: 99 })
    );
}

#[test]
fn malformed_or_legacy_archive_metadata_is_not_guessed() {
    let current = RootRecord::encode_archive(VocabularyRoot::Universal);
    assert!(matches!(
        RootRecord::decode_archive(&current[..current.len() - 1]),
        Err(RootRecordError::InvalidLength { .. })
    ));

    let mut legacy = current;
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
fn production_table_policy_is_uniform_below_each_root() {
    let universal = universal_declarations(vec![
        member("Integer"),
        module_declaration(
            VocabularyRoot::Universal,
            "billing",
            vec![member("Invoice")],
        ),
    ]);
    let rust = rust_vocabulary_release(vec![module_declaration(
        VocabularyRoot::Rust,
        "std",
        vec![member("String")],
    )]);
    assert_eq!(validate_table_policy(&universal), Ok(()));
    assert_eq!(validate_table_policy(&rust), Ok(()));

    let invalid = RootDeclaration::new(
        VocabularyRoot::Rust,
        TableMutability::Immutable,
        vec![Declaration::Module(ModuleDeclaration::new(
            "std",
            TableMutability::Mutable,
            vec![],
        ))],
    );
    assert_eq!(
        validate_table_policy(&invalid),
        Err(TablePolicyError {
            root: VocabularyRoot::Rust,
            submitted: TableMutability::Mutable,
            expected: TableMutability::Immutable,
        })
    );
}

#[test]
fn universal_builtins_are_ordinary_mutable_entries() {
    let mut names = VocabularyNameTable::new();
    let receipt = names
        .seal(SealRequest::new(
            key(1),
            vec![universal_declarations(vec![
                member("Integer"),
                member("billing"),
            ])],
            vec![],
        ))
        .expect("provision Universal");

    let integer = receipt
        .declarations()
        .iter()
        .find(|resolved| resolved.path().spelling().as_str() == "Integer")
        .expect("Integer assignment")
        .encoded_id()
        .clone();
    assert_eq!(
        names
            .head(&TableAddress::root(VocabularyRoot::Universal))
            .expect("Universal head")
            .mutability(),
        TableMutability::Mutable
    );

    let renamed = names
        .rename(RenameRequest::new(key(2), integer.clone(), "WholeNumber"))
        .expect("ordinary mutable rename");
    assert_eq!(renamed.target(), &integer);
}

#[test]
fn rust_releases_append_but_cannot_rename_existing_entries() {
    let mut names = VocabularyNameTable::new();
    let first = names
        .seal(SealRequest::new(
            key(3),
            vec![rust_vocabulary_release(vec![member("struct")])],
            vec![],
        ))
        .expect("first Rust release");
    let struct_id = first.declarations()[0].encoded_id().clone();

    let second = names
        .seal(SealRequest::new(
            key(4),
            vec![rust_vocabulary_release(vec![
                member("struct"),
                member("pub"),
            ])],
            vec![],
        ))
        .expect("append Rust release");
    let struct_again = second
        .declarations()
        .iter()
        .find(|resolved| resolved.path().spelling().as_str() == "struct")
        .expect("struct assignment")
        .encoded_id();
    assert_eq!(struct_again, &struct_id);

    let error = names
        .rename(RenameRequest::new(key(5), struct_id, "record"))
        .expect_err("Rust spelling is immutable");
    assert!(matches!(
        error,
        name_table::NameTableError::ImmutableTable { .. }
    ));
}

#[test]
fn lookup_never_falls_back_between_roots() {
    let mut names = VocabularyNameTable::new();
    names
        .seal(SealRequest::new(
            key(6),
            vec![
                universal_declarations(vec![member("Integer")]),
                rust_vocabulary_release(vec![member("u64")]),
            ],
            vec![],
        ))
        .expect("provision both roots");

    let universal = TableAddress::root(VocabularyRoot::Universal);
    let rust = TableAddress::root(VocabularyRoot::Rust);
    assert!(
        names
            .lookup(&universal, &Name::new("Integer"))
            .unwrap()
            .is_some()
    );
    assert_eq!(names.lookup(&rust, &Name::new("Integer")).unwrap(), None);
    assert!(names.lookup(&rust, &Name::new("u64")).unwrap().is_some());
    assert_eq!(names.lookup(&universal, &Name::new("u64")).unwrap(), None);
}

#[test]
fn production_root_archives_reject_corrupt_metadata() {
    let mut names = VocabularyNameTable::new();
    names
        .seal(SealRequest::new(
            key(7),
            vec![universal_declarations(vec![member("Integer")])],
            vec![],
        ))
        .expect("provision Universal");

    let archive = names.to_archive_bytes().expect("archive production roots");
    let restored =
        VocabularyNameTable::from_archive_bytes(&archive).expect("restore production roots");
    assert_eq!(
        restored
            .resolve(
                &name_table::EncodedId::new(
                    VocabularyRoot::Universal,
                    vec![LocalEncodedId::new(0)],
                )
                .unwrap(),
            )
            .expect("resolve restored"),
        &Name::new("Integer")
    );

    let mut corrupt = archive.to_vec();
    let middle = corrupt.len() / 2;
    corrupt[middle] ^= 0x80;
    assert!(VocabularyNameTable::from_archive_bytes(&corrupt).is_err());
}
