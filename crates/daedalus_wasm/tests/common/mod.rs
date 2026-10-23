mod imported_functions;

use crate::common::imported_functions::ImportCallState;
use daedalus_wasm::daedalus;
use daedalus_wasm::wasm::{BinaryBuilder, parse_base_module};
use std::env::home_dir;
use std::path::PathBuf;
use std::process::Command;
use wasmtime::{Config, Engine, Store};

use wasmtime::component::{Component, HasSelf, Linker, bindgen};

use self::exports::mmcomando::zengine_scripts::manage::{ItemInfo, NpcInfo};

bindgen!({
    path:"wit",
});

#[allow(dead_code)]
pub struct WasmExecutor {
    pub store: Store<ImportCallState>,
    pub instance: MmWorld,
}

impl ImportCallState {
    fn record_call(&mut self, name: &'static str) {
        *self.call_counts.entry(name.to_owned()).or_insert(0) += 1;
    }
}

macro_rules! count_import {
    ($name:ident($($arg:ident),*)) => {
        fn $name(&mut self, $($arg: i32),*) {
            self.record_call(stringify!($name));
        }
    };
    ($name:ident($($arg:ident),*) -> i32) => {
        fn $name(&mut self, $($arg: i32),*) -> i32 {
            self.record_call(stringify!($name));
            return 0;
        }
    };
    ($name:ident($($arg:ident),*) -> Vec<u8>) => {
        fn $name(&mut self, $($arg: i32),*) -> Vec<u8> {
            self.record_call(stringify!($name));
            return Vec::new();
        }
    };
}

impl mmcomando::zengine_scripts::scripts::Host for ImportCallState {
    // String-returning externals return the raw content they want the guest
    // to copy into the DYN_STRINGS ring, the slot mirrors the guest ring cursor
    fn concatstrings(&mut self, v_0: i32, v_1: i32) -> Vec<u8> {
        self.record_call("concatstrings");
        let slot = self.record_string_call("concatstrings", vec![v_0, v_1]);
        let mut content = self.string_content(v_0);
        content.extend(self.string_content(v_1));
        self.store_dynamic_string(slot, content.clone());
        content
    }

    fn inttostring(&mut self, v_0: i32) -> Vec<u8> {
        self.record_call("inttostring");
        let slot = self.record_string_call("inttostring", vec![v_0]);
        let content = v_0.to_string().into_bytes();
        self.store_dynamic_string(slot, content.clone());
        content
    }

    fn floattostring(&mut self, v_0: i32) -> Vec<u8> {
        self.record_call("floattostring");
        let slot = self.record_string_call("floattostring", vec![v_0]);
        let content = format!("{:.6}", f32::from_bits(v_0 as u32)).into_bytes();
        self.store_dynamic_string(slot, content.clone());
        content
    }

    count_import!(floattoint(_0) -> i32);
    count_import!(inttofloat(_0) -> i32);
    count_import!(print(_0));
    count_import!(printdebug(_0));
    count_import!(printscreen(_0, _1, _2, _3, _4) -> i32);
    count_import!(ai_printscreen(_0, _1, _2, _3, _4) -> i32);
    count_import!(printdialog(_0, _1, _2, _3, _4, _5) -> i32);
    count_import!(printdebuginst(_0));

    fn printdebuginstch(&mut self, v_0: i32, v_1: i32) {
        self.record_call("printdebuginstch");
        self.print_debug_inst_ch_calls.push((v_0, v_1));
    }

    count_import!(printdebugch(_0, _1));
    count_import!(printmulti(_0, _1, _2, _3, _4));
    count_import!(exitgame());
    count_import!(exitsession());
    count_import!(playvideo(_0) -> i32);
    count_import!(playvideoex(_0, _1, _2) -> i32);
    count_import!(setpercentdone(_0));
    count_import!(introducechapter(_0, _1, _2, _3, _4));
    count_import!(apply_options_audio());
    count_import!(apply_options_controls());
    count_import!(apply_options_game());
    count_import!(apply_options_performance());
    count_import!(apply_options_video());
    count_import!(update_choicebox(_0));
    count_import!(npc_memoryentry(_0, _1, _2, _3, _4));
    count_import!(npc_memoryentryguild(_0, _1, _2, _3, _4));
    count_import!(npc_hasnews(_0, _1, _2, _3) -> i32);
    count_import!(npc_getnewswitness(_0, _1) -> i32);
    count_import!(npc_getnewsoffender(_0, _1) -> i32);
    count_import!(npc_getnewsvictim(_0, _1) -> i32);
    count_import!(npc_isnewsgossip(_0, _1) -> i32);
    count_import!(npc_deletenews(_0, _1) -> i32);
    count_import!(npc_getguildattitude(_0, _1) -> i32);
    count_import!(npc_getactivespell(_0) -> i32);
    count_import!(npc_getlasthitspellid(_0) -> i32);
    count_import!(npc_getlasthitspellcat(_0) -> i32);
    count_import!(npc_getactivespellcat(_0) -> i32);
    count_import!(npc_getactivespelllevel(_0) -> i32);
    count_import!(npc_getactivespellisscroll(_0) -> i32);
    count_import!(npc_setactivespellinfo(_0, _1) -> i32);
    count_import!(wld_detectitem(_0, _1) -> i32);
    count_import!(wld_detectplayer(_0) -> i32);
    count_import!(wld_detectnpc(_0, _1, _2, _3) -> i32);
    count_import!(wld_detectnpcex(_0, _1, _2, _3, _4) -> i32);
    count_import!(wld_detectnpcexatt(_0, _1, _2, _3, _4, _5) -> i32);
    count_import!(wld_setguildattitude(_0, _1, _2));
    count_import!(wld_getguildattitude(_0, _1) -> i32);
    count_import!(wld_ismobavailable(_0, _1) -> i32);
    count_import!(wld_isfpavailable(_0, _1) -> i32);
    count_import!(wld_isnextfpavailable(_0, _1) -> i32);
    count_import!(npc_createspell(_0, _1));
    count_import!(npc_learnspell(_0, _1));

    fn npc_hasitems(&mut self, _v_0: i32, _v_1: i32) -> i32 {
        self.record_call("npc_hasitems");
        self.npc_hasitems_return
    }

    count_import!(npc_giveitem(_0, _1, _2));
    count_import!(createinvitem(_0, _1));
    count_import!(createinvitems(_0, _1, _2));
    count_import!(npc_getinvitem(_0, _1) -> i32);
    count_import!(npc_getinvitembyslot(_0, _1, _2) -> i32);
    count_import!(npc_removeinvitem(_0, _1) -> i32);
    count_import!(npc_removeinvitems(_0, _1, _2) -> i32);
    count_import!(npc_clearinventory(_0));
    count_import!(npc_isinstate(_0, _1) -> i32);
    count_import!(npc_wasinstate(_0, _1) -> i32);
    count_import!(npc_isinroutine(_0, _1) -> i32);
    count_import!(ai_setnpcstostate(_0, _1, _2));
    count_import!(npc_hasdetectednpc(_0, _1) -> i32);
    count_import!(npc_setattitude(_0, _1));
    count_import!(npc_settempattitude(_0, _1));
    count_import!(npc_getattitude(_0, _1) -> i32);
    count_import!(npc_getpermattitude(_0, _1) -> i32);
    count_import!(npc_changeattribute(_0, _1, _2));
    count_import!(npc_getcomrades(_0) -> i32);
    count_import!(npc_isnear(_0, _1) -> i32);
    count_import!(npc_getdisttonpc(_0, _1) -> i32);
    count_import!(npc_getheighttonpc(_0, _1) -> i32);
    count_import!(npc_getheighttoitem(_0, _1) -> i32);
    count_import!(npc_getdisttoplayer(_0) -> i32);
    count_import!(npc_getdisttoitem(_0, _1) -> i32);
    count_import!(npc_getdisttowp(_0, _1) -> i32);
    count_import!(npc_canseenpc(_0, _1) -> i32);
    count_import!(npc_canseenpcfreelos(_0, _1) -> i32);
    count_import!(npc_canseeitem(_0, _1) -> i32);
    count_import!(npc_isplayer(_0) -> i32);
    count_import!(npc_knowsplayer(_0, _1) -> i32);
    count_import!(npc_setknowsplayer(_0, _1));
    count_import!(npc_isinfightmode(_0, _1) -> i32);
    count_import!(npc_isaiming(_0, _1) -> i32);
    count_import!(npc_startitemreactmodules(_0, _1, _2) -> i32);
    count_import!(npc_hasoffered(_0, _1, _2) -> i32);
    count_import!(equipitem(_0, _1));
    count_import!(npc_settofistmode(_0));
    count_import!(npc_settofightmode(_0, _1));
    count_import!(npc_isdead(_0) -> i32);
    count_import!(ai_startstate(_0, _1, _2, _3));
    count_import!(ai_continueroutine(_0));
    count_import!(npc_getstatetime(_0) -> i32);
    count_import!(npc_setstatetime(_0, _1));
    count_import!(ai_output(_0, _1, _2));
    count_import!(ai_outputsvm(_0, _1, _2));
    count_import!(ai_outputsvm_overlay(_0, _1, _2));
    count_import!(ai_playcutscene(_0, _1));
    count_import!(ai_canseenpc(_0, _1, _2));
    count_import!(ai_wait(_0, _1));
    count_import!(ai_waitms(_0, _1));
    count_import!(ai_waittillend(_0, _1));
    count_import!(ai_aligntowp(_0));
    count_import!(ai_setwalkmode(_0, _1));
    fn ai_playani(&mut self, v_0: i32, v_1: i32) {
        self.record_call("ai_playani");
        self.play_ani_calls.push((v_0, v_1, 0));
    }
    fn ai_playanibs(&mut self, v_0: i32, v_1: i32, v_2: i32) {
        self.record_call("ai_playanibs");
        self.play_ani_calls.push((v_0, v_1, v_2));
    }
    count_import!(ai_gotowp(_0, _1));
    count_import!(ai_teleport(_0, _1));
    count_import!(ai_gotoitem(_0, _1));
    count_import!(ai_gotonpc(_0, _1));
    count_import!(ai_aligntofp(_0));
    count_import!(ai_gotofp(_0, _1));
    count_import!(npc_isonfp(_0, _1) -> i32);
    count_import!(ai_gotonextfp(_0, _1));
    count_import!(ai_gotosound(_0));
    count_import!(ai_takeitem(_0, _1));
    count_import!(ai_dropitem(_0, _1));
    count_import!(ai_drawweapon(_0));
    count_import!(ai_removeweapon(_0));
    count_import!(npc_hasrangedweaponwithammo(_0) -> i32);
    count_import!(npc_hasequippedweapon(_0) -> i32);
    count_import!(npc_hasequippedmeleeweapon(_0) -> i32);
    count_import!(npc_hasequippedrangedweapon(_0) -> i32);
    count_import!(npc_hasequippedarmor(_0) -> i32);
    count_import!(npc_hasreadiedweapon(_0) -> i32);
    count_import!(npc_hasreadiedmeleeweapon(_0) -> i32);
    count_import!(npc_hasreadiedrangedweapon(_0) -> i32);
    count_import!(npc_gettrueguild(_0) -> i32);
    count_import!(npc_settrueguild(_0, _1) -> i32);
    count_import!(npc_arewestronger(_0, _1) -> i32);
    count_import!(npc_gettarget(_0) -> i32);
    count_import!(npc_getnexttarget(_0) -> i32);
    count_import!(npc_getlookattarget(_0) -> i32);
    count_import!(npc_isnexttargetavailable(_0) -> i32);
    count_import!(npc_settarget(_0, _1));
    count_import!(npc_getbodystate(_0) -> i32);
    count_import!(npc_hasbodyflag(_0, _1) -> i32);
    count_import!(npc_isincutscene(_0) -> i32);
    count_import!(npc_isvoiceactive(_0) -> i32);
    count_import!(npc_stopani(_0, _1));
    count_import!(npc_getdetectedmob(_0) -> i32);
    count_import!(npc_playani(_0, _1));
    count_import!(npc_isdetectedmobownedbynpc(_0, _1) -> i32);
    count_import!(npc_isdetectedmobownedbyguild(_0, _1) -> i32);
    count_import!(wld_getmobstate(_0, _1) -> i32);
    count_import!(npc_clearaiqueue(_0));
    count_import!(infomanager_hasfinished() -> i32);
    count_import!(ai_stopprocessinfos(_0));
    count_import!(ai_processinfos(_0));
    count_import!(info_addchoice(_0, _1, _2));
    count_import!(info_clearchoices(_0));
    count_import!(npc_knowsinfo(_0, _1) -> i32);
    count_import!(npc_checkinfo(_0, _1) -> i32);
    count_import!(npc_giveinfo(_0, _1) -> i32);
    count_import!(npc_gettalentskill(_0, _1) -> i32);
    count_import!(npc_gettalentvalue(_0, _1) -> i32);
    count_import!(npc_settalentskill(_0, _1, _2));
    count_import!(npc_settalentvalue(_0, _1, _2));
    count_import!(tal_configure(_0, _1));
    count_import!(npc_getnearestwp(_0) -> i32);
    count_import!(npc_getnextwp(_0) -> i32);
    count_import!(npc_iswayblocked(_0) -> i32);
    count_import!(npc_getreadiedweapon(_0) -> i32);
    count_import!(npc_getequippedmeleeweapon(_0) -> i32);
    count_import!(npc_getequippedrangedweapon(_0) -> i32);
    count_import!(npc_getequippedarmor(_0) -> i32);
    count_import!(ai_equipbestmeleeweapon(_0) -> i32);
    count_import!(ai_equipbestrangedweapon(_0) -> i32);
    count_import!(ai_equipbestarmor(_0));
    count_import!(ai_unequipweapons(_0));
    count_import!(ai_unequiparmor(_0));
    count_import!(ai_equiparmor(_0, _1));
    count_import!(ai_readymeleeweapon(_0));
    count_import!(ai_readyrangedweapon(_0));
    count_import!(ai_turnaway(_0, _1));
    count_import!(ai_turntonpc(_0, _1));
    count_import!(ai_whirlaround(_0, _1));
    count_import!(ai_whirlaroundtosource(_0));
    count_import!(ai_turntosound(_0));
    count_import!(ai_quicklook(_0, _1));
    count_import!(ai_lookat(_0, _1));
    count_import!(ai_lookatnpc(_0, _1));
    count_import!(ai_stoplookat(_0));
    count_import!(ai_pointat(_0, _1));
    count_import!(ai_pointatnpc(_0, _1));
    count_import!(ai_stoppointat(_0));
    count_import!(ai_standup(_0));
    count_import!(ai_standupquick(_0));
    count_import!(ai_flee(_0));
    count_import!(ai_aimat(_0, _1));
    count_import!(ai_stopaim(_0));
    count_import!(ai_shootat(_0, _1));
    count_import!(ai_defend(_0));
    count_import!(ai_combatreacttodamage(_0));
    count_import!(ai_readyspell(_0, _1, _2));
    count_import!(ai_unreadyspell(_0));
    count_import!(npc_hasspell(_0, _1) -> i32);
    count_import!(ai_attack(_0));
    count_import!(ai_finishingmove(_0, _1));
    count_import!(ai_dodge(_0));
    count_import!(ai_useitem(_0, _1));
    count_import!(ai_useitemtostate(_0, _1, _2));
    count_import!(npc_refusetalk(_0) -> i32);
    count_import!(npc_setrefusetalk(_0, _1));
    count_import!(ai_ask(_0, _1, _2));
    count_import!(ai_asktext(_0, _1, _2, _3, _4));
    count_import!(ai_waitforquestion(_0, _1));
    count_import!(hlp_isitem(_0, _1) -> i32);
    count_import!(hlp_getinstanceid(_0) -> i32);
    count_import!(hlp_random(_0) -> i32);
    count_import!(hlp_strcmp(_0, _1) -> i32);
    count_import!(hlp_getnpc(_0) -> i32);
    count_import!(hlp_isvalidnpc(_0) -> i32);
    count_import!(hlp_isvaliditem(_0) -> i32);
    count_import!(ai_snd_play(_0, _1));
    count_import!(ai_snd_play3d(_0, _1, _2));
    count_import!(snd_play(_0));
    count_import!(snd_play3d(_0, _1));
    count_import!(snd_getdisttosource(_0) -> i32);
    count_import!(snd_issourcenpc(_0) -> i32);
    count_import!(snd_issourceitem(_0) -> i32);
    count_import!(npc_canseesource(_0) -> i32);
    count_import!(mis_getstatus(_0) -> i32);
    count_import!(mis_setstatus(_0, _1));
    count_import!(mis_ontime(_0) -> i32);
    count_import!(log_createtopic(_0, _1));
    count_import!(log_addentry(_0, _1));
    count_import!(log_settopicstatus(_0, _1));
    count_import!(mis_addmissionentry(_0, _1));
    count_import!(mis_removemission(_0));
    count_import!(npc_checkavailablemission(_0, _1, _2) -> i32);
    count_import!(npc_checkrunningmission(_0, _1) -> i32);
    count_import!(npc_isdrawingweapon(_0) -> i32);
    count_import!(npc_isdrawingspell(_0) -> i32);
    count_import!(npc_checkoffermission(_0, _1) -> i32);
    count_import!(mdl_setvisual(_0, _1));
    count_import!(mdl_setvisualbody(_0, _1, _2, _3, _4, _5, _6, _7));
    count_import!(mdl_applyoverlaymds(_0, _1));
    count_import!(mdl_applyoverlaymdstimed(_0, _1, _2));
    count_import!(mdl_removeoverlaymds(_0, _1));
    count_import!(mdl_applyrandomani(_0, _1, _2));
    count_import!(mdl_applyrandomanifreq(_0, _1, _2));
    count_import!(mdl_setmodelscale(_0, _1, _2, _3));
    count_import!(mdl_setmodelfatness(_0, _1));
    count_import!(mdl_startfaceani(_0, _1, _2, _3));
    count_import!(mdl_applyrandomfaceani(_0, _1, _2, _3, _4, _5, _6));
    count_import!(wld_istime(_0, _1, _2, _3) -> i32);
    count_import!(wld_getday() -> i32);
    count_import!(wld_israining() -> i32);
    count_import!(wld_settime(_0, _1));
    count_import!(wld_insertnpc(_0, _1));
    count_import!(wld_spawnnpcrange(_0, _1, _2, _3));
    count_import!(wld_playeffect(_0, _1, _2, _3, _4, _5, _6));
    count_import!(wld_stopeffect(_0));
    count_import!(ai_playfx(_0, _1, _2));
    count_import!(ai_stopfx(_0, _1));
    count_import!(wld_removenpc(_0));
    count_import!(wld_insertnpcandrespawn(_0, _1, _2));
    count_import!(wld_insertitem(_0, _1));
    count_import!(wld_insertobject(_0, _1));
    count_import!(wld_removeitem(_0) -> i32);
    count_import!(wld_exchangeguildattitudes(_0));
    count_import!(wld_setobjectroutine(_0, _1, _2, _3));
    count_import!(wld_setmobroutine(_0, _1, _2, _3));
    count_import!(wld_sendtrigger(_0));
    count_import!(wld_senduntrigger(_0));
    count_import!(npc_ownedbynpc(_0, _1) -> i32);
    count_import!(npc_ownedbyguild(_0, _1) -> i32);
    count_import!(ai_takemob(_0, _1));
    count_import!(ai_dropmob(_0));
    count_import!(ai_usemob(_0, _1, _2) -> i32);
    count_import!(mob_createitems(_0, _1, _2));
    count_import!(mob_hasitems(_0, _1) -> i32);
    count_import!(doc_create() -> i32);
    count_import!(doc_createmap() -> i32);
    count_import!(doc_setpages(_0, _1));
    count_import!(doc_setpage(_0, _1, _2, _3));
    count_import!(doc_setfont(_0, _1, _2));
    count_import!(doc_setlevel(_0, _1));
    count_import!(doc_setlevelcoords(_0, _1, _2, _3, _4));
    count_import!(doc_setmargins(_0, _1, _2, _3, _4, _5, _6));
    count_import!(doc_printline(_0, _1, _2));
    count_import!(doc_printlines(_0, _1, _2));
    count_import!(doc_show(_0));
    count_import!(doc_open(_0));
    count_import!(doc_font(_0));
    count_import!(doc_print(_0));
    count_import!(doc_mapcoordinates(_0, _1, _2, _3, _4, _5, _6, _7, _8));
    count_import!(ta(_0, _1, _2, _3, _4));
    count_import!(ta_min(_0, _1, _2, _3, _4, _5, _6));
    count_import!(ta_cs(_0, _1, _2));
    count_import!(ta_beginoverlay(_0));
    count_import!(ta_endoverlay(_0));
    count_import!(ta_removeoverlay(_0));
    count_import!(npc_exchangeroutine(_0, _1));
    count_import!(rtn_exchange(_0, _1));
    count_import!(perc_setrange(_0, _1));
    count_import!(npc_setperctime(_0, _1));
    count_import!(npc_percenable(_0, _1, _2));
    count_import!(npc_percdisable(_0, _1));
    count_import!(npc_sendpassiveperc(_0, _1, _2, _3));
    count_import!(npc_sendsingleperc(_0, _1, _2));
    count_import!(npc_perceiveall(_0));
    count_import!(wld_assignroomtoguild(_0, _1));
    count_import!(wld_assignroomtonpc(_0, _1));
    count_import!(wld_getplayerportalowner() -> i32);
    count_import!(wld_getplayerportalguild() -> i32);
    count_import!(wld_getformerplayerportalowner() -> i32);
    count_import!(wld_getformerplayerportalguild() -> i32);
    count_import!(npc_isplayerinmyroom(_0) -> i32);
    count_import!(npc_isinplayersroom(_0) -> i32);
    count_import!(npc_wasplayerinmyroom(_0) -> i32);
    count_import!(npc_getportalowner(_0) -> i32);
    count_import!(npc_getportalguild(_0) -> i32);
    count_import!(hlp_cutsceneplayed(_0) -> i32);
    count_import!(game_initgerman() -> i32);
    count_import!(game_initenglish() -> i32);
    count_import!(game_initengintl() -> i32);
}

#[allow(dead_code)]
impl WasmExecutor {
    pub fn new(wasm_bytes: &[u8]) -> Self {
        let mut config = Config::new();
        config.debug_info(true);
        config.cranelift_debug_verifier(true);
        config.coredump_on_trap(true);

        let engine = Engine::new(&config).unwrap();
        let component = Component::from_binary(&engine, wasm_bytes).unwrap();
        let mut linker = Linker::new(&engine);
        MmWorld::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| state).unwrap();

        let my_state = ImportCallState::default();
        let mut store = Store::new(&engine, my_state);
        let instance = MmWorld::instantiate(&mut store, &component, &linker).unwrap();

        WasmExecutor { store, instance }
    }

    pub fn real_func_test(
        dat_file: &daedalus::CompiledDaedalusScript,
        functions_names: &[&str],
        instructions: &[daedalus::Instruction],
    ) -> WasmExecutor {
        Self::main_prepare_test(
            &dat_file.symbols,
            instructions,
            functions_names,
            &dat_file.static_strings,
        )
    }

    pub fn prepare_test(
        instructions: &[daedalus::Instruction],
        symbols: &[daedalus::Symbol],
    ) -> WasmExecutor {
        Self::main_prepare_test(symbols, instructions, &[], &[])
    }

    pub fn return_test(
        instructions: &[daedalus::Instruction],
        symbols: &[daedalus::Symbol],
    ) -> i32 {
        let mut executor = Self::prepare_test(instructions, symbols);
        executor.call_test_func()
    }

    pub fn main_prepare_test(
        symbols: &[daedalus::Symbol],
        instructions: &[daedalus::Instruction],
        functions_names: &[&str],
        static_strings: &[String],
    ) -> WasmExecutor {
        let mut builder =
            BinaryBuilder::new(parse_base_module(daedalus::imported_functions_num(symbols)));

        builder.whitelist_functions(functions_names);
        builder.add_symbols(symbols);
        builder.add_static_strings(static_strings);
        builder.add_test_function(instructions);

        let wasm_bytes = builder.finalize();
        save_test_binary(&get_test_name(), &wasm_bytes);

        WasmExecutor::new(&wasm_bytes)
    }

    pub fn simple_return_test(instructions: &[daedalus::Instruction]) -> i32 {
        Self::return_test(instructions, &[daedalus::Symbol::new(0, false, 1, None)])
    }

    pub fn call_count(&self, name: &str) -> u64 {
        let key = name.to_string();
        *self.store.data().call_counts.get(&key).unwrap_or(&0)
    }

    pub fn set_instance(&mut self, instance_index: u32) {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_set_instance(&mut self.store, instance_index as i32)
            .unwrap()
    }

    #[allow(dead_code)]
    pub fn set_self(&mut self, instance_index: u32) {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_set_self(&mut self.store, instance_index as i32)
            .unwrap()
    }

    pub fn call_npc_routine(&mut self, instance_index: u32) {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_call_npc_routine(&mut self.store, instance_index as i32)
            .unwrap()
    }

    pub fn call_function(&mut self, symbol_id: u32) {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_call_function(&mut self.store, symbol_id as i32)
            .unwrap()
    }

    pub fn call_get_name(&mut self, symbol_id: u32) -> String {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_get_name(&mut self.store, symbol_id as i32)
            .unwrap()
    }

    pub fn get_symbol_id(&mut self, symbol_name: &str) -> i32 {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_get_symbol_id(&mut self.store, symbol_name)
            .unwrap()
            .expect("symbol should exist")
    }

    /// Resolves a symbol id by name, `None` when the symbol does not exist.
    /// The guest lookup does not trap for missing symbols, a trap would make
    /// the component instance unusable for all further calls.
    pub fn try_get_symbol_id(&mut self, symbol_name: &str) -> Option<i32> {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_get_symbol_id(&mut self.store, symbol_name)
            .unwrap()
    }

    pub fn call_function_bool(&mut self, symbol_id: i32) -> bool {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_call_function_bool(&mut self.store, symbol_id)
            .unwrap()
    }

    pub fn call_function_discard(&mut self, symbol_id: i32) {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_call_function_discard(&mut self.store, symbol_id)
            .unwrap()
    }

    pub fn play_ani_calls(&self) -> &[(i32, i32, i32)] {
        &self.store.data().play_ani_calls
    }
    pub fn get_item_visual(&mut self, symbol_id: i32) -> Option<String> {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_get_item_visual(&mut self.store, symbol_id)
            .unwrap()
    }
    pub fn get_item_visual_change(&mut self, symbol_id: i32) -> Option<String> {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_get_item_visual_change(&mut self.store, symbol_id)
            .unwrap()
    }
    pub fn get_npc_info(&mut self, symbol_id: i32) -> NpcInfo {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest.call_get_npc_info(&mut self.store, symbol_id).unwrap()
    }
    pub fn get_item_info(&mut self, symbol_id: i32) -> ItemInfo {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_get_item_info(&mut self.store, symbol_id)
            .unwrap()
    }

    pub fn call_test_func(&mut self) -> i32 {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest.call_test_func(&mut self.store).unwrap()
    }
    pub fn initialize_all_instances(&mut self) {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_initialize_all_instances(&mut self.store)
            .unwrap()
    }

    pub fn get_all_symbols_num(&mut self) -> i32 {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest.call_get_all_symbols_num(&mut self.store).unwrap()
    }

    pub fn get_all_strings_num(&mut self) -> i32 {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest.call_get_all_strings_num(&mut self.store).unwrap()
    }

    /// Loads the contents of all static string pairs into the import state, so
    /// string-returning externals can resolve their string arguments
    pub fn load_static_strings(&mut self) {
        let strings_num = self.get_all_strings_num();
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        let mut static_strings = Vec::with_capacity(strings_num.max(0) as usize);
        for raw in 0..strings_num {
            let string = guest
                .call_get_name(&mut self.store, raw)
                .unwrap_or_else(|error| panic!("get_name({raw}) failed: {error}"));
            static_strings.push(string);
        }
        let data = self.store.data_mut();
        data.strings_num = strings_num;
        data.static_strings = static_strings;
    }

    /// Reads the content of a string id through the `get-name` export, which
    /// resolves any (ptr, len) pair including DYN_STRINGS slots
    pub fn read_string_by_id(&mut self, id: i32) -> wasmtime::Result<String> {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest.call_get_name(&mut self.store, id)
    }

    pub fn get_symbol_info(
        &mut self,
        symbol_id: i32,
    ) -> crate::common::exports::mmcomando::zengine_scripts::manage::SymbolInfo {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_get_symbol_info(&mut self.store, symbol_id)
            .unwrap()
    }

    pub fn get_dialog_info(
        &mut self,
        symbol_id: i32,
    ) -> crate::common::exports::mmcomando::zengine_scripts::manage::DialogInfo {
        let guest = self.instance.mmcomando_zengine_scripts_manage();
        guest
            .call_get_dialog_info(&mut self.store, symbol_id)
            .unwrap()
    }
}

fn get_test_name() -> String {
    std::thread::current().name().unwrap().to_string()
}

fn save_test_binary(test_name: &str, wasm_bytes: &[u8]) {
    let wasm_file_path = format!("tests_output/{}.wasm", test_name);
    let wat_file_path = format!("tests_output/{}.wat", test_name);
    std::fs::create_dir_all("tests_output").unwrap();
    std::fs::write(&wasm_file_path, wasm_bytes).unwrap();

    Command::new("wasm-tools")
        .arg("print")
        .arg(&wasm_file_path)
        .arg("--print-operand-stack")
        // .arg("--print-offsets")
        .args(["-o", &wat_file_path])
        .status()
        .expect("Failed to run wasm-tools print");
}

#[allow(dead_code)]
pub fn gothic2_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("GOTHIC2_DIR") {
        let path = PathBuf::from(dir);
        return path;
    }

    // Try to detect gothic2 location

    if let Some(home) = home_dir() {
        let linux_steam = home.join(".local/share/Steam/steamapps/common/Gothic II");
        if linux_steam.is_dir() {
            return linux_steam;
        }
    }

    panic!(
        "Gothic2 location not found automatically. Set GOTHIC2_DIR environment variable to explicitly state game directory."
    );
}

#[allow(dead_code)]
pub fn gothic2_main_script_path() -> PathBuf {
    gothic2_dir().join("_work/Data/Scripts/_compiled/GOTHIC.DAT")
}
