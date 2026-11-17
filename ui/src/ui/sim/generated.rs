// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

impl Generated {
    pub(super) fn evaluate(request: &Request, samples: Vec<f32>) -> Self {
        if let Some(plan) = &request.bus_trigger {
            return match plan
                .as_ref()
                .map_err(|error| *error)
                .and_then(|plan| plan.evaluate(&samples, serial::dt_of(&request.lane)))
            {
                Ok(decoded) => Self {
                    samples,
                    hits: Some((decoded.primary_slot, Arc::clone(&decoded.hits))),
                    bus_decode: Some(decoded),
                    error: None,
                },
                Err(error) => Self { samples, hits: None, bus_decode: None, error: Some(error) },
            };
        }
        let hits = request
            .trigger
            .as_ref()
            .map(|trigger| (trigger.primary_slot, trigger.evaluate(&samples, serial::dt_of(&request.lane))));
        Self { samples, hits, bus_decode: None, error: None }
    }
}
