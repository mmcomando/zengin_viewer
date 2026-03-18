use bevy::ecs::error::Result;

use crate::{
    warn_unimplemented,
    zengin::script::script_vm::{ScriptVM, State},
};

const UNKNOWN_STACK_VAR: u32 = 7_777_777;

impl ScriptVM {
    pub fn handle_createinvitem(state: &mut State) -> Result {
        warn_unimplemented!("createinvitem not implemented");
        let _par_a = state.pop_stack_var()?;
        let _par_b = state.pop_stack_var()?;
        return Ok(());
    }

    pub fn handle_npc_changeattribute(state: &mut State) -> Result {
        warn_unimplemented!("npc_changeattribute not implemented");
        let _instance = state.pop_stack_var()?;
        let _par_b = state.pop_stack_var()?;
        let _par_c = state.pop_stack_var()?;
        return Ok(());
    }

    pub fn handle_npc_isdead(state: &mut State) -> Result {
        warn_unimplemented!("npc_isdead not implemented");
        let _instance = state.pop_stack_var()?;
        state.push_stack_int(0);
        return Ok(());
    }

    pub fn handle_playvideo(state: &mut State) -> Result {
        warn_unimplemented!("playvideo not implemented");
        let _video_string = state.pop_stack_var()?;
        state.push_stack_int(0);
        return Ok(());
    }

    pub fn handle_hlp_getnpc(state: &mut State) -> Result {
        warn_unimplemented!("hlp_getnpc not implemented");
        let _instance = state.pop_stack_var()?;
        state.push_stack_int(UNKNOWN_STACK_VAR);
        return Ok(());
    }

    pub fn handle_hlp_isvalidnpc(state: &mut State) -> Result {
        warn_unimplemented!("hlp_isvalidnpc not implemented");
        let _instance = state.pop_stack_var()?;
        state.push_stack_int(0);
        return Ok(());
    }

    pub fn handle_createinvitems(state: &mut State) -> Result {
        warn_unimplemented!("createinvitems not implemented");
        let _instance_a = state.pop_stack_var()?;
        let _instance_b = state.pop_stack_var()?;
        let _par_c = state.pop_stack_var()?;
        return Ok(());
    }
    pub fn handle_mdl_setvisual(state: &mut State) -> Result {
        warn_unimplemented!("mdl_setvisual not implemented");
        let _par_a = state.pop_stack_var()?;
        let _par_b = state.pop_stack_var()?;
        return Ok(());
    }

    pub fn handle_mdl_setmodelscale(state: &mut State) -> Result {
        warn_unimplemented!("mdl_setmodelscale not implemented");
        let _par_0 = state.pop_stack_var()?;
        let _par_1 = state.pop_stack_var()?;
        let _par_2 = state.pop_stack_var()?;
        let _par_3 = state.pop_stack_var()?;
        return Ok(());
    }
    pub fn handle_mdl_setmodelfatness(state: &mut State) -> Result {
        warn_unimplemented!("mdl_setmodelfatness not implemented");
        let _par_0 = state.pop_stack_var()?;
        let _par_1 = state.pop_stack_var()?;
        return Ok(());
    }
    pub fn handle_mdl_applyoverlaymds(state: &mut State) -> Result {
        warn_unimplemented!("mdl_applyoverlaymds not implemented");
        let _par_0 = state.pop_stack_var()?;
        let _par_1 = state.pop_stack_var()?;
        return Ok(());
    }
    pub fn handle_mdl_applyoverlaymdstimed(state: &mut State) -> Result {
        warn_unimplemented!("mdl_applyoverlaymdstimed not implemented");
        let _par_0 = state.pop_stack_var()?;
        let _par_1 = state.pop_stack_var()?;
        let _par_2 = state.pop_stack_var()?;
        return Ok(());
    }
    pub fn handle_npc_settalentskill(state: &mut State) -> Result {
        warn_unimplemented!("npc_settalentskill not implemented");
        let _par_0 = state.pop_stack_var()?;
        let _par_1 = state.pop_stack_var()?;
        let _par_2 = state.pop_stack_var()?;
        return Ok(());
    }
    pub fn handle_equipitem(state: &mut State) -> Result {
        warn_unimplemented!("equipitem not implemented");
        let _par_0 = state.pop_stack_var()?;
        let _par_1 = state.pop_stack_var()?;
        return Ok(());
    }
    pub fn handle_hlp_random(state: &mut State) -> Result {
        warn_unimplemented!("hlp_random not implemented");
        let _par_0 = state.pop_stack_var()?;
        state.push_stack_int(UNKNOWN_STACK_VAR);
        return Ok(());
    }

    pub fn handle_npc_settofistmode(state: &mut State) -> Result {
        warn_unimplemented!("npc_settofistmode not implemented");
        let _par_0 = state.pop_stack_var()?;
        return Ok(());
    }
    pub fn handle_npc_settofightmode(state: &mut State) -> Result {
        warn_unimplemented!("npc_settofightmode not implemented");
        let _par_0 = state.pop_stack_var()?;
        let _par_1 = state.pop_stack_var()?;
        return Ok(());
    }

    pub fn handle_inttostring(state: &mut State) -> Result {
        warn_unimplemented!("inttostring not implemented");
        let _par_0 = state.pop_stack_var()?;
        state.push_stack_int(UNKNOWN_STACK_VAR);
        return Ok(());
    }
    pub fn handle_npc_setattitude(state: &mut State) -> Result {
        warn_unimplemented!("npc_setattitude not implemented");
        let _par_0 = state.pop_stack_var()?;
        let _par_1 = state.pop_stack_var()?;
        return Ok(());
    }
    pub fn handle_npc_settempattitude(state: &mut State) -> Result {
        warn_unimplemented!("npc_settempattitude not implemented");
        let _par_0 = state.pop_stack_var()?;
        let _par_1 = state.pop_stack_var()?;
        return Ok(());
    }
    pub fn handle_concatstrings(state: &mut State) -> Result {
        warn_unimplemented!("concatstrings not implemented");
        let _par_0 = state.pop_stack_var()?;
        let _par_1 = state.pop_stack_var()?;
        state.push_stack_int(UNKNOWN_STACK_VAR);
        return Ok(());
    }
}
