use crate::creeps::actions::drop_when_able;
use crate::creeps::creeps::CreepRef;
use crate::errors::XiError;
use crate::hauling::transfers::TransferStage::AfterAllTransfers;

/// Stores everything it contains anywhere or drops if there is no storage available.
pub async fn store_anywhere_or_drop(creep_ref: &CreepRef) -> Result<(), XiError> {
    let store = creep_ref.borrow_mut().used_capacities(AfterAllTransfers)?;

    for (resource_type, amount) in store.into_iter() {
        drop_when_able(
            creep_ref,
            resource_type,
            amount
        ).await?;
    }

    Ok(())
}