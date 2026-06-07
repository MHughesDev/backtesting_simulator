use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::types::{EngineType, InstrumentId, Timestamp};

/// Data manifest for a single instrument/engine pair. Declares which payload
/// classes are required, optional, or can be derived from other payloads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataManifest {
    pub instrument_id: InstrumentId,
    pub engine: EngineType,
    /// PayloadClass names that MUST be present in the event stream.
    pub required: Vec<String>,
    /// PayloadClass names that enhance fidelity but are not required.
    pub optional: Vec<String>,
    /// PayloadClass names that the engine can provide or derive if not supplied.
    pub provide_or_derive: Vec<String>,
}

/// Describes a conflict where the required resolution differs from available data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolutionConflict {
    pub required_interval_secs: u64,
    pub available_interval_secs: u64,
}

/// Warning about reduced fidelity due to missing optional data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FidelityWarning {
    pub missing_payload_class: String,
    pub fidelity_impact: String,
}

/// All simulator errors. Bad run requests are rejected with typed errors (SC-3).
#[derive(Debug, Error)]
pub enum SimError {
    /// Required payloads missing from event stream.
    #[error(
        "manifest violation for {instrument_id}: missing required payloads: {missing_required:?}"
    )]
    ManifestViolation {
        instrument_id: InstrumentId,
        engine: EngineType,
        missing_required: Vec<String>,
        warning_optional: Vec<String>,
    },

    /// Data is present but insufficient for the requested fidelity.
    #[error("data sufficiency error for {instrument_id}")]
    DataSufficiencyError {
        instrument_id: InstrumentId,
        engine: EngineType,
        missing_required: Vec<String>,
        non_derivable_conflicts: Vec<ResolutionConflict>,
        degraded_fidelity: Vec<FidelityWarning>,
    },

    /// A required port (Account, Model, Trainer) was not injected.
    #[error("port missing: {0}")]
    PortMissing(String),

    /// General validation error (malformed request, inconsistent flags, etc.)
    #[error("validation error: {0}")]
    ValidationError(String),

    /// A strategy or model attempted to observe future data.
    #[error("look-ahead violation at ts={ts}: feature ts_event={feature_ts} > current_ts")]
    LookAheadViolation {
        ts: Timestamp,
        feature_ts: Timestamp,
    },

    /// Engine not yet implemented for the requested price_formation.
    #[error("engine not implemented for price_formation: {0}")]
    EngineNotImplemented(String),
}
