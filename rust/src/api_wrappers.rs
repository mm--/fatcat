//! API endpoint handlers

use api_entity_crud::EntityCrud;
use api_helpers::*;
use api_server::Server;
use database_models::EntityEditRow;
use diesel::Connection;
use errors::*;
use fatcat_api_spec::models;
use fatcat_api_spec::models::*;
use fatcat_api_spec::*;
use futures::{self, Future};
use std::str::FromStr;
use uuid::Uuid;

/// Helper for generating wrappers (which return "Box::new(futures::done(Ok(BLAH)))" like the
/// codegen fatcat-api-spec code wants) that call through to actual helpers (which have simple
/// Result<> return types)
macro_rules! wrap_entity_handlers {
    // Would much rather just have entity ident, then generate the other fields from that, but Rust
    // stable doesn't have a mechanism to "concat" or generate new identifiers in macros, at least
    // in the context of defining new functions.
    // The only stable approach I know of would be: https://github.com/dtolnay/mashup
    ($get_fn:ident, $get_resp:ident, $post_fn:ident, $post_resp:ident, $post_batch_fn:ident,
    $post_batch_handler:ident, $post_batch_resp:ident, $update_fn:ident, $update_resp:ident,
    $delete_fn:ident, $delete_resp:ident, $get_history_fn:ident, $get_history_resp:ident,
    $get_edit_fn:ident, $get_edit_resp:ident, $delete_edit_fn:ident, $delete_edit_resp:ident,
    $get_rev_fn:ident, $get_rev_resp:ident, $get_redirects_fn:ident, $get_redirects_resp:ident,
    $model:ident) => {

        fn $get_fn(
            &self,
            id: String,
            expand: Option<String>,
            hide: Option<String>,
            _context: &Context,
        ) -> Box<Future<Item = $get_resp, Error = ApiError> + Send> {
            let conn = self.db_pool.get().expect("db_pool error");
            // No transaction for GET
            let ret = match conn.transaction(|| {
                let entity_id = FatCatId::from_str(&id)?;
                let hide_flags = match hide {
                    None => HideFlags::none(),
                    Some(param) => HideFlags::from_str(&param)?,
                };
                match expand {
                    None => $model::db_get(&conn, entity_id, hide_flags),
                    Some(param) => {
                        let expand_flags = ExpandFlags::from_str(&param)?;
                        let mut entity = $model::db_get(&conn, entity_id, hide_flags)?;
                        entity.db_expand(&conn, expand_flags)?;
                        Ok(entity)
                    },
                }
            }) {
                Ok(entity) =>
                    $get_resp::FoundEntity(entity),
                Err(Error(ErrorKind::Diesel(::diesel::result::Error::NotFound), _)) =>
                    $get_resp::NotFound(ErrorResponse { message: format!("No such entity {}: {}", stringify!($model), id) }),
                Err(Error(ErrorKind::Uuid(e), _)) =>
                    $get_resp::BadRequest(ErrorResponse { message: e.to_string() }),
                Err(Error(ErrorKind::InvalidFatcatId(e), _)) =>
                    $get_resp::BadRequest(ErrorResponse {
                        message: ErrorKind::InvalidFatcatId(e).to_string() }),
                Err(Error(ErrorKind::MalformedExternalId(e), _)) =>
                    $get_resp::BadRequest(ErrorResponse { message: e.to_string() }),
                Err(e) => {
                    error!("{}", e);
                    $get_resp::GenericError(ErrorResponse { message: e.to_string() })
                },
            };
            Box::new(futures::done(Ok(ret)))
        }

        fn $post_fn(
            &self,
            entity: models::$model,
            editgroup: Option<String>,
            _context: &Context,
        ) -> Box<Future<Item = $post_resp, Error = ApiError> + Send> {
            let conn = self.db_pool.get().expect("db_pool error");
            let ret = match conn.transaction(|| {
                let editgroup_id = if let Some(s) = editgroup {
                    Some(FatCatId::from_str(&s)?)
                } else { None };
                let edit_context = make_edit_context(&conn, editgroup_id, false)?;
                entity.db_create(&conn, &edit_context)?.into_model()
            }) {
                Ok(edit) =>
                    $post_resp::CreatedEntity(edit),
                Err(Error(ErrorKind::Diesel(e), _)) =>
                    $post_resp::BadRequest(ErrorResponse { message: e.to_string() }),
                Err(Error(ErrorKind::Uuid(e), _)) =>
                    $post_resp::BadRequest(ErrorResponse { message: e.to_string() }),
                Err(Error(ErrorKind::InvalidFatcatId(e), _)) =>
                    $post_resp::BadRequest(ErrorResponse {
                        message: ErrorKind::InvalidFatcatId(e).to_string() }),
                Err(Error(ErrorKind::MalformedExternalId(e), _)) =>
                    $post_resp::BadRequest(ErrorResponse { message: e.to_string() }),
                Err(Error(ErrorKind::NotInControlledVocabulary(e), _)) =>
                    $post_resp::BadRequest(ErrorResponse { message: e.to_string() }),
                Err(e) => {
                    error!("{}", e);
                    $post_resp::GenericError(ErrorResponse { message: e.to_string() })
                },
            };
            Box::new(futures::done(Ok(ret)))
        }

        fn $post_batch_fn(
            &self,
            entity_list: &Vec<models::$model>,
            autoaccept: Option<bool>,
            editgroup: Option<String>,
            _context: &Context,
        ) -> Box<Future<Item = $post_batch_resp, Error = ApiError> + Send> {
            let conn = self.db_pool.get().expect("db_pool error");
            let ret = match conn.transaction(|| {
                let editgroup_id = if let Some(s) = editgroup {
                    Some(FatCatId::from_str(&s)?)
                } else { None };
                self.$post_batch_handler(entity_list, autoaccept.unwrap_or(false), editgroup_id, &conn)
            }) {
                Ok(edit) =>
                    $post_batch_resp::CreatedEntities(edit),
                Err(Error(ErrorKind::Diesel(e), _)) =>
                    $post_batch_resp::BadRequest(ErrorResponse { message: e.to_string() }),
                Err(Error(ErrorKind::Uuid(e), _)) =>
                    $post_batch_resp::BadRequest(ErrorResponse { message: e.to_string() }),
                Err(Error(ErrorKind::InvalidFatcatId(e), _)) =>
                    $post_batch_resp::BadRequest(ErrorResponse {
                        message: ErrorKind::InvalidFatcatId(e).to_string() }),
                Err(Error(ErrorKind::MalformedExternalId(e), _)) =>
                    $post_batch_resp::BadRequest(ErrorResponse { message: e.to_string() }),
                Err(Error(ErrorKind::NotInControlledVocabulary(e), _)) =>
                    $post_batch_resp::BadRequest(ErrorResponse { message: e.to_string() }),
                Err(e) => {
                    error!("{}", e);
                    $post_batch_resp::GenericError(ErrorResponse { message: e.to_string() })
                },
            };
            Box::new(futures::done(Ok(ret)))
        }

        fn $update_fn(
            &self,
            id: String,
            entity: models::$model,
            editgroup: Option<String>,
            _context: &Context,
        ) -> Box<Future<Item = $update_resp, Error = ApiError> + Send> {
            let conn = self.db_pool.get().expect("db_pool error");
            let ret = match conn.transaction(|| {
                let entity_id = FatCatId::from_str(&id)?;
                let editgroup_id = if let Some(s) = editgroup {
                    Some(FatCatId::from_str(&s)?)
                } else { None };
                let edit_context = make_edit_context(&conn, editgroup_id, false)?;
                entity.db_update(&conn, &edit_context, entity_id)?.into_model()
            }) {
                Ok(edit) =>
                    $update_resp::UpdatedEntity(edit),
                Err(Error(ErrorKind::Diesel(::diesel::result::Error::NotFound), _)) =>
                    $update_resp::NotFound(ErrorResponse { message: format!("No such entity {}: {}", stringify!($model), id) }),
                Err(Error(ErrorKind::Diesel(e), _)) =>
                    $update_resp::BadRequest(ErrorResponse { message: e.to_string() }),
                Err(Error(ErrorKind::Uuid(e), _)) =>
                    $update_resp::BadRequest(ErrorResponse { message: e.to_string() }),
                Err(Error(ErrorKind::InvalidFatcatId(e), _)) =>
                    $update_resp::BadRequest(ErrorResponse {
                        message: ErrorKind::InvalidFatcatId(e).to_string() }),
                Err(Error(ErrorKind::MalformedExternalId(e), _)) =>
                    $update_resp::BadRequest(ErrorResponse { message: e.to_string() }),
                Err(Error(ErrorKind::NotInControlledVocabulary(e), _)) =>
                    $update_resp::BadRequest(ErrorResponse { message: e.to_string() }),
                Err(e) => {
                    error!("{}", e);
                    $update_resp::GenericError(ErrorResponse { message: e.to_string() })
                },
            };
            Box::new(futures::done(Ok(ret)))
        }

        fn $delete_fn(
            &self,
            id: String,
            editgroup_id: Option<String>,
            _context: &Context,
        ) -> Box<Future<Item = $delete_resp, Error = ApiError> + Send> {
            let conn = self.db_pool.get().expect("db_pool error");
            let ret = match conn.transaction(|| {
                let entity_id = FatCatId::from_str(&id)?;
                let editgroup_id: Option<FatCatId> = match editgroup_id {
                    Some(s) => Some(FatCatId::from_str(&s)?),
                    None => None,
                };
                let edit_context = make_edit_context(&conn, editgroup_id, false)?;
                $model::db_delete(&conn, &edit_context, entity_id)?.into_model()
            }) {
                Ok(edit) =>
                    $delete_resp::DeletedEntity(edit),
                Err(Error(ErrorKind::Diesel(::diesel::result::Error::NotFound), _)) =>
                    $delete_resp::NotFound(ErrorResponse { message: format!("No such entity {}: {}", stringify!($model), id) }),
                Err(Error(ErrorKind::Diesel(e), _)) =>
                    $delete_resp::BadRequest(ErrorResponse { message: e.to_string() }),
                Err(Error(ErrorKind::Uuid(e), _)) =>
                    $delete_resp::BadRequest(ErrorResponse { message: e.to_string() }),
                Err(Error(ErrorKind::InvalidFatcatId(e), _)) =>
                    $delete_resp::BadRequest(ErrorResponse {
                        message: ErrorKind::InvalidFatcatId(e).to_string() }),
                Err(Error(ErrorKind::MalformedExternalId(e), _)) =>
                    $delete_resp::BadRequest(ErrorResponse { message: e.to_string() }),
                Err(e) => {
                    error!("{}", e);
                    $delete_resp::GenericError(ErrorResponse { message: e.to_string() })
                },
            };
            Box::new(futures::done(Ok(ret)))
        }

        fn $get_history_fn(
            &self,
            id: String,
            limit: Option<i64>,
            _context: &Context,
        ) -> Box<Future<Item = $get_history_resp, Error = ApiError> + Send> {
            let conn = self.db_pool.get().expect("db_pool error");
            // No transaction for GET
            let ret = match conn.transaction(|| {
                let entity_id = FatCatId::from_str(&id)?;
                $model::db_get_history(&conn, entity_id, limit)
            }) {
                Ok(history) =>
                    $get_history_resp::FoundEntityHistory(history),
                Err(Error(ErrorKind::Diesel(::diesel::result::Error::NotFound), _)) =>
                    $get_history_resp::NotFound(ErrorResponse { message: format!("No such entity {}: {}", stringify!($model), id) }),
                Err(Error(ErrorKind::Uuid(e), _)) =>
                    $get_history_resp::BadRequest(ErrorResponse { message: e.to_string() }),
                Err(Error(ErrorKind::InvalidFatcatId(e), _)) =>
                    $get_history_resp::BadRequest(ErrorResponse {
                        message: ErrorKind::InvalidFatcatId(e).to_string() }),
                Err(e) => {
                    error!("{}", e);
                    $get_history_resp::GenericError(ErrorResponse { message: e.to_string() })
                },
            };
            Box::new(futures::done(Ok(ret)))
        }

        fn $get_rev_fn(
            &self,
            id: String,
            expand: Option<String>,
            hide: Option<String>,
            _context: &Context,
        ) -> Box<Future<Item = $get_rev_resp, Error = ApiError> + Send> {
            let conn = self.db_pool.get().expect("db_pool error");
            // No transaction for GET
            let ret = match conn.transaction(|| {
                let rev_id = Uuid::from_str(&id)?;
                let hide_flags = match hide {
                    None => HideFlags::none(),
                    Some(param) => HideFlags::from_str(&param)?,
                };
                match expand {
                    None => $model::db_get_rev(&conn, rev_id, hide_flags),
                    Some(param) => {
                        let expand_flags = ExpandFlags::from_str(&param)?;
                        let mut entity = $model::db_get_rev(&conn, rev_id, hide_flags)?;
                        entity.db_expand(&conn, expand_flags)?;
                        Ok(entity)
                    },
                }
            }) {
                Ok(entity) =>
                    $get_rev_resp::FoundEntityRevision(entity),
                Err(Error(ErrorKind::Diesel(::diesel::result::Error::NotFound), _)) =>
                    $get_rev_resp::NotFound(ErrorResponse { message: format!("No such entity {}: {}", stringify!($model), id) }),
                Err(Error(ErrorKind::Uuid(e), _)) =>
                    $get_rev_resp::BadRequest(ErrorResponse { message: e.to_string() }),
                Err(Error(ErrorKind::MalformedExternalId(e), _)) =>
                    $get_rev_resp::BadRequest(ErrorResponse { message: e.to_string() }),
                Err(e) => {
                    error!("{}", e);
                    $get_rev_resp::GenericError(ErrorResponse { message: e.to_string() })
                },
            };
            Box::new(futures::done(Ok(ret)))
        }

        fn $get_edit_fn(
            &self,
            edit_id: i64,
            _context: &Context,
        ) -> Box<Future<Item = $get_edit_resp, Error = ApiError> + Send> {
            let conn = self.db_pool.get().expect("db_pool error");
            // No transaction for GET
            let ret = match conn.transaction(|| {
                $model::db_get_edit(&conn, edit_id)?.into_model()
            }) {
                Ok(edit) =>
                    $get_edit_resp::FoundEdit(edit),
                Err(Error(ErrorKind::Diesel(::diesel::result::Error::NotFound), _)) =>
                    $get_edit_resp::NotFound(ErrorResponse { message: format!("No such {} entity edit: {}", stringify!($model), edit_id) }),
                Err(e) => {
                    error!("{}", e);
                    $get_edit_resp::GenericError(ErrorResponse { message: e.to_string() })
                },
            };
            Box::new(futures::done(Ok(ret)))
        }

        fn $delete_edit_fn(
            &self,
            edit_id: i64,
            _context: &Context,
        ) -> Box<Future<Item = $delete_edit_resp, Error = ApiError> + Send> {
            let conn = self.db_pool.get().expect("db_pool error");
            let ret = match conn.transaction(|| {
                $model::db_delete_edit(&conn, edit_id)
            }) {
                Ok(()) =>
                    $delete_edit_resp::DeletedEdit(Success { message: format!("Successfully deleted work-in-progress {} edit: {}", stringify!($model), edit_id) } ),
                Err(Error(ErrorKind::Diesel(::diesel::result::Error::NotFound), _)) =>
                    $delete_edit_resp::NotFound(ErrorResponse { message: format!("No such {} edit: {}", stringify!($model), edit_id) }),
                Err(Error(ErrorKind::Diesel(e), _)) =>
                    $delete_edit_resp::BadRequest(ErrorResponse { message: e.to_string() }),
                Err(e) => {
                    error!("{}", e);
                    $delete_edit_resp::GenericError(ErrorResponse { message: e.to_string() })
                },
            };
            Box::new(futures::done(Ok(ret)))
        }

        fn $get_redirects_fn(
            &self,
            id: String,
            _context: &Context,
        ) -> Box<Future<Item = $get_redirects_resp, Error = ApiError> + Send> {
            let conn = self.db_pool.get().expect("db_pool error");
            // No transaction for GET
            let ret = match conn.transaction(|| {
                let entity_id = FatCatId::from_str(&id)?;
                let redirects: Vec<FatCatId> = $model::db_get_redirects(&conn, entity_id)?;
                Ok(redirects.into_iter().map(|fcid| fcid.to_string()).collect())
            }) {
                Ok(redirects) =>
                    $get_redirects_resp::FoundEntityRedirects(redirects),
                Err(Error(ErrorKind::Diesel(::diesel::result::Error::NotFound), _)) =>
                    $get_redirects_resp::NotFound(ErrorResponse { message: format!("No such entity {}: {}", stringify!($model), id) }),
                Err(Error(ErrorKind::Uuid(e), _)) =>
                    $get_redirects_resp::BadRequest(ErrorResponse { message: e.to_string() }),
                Err(Error(ErrorKind::InvalidFatcatId(e), _)) =>
                    $get_redirects_resp::BadRequest(ErrorResponse {
                        message: ErrorKind::InvalidFatcatId(e).to_string() }),
                Err(e) => {
                    error!("{}", e);
                    $get_redirects_resp::GenericError(ErrorResponse { message: e.to_string() })
                },
            };
            Box::new(futures::done(Ok(ret)))
        }

    }
}

macro_rules! wrap_lookup_handler {
    ($get_fn:ident, $get_handler:ident, $get_resp:ident, $idname:ident) => {
        fn $get_fn(
            &self,
            $idname: Option<String>,
            wikidata_qid: Option<String>,
            expand: Option<String>,
            hide: Option<String>,
            _context: &Context,
        ) -> Box<Future<Item = $get_resp, Error = ApiError> + Send> {
            let conn = self.db_pool.get().expect("db_pool error");
            let expand_flags = match expand {
                None => ExpandFlags::none(),
                Some(param) => ExpandFlags::from_str(&param).unwrap(),
            };
            let hide_flags = match hide {
                None => HideFlags::none(),
                Some(param) => HideFlags::from_str(&param).unwrap(),
            };
            // No transaction for GET
            let ret = match self.$get_handler(&$idname, &wikidata_qid, expand_flags, hide_flags, &conn) {
                Ok(entity) =>
                    $get_resp::FoundEntity(entity),
                Err(Error(ErrorKind::Diesel(::diesel::result::Error::NotFound), _)) =>
                    $get_resp::NotFound(ErrorResponse { message: format!("Not found: {:?} / {:?}", $idname, wikidata_qid) }),
                Err(Error(ErrorKind::MalformedExternalId(e), _)) =>
                    $get_resp::BadRequest(ErrorResponse { message: e.to_string() }),
                Err(Error(ErrorKind::MissingOrMultipleExternalId(e), _)) => {
                    $get_resp::BadRequest(ErrorResponse { message: e.to_string(), }) },
                Err(e) => {
                    error!("{}", e);
                    $get_resp::BadRequest(ErrorResponse { message: e.to_string() })
                },
            };
            Box::new(futures::done(Ok(ret)))
        }
    }
}

macro_rules! wrap_fcid_handler {
    ($get_fn:ident, $get_handler:ident, $get_resp:ident) => {
        fn $get_fn(
            &self,
            id: String,
            _context: &Context,
        ) -> Box<Future<Item = $get_resp, Error = ApiError> + Send> {
            let conn = self.db_pool.get().expect("db_pool error");
            // No transaction for GET
            let ret = match (|| {
                let fcid = FatCatId::from_str(&id)?;
                self.$get_handler(fcid, &conn)
            })() {
                Ok(entity) =>
                    $get_resp::Found(entity),
                Err(Error(ErrorKind::Diesel(::diesel::result::Error::NotFound), _)) =>
                    $get_resp::NotFound(ErrorResponse { message: format!("Not found: {}", id) }),
                Err(Error(ErrorKind::MalformedExternalId(e), _)) =>
                    $get_resp::BadRequest(ErrorResponse { message: e.to_string() }),
                Err(Error(ErrorKind::NotInControlledVocabulary(e), _)) =>
                    $get_resp::BadRequest(ErrorResponse { message: e.to_string() }),
                Err(e) => {
                    error!("{}", e);
                    $get_resp::BadRequest(ErrorResponse { message: e.to_string() })
                },
            };
            Box::new(futures::done(Ok(ret)))
        }
    }
}

macro_rules! wrap_fcid_hide_handler {
    ($get_fn:ident, $get_handler:ident, $get_resp:ident) => {
        fn $get_fn(
            &self,
            id: String,
            hide: Option<String>,
            _context: &Context,
        ) -> Box<Future<Item = $get_resp, Error = ApiError> + Send> {
            let conn = self.db_pool.get().expect("db_pool error");
            // No transaction for GET
            let ret = match (|| {
                let fcid = FatCatId::from_str(&id)?;
                let hide_flags = match hide {
                    None => HideFlags::none(),
                    Some(param) => HideFlags::from_str(&param)?,
                };
                self.$get_handler(fcid, hide_flags, &conn)
            })() {
                Ok(entity) =>
                    $get_resp::Found(entity),
                Err(Error(ErrorKind::Diesel(::diesel::result::Error::NotFound), _)) =>
                    $get_resp::NotFound(ErrorResponse { message: format!("Not found: {}", id) }),
                Err(Error(ErrorKind::MalformedExternalId(e), _)) =>
                    $get_resp::BadRequest(ErrorResponse { message: e.to_string() }),
                Err(Error(ErrorKind::NotInControlledVocabulary(e), _)) =>
                    $get_resp::BadRequest(ErrorResponse { message: e.to_string() }),
                Err(e) => {
                    error!("{}", e);
                    $get_resp::BadRequest(ErrorResponse { message: e.to_string() })
                },
            };
            Box::new(futures::done(Ok(ret)))
        }
    }
}

impl Api for Server {
    wrap_entity_handlers!(
        get_container,
        GetContainerResponse,
        create_container,
        CreateContainerResponse,
        create_container_batch,
        create_container_batch_handler,
        CreateContainerBatchResponse,
        update_container,
        UpdateContainerResponse,
        delete_container,
        DeleteContainerResponse,
        get_container_history,
        GetContainerHistoryResponse,
        get_container_edit,
        GetContainerEditResponse,
        delete_container_edit,
        DeleteContainerEditResponse,
        get_container_revision,
        GetContainerRevisionResponse,
        get_container_redirects,
        GetContainerRedirectsResponse,
        ContainerEntity
    );

    wrap_entity_handlers!(
        get_creator,
        GetCreatorResponse,
        create_creator,
        CreateCreatorResponse,
        create_creator_batch,
        create_creator_batch_handler,
        CreateCreatorBatchResponse,
        update_creator,
        UpdateCreatorResponse,
        delete_creator,
        DeleteCreatorResponse,
        get_creator_history,
        GetCreatorHistoryResponse,
        get_creator_edit,
        GetCreatorEditResponse,
        delete_creator_edit,
        DeleteCreatorEditResponse,
        get_creator_revision,
        GetCreatorRevisionResponse,
        get_creator_redirects,
        GetCreatorRedirectsResponse,
        CreatorEntity
    );
    wrap_entity_handlers!(
        get_file,
        GetFileResponse,
        create_file,
        CreateFileResponse,
        create_file_batch,
        create_file_batch_handler,
        CreateFileBatchResponse,
        update_file,
        UpdateFileResponse,
        delete_file,
        DeleteFileResponse,
        get_file_history,
        GetFileHistoryResponse,
        get_file_edit,
        GetFileEditResponse,
        delete_file_edit,
        DeleteFileEditResponse,
        get_file_revision,
        GetFileRevisionResponse,
        get_file_redirects,
        GetFileRedirectsResponse,
        FileEntity
    );
    wrap_entity_handlers!(
        get_release,
        GetReleaseResponse,
        create_release,
        CreateReleaseResponse,
        create_release_batch,
        create_release_batch_handler,
        CreateReleaseBatchResponse,
        update_release,
        UpdateReleaseResponse,
        delete_release,
        DeleteReleaseResponse,
        get_release_history,
        GetReleaseHistoryResponse,
        get_release_edit,
        GetReleaseEditResponse,
        delete_release_edit,
        DeleteReleaseEditResponse,
        get_release_revision,
        GetReleaseRevisionResponse,
        get_release_redirects,
        GetReleaseRedirectsResponse,
        ReleaseEntity
    );
    wrap_entity_handlers!(
        get_work,
        GetWorkResponse,
        create_work,
        CreateWorkResponse,
        create_work_batch,
        create_work_batch_handler,
        CreateWorkBatchResponse,
        update_work,
        UpdateWorkResponse,
        delete_work,
        DeleteWorkResponse,
        get_work_history,
        GetWorkHistoryResponse,
        get_work_edit,
        GetWorkEditResponse,
        delete_work_edit,
        DeleteWorkEditResponse,
        get_work_revision,
        GetWorkRevisionResponse,
        get_work_redirects,
        GetWorkRedirectsResponse,
        WorkEntity
    );

    wrap_lookup_handler!(
        lookup_container,
        lookup_container_handler,
        LookupContainerResponse,
        issnl
    );
    wrap_lookup_handler!(
        lookup_creator,
        lookup_creator_handler,
        LookupCreatorResponse,
        orcid
    );

    wrap_fcid_hide_handler!(
        get_release_files,
        get_release_files_handler,
        GetReleaseFilesResponse
    );
    wrap_fcid_hide_handler!(
        get_work_releases,
        get_work_releases_handler,
        GetWorkReleasesResponse
    );
    wrap_fcid_hide_handler!(
        get_creator_releases,
        get_creator_releases_handler,
        GetCreatorReleasesResponse
    );
    wrap_fcid_handler!(get_editor, get_editor_handler, GetEditorResponse);
    wrap_fcid_handler!(
        get_editor_changelog,
        get_editor_changelog_handler,
        GetEditorChangelogResponse
    );

    fn lookup_file(
        &self,
        md5: Option<String>,
        sha1: Option<String>,
        sha256: Option<String>,
        expand: Option<String>,
        hide: Option<String>,
        _context: &Context,
    ) -> Box<Future<Item = LookupFileResponse, Error = ApiError> + Send> {
        let conn = self.db_pool.get().expect("db_pool error");
        let expand_flags = match expand {
            None => ExpandFlags::none(),
            Some(param) => ExpandFlags::from_str(&param).unwrap(),
        };
        let hide_flags = match hide {
            None => HideFlags::none(),
            Some(param) => HideFlags::from_str(&param).unwrap(),
        };
        // No transaction for GET
        let ret = match self.lookup_file_handler(&md5, &sha1, &sha256, expand_flags, hide_flags, &conn) {
            Ok(entity) => LookupFileResponse::FoundEntity(entity),
            Err(Error(ErrorKind::Diesel(::diesel::result::Error::NotFound), _)) => {
                LookupFileResponse::NotFound(ErrorResponse {
                    message: format!("Not found: {:?} / {:?} / {:?}", md5, sha1, sha256),
                })
            }
            Err(Error(ErrorKind::MalformedExternalId(e), _)) => {
                LookupFileResponse::BadRequest(ErrorResponse {
                    message: e.to_string(),
                })
            }
            Err(Error(ErrorKind::MissingOrMultipleExternalId(e), _)) => {
                LookupFileResponse::BadRequest(ErrorResponse {
                    message: e.to_string(),
                })
            }
            Err(e) => {
                error!("{}", e);
                LookupFileResponse::BadRequest(ErrorResponse {
                    message: e.to_string(),
                })
            }
        };
        Box::new(futures::done(Ok(ret)))
    }

    fn lookup_release(
        &self,
        doi: Option<String>,
        wikidata_qid: Option<String>,
        isbn13: Option<String>,
        pmid: Option<String>,
        pmcid: Option<String>,
        core_id: Option<String>,
        expand: Option<String>,
        hide: Option<String>,
        _context: &Context,
    ) -> Box<Future<Item = LookupReleaseResponse, Error = ApiError> + Send> {
        let conn = self.db_pool.get().expect("db_pool error");
        let expand_flags = match expand {
            None => ExpandFlags::none(),
            Some(param) => ExpandFlags::from_str(&param).unwrap(),
        };
        let hide_flags = match hide {
            None => HideFlags::none(),
            Some(param) => HideFlags::from_str(&param).unwrap(),
        };
        // No transaction for GET
        let ret = match self.lookup_release_handler(
            &doi,
            &wikidata_qid,
            &isbn13,
            &pmid,
            &pmcid,
            &core_id,
            expand_flags,
            hide_flags,
            &conn,
        ) {
            Ok(entity) => LookupReleaseResponse::FoundEntity(entity),
            Err(Error(ErrorKind::Diesel(::diesel::result::Error::NotFound), _)) => {
                LookupReleaseResponse::NotFound(ErrorResponse {
                    message: format!(
                        "Not found: {:?} / {:?} / {:?} / {:?} / {:?} / {:?}",
                        doi, wikidata_qid, isbn13, pmid, pmcid, core_id
                    ),
                })
            }
            Err(Error(ErrorKind::MalformedExternalId(e), _)) => {
                LookupReleaseResponse::BadRequest(ErrorResponse {
                    message: e.to_string(),
                })
            }
            Err(Error(ErrorKind::MissingOrMultipleExternalId(e), _)) => {
                LookupReleaseResponse::BadRequest(ErrorResponse {
                    message: e.to_string(),
                })
            }
            Err(e) => {
                error!("{}", e);
                LookupReleaseResponse::BadRequest(ErrorResponse {
                    message: e.to_string(),
                })
            }
        };
        Box::new(futures::done(Ok(ret)))
    }

    fn accept_editgroup(
        &self,
        id: String,
        _context: &Context,
    ) -> Box<Future<Item = AcceptEditgroupResponse, Error = ApiError> + Send> {
        let conn = self.db_pool.get().expect("db_pool error");
        let ret = match conn.transaction(|| {
            let id = FatCatId::from_str(&id)?;
            self.accept_editgroup_handler(id, &conn)
        }) {
            Ok(()) => AcceptEditgroupResponse::MergedSuccessfully(Success {
                message: "horray!".to_string(),
            }),
            Err(Error(ErrorKind::Diesel(::diesel::result::Error::NotFound), _)) => {
                AcceptEditgroupResponse::NotFound(ErrorResponse {
                    message: format!("No such editgroup: {}", id),
                })
            }
            Err(Error(ErrorKind::EditgroupAlreadyAccepted(e), _)) => {
                AcceptEditgroupResponse::BadRequest(ErrorResponse {
                    message: ErrorKind::EditgroupAlreadyAccepted(e).to_string(),
                })
            }
            Err(e) => AcceptEditgroupResponse::GenericError(ErrorResponse {
                message: e.to_string(),
            }),
        };
        Box::new(futures::done(Ok(ret)))
    }

    fn get_editgroup(
        &self,
        id: String,
        _context: &Context,
    ) -> Box<Future<Item = GetEditgroupResponse, Error = ApiError> + Send> {
        let conn = self.db_pool.get().expect("db_pool error");
        let ret = match conn.transaction(|| {
            let id = FatCatId::from_str(&id)?;
            self.get_editgroup_handler(id, &conn)
        }) {
            Ok(entity) => GetEditgroupResponse::Found(entity),
            Err(Error(ErrorKind::Diesel(::diesel::result::Error::NotFound), _)) => {
                GetEditgroupResponse::NotFound(ErrorResponse {
                    message: format!("No such editgroup: {}", id),
                })
            }
            Err(e) =>
            // TODO: dig in to error type here
            {
                GetEditgroupResponse::GenericError(ErrorResponse {
                    message: e.to_string(),
                })
            }
        };
        Box::new(futures::done(Ok(ret)))
    }

    fn create_editgroup(
        &self,
        entity: models::Editgroup,
        _context: &Context,
    ) -> Box<Future<Item = CreateEditgroupResponse, Error = ApiError> + Send> {
        let conn = self.db_pool.get().expect("db_pool error");
        let ret = match conn.transaction(|| self.create_editgroup_handler(entity, &conn)) {
            Ok(eg) => CreateEditgroupResponse::SuccessfullyCreated(eg),
            Err(e) =>
            // TODO: dig in to error type here
            {
                CreateEditgroupResponse::GenericError(ErrorResponse {
                    message: e.to_string(),
                })
            }
        };
        Box::new(futures::done(Ok(ret)))
    }

    fn get_changelog(
        &self,
        limit: Option<i64>,
        _context: &Context,
    ) -> Box<Future<Item = GetChangelogResponse, Error = ApiError> + Send> {
        let conn = self.db_pool.get().expect("db_pool error");
        // No transaction for GET
        let ret = match self.get_changelog_handler(limit, &conn) {
            Ok(changelog) => GetChangelogResponse::Success(changelog),
            Err(e) => {
                error!("{}", e);
                GetChangelogResponse::GenericError(ErrorResponse {
                    message: e.to_string(),
                })
            }
        };
        Box::new(futures::done(Ok(ret)))
    }

    fn get_changelog_entry(
        &self,
        id: i64,
        _context: &Context,
    ) -> Box<Future<Item = GetChangelogEntryResponse, Error = ApiError> + Send> {
        let conn = self.db_pool.get().expect("db_pool error");
        // No transaction for GET
        let ret = match self.get_changelog_entry_handler(id, &conn) {
            Ok(entry) => GetChangelogEntryResponse::FoundChangelogEntry(entry),
            Err(Error(ErrorKind::Diesel(::diesel::result::Error::NotFound), _)) => {
                GetChangelogEntryResponse::NotFound(ErrorResponse {
                    message: format!("No such changelog entry: {}", id),
                })
            }
            Err(e) => {
                error!("{}", e);
                GetChangelogEntryResponse::GenericError(ErrorResponse {
                    message: e.to_string(),
                })
            }
        };
        Box::new(futures::done(Ok(ret)))
    }

    fn get_stats(
        &self,
        more: Option<String>,
        _context: &Context,
    ) -> Box<Future<Item = GetStatsResponse, Error = ApiError> + Send> {
        let conn = self.db_pool.get().expect("db_pool error");
        // No transaction for GET
        let ret = match self.get_stats_handler(&more, &conn) {
            Ok(stats) => GetStatsResponse::Success(stats),
            Err(e) => {
                error!("{}", e);
                GetStatsResponse::GenericError(ErrorResponse {
                    message: e.to_string(),
                })
            }
        };
        Box::new(futures::done(Ok(ret)))
    }
}
