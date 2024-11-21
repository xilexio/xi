use crate::creeps::actions::drop_when_able;
use crate::creeps::CreepRef;
use crate::errors::XiError;

/// Stores everything it contains anywhere or drops if there is no storage available.
pub async fn store_anywhere_or_drop(creep_ref: &CreepRef) -> Result<(), XiError> {
    let creep_store = creep_ref.borrow_mut().store()?;

    for resource_type in creep_store.store_types() {
        drop_when_able(
            creep_ref,
            resource_type,
            creep_store.get_used_capacity(Some(resource_type))
        ).await?;
    }

    Ok(())
}