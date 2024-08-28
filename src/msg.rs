use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Uint128};
use oraiswap::mixed_router::SwapOperation;

use crate::state::{Config, DistributeTarget};

#[cw_serde]
pub struct InstantiateMsg {
    pub owner: Addr,
    pub distribute_token: Addr,
    pub router: Option<Addr>,
    pub init_distribution_targets: Vec<DistributeTarget>,
    pub executors: Vec<Addr>,
}

#[cw_serde]
pub enum ExecuteMsg {
    /////////////////
    /// Owner API ///
    ////////////////
    UpdateConfig {
        owner: Option<Addr>,
        distribute_token: Option<Addr>,
    },
    UpdateDistributeTarget {
        distribute_targets: Vec<DistributeTarget>,
    },
    UpdateExecutors {
        executors: Vec<Addr>,
        permission: bool,
    },
    Distribute {
        amount_distribute: Uint128,
    },
    /////////////////
    ///Executors////
    ///////////////
    CollectFees {
        collect_fee_requirements: Vec<CollectFeeRequirement>,
    },
}

#[cw_serde]
pub struct CollectFeeRequirement {
    pub approver: Addr,
    pub swap_operations: Vec<SwapOperation>,
    pub minimum_receive: Option<Uint128>,
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    Config {},
    #[returns(DistributeTargetsResponse)]
    DistributeTargets {},
    #[returns(bool)]
    IsExecutor { addr: Addr },
}

#[cw_serde]
pub struct ConfigResponse(pub Config);

#[cw_serde]
pub struct DistributeTargetsResponse(pub Vec<DistributeTarget>);

#[cw_serde]
pub struct MigrateMsg {
    pub new_router: Addr,
}
