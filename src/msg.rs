use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Uint128};
use oraiswap::asset::AssetInfo;

use crate::state::{Config, DistributeTarget};

#[cw_serde]
pub struct InstantiateMsg {
    pub owner: Addr,
    pub distribute_token: Addr,
    pub approver: Option<Vec<Addr>>,
    pub router: Option<Addr>,
    pub init_distribution_targets: Vec<DistributeTarget>,
}

#[cw_serde]
pub enum ExecuteMsg {
    UpdateConfig {
        owner: Option<Addr>,
        distribute_token: Option<Addr>,
        approver: Option<Vec<Addr>>,
    },
    UpdateDistributeTarget {
        distribute_targets: Vec<DistributeTarget>,
    },
    Distribute {
        amount_distribute: Uint128,
    },
    CollectFees {
        collect_fee_requirements: Vec<CollectFeeRequirement>,
    },
}

#[cw_serde]
pub struct CollectFeeRequirement {
    pub asset: AssetInfo,
    pub minimum_receive: Option<Uint128>,
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(ConfigResponse)]
    Config {},
    #[returns(DistributeTargetsResponse)]
    DistributeTargets {},
}

#[cw_serde]
pub struct ConfigResponse(pub Config);

#[cw_serde]
pub struct DistributeTargetsResponse(pub Vec<DistributeTarget>);

#[cw_serde]
pub struct MigrateMsg {}
