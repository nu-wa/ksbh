use crate::HostModuleCtxHandle;
use ksbh_modules_abi::{
    functions::{HostFnReputationGetScore, HostFnReputationGoodBoy},
    types::{KSBHHostCtxHandle, KSBHHostFnReturn},
};

/// Handle for reporting reputation to the host.
///
/// Used for score-based rate limiting: modules can reward good behavior
/// (e.g., completing a challenge) with `reputation_good_boy()` and query the
/// current score with `reputation_score()`.
pub struct ReputationHandle {
    ctx_handle: HostModuleCtxHandle,
    good_boy: HostFnReputationGoodBoy,
    get_score: HostFnReputationGetScore,
}

impl ReputationHandle {
    /// Creates a ReputationHandle from FFI function pointers.
    pub fn from_ffi(
        ctx_handle: HostModuleCtxHandle,
        good_boy: HostFnReputationGoodBoy,
        get_score: HostFnReputationGetScore,
    ) -> Self {
        Self {
            ctx_handle,
            good_boy,
            get_score,
        }
    }

    /// Rewards good behavior by reducing the client's score by 50.
    ///
    /// Used when a client completes a challenge or demonstrates good behavior.
    /// Returns `Ok(())` if the score was updated successfully.
    pub fn reputation_good_boy(&self) -> Result<(), crate::ModuleError> {
        unsafe {
            match (self.good_boy)(KSBHHostCtxHandle {
                inner: self.ctx_handle,
            }) {
                KSBHHostFnReturn::Success => Ok(()),
                KSBHHostFnReturn::BadArgument => Err(crate::ModuleError::Host {
                    message: "reputation good_boy rejected: bad argument".to_string(),
                }),
                KSBHHostFnReturn::HostFailure => Err(crate::ModuleError::Host {
                    message: "reputation good_boy failed".to_string(),
                }),
                KSBHHostFnReturn::NotFound => Err(crate::ModuleError::Host {
                    message: "reputation good_boy failed: host context not found".to_string(),
                }),
            }
        }
    }

    /// Gets the current score for a client identified by the reputation key.
    ///
    /// The score is used by the rate-limiting module to make blocking decisions.
    /// Higher scores indicate worse behavior/more requests.
    pub fn reputation_score(&self) -> Result<Option<u64>, crate::ModuleError> {
        unsafe {
            let mut result = 0_u64;

            match (self.get_score)(
                KSBHHostCtxHandle {
                    inner: self.ctx_handle,
                },
                &mut result,
            ) {
                KSBHHostFnReturn::Success => Ok(Some(result)),
                KSBHHostFnReturn::NotFound => Ok(None),
                KSBHHostFnReturn::BadArgument => Err(crate::ModuleError::Host {
                    message: "reputation get_score rejected: bad argument".to_string(),
                }),
                KSBHHostFnReturn::HostFailure => Err(crate::ModuleError::Host {
                    message: "reputation get_score failed".to_string(),
                }),
            }
        }
    }
}
