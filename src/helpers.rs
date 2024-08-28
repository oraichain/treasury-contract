use cosmwasm_std::Api;
use oraiswap::asset::AssetInfo;

pub fn asset_info_from_string(api: &dyn Api, asset: String) -> AssetInfo {
    #[cfg(test)]
    {
        let native_tokens = vec!["orai", "atom"];
        if native_tokens.contains(&asset.as_str()) {
            return AssetInfo::NativeToken {
                denom: asset.to_string(),
            };
        }
    }

    match api.addr_validate(&asset) {
        Ok(token_addr) => AssetInfo::Token {
            contract_addr: token_addr,
        },
        Err(_) => AssetInfo::NativeToken {
            denom: asset.to_string(),
        },
    }
}
