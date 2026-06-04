use crate::models::search::Pagination;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ProfileSearchRequest {
    pub query: Option<String>,
    pub pagination: Option<Pagination>,
}
