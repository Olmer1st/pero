use serde::Serialize;
use ts_rs::TS;

#[derive(TS, Clone, Serialize, Debug, PartialEq)]
pub struct ServiceStatus {
    pub status: &'static str,
    #[ts(type = "number")]
    pub latency_ms: u64,
}

#[derive(TS, Clone, Serialize, Debug, PartialEq)]
pub struct Services {
    pub llm: ServiceStatus,
    pub languagetool: ServiceStatus,
}

#[derive(TS, Clone, Serialize, Debug, PartialEq)]
pub struct HealthReport {
    /// `ok` when both local services answer, otherwise `degraded`.
    pub status: &'static str,
    pub version: &'static str,
    #[ts(type = "number")]
    pub uptime_seconds: u64,
    pub services: Services,
}
