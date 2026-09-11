use super::{Api, ApiError};
use crate::accounts::{AccountSummary, Accounts, Profile};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;

#[derive(Deserialize)]
pub(crate) struct AccountArg {
    name: String,
    copy_from: Option<String>,
}

pub(crate) async fn list(State(accounts): State<Accounts>) -> Api<Vec<AccountSummary>> {
    Ok(Json(accounts.list()))
}
pub(crate) async fn create(
    State(accounts): State<Accounts>,
    Json(arg): Json<AccountArg>,
) -> Api<Profile> {
    accounts
        .create(&arg.name, arg.copy_from.as_deref())
        .map(Json)
        .map_err(ApiError)
}
pub(crate) async fn rename(
    State(accounts): State<Accounts>,
    Path(id): Path<String>,
    Json(arg): Json<AccountArg>,
) -> Api<()> {
    accounts.rename(&id, &arg.name)?;
    Ok(Json(()))
}
