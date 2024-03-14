use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},
    #[error("Exceeds the contract balance")]
    ExceedContractBalance {},

    #[error("Exceeds the contract balance")]
    TestError {},
    // Add any other custom errors you like here.
    // Look at https://docs.rs/thiserror/1.0.21/thiserror/ for details.
    #[error("Router and approver are not set")]
    RouterAndApproverNotSet {},
}
