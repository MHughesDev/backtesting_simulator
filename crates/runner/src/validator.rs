use std::collections::HashSet;

use contracts::{CapabilityFlags, EngineType, MarketPayload, PriceFormation, SimError};

use crate::run_request::RunRequest;

/// Validates a RunRequest before execution. Implements the first steps of
/// DATA-002 §10 — bad requests are rejected with typed errors (SC-3).
pub struct ContractValidator;

impl ContractValidator {
    /// Validate the run request. Returns Ok(()) if valid, or a SimError describing
    /// the first violation found.
    pub fn validate(request: &RunRequest) -> Result<(), SimError> {
        // Step 1: Check instruments have valid price_formation and basic fields
        for instrument in &request.instruments {
            Self::validate_instrument_price_formation(instrument)?;
            Self::validate_capability_consistency(instrument)?;
        }

        // Step 2: Check time range is valid
        if request.time_end <= request.time_start {
            return Err(SimError::ValidationError(format!(
                "time_end ({}) must be after time_start ({})",
                request.time_end, request.time_start
            )));
        }

        if let Some(warmup_start) = request.warmup_start {
            if warmup_start > request.time_start {
                return Err(SimError::ValidationError(format!(
                    "warmup_start ({}) must be <= time_start ({})",
                    warmup_start, request.time_start
                )));
            }
        }

        // Step 3: Check at least one event exists per instrument
        let instrument_ids: HashSet<&str> =
            request.instruments.iter().map(|i| i.id.as_str()).collect();

        let event_instrument_ids: HashSet<&str> = request
            .events
            .iter()
            .map(|e| e.instrument_id.as_str())
            .collect();

        for id in &instrument_ids {
            if !event_instrument_ids.contains(id) {
                return Err(SimError::DataSufficiencyError {
                    instrument_id: id.to_string(),
                    engine: EngineType::A,
                    missing_required: vec!["any market event".to_string()],
                    non_derivable_conflicts: vec![],
                    degraded_fidelity: vec![],
                });
            }
        }

        // Step 4: Data sufficiency check per engine
        for instrument in &request.instruments {
            Self::validate_data_sufficiency(instrument, &request.events)?;
        }

        // Step 5: Latency must be non-negative
        if request.latency_ns < 0 {
            return Err(SimError::ValidationError(format!(
                "latency_ns must be >= 0, got {}",
                request.latency_ns
            )));
        }

        Ok(())
    }

    fn validate_instrument_price_formation(
        instrument: &contracts::Instrument,
    ) -> Result<(), SimError> {
        // Only CLOB is currently implemented
        match instrument.price_formation {
            PriceFormation::Clob => Ok(()),
            ref pf => Err(SimError::EngineNotImplemented(format!("{:?}", pf))),
        }
    }

    fn validate_capability_consistency(instrument: &contracts::Instrument) -> Result<(), SimError> {
        let caps = instrument.capabilities;

        // HasFunding requires HasMarkPrice
        if caps.contains(CapabilityFlags::HasFunding)
            && !caps.contains(CapabilityFlags::HasMarkPrice)
        {
            return Err(SimError::ValidationError(format!(
                "instrument {}: HasFunding requires HasMarkPrice",
                instrument.id
            )));
        }

        // HasLiquidation requires IsLeveraged
        if caps.contains(CapabilityFlags::HasLiquidation)
            && !caps.contains(CapabilityFlags::IsLeveraged)
        {
            return Err(SimError::ValidationError(format!(
                "instrument {}: HasLiquidation requires IsLeveraged",
                instrument.id
            )));
        }

        // IsUnique and IsFungibleSKU are mutually exclusive
        if caps.contains(CapabilityFlags::IsUnique) && caps.contains(CapabilityFlags::IsFungibleSKU)
        {
            return Err(SimError::ValidationError(format!(
                "instrument {}: IsUnique and IsFungibleSKU are mutually exclusive",
                instrument.id
            )));
        }

        // HasShortBorrow should come with HasBorrowRate
        if caps.contains(CapabilityFlags::HasShortBorrow)
            && !caps.contains(CapabilityFlags::HasBorrowRate)
        {
            // Warning but not fatal — borrow rate may be provided as a static field
        }

        Ok(())
    }

    fn validate_data_sufficiency(
        instrument: &contracts::Instrument,
        events: &[contracts::MarketEvent],
    ) -> Result<(), SimError> {
        // Engine A requires at least one of: Bar, Quote, Trade, BookSnapshot, BookDelta
        let instrument_events: Vec<&contracts::MarketEvent> = events
            .iter()
            .filter(|e| e.instrument_id == instrument.id)
            .collect();

        let has_price_data = instrument_events.iter().any(|e| {
            matches!(
                e.payload,
                MarketPayload::Bar { .. }
                    | MarketPayload::Quote { .. }
                    | MarketPayload::Trade { .. }
                    | MarketPayload::BookSnapshot { .. }
                    | MarketPayload::BookDelta { .. }
                    | MarketPayload::Mark { .. }
                    | MarketPayload::DerivedBar { .. }
            )
        });

        if !has_price_data {
            return Err(SimError::DataSufficiencyError {
                instrument_id: instrument.id.clone(),
                engine: EngineType::A,
                missing_required: vec!["Bar OR Quote OR Trade OR BookSnapshot OR Mark".to_string()],
                non_derivable_conflicts: vec![],
                degraded_fidelity: vec![],
            });
        }

        Ok(())
    }
}
