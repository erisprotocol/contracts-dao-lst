use cosmwasm_std::{DepsMut, Env, Event, MessageInfo, Response};
use eris_chain_adapter::types::CustomQueryType;

use crate::{error::ContractResult, state::State};

pub fn vote(
    deps: DepsMut<CustomQueryType>,
    _env: Env,
    info: MessageInfo,
    proposal_id: u64,
    vote: cosmwasm_std::VoteOption,
) -> ContractResult {
    let state = State::default();
    state.assert_vote_operator(deps.storage, &info.sender)?;
    let stake = state.stake_token.load(deps.storage)?;

    let event = Event::new("erishub/voted").add_attribute("prop", proposal_id.to_string());

    let vote = stake.vote_msg(proposal_id, vote)?;

    Ok(Response::new().add_message(vote).add_event(event).add_attribute("action", "erishub/vote"))
}
