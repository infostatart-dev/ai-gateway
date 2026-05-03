use std::collections::HashMap;

use uuid::Uuid;

#[derive(Debug)]
pub(super) struct MemoryState {
    pub(super) usage: HashMap<String, i64>,
    pub(super) reservations: HashMap<String, (String, i64)>,
}

impl MemoryState {
    pub(super) fn empty() -> Self {
        Self {
            usage: HashMap::new(),
            reservations: HashMap::new(),
        }
    }

    pub(super) fn reserve(&mut self, key: &str, amount: i64) -> String {
        let reservation_id = Uuid::new_v4().to_string();
        self.reservations
            .insert(reservation_id.clone(), (key.to_string(), amount));
        *self.usage.entry(key.to_string()).or_insert(0) += amount;
        reservation_id
    }

    pub(super) fn apply_commit(
        &mut self,
        key: &str,
        reservation_id: &str,
        final_amount: i64,
    ) -> Result<(), String> {
        let Some((stored_key, reserved_amount)) =
            self.reservations.remove(reservation_id)
        else {
            return Err("reservation not found".to_string());
        };
        if stored_key != key {
            return Err("reservation key mismatch".to_string());
        }

        *self.usage.entry(key.to_string()).or_insert(0) +=
            final_amount - reserved_amount;
        Ok(())
    }
}
