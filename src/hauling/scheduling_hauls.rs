use std::cell::RefCell;
use std::rc::Rc;
use crate::hauling::requests::{
    with_haul_requests,
    HaulRequest,
    HaulRequestHandle,
    HaulRequestRef
};
use crate::hauling::requests::HaulRequestKind::DepositRequest;
use crate::local_debug;

const DEBUG: bool = true;

pub fn schedule_haul(mut request: HaulRequest, mut replaced_haul_request_handle: Option<HaulRequestHandle>) -> HaulRequestHandle {
    local_debug!(
        "Scheduling a haul (replacing: {}): {}.",
        replaced_haul_request_handle.is_some(),
        request
    );
    
    let mut previous_id = None;
    if let Some(mut replaced_haul_request_handle) = replaced_haul_request_handle.take() {
        replaced_haul_request_handle.droppable = false;
        previous_id = Some(replaced_haul_request_handle.request.borrow().id());
    }
    
    let request_ref = with_haul_requests(request.room_name, |haul_requests| {
        let id = request.id();
        let container = if request.kind == DepositRequest {
            &mut haul_requests.deposit_requests
        } else {
            &mut haul_requests.withdraw_requests
        };
        let request_ref;
        if let Some(previous_id) = previous_id {
            // The IDs may be different, e.g., if the previous resource pile expired.
            if let Some(previous_request) = container.remove(&previous_id) {
                request.reserved_amount = previous_request.borrow().reserved_amount;
                // This is where the request is updated for everyone.
                previous_request.replace(request);
                request_ref = previous_request;
            } else {
                request_ref = Rc::new(RefCell::new(request));
            }
        } else {
            request_ref = Rc::new(RefCell::new(request));
        }
        container.insert(id, request_ref.clone());
        request_ref
    });
    
    HaulRequestHandle {
        request: request_ref,
        droppable: true,
    }
}

pub fn cancel_haul_request(request: HaulRequestRef) {
    let mut borrowed_request = request.borrow_mut();
    local_debug!(
        "Cancelling {:?} request {}/{} for {} {} ({} reserved).",
        borrowed_request.kind,
        borrowed_request.room_name,
        borrowed_request.target,
        borrowed_request.amount,
        borrowed_request.resource_type,
        borrowed_request.reserved_amount
    );
    // Setting the request to not require any more resources.
    borrowed_request.amount = 0;
    with_haul_requests(borrowed_request.room_name, |haul_requests| {
        // TODO Cancelling haul that is already in progress.
        match borrowed_request.kind {
            DepositRequest => {
                haul_requests.deposit_requests.remove(&borrowed_request.id());
            },
            _ => {
                haul_requests.withdraw_requests.remove(&borrowed_request.id());
            },
        }
    });
}