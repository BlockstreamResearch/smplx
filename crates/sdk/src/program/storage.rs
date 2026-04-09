use crate::program::ProgramError;

#[derive(Debug, Clone, Default)]
pub struct ProgramStorage {
    slots: Vec<[u8; 32]>,
}

impl ProgramStorage {
    pub fn new(slots_count: usize) -> Self {
        Self {
            slots: vec![[0u8; 32]; slots_count],
        }
    }

    pub fn set_slots(&mut self, values: &[[u8; 32]]) -> Result<(), ProgramError> {
        let slots_count = self.len();
        if values.len() != slots_count {
            return Err(ProgramError::StorageSlotCountMismatch {
                expected: slots_count,
                actual: values.len(),
            });
        }

        self.slots.copy_from_slice(values);

        Ok(())
    }

    pub fn set_slot(&mut self, index: usize, new_value: [u8; 32]) -> Result<(), ProgramError> {
        let slots_count = self.len();
        let slot = self
            .slots
            .get_mut(index)
            .ok_or(ProgramError::StorageSlotOutOfBounds { index, slots_count })?;

        *slot = new_value;

        Ok(())
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn slots(&self) -> &[[u8; 32]] {
        &self.slots
    }

    pub fn slot(&self, index: usize) -> Option<&[u8; 32]> {
        self.slots.get(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_initializes_requested_slot_count_with_zeros() {
        let storage = ProgramStorage::new(3);

        assert_eq!(storage.len(), 3);
        assert_eq!(storage.slots(), &[[0u8; 32], [0u8; 32], [0u8; 32]]);
    }

    #[test]
    fn set_slot_updates_value_in_place() {
        let mut storage = ProgramStorage::new(2);
        let value = [0xAB; 32];

        storage.set_slot(1, value).unwrap();

        assert_eq!(storage.slot(0), Some(&[0u8; 32]));
        assert_eq!(storage.slot(1), Some(&value));
    }

    #[test]
    fn set_slot_returns_error_for_out_of_bounds_index() {
        let mut storage = ProgramStorage::new(1);
        let err = storage.set_slot(5, [1u8; 32]).unwrap_err();

        assert!(matches!(
            err,
            ProgramError::StorageSlotOutOfBounds {
                index: 5,
                slots_count: 1
            }
        ));
    }

    #[test]
    fn set_slots_overwrites_all_slots_when_lengths_match() {
        let mut storage = ProgramStorage::new(2);
        let values = [[1u8; 32], [2u8; 32]];

        storage.set_slots(&values).unwrap();

        assert_eq!(storage.slots(), &values);
    }

    #[test]
    fn set_slots_returns_error_for_length_mismatch() {
        let mut storage = ProgramStorage::new(2);
        let values = [[1u8; 32]];
        let err = storage.set_slots(&values).unwrap_err();

        assert!(matches!(
            err,
            ProgramError::StorageSlotCountMismatch { expected: 2, actual: 1 }
        ));
    }

    #[test]
    fn default_storage_is_empty() {
        let storage = ProgramStorage::default();
        assert_eq!(storage.len(), 0);
        assert!(storage.slots().is_empty());
    }
}
