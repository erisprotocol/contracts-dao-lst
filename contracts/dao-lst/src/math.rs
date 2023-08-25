use cosmwasm_std::Uint128;

//--------------------------------------------------------------------------------------------------
// Minting/burning logics
//--------------------------------------------------------------------------------------------------

/// Compute the amount of Stake token to mint for a specific Token stake amount. If current total
/// staked amount is zero, we use 1 ustake = 1 utoken; otherwise, we calculate base on the current
/// utoken per ustake ratio.
pub(crate) fn compute_mint_amount(
    ustake_supply: Uint128,
    utoken_to_bond: Uint128,
    utoken_bonded: Uint128,
) -> Uint128 {
    if utoken_bonded.is_zero() {
        utoken_to_bond
    } else {
        ustake_supply.multiply_ratio(utoken_to_bond, utoken_bonded)
    }
}

/// Compute the amount of `utoken` to unbond for a specific `ustake` burn amount
///
/// There is no way `ustake` total supply is zero when the user is senting a non-zero amount of `ustake`
/// to burn, so we don't need to handle division-by-zero here
pub(crate) fn compute_unbond_amount(
    ustake_supply: Uint128,
    ustake_to_burn: Uint128,
    utoken_bonded: Uint128,
) -> Uint128 {
    utoken_bonded.multiply_ratio(ustake_to_burn, ustake_supply)
}
