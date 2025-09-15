use serde::{Deserialize,Serialize};
use sqlx;

#[derive(Debug,Deserialize,Serialize,sqlx::FromRow,Clone)]
pub struct IpInfo {

    pub url: String,
    pub ip: String,
    pub isp: String,
    pub country: String,
    pub latency: i32,
    pub code: String,
}

#[derive(Debug,sqlx::FromRow)]
pub struct IpList {

    pub ip: String,
}