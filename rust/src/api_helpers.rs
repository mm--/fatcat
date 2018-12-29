use api_entity_crud::EntityCrud;
use data_encoding::BASE32_NOPAD;
use database_models::*;
use database_schema::*;
use diesel;
use diesel::prelude::*;
use errors::*;
use fatcat_api_spec::models::*;
use regex::Regex;
use serde_json;
use std::str::FromStr;
use uuid::Uuid;

pub type DbConn =
    diesel::r2d2::PooledConnection<diesel::r2d2::ConnectionManager<diesel::PgConnection>>;

pub struct EditContext {
    pub editor_id: FatCatId,
    pub editgroup_id: FatCatId,
    pub extra_json: Option<serde_json::Value>,
    pub autoaccept: bool,
}

impl EditContext {
    /// This function should always be run within a transaction
    pub fn check(&self, conn: &DbConn) -> Result<()> {
        let count: i64 = changelog::table
            .filter(changelog::editgroup_id.eq(&self.editgroup_id.to_uuid()))
            .count()
            .get_result(conn)?;
        if count > 0 {
            return Err(ErrorKind::EditgroupAlreadyAccepted(self.editgroup_id.to_string()).into());
        }
        return Ok(());
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct ExpandFlags {
    pub files: bool,
    pub filesets: bool,
    pub webcaptures: bool,
    pub container: bool,
    pub releases: bool,
    pub creators: bool,
}

impl FromStr for ExpandFlags {
    type Err = Error;
    fn from_str(param: &str) -> Result<ExpandFlags> {
        let list: Vec<&str> = param.split_terminator(",").collect();
        Ok(ExpandFlags::from_str_list(&list))
    }
}

impl ExpandFlags {
    pub fn from_str_list(list: &[&str]) -> ExpandFlags {
        ExpandFlags {
            files: list.contains(&"files"),
            filesets: list.contains(&"filesets"),
            webcaptures: list.contains(&"webcaptures"),
            container: list.contains(&"container"),
            releases: list.contains(&"releases"),
            creators: list.contains(&"creators"),
        }
    }
    pub fn none() -> ExpandFlags {
        ExpandFlags {
            files: false,
            filesets: false,
            webcaptures: false,
            container: false,
            releases: false,
            creators: false,
        }
    }
}

#[test]
fn test_expand_flags() {
    assert!(ExpandFlags::from_str_list(&vec![]).files == false);
    assert!(ExpandFlags::from_str_list(&vec!["files"]).files == true);
    assert!(ExpandFlags::from_str_list(&vec!["file"]).files == false);
    let all = ExpandFlags::from_str_list(&vec![
        "files",
        "filesets",
        "webcaptures",
        "container",
        "other_thing",
        "releases",
        "creators",
    ]);
    assert!(
        all == ExpandFlags {
            files: true,
            filesets: true,
            webcaptures: true,
            container: true,
            releases: true,
            creators: true
        }
    );
    assert!(ExpandFlags::from_str("").unwrap().files == false);
    assert!(ExpandFlags::from_str("files").unwrap().files == true);
    assert!(ExpandFlags::from_str("something,,files").unwrap().files == true);
    assert!(ExpandFlags::from_str("file").unwrap().files == false);
    let all =
        ExpandFlags::from_str("files,container,other_thing,releases,creators,filesets,webcaptures")
            .unwrap();
    assert!(
        all == ExpandFlags {
            files: true,
            filesets: true,
            webcaptures: true,
            container: true,
            releases: true,
            creators: true
        }
    );
}

#[derive(Clone, Copy, PartialEq)]
pub struct HideFlags {
    // release
    pub abstracts: bool,
    pub refs: bool,
    pub contribs: bool,
    // fileset
    pub manifest: bool,
    // webcapture
    pub cdx: bool,
}

impl FromStr for HideFlags {
    type Err = Error;
    fn from_str(param: &str) -> Result<HideFlags> {
        let list: Vec<&str> = param.split_terminator(",").collect();
        Ok(HideFlags::from_str_list(&list))
    }
}

impl HideFlags {
    pub fn from_str_list(list: &[&str]) -> HideFlags {
        HideFlags {
            abstracts: list.contains(&"abstracts"),
            refs: list.contains(&"refs"),
            contribs: list.contains(&"contribs"),
            manifest: list.contains(&"contribs"),
            cdx: list.contains(&"contribs"),
        }
    }
    pub fn none() -> HideFlags {
        HideFlags {
            abstracts: false,
            refs: false,
            contribs: false,
            manifest: false,
            cdx: false,
        }
    }
}

#[test]
fn test_hide_flags() {
    assert!(HideFlags::from_str_list(&vec![]).abstracts == false);
    assert!(HideFlags::from_str_list(&vec!["abstracts"]).abstracts == true);
    assert!(HideFlags::from_str_list(&vec!["abstract"]).abstracts == false);
    let all = HideFlags::from_str_list(&vec![
        "abstracts",
        "refs",
        "other_thing",
        "contribs",
        "manifest",
        "cdx",
    ]);
    assert!(
        all == HideFlags {
            abstracts: true,
            refs: true,
            contribs: true,
            manifest: true,
            cdx: true,
        }
    );
    assert!(HideFlags::from_str("").unwrap().abstracts == false);
    assert!(HideFlags::from_str("abstracts").unwrap().abstracts == true);
    assert!(
        HideFlags::from_str("something,,abstracts")
            .unwrap()
            .abstracts
            == true
    );
    assert!(HideFlags::from_str("file").unwrap().abstracts == false);
    let all = HideFlags::from_str("abstracts,cdx,refs,manifest,other_thing,contribs").unwrap();
    assert!(
        all == HideFlags {
            abstracts: true,
            refs: true,
            contribs: true,
            manifest: true,
            cdx: true,
        }
    );
}

pub fn make_edit_context(
    conn: &DbConn,
    editgroup_id: Option<FatCatId>,
    autoaccept: bool,
) -> Result<EditContext> {
    let editor_id = Uuid::parse_str("00000000-0000-0000-AAAA-000000000001")?; // TODO: auth
    let editgroup_id: FatCatId = match (editgroup_id, autoaccept) {
        (Some(eg), _) => eg,
        // If autoaccept and no editgroup_id passed, always create a new one for this transaction
        (None, true) => {
            let eg_row: EditgroupRow = diesel::insert_into(editgroup::table)
                .values((editgroup::editor_id.eq(editor_id),))
                .get_result(conn)?;
            FatCatId::from_uuid(&eg_row.id)
        }
        (None, false) => FatCatId::from_uuid(&get_or_create_editgroup(editor_id, conn)?),
    };
    Ok(EditContext {
        editor_id: FatCatId::from_uuid(&editor_id),
        editgroup_id: editgroup_id,
        extra_json: None,
        autoaccept: autoaccept,
    })
}

// TODO: verify username (alphanum, etc)
pub fn create_editor(conn: &DbConn, username: String, is_admin: bool, is_bot: bool) -> Result<EditorRow> {
    let ed: EditorRow = diesel::insert_into(editor::table)
        .values((
            editor::username.eq(username),
            editor::is_admin.eq(is_admin),
            editor::is_bot.eq(is_bot),
        ))
        .get_result(conn)?;
    Ok(ed) 
}

/// This function should always be run within a transaction
pub fn get_or_create_editgroup(editor_id: Uuid, conn: &DbConn) -> Result<Uuid> {
    // check for current active
    let ed_row: EditorRow = editor::table.find(editor_id).first(conn)?;
    if let Some(current) = ed_row.active_editgroup_id {
        return Ok(current);
    }

    // need to insert and update
    let eg_row: EditgroupRow = diesel::insert_into(editgroup::table)
        .values((editgroup::editor_id.eq(ed_row.id),))
        .get_result(conn)?;
    diesel::update(editor::table.find(ed_row.id))
        .set(editor::active_editgroup_id.eq(eg_row.id))
        .execute(conn)?;
    Ok(eg_row.id)
}

/// This function should always be run within a transaction
pub fn accept_editgroup(editgroup_id: FatCatId, conn: &DbConn) -> Result<ChangelogRow> {
    // check that we haven't accepted already (in changelog)
    // NB: could leave this to a UNIQUE constraint
    // TODO: redundant with check_edit_context
    let count: i64 = changelog::table
        .filter(changelog::editgroup_id.eq(editgroup_id.to_uuid()))
        .count()
        .get_result(conn)?;
    if count > 0 {
        return Err(ErrorKind::EditgroupAlreadyAccepted(editgroup_id.to_string()).into());
    }

    // copy edit columns to ident table
    ContainerEntity::db_accept_edits(conn, editgroup_id)?;
    CreatorEntity::db_accept_edits(conn, editgroup_id)?;
    FileEntity::db_accept_edits(conn, editgroup_id)?;
    FilesetEntity::db_accept_edits(conn, editgroup_id)?;
    WebcaptureEntity::db_accept_edits(conn, editgroup_id)?;
    ReleaseEntity::db_accept_edits(conn, editgroup_id)?;
    WorkEntity::db_accept_edits(conn, editgroup_id)?;

    // append log/changelog row
    let entry: ChangelogRow = diesel::insert_into(changelog::table)
        .values((changelog::editgroup_id.eq(editgroup_id.to_uuid()),))
        .get_result(conn)?;

    // update any editor's active editgroup
    let no_active: Option<Uuid> = None;
    diesel::update(editor::table)
        .filter(editor::active_editgroup_id.eq(editgroup_id.to_uuid()))
        .set(editor::active_editgroup_id.eq(no_active))
        .execute(conn)?;
    Ok(entry)
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FatCatId(Uuid);

impl ToString for FatCatId {
    fn to_string(&self) -> String {
        uuid2fcid(&self.to_uuid())
    }
}

impl FromStr for FatCatId {
    type Err = Error;
    fn from_str(s: &str) -> Result<FatCatId> {
        fcid2uuid(s).map(|u| FatCatId(u))
    }
}

impl FatCatId {
    pub fn to_uuid(&self) -> Uuid {
        self.0
    }
    // TODO: just make it u: Uuid and clone (not by ref)
    pub fn from_uuid(u: &Uuid) -> FatCatId {
        FatCatId(*u)
    }
}

/// Convert fatcat IDs (base32 strings) to UUID
pub fn fcid2uuid(fcid: &str) -> Result<Uuid> {
    if fcid.len() != 26 {
        return Err(ErrorKind::InvalidFatcatId(fcid.to_string()).into());
    }
    let mut raw = vec![0; 16];
    BASE32_NOPAD
        .decode_mut(fcid.to_uppercase().as_bytes(), &mut raw)
        .map_err(|_dp| ErrorKind::InvalidFatcatId(fcid.to_string()))?;
    // unwrap() is safe here, because we know raw is always 16 bytes
    Ok(Uuid::from_bytes(&raw).unwrap())
}

/// Convert UUID to fatcat ID string (base32 encoded)
pub fn uuid2fcid(id: &Uuid) -> String {
    let raw = id.as_bytes();
    BASE32_NOPAD.encode(raw).to_lowercase()
}

pub fn check_pmcid(raw: &str) -> Result<()> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"^PMC\d+$").unwrap();
    }
    if RE.is_match(raw) {
        Ok(())
    } else {
        Err(ErrorKind::MalformedExternalId(format!(
            "not a valid PubMed Central ID (PMCID): '{}' (expected, eg, 'PMC12345')",
            raw
        ))
        .into())
    }
}

pub fn check_pmid(raw: &str) -> Result<()> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"^\d+$").unwrap();
    }
    if RE.is_match(raw) {
        Ok(())
    } else {
        Err(ErrorKind::MalformedExternalId(format!(
            "not a valid PubMed ID (PMID): '{}' (expected, eg, '1234')",
            raw
        ))
        .into())
    }
}

pub fn check_wikidata_qid(raw: &str) -> Result<()> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"^Q\d+$").unwrap();
    }
    if RE.is_match(raw) {
        Ok(())
    } else {
        Err(ErrorKind::MalformedExternalId(format!(
            "not a valid Wikidata QID: '{}' (expected, eg, 'Q1234')",
            raw
        ))
        .into())
    }
}

pub fn check_doi(raw: &str) -> Result<()> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"^10.\d{3,6}/.+$").unwrap();
    }
    if RE.is_match(raw) {
        Ok(())
    } else {
        Err(ErrorKind::MalformedExternalId(format!(
            "not a valid DOI: '{}' (expected, eg, '10.1234/aksjdfh')",
            raw
        ))
        .into())
    }
}

pub fn check_issn(raw: &str) -> Result<()> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"^\d{4}-\d{3}[0-9X]$").unwrap();
    }
    if RE.is_match(raw) {
        Ok(())
    } else {
        Err(ErrorKind::MalformedExternalId(format!(
            "not a valid ISSN: '{}' (expected, eg, '1234-5678')",
            raw
        ))
        .into())
    }
}

pub fn check_orcid(raw: &str) -> Result<()> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"^\d{4}-\d{4}-\d{4}-\d{3}[\dX]$").unwrap();
    }
    if RE.is_match(raw) {
        Ok(())
    } else {
        Err(ErrorKind::MalformedExternalId(format!(
            "not a valid ORCID: '{}' (expected, eg, '0123-4567-3456-6789')",
            raw
        ))
        .into())
    }
}

#[test]
fn test_check_orcid() {
    assert!(check_orcid("0123-4567-3456-6789").is_ok());
    assert!(check_orcid("0123-4567-3456-678X").is_ok());
    assert!(check_orcid("01234567-3456-6780").is_err());
    assert!(check_orcid("0x23-4567-3456-6780").is_err());
}

pub fn check_md5(raw: &str) -> Result<()> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"^[a-f0-9]{32}$").unwrap();
    }
    if RE.is_match(raw) {
        Ok(())
    } else {
        Err(ErrorKind::MalformedChecksum(format!(
            "not a valid MD5: '{}' (expected lower-case hex, eg, '1b39813549077b2347c0f370c3864b40')",
            raw
        ))
        .into())
    }
}

#[test]
fn test_check_md5() {
    assert!(check_md5("1b39813549077b2347c0f370c3864b40").is_ok());
    assert!(check_md5("1g39813549077b2347c0f370c3864b40").is_err());
    assert!(check_md5("1B39813549077B2347C0F370c3864b40").is_err());
    assert!(check_md5("1b39813549077b2347c0f370c3864b4").is_err());
    assert!(check_md5("1b39813549077b2347c0f370c3864b411").is_err());
}

pub fn check_sha1(raw: &str) -> Result<()> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"^[a-f0-9]{40}$").unwrap();
    }
    if RE.is_match(raw) {
        Ok(())
    } else {
        Err(ErrorKind::MalformedChecksum(format!(
            "not a valid SHA-1: '{}' (expected lower-case hex, eg, 'e9dd75237c94b209dc3ccd52722de6931a310ba3')",
            raw
        ))
        .into())
    }
}

#[test]
fn test_check_sha1() {
    assert!(check_sha1("e9dd75237c94b209dc3ccd52722de6931a310ba3").is_ok());
    assert!(check_sha1("g9dd75237c94b209dc3ccd52722de6931a310ba3").is_err());
    assert!(check_sha1("e9DD75237C94B209DC3CCD52722de6931a310ba3").is_err());
    assert!(check_sha1("e9dd75237c94b209dc3ccd52722de6931a310ba").is_err());
    assert!(check_sha1("e9dd75237c94b209dc3ccd52722de6931a310ba33").is_err());
}

pub fn check_sha256(raw: &str) -> Result<()> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"^[a-f0-9]{64}$").unwrap();
    }
    if RE.is_match(raw) {
        Ok(())
    } else {
        Err(ErrorKind::MalformedChecksum(format!(
            "not a valid SHA-256: '{}' (expected lower-case hex, eg, 'cb1c378f464d5935ddaa8de28446d82638396c61f042295d7fb85e3cccc9e452')",
            raw
        ))
        .into())
    }
}

#[test]
fn test_check_sha256() {
    assert!(
        check_sha256("cb1c378f464d5935ddaa8de28446d82638396c61f042295d7fb85e3cccc9e452").is_ok()
    );
    assert!(
        check_sha256("gb1c378f464d5935ddaa8de28446d82638396c61f042295d7fb85e3cccc9e452").is_err()
    );
    assert!(
        check_sha256("UB1C378F464d5935ddaa8de28446d82638396c61f042295d7fb85e3cccc9e452").is_err()
    );
    assert!(
        check_sha256("cb1c378f464d5935ddaa8de28446d82638396c61f042295d7fb85e3cccc9e45").is_err()
    );
    assert!(
        check_sha256("cb1c378f464d5935ddaa8de28446d82638396c61f042295d7fb85e3cccc9e4522").is_err()
    );
}

pub fn check_release_type(raw: &str) -> Result<()> {
    let valid_types = vec![
        // Citation Style Language official types
        "article",
        "article-magazine",
        "article-newspaper",
        "article-journal",
        "bill",
        "book",
        "broadcast",
        "chapter",
        "dataset",
        "entry",
        "entry-dictionary",
        "entry-encyclopedia",
        "figure",
        "graphic",
        "interview",
        "legislation",
        "legal_case",
        "manuscript",
        "map",
        "motion_picture",
        "musical_score",
        "pamphlet",
        "paper-conference",
        "patent",
        "post",
        "post-weblog",
        "personal_communication",
        "report",
        "review",
        "review-book",
        "song",
        "speech",
        "thesis",
        "treaty",
        "webpage",
        // fatcat-specific extensions
        "peer_review",
        "software",
        "standard",
    ];
    for good in valid_types {
        if raw == good {
            return Ok(());
        }
    }
    Err(ErrorKind::NotInControlledVocabulary(format!(
        "not a valid release_type: '{}' (expected a CSL type, eg, 'article-journal', 'book')",
        raw
    ))
    .into())
}

#[test]
fn test_check_release_type() {
    assert!(check_release_type("book").is_ok());
    assert!(check_release_type("article-journal").is_ok());
    assert!(check_release_type("standard").is_ok());
    assert!(check_release_type("journal-article").is_err());
    assert!(check_release_type("BOOK").is_err());
    assert!(check_release_type("book ").is_err());
}

pub fn check_contrib_role(raw: &str) -> Result<()> {
    let valid_types = vec![
        // Citation Style Language official role types
        "author",
        "collection-editor",
        "composer",
        "container-author",
        "director",
        "editor",
        "editorial-director",
        "editortranslator",
        "illustrator",
        "interviewer",
        "original-author",
        "recipient",
        "reviewed-author",
        "translator",
        // common extension (for conference proceeding chair)
        //"chair",
    ];
    for good in valid_types {
        if raw == good {
            return Ok(());
        }
    }
    Err(ErrorKind::NotInControlledVocabulary(format!(
        "not a valid contrib.role: '{}' (expected a CSL type, eg, 'author', 'editor')",
        raw
    ))
    .into())
}

#[test]
fn test_check_contrib_role() {
    assert!(check_contrib_role("author").is_ok());
    assert!(check_contrib_role("editor").is_ok());
    assert!(check_contrib_role("chair").is_err());
    assert!(check_contrib_role("EDITOR").is_err());
    assert!(check_contrib_role("editor ").is_err());
}

// TODO: make the above checks "more correct"
// TODO: check ISBN-13
